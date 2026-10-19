use std::f32::consts::{PI, TAU};

use crate::shared::{
    BassFilterMode, BassFilterSlope, LfoParams, ModDest, ModRouting, ModSource, Waveform,
    BASS_LFO_COUNT, BASS_MOD_SLOT_COUNT, CURVE_LUT_SIZE,
};

use super::lfo::Lfo;
use super::oscillator::Oscillator;

/// Full-depth modulation ranges. A routing at amount 1.0 with a source at
/// 1.0 moves the destination by exactly this much.
const CUTOFF_MOD_OCTAVES: f32 = 4.0;
const PITCH_MOD_OCTAVES: f32 = 2.0;
const OSC_B_DETUNE_MOD_CENTS: f32 = 100.0;
const UNISON_DETUNE_MOD_CENTS: f32 = 50.0;

/// Ladder feedback at resonance 1.0. A 4-pole ladder reaches self-oscillation
/// around 4.0; stopping just below keeps the peak screaming without the
/// filter turning into a pure sine that drowns the oscillators.
const RESONANCE_FEEDBACK: f32 = 3.8;

/// Fixed xorshift seed so a retriggered noise source is deterministic.
const NOISE_SEED: u32 = 0x1234_5678;

/// Frequency multiplier for a whole-octave offset.
fn octave_ratio(octaves: i32) -> f32 {
    2.0_f32.powi(octaves.clamp(-4, 4))
}

/// Frequency multiplier for a fractional octave offset, used by modulation.
fn octaves_ratio(octaves: f32) -> f32 {
    2.0_f32.powf(octaves.clamp(-8.0, 8.0))
}

/// Linear-interpolated LUT read: `t` in 0..1 maps across the table, so
/// envelope sweeps are continuous instead of stepping every 1/255. The
/// stepping is inaudible on slow moves but quantizes fast sweeps — and a
/// resonant filter tracking a stepped cutoff produces a warbling squeal.
fn lut_lookup(lut: &[f32; CURVE_LUT_SIZE], t: f32) -> f32 {
    let pos = t.clamp(0.0, 1.0) * (CURVE_LUT_SIZE as f32 - 1.0);
    let index = pos as usize;
    let frac = pos - index as f32;
    let next = (index + 1).min(CURVE_LUT_SIZE - 1);
    lut[index] + (lut[next] - lut[index]) * frac
}

/// Per-destination modulation totals accumulated for one sample.
#[derive(Default, Clone, Copy)]
struct ModAccum {
    cutoff_octaves: f32,
    pitch_octaves: f32,
    level: f32,
    osc_a_mix: f32,
    osc_b_mix: f32,
    sub_mix: f32,
    noise_mix: f32,
    osc_b_detune_cents: f32,
    unison_detune_cents: f32,
    filter_drive: f32,
    resonance: f32,
}

/// Scales ladder feedback by stage count. Each one-pole stage contributes
/// 45 degrees of phase shift, and a peak needs 180 degrees of it, so fewer
/// stages reach less resonance for the same feedback. A single pole cannot
/// form a complex pole pair at all, so it stays essentially flat by design.
fn pole_feedback_scale(poles: usize) -> f32 {
    match poles {
        0 | 1 => 0.0,
        2 => 0.5,
        3 => 0.75,
        _ => 1.0,
    }
}

/// Snapshot of a voice's live modulation values, read by the engine so the
/// UI can animate playheads, LFO dots and knob mod rings.
#[derive(Clone, Copy, Default)]
pub struct VoiceModSnapshot {
    pub env_phase: f32,
    pub env_amp: f32,
    pub env_filter_1: f32,
    pub env_filter_2: f32,
    /// Current LFO outputs, -1..1 (already scaled by depth).
    pub lfo_values: [f32; BASS_LFO_COUNT],
    /// Current LFO phases, 0..1 — lets the UI dot sit on the drawn curve.
    pub lfo_phases: [f32; BASS_LFO_COUNT],
    /// Velocity of the sounding note.
    pub velocity: f32,
    pub cutoff_hz: f32,
}

/// Frequency multiplier for a fine detune in cents.
fn cents_ratio(cents: f32) -> f32 {
    2.0_f32.powf(cents.clamp(-1200.0, 1200.0) / 1200.0)
}

/// Maximum unison voices per oscillator.
pub const MAX_UNISON: usize = 8;

/// Renders a unison stack and returns the gain-normalised mix.
///
/// Voices are spread symmetrically across `detune_cents`, with `blend`
/// controlling how loud the outer voices are relative to the centre. The
/// result is divided by the total gain so changing voice count does not
/// change perceived level. A count of 1 bypasses all of this and renders
/// exactly like a single oscillator.
fn render_unison(
    voices: &mut [Oscillator; MAX_UNISON],
    waveform: Waveform,
    base_hz: f32,
    count: usize,
    detune_cents: f32,
    blend: f32,
) -> f32 {
    let count = count.clamp(1, MAX_UNISON);
    if count == 1 || detune_cents.abs() < 0.01 {
        // With no detune the note-on phase spread makes the identical-frequency
        // voices cancel each other, so collapse the stack to a single voice.
        return voices[0].render_sample(waveform, base_hz, 1.0);
    }

    let blend = blend.clamp(0.0, 1.0);
    let mut sum = 0.0_f32;
    let mut total_gain = 0.0_f32;
    for (index, voice) in voices.iter_mut().take(count).enumerate() {
        // Position across the stack, -1.0 (flat) .. 1.0 (sharp).
        let position = (index as f32 / (count - 1) as f32) * 2.0 - 1.0;
        let hz = (base_hz * cents_ratio(position * detune_cents)).clamp(20.0, 20_000.0);
        let gain = 1.0 - position.abs() * (1.0 - blend);
        sum += voice.render_sample(waveform, hz, 1.0) * gain;
        total_gain += gain;
    }

    if total_gain > f32::EPSILON {
        sum / total_gain
    } else {
        0.0
    }
}

#[derive(Clone, Copy)]
pub struct VoiceParams {
    pub level: f32,
    pub keytrack_enabled: bool,
    pub tuning_scale: f32,
    pub note_length_ms: f32,
    /// Fundamental oscillator pitch; the pitch envelope sweeps above it.
    pub pitch_hz: f32,
    pub waveform: Waveform,
    /// AMP envelope power. Off means a constant-1 amplitude (no shaping) —
    /// a disabled module must contribute nothing to the signal.
    pub amp_env_enabled: bool,
    /// PITCH envelope power. Off means a flat pitch (no sweep).
    pub pitch_env_enabled: bool,
}

#[derive(Clone, Copy)]
pub struct BassVoiceParams {
    pub level: f32,
    pub tuning_scale: f32,
    pub note_length_ms: f32,
    pub base_cutoff_hz: f32,
    pub filter_mode: BassFilterMode,
    pub filter_slope: BassFilterSlope,
    pub filter_drive: f32,
    /// Filter resonance, 0..1. Feeds the filter output back into its input to
    /// build a peak at the cutoff; needs 2+ poles to be audible.
    pub resonance: f32,
    pub filter_enabled: bool,
    pub filter_2_enabled: bool,
    pub waveform: Waveform,
    /// OSC A level in the source mixer. 1.0 reproduces single-oscillator behaviour.
    pub osc_a_level: f32,
    /// Unison voice count for OSC A. 1 disables unison.
    pub osc_a_unison: i32,
    /// Total unison detune spread for OSC A, in cents.
    pub osc_a_unison_detune: f32,
    /// Outer-voice level for OSC A unison, 0..1.
    pub osc_a_unison_blend: f32,
    pub osc_b_enabled: bool,
    pub osc_b_waveform: Waveform,
    pub osc_b_level: f32,
    pub osc_b_octave: i32,
    pub osc_b_detune_cents: f32,
    pub osc_b_unison: i32,
    pub osc_b_unison_detune: f32,
    pub osc_b_unison_blend: f32,
    pub sub_enabled: bool,
    pub sub_waveform: Waveform,
    pub sub_level: f32,
    pub sub_octave: i32,
    pub noise_enabled: bool,
    pub noise_level: f32,
    /// Portamento time in milliseconds — the pitch slews exponentially
    /// toward each new note over this constant time. 0 snaps instantly.
    pub glide_ms: f32,
    /// Per-slot LFO settings, indexed by LFO number.
    pub lfos: [LfoParams; BASS_LFO_COUNT],
    /// Modulation matrix rows routing sources to destinations.
    pub mod_matrix: [ModRouting; BASS_MOD_SLOT_COUNT],
    /// Host tempo in beats per second, for tempo-synced LFOs.
    pub beats_per_second: f32,
}

pub struct KickVoice {
    sample_rate: f32,
    oscillator: Oscillator,
    time_seconds: f32,
    hit_gain: f32,
    hit_note_hz: Option<f32>,
}

pub struct BassVoice {
    sample_rate: f32,
    osc_a: [Oscillator; MAX_UNISON],
    osc_b: [Oscillator; MAX_UNISON],
    sub: Oscillator,
    /// Xorshift state for the noise source; reseeded on each note-on so a
    /// retriggered note is deterministic.
    noise_state: u32,
    lfos: [Lfo; BASS_LFO_COUNT],
    /// Live modulation values from the most recent rendered sample.
    mod_snapshot: VoiceModSnapshot,
    time_seconds: f32,
    note_hz: f32,
    velocity: f32,
    /// Output multiplier ramping 0→1 after a trigger and 1→0 during release,
    /// so note boundaries never jump discontinuously (de-click).
    ramp_gain: f32,
    /// Current slewed oscillator pitch; converges on `note_hz` at the
    /// `glide_ms` rate. Set to `note_hz` directly when glide is off.
    glide_hz: f32,
    /// True after `note_off` while the release ramp is still fading out.
    releasing: bool,
    hp_prev_in: [f32; BassFilterSlope::S24dB.poles()],
    hp_prev_out: [f32; BassFilterSlope::S24dB.poles()],
    lp_prev_out: [f32; BassFilterSlope::S24dB.poles()],
    /// Trailing lowpass stage state for the BandPass mode (HP then LP).
    /// Kept separate from `lp_prev_out`, which always runs the resonance
    /// feedback core regardless of the selected mode.
    bp_prev_out: [f32; BassFilterSlope::S24dB.poles()],
}

impl Default for KickVoice {
    fn default() -> Self {
        Self {
            sample_rate: 44_100.0,
            oscillator: Oscillator::default(),
            time_seconds: 0.0,
            hit_gain: 1.0,
            hit_note_hz: None,
        }
    }
}

impl Default for BassVoice {
    fn default() -> Self {
        Self {
            sample_rate: 44_100.0,
            osc_a: [Oscillator::default(); MAX_UNISON],
            osc_b: [Oscillator::default(); MAX_UNISON],
            sub: Oscillator::default(),
            noise_state: NOISE_SEED,
            lfos: std::array::from_fn(|_| Lfo::default()),
            mod_snapshot: VoiceModSnapshot::default(),
            time_seconds: 0.0,
            note_hz: 55.0,
            velocity: 1.0,
            ramp_gain: 1.0,
            releasing: false,
            glide_hz: 55.0,
            hp_prev_in: [0.0; BassFilterSlope::S24dB.poles()],
            hp_prev_out: [0.0; BassFilterSlope::S24dB.poles()],
            lp_prev_out: [0.0; BassFilterSlope::S24dB.poles()],
            bp_prev_out: [0.0; BassFilterSlope::S24dB.poles()],
        }
    }
}

impl KickVoice {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
        self.oscillator.set_sample_rate(self.sample_rate);
    }

    pub fn set_anti_aliasing(&mut self, enabled: bool) {
        self.oscillator.set_anti_aliasing(enabled);
    }

    pub fn trigger(&mut self, phase_offset: f32, retrigger: bool, legato_voice_steal: bool) {
        self.trigger_with_velocity(1.0, phase_offset, retrigger, legato_voice_steal);
    }

    pub fn is_active(&self) -> bool {
        self.oscillator.is_active()
    }

    pub fn trigger_with_velocity(
        &mut self,
        velocity: f32,
        phase_offset: f32,
        retrigger: bool,
        legato_voice_steal: bool,
    ) {
        if !self
            .oscillator
            .note_on(retrigger, legato_voice_steal, phase_offset)
        {
            return;
        }
        self.time_seconds = 0.0;
        self.hit_gain = velocity.clamp(0.0, 1.0);
        self.hit_note_hz = None;
    }

    pub fn trigger_with_note_velocity(
        &mut self,
        note_hz: f32,
        velocity: f32,
        phase_offset: f32,
        retrigger: bool,
        legato_voice_steal: bool,
    ) {
        if !self
            .oscillator
            .note_on(retrigger, legato_voice_steal, phase_offset)
        {
            return;
        }
        self.time_seconds = 0.0;
        self.hit_gain = velocity.clamp(0.0, 1.0);
        self.hit_note_hz = Some(note_hz.max(20.0));
    }

    pub fn next_sample(
        &mut self,
        params: VoiceParams,
        amp_lut: &[f32; CURVE_LUT_SIZE],
        pitch_lut: &[f32; CURVE_LUT_SIZE],
    ) -> f32 {
        if !self.oscillator.is_active() {
            return 0.0;
        }

        let note_length_seconds = (params.note_length_ms * 0.001).clamp(0.0, 1.0);
        if note_length_seconds <= 0.0 {
            self.oscillator.note_off();
            return 0.0;
        }

        let duration_seconds = note_length_seconds.max(f32::EPSILON);
        let normalized_time = (self.time_seconds / duration_seconds).clamp(0.0, 1.0);

        // Powered-off envelopes must not touch the signal: AMP off plays
        // flat at full level, PITCH off stays on the base pitch.
        let amp_curve = if params.amp_env_enabled {
            lut_lookup(amp_lut, normalized_time).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let pitch_curve = if params.pitch_env_enabled {
            lut_lookup(pitch_lut, normalized_time).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // The pitch envelope is a multiplier above the base pitch:
        // 1x at the bottom of the curve, 2^(8 octaves) at the top.
        const KICK_PITCH_OCTAVES: f32 = 8.0;
        let pitch_ratio = 2.0_f32.powf(pitch_curve * KICK_PITCH_OCTAVES);
        let base_hz = if params.keytrack_enabled {
            self.hit_note_hz.unwrap_or(params.pitch_hz)
        } else {
            params.pitch_hz
        };

        let amplitude = params.level.clamp(0.0, 1.0) * self.hit_gain * amp_curve;
        let frequency =
            (base_hz * pitch_ratio * params.tuning_scale.max(0.5)).clamp(20.0, 20_000.0);

        let sample = self
            .oscillator
            .render_sample(params.waveform, frequency, amplitude);

        self.time_seconds += 1.0 / self.sample_rate;
        if self.time_seconds >= note_length_seconds || normalized_time >= 1.0 || amplitude < 0.0005
        {
            self.oscillator.note_off();
        }

        sample
    }
}

impl BassVoice {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
        for voice in self.osc_a.iter_mut().chain(self.osc_b.iter_mut()) {
            voice.set_sample_rate(self.sample_rate);
        }
        self.sub.set_sample_rate(self.sample_rate);
        for lfo in self.lfos.iter_mut() {
            lfo.set_sample_rate(self.sample_rate);
        }
    }

    pub fn set_anti_aliasing(&mut self, enabled: bool) {
        for voice in self.osc_a.iter_mut().chain(self.osc_b.iter_mut()) {
            voice.set_anti_aliasing(enabled);
        }
        self.sub.set_anti_aliasing(enabled);
    }

    /// Live modulation values from the most recent rendered sample.
    pub fn mod_snapshot(&self) -> VoiceModSnapshot {
        self.mod_snapshot
    }

    /// Uniform white noise in -1.0..=1.0 from a cheap xorshift generator.
    fn next_noise(&mut self) -> f32 {
        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_state = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn reset_filter_state(&mut self) {
        self.hp_prev_in = [0.0; BassFilterSlope::S24dB.poles()];
        self.hp_prev_out = [0.0; BassFilterSlope::S24dB.poles()];
        self.lp_prev_out = [0.0; BassFilterSlope::S24dB.poles()];
        self.bp_prev_out = [0.0; BassFilterSlope::S24dB.poles()];
    }

    pub fn note_on(
        &mut self,
        note_hz: f32,
        velocity: f32,
        phase_offset: f32,
        retrigger: bool,
        legato_voice_steal: bool,
        lfos: &[LfoParams; BASS_LFO_COUNT],
        // Portamento time in ms — slews `glide_hz` toward `note_hz`.
        glide_ms: f32,
        // When true, glide only on overlapping notes (legato steals);
        // a note that retriggers the envelope snaps to pitch instead.
        glide_auto: bool,
    ) {
        let was_sounding = self.osc_a[0].is_active() || self.releasing;
        if self.releasing {
            // Still fading out — hard-stop so the steal guard can't refuse a
            // note that arrives inside the release window.
            self.kill();
        }
        if !self.osc_a[0].note_on(retrigger, legato_voice_steal, phase_offset) {
            return;
        }

        // The companion sources always follow OSC A's gate so the mixer stays
        // phase-coherent; `true` bypasses their own voice-steal guard.
        // Unison voices beyond the first are spread evenly around the cycle so
        // the stack does not stack up into one oversized transient.
        for (index, voice) in self.osc_a.iter_mut().enumerate().skip(1) {
            let spread = phase_offset + index as f32 / MAX_UNISON as f32;
            voice.note_on(retrigger, true, spread);
        }
        for (index, voice) in self.osc_b.iter_mut().enumerate() {
            let spread = phase_offset + index as f32 / MAX_UNISON as f32;
            voice.note_on(retrigger, true, spread);
        }
        self.sub.note_on(retrigger, true, phase_offset);
        if retrigger {
            self.noise_state = NOISE_SEED;
        }

        // Only LFOs marked as retriggering restart; the rest keep running free
        // so they stay locked to the timeline across notes.
        for (lfo, settings) in self.lfos.iter_mut().zip(lfos.iter()) {
            if settings.retrigger {
                lfo.retrigger(settings);
            }
        }

        self.time_seconds = 0.0;
        self.note_hz = note_hz.max(20.0);
        // Glide only when the previous note is still ringing — a fresh
        // start would otherwise sweep up from an ancient pitch. In Auto
        // mode, retriggers (non-overlapping steals) snap to pitch.
        let glides = glide_ms > 0.0 && was_sounding && (!glide_auto || legato_voice_steal);
        if !glides {
            self.glide_hz = self.note_hz;
        }
        self.velocity = velocity.clamp(0.0, 1.0);
        self.releasing = false;
        // Ramp in from silence so the phase/envelope reset can't click.
        self.ramp_gain = 0.0;
    }

    /// Starts the release ramp; the oscillators keep ringing while the output
    /// fades to zero over a few milliseconds, then `kill` stops them for real.
    pub fn note_off(&mut self) {
        self.releasing = true;
    }

    /// Immediate silence — the old `note_off` behaviour, kept for the end of
    /// the release ramp and for steals that overlap an in-progress fade.
    fn kill(&mut self) {
        for voice in self.osc_a.iter_mut().chain(self.osc_b.iter_mut()) {
            voice.note_off();
        }
        self.sub.note_off();
        self.reset_filter_state();
        self.releasing = false;
        self.ramp_gain = 0.0;
    }

    pub fn is_active(&self) -> bool {
        self.osc_a[0].is_active()
    }

    pub fn next_sample(
        &mut self,
        params: BassVoiceParams,
        bass_amp_lut: &[f32; CURVE_LUT_SIZE],
        bass_filter_lut: &[f32; CURVE_LUT_SIZE],
        bass_filter_2_lut: &[f32; CURVE_LUT_SIZE],
    ) -> f32 {
        if !self.osc_a[0].is_active() {
            return 0.0;
        }

        let note_length_seconds = (params.note_length_ms * 0.001).clamp(0.001, 2.0);
        let normalized_time = (self.time_seconds / note_length_seconds).clamp(0.0, 1.0);
        let amp_env = lut_lookup(bass_amp_lut, normalized_time).clamp(0.0, 1.0);
        // Powered-off envelopes read as zero — they must reach neither the
        // cutoff below nor the mod matrix's EnvFilter sources. Only an
        // engaged module may touch the audio path.
        let cutoff_env_1 = if params.filter_enabled {
            lut_lookup(bass_filter_lut, normalized_time).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let cutoff_env_2 = if params.filter_2_enabled {
            lut_lookup(bass_filter_2_lut, normalized_time).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Combine the two filter envelopes additively around the neutral point
        // (cutoff_env = 1.0 means the base cutoff). Each active envelope adds
        // its own deviation from 1.0, so the two filters run in tandem rather
        // than as a series (multiplicative) chain.
        let mut combined_env = 1.0_f32;
        if params.filter_enabled {
            combined_env += cutoff_env_1 - 1.0;
        }
        if params.filter_2_enabled {
            combined_env += cutoff_env_2 - 1.0;
        }
        let cutoff_env = combined_env.clamp(0.0, 1.0);

        // Advance every LFO once per sample, even unrouted ones, so a
        // free-running LFO stays locked to the timeline rather than jumping
        // when a routing is assigned mid-note.
        let mut lfo_values = [0.0_f32; BASS_LFO_COUNT];
        for (index, lfo) in self.lfos.iter_mut().enumerate() {
            lfo_values[index] = lfo.next_sample(&params.lfos[index], params.beats_per_second);
        }

        let velocity = self.velocity;
        // Map the note across the usable bass range so keytrack is 0..1.
        let keytrack = {
            const MIN_HZ: f32 = 20.0;
            const MAX_HZ: f32 = 2_000.0;
            ((self.note_hz.clamp(MIN_HZ, MAX_HZ) / MIN_HZ).ln() / (MAX_HZ / MIN_HZ).ln())
                .clamp(0.0, 1.0)
        };

        let source_value = |source: ModSource| -> f32 {
            match source {
                ModSource::None => 0.0,
                ModSource::Lfo1 => lfo_values[0],
                ModSource::Lfo2 => lfo_values[1],
                ModSource::Lfo3 => lfo_values[2],
                ModSource::Lfo4 => lfo_values[3],
                ModSource::EnvAmp => amp_env,
                ModSource::EnvFilter1 => cutoff_env_1,
                ModSource::EnvFilter2 => cutoff_env_2,
                ModSource::Velocity => velocity,
                ModSource::KeyTrack => keytrack,
            }
        };

        // Sum every active routing into per-destination totals. Multiple rows
        // can target the same destination and they add, matching how Serum
        // stacks modulation.
        let mut modulation = ModAccum::default();
        for routing in params.mod_matrix.iter() {
            if !routing.is_active() {
                continue;
            }
            let value = source_value(routing.source) * routing.amount;
            match routing.dest {
                ModDest::None => {}
                ModDest::Cutoff => modulation.cutoff_octaves += value * CUTOFF_MOD_OCTAVES,
                ModDest::Pitch => modulation.pitch_octaves += value * PITCH_MOD_OCTAVES,
                ModDest::Level => modulation.level += value,
                ModDest::OscAMix => modulation.osc_a_mix += value,
                ModDest::OscBMix => modulation.osc_b_mix += value,
                ModDest::SubMix => modulation.sub_mix += value,
                ModDest::NoiseMix => modulation.noise_mix += value,
                ModDest::OscBDetune => {
                    modulation.osc_b_detune_cents += value * OSC_B_DETUNE_MOD_CENTS
                }
                ModDest::UnisonDetune => {
                    modulation.unison_detune_cents += value * UNISON_DETUNE_MOD_CENTS
                }
                ModDest::FilterDrive => modulation.filter_drive += value,
                ModDest::Resonance => modulation.resonance += value,
            }
        }

        let amplitude =
            (params.level.clamp(0.0, 1.0) * velocity * amp_env * (1.0 + modulation.level))
                .clamp(0.0, 4.0);
        // Portamento: exponential approach in Hz — an RC-capacitor slew,
        // constant time regardless of interval, like a 303 glide. Mod
        // (pitch octave shifts) applies on top of the slewed pitch.
        if params.glide_ms > 0.0 {
            let tau = (params.glide_ms / 1_000.0).max(1.0e-4);
            let k = 1.0 - (-1.0 / (self.sample_rate * tau)).exp();
            self.glide_hz += (self.note_hz - self.glide_hz) * k;
        } else {
            self.glide_hz = self.note_hz;
        }
        let frequency = (self.glide_hz
            * params.tuning_scale.max(0.5)
            * octaves_ratio(modulation.pitch_octaves))
        .clamp(20.0, 20_000.0);

        let osc_a_level = (params.osc_a_level + modulation.osc_a_mix).clamp(0.0, 1.0);
        let unison_detune =
            (params.osc_a_unison_detune + modulation.unison_detune_cents).clamp(0.0, 200.0);

        // Source mixer: each source renders at unity and is scaled by its own
        // level, then the whole mix is scaled by the voice amplitude. With
        // OSC A at 1.0 and the others off this is identical to the previous
        // single-oscillator path.
        let mut mix = render_unison(
            &mut self.osc_a,
            params.waveform,
            frequency,
            params.osc_a_unison.max(1) as usize,
            unison_detune,
            params.osc_a_unison_blend,
        ) * osc_a_level;

        if params.osc_b_enabled {
            let ratio = octave_ratio(params.osc_b_octave)
                * cents_ratio(params.osc_b_detune_cents + modulation.osc_b_detune_cents);
            let osc_b_hz = (frequency * ratio).clamp(20.0, 20_000.0);
            let osc_b_detune =
                (params.osc_b_unison_detune + modulation.unison_detune_cents).clamp(0.0, 200.0);
            mix += render_unison(
                &mut self.osc_b,
                params.osc_b_waveform,
                osc_b_hz,
                params.osc_b_unison.max(1) as usize,
                osc_b_detune,
                params.osc_b_unison_blend,
            ) * (params.osc_b_level + modulation.osc_b_mix).clamp(0.0, 1.0);
        }

        // The sub renders but is not summed yet — it bypasses the filter so
        // the low end stays clean under the filtered top (classic psy split).
        let sub_out = if params.sub_enabled {
            let sub_hz = (frequency * octave_ratio(params.sub_octave)).clamp(20.0, 20_000.0);
            self.sub.render_sample(params.sub_waveform, sub_hz, 1.0)
                * (params.sub_level + modulation.sub_mix).clamp(0.0, 1.0)
        } else {
            0.0
        };

        if params.noise_enabled {
            mix += self.next_noise() * (params.noise_level + modulation.noise_mix).clamp(0.0, 1.0);
        }

        let raw = mix * amplitude;

        let base_cutoff = params.base_cutoff_hz.clamp(20.0, 8_000.0);
        let cutoff_hz =
            (base_cutoff * (0.25 + cutoff_env * 0.75) * octaves_ratio(modulation.cutoff_octaves))
                .clamp(20.0, 8_000.0);

        self.mod_snapshot = VoiceModSnapshot {
            env_phase: normalized_time,
            env_amp: amp_env,
            env_filter_1: cutoff_env_1,
            env_filter_2: cutoff_env_2,
            lfo_values,
            lfo_phases: std::array::from_fn(|i| self.lfos[i].phase()),
            velocity,
            cutoff_hz,
        };
        let dt = 1.0 / self.sample_rate;
        let rc = 1.0 / (TAU * cutoff_hz.max(20.0));
        let hp_alpha = rc / (rc + dt);

        let poles = params.filter_slope.poles();
        let drive = (params.filter_drive + modulation.filter_drive).clamp(0.0, 1.0);
        let driven = if drive > 0.0 {
            (raw * (1.0 + drive * 4.0)).tanh()
        } else {
            raw
        };

        // Topology: cascaded TPT one-poles with ladder-style resonance —
        // the cascade's phase shift turns negative feedback into a peak at
        // the cutoff — the squelch. (Not an SVF; modes/slopes come from the
        // pole arrangement below.) No clipping in the loop: feedback tops
        // out at 3.8, just under the 4-pole self-oscillation threshold, so
        // the linear loop stays bounded on its own and we hear the raw
        // resonance. Like a real ladder, resonance also thins the low end
        // as it rises, which is part of the character.
        let g = (PI * cutoff_hz / self.sample_rate).tan();
        let g_over = g / (1.0 + g); // per-stage gain G

        let resonance = (params.resonance + modulation.resonance).clamp(0.0, 1.0);
        let feedback_gain = resonance * RESONANCE_FEEDBACK * pole_feedback_scale(poles);
        let filter_input = if feedback_gain > 0.0 {
            // Zero-delay-feedback solve: with per-stage gain G the cascade
            // output is y_N = G^N·u + S_N, where u is this sample's input and
            // S_N gathers each integrator state's contribution. Solving
            // u = driven − k·y_N instantaneously removes the one-sample
            // feedback delay that detuned the resonance peak at high cutoffs.
            let g_total = g_over.powi(poles as i32);
            let mut s_acc = 0.0;
            for i in 0..poles {
                let through_stages = (poles - 1 - i) as i32;
                s_acc += (1.0 - g_over) * g_over.powi(through_stages) * self.lp_prev_out[i];
            }
            (driven - feedback_gain * s_acc) / (1.0 + feedback_gain * g_total)
        } else {
            driven
        };

        let mut hp = filter_input;
        for i in 0..poles {
            let y = hp_alpha * (self.hp_prev_out[i] + hp - self.hp_prev_in[i]);
            self.hp_prev_in[i] = hp;
            self.hp_prev_out[i] = y;
            hp = y;
        }

        // The lowpass core always runs: it is both the LowPass output and
        // the ladder resonance core in every mode (the ZDF solve above reads
        // its integrator states). TPT trapezoidal stages with prewarped
        // tan() cutoff keep the peak glued to the cutoff at high frequencies.
        let mut lp = filter_input;
        for i in 0..poles {
            let s = self.lp_prev_out[i];
            let v = (lp - s) * g_over;
            lp = v + s;
            self.lp_prev_out[i] = lp + v;
        }

        let filtered = match params.filter_mode {
            BassFilterMode::LowPass => lp,
            BassFilterMode::HighPass => hp,
            BassFilterMode::BandPass => {
                // Trailing TPT lowpass after the HP cascade.
                let mut x = hp;
                for i in 0..poles {
                    let s = self.bp_prev_out[i];
                    let v = (x - s) * g_over;
                    x = v + s;
                    self.bp_prev_out[i] = x + v;
                }
                x
            }
        };

        // Sub enters after the filter, still under the amp envelope.
        let filtered = filtered + sub_out * amplitude;

        self.time_seconds += dt;
        if self.time_seconds >= note_length_seconds || normalized_time >= 1.0 || amplitude < 0.0005
        {
            self.note_off();
        }

        // De-click ramps: ~3 ms in on (re)trigger, ~8 ms out on release, so
        // consecutive notes never produce hard sample-level discontinuities.
        let output = if self.releasing {
            self.ramp_gain = (self.ramp_gain - 1.0 / (self.sample_rate * 0.008).max(1.0)).max(0.0);
            filtered * self.ramp_gain
        } else {
            self.ramp_gain = (self.ramp_gain + 1.0 / (self.sample_rate * 0.003).max(1.0)).min(1.0);
            filtered * self.ramp_gain
        };
        if self.releasing && self.ramp_gain <= 0.0 {
            self.kill();
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::oscillator::Oscillator;

    /// A unison stack with zero detune must sound like a single oscillator.
    /// Note-on spreads the extra voices evenly around the cycle, which used
    /// to make identical-frequency copies cancel each other — the stack read
    /// ~30 dB down instead of full level.
    #[test]
    fn zero_detune_unison_renders_like_single_voice() {
        const HZ: f32 = 55.0;
        const SAMPLES: usize = 4000;

        let mut stack = [Oscillator::default(); MAX_UNISON];
        stack[0].note_on(true, true, 0.0);
        for (index, voice) in stack.iter_mut().enumerate().skip(1) {
            voice.note_on(true, true, index as f32 / MAX_UNISON as f32);
        }
        let mut single = Oscillator::default();
        single.note_on(true, true, 0.0);

        let mut stack_energy = 0.0f64;
        let mut single_energy = 0.0f64;
        for _ in 0..SAMPLES {
            let stacked = render_unison(&mut stack, Waveform::Saw, HZ, MAX_UNISON, 0.0, 1.0);
            let solo = single.render_sample(Waveform::Saw, HZ, 1.0);
            stack_energy += (stacked * stacked) as f64;
            single_energy += (solo * solo) as f64;
        }
        let stack_rms = (stack_energy / SAMPLES as f64).sqrt();
        let single_rms = (single_energy / SAMPLES as f64).sqrt();
        assert!(
            stack_rms > single_rms * 0.8,
            "zero-detune unison cancelled itself: stack_rms={stack_rms}, single_rms={single_rms}"
        );
    }

    /// Neutral envelopes and a long note so the voice sustains for the whole
    /// measurement window.
    fn resonant_test_params(
        slope: BassFilterSlope,
        mode: BassFilterMode,
        resonance: f32,
    ) -> BassVoiceParams {
        BassVoiceParams {
            level: 1.0,
            tuning_scale: 1.0,
            note_length_ms: 2_000.0,
            base_cutoff_hz: 700.0,
            filter_mode: mode,
            filter_slope: slope,
            filter_drive: 0.0,
            resonance,
            filter_enabled: true,
            filter_2_enabled: false,
            waveform: Waveform::Saw,
            osc_a_level: 1.0,
            osc_a_unison: 1,
            osc_a_unison_detune: 0.0,
            osc_a_unison_blend: 1.0,
            osc_b_enabled: false,
            osc_b_waveform: Waveform::Saw,
            osc_b_level: 0.0,
            osc_b_octave: 0,
            osc_b_detune_cents: 0.0,
            osc_b_unison: 1,
            osc_b_unison_detune: 0.0,
            osc_b_unison_blend: 0.0,
            sub_enabled: false,
            sub_waveform: Waveform::Sine,
            sub_level: 0.0,
            sub_octave: -1,
            noise_enabled: false,
            noise_level: 0.0,
            glide_ms: 0.0,
            lfos: [LfoParams::default(); BASS_LFO_COUNT],
            mod_matrix: [ModRouting::default(); BASS_MOD_SLOT_COUNT],
            beats_per_second: 2.0,
        }
    }

    fn render_bass(params: BassVoiceParams, samples: usize) -> Vec<f32> {
        let amp_lut = [1.0_f32; CURVE_LUT_SIZE];
        let filter_lut = [1.0_f32; CURVE_LUT_SIZE];
        let mut voice = BassVoice::default();
        voice.set_sample_rate(44_100.0);
        voice.note_on(
            55.0,
            1.0,
            0.0,
            true,
            true,
            &[LfoParams::default(); BASS_LFO_COUNT],
            0.0,
            false,
        );
        (0..samples)
            .map(|_| voice.next_sample(params, &amp_lut, &filter_lut, &filter_lut))
            .collect()
    }

    /// Goertzel magnitude at `hz` — cheap single-bin DFT.
    #[allow(unused_assignments)]
    fn goertzel(samples: &[f32], hz: f32, sample_rate: f32) -> f64 {
        let n = samples.len() as f64;
        let k = (hz as f64 * n / sample_rate as f64).round();
        let w = 2.0 * std::f64::consts::PI * k / n;
        let coeff = 2.0 * w.cos();
        let (mut s0, mut s1, mut s2) = (0.0f64, 0.0f64, 0.0f64);
        for &x in samples {
            s0 = x as f64 + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coeff * s1 * s2).sqrt() / n
    }

    /// Resonance must build a peak at the cutoff. Broadband noise at the
    /// input makes the peak measurable regardless of which exact bin it lands
    /// on: scan the band around the 700 Hz cutoff and compare the loudest bin
    /// with and without resonance.
    #[test]
    fn resonance_boosts_energy_at_cutoff() {
        const SAMPLES: usize = 44_100;
        let mut params = resonant_test_params(BassFilterSlope::S24dB, BassFilterMode::LowPass, 0.0);
        params.noise_enabled = true;
        params.noise_level = 1.0;
        params.osc_a_level = 0.0;
        let flat = render_bass(params, SAMPLES);
        params.resonance = 0.9;
        let resonant = render_bass(params, SAMPLES);

        let mut flat_peak = 0.0f64;
        let mut res_peak = 0.0f64;
        let mut hz = 500.0;
        while hz <= 1_000.0 {
            flat_peak = flat_peak.max(goertzel(&flat[SAMPLES / 2..], hz, 44_100.0));
            res_peak = res_peak.max(goertzel(&resonant[SAMPLES / 2..], hz, 44_100.0));
            hz += 25.0;
        }
        assert!(
            res_peak > flat_peak * 1.5,
            "resonance produced no peak near cutoff: res={res_peak}, flat={flat_peak}"
        );
    }

    /// The unclipped feedback loop must stay bounded even at maximum
    /// resonance with full pre-filter drive, across every mode and slope —
    /// k tops out at 3.8, under the 4-pole self-oscillation threshold.
    #[test]
    fn max_resonance_and_drive_stay_bounded() {
        for slope in [
            BassFilterSlope::S6dB,
            BassFilterSlope::S12dB,
            BassFilterSlope::S18dB,
            BassFilterSlope::S24dB,
        ] {
            for mode in [
                BassFilterMode::LowPass,
                BassFilterMode::HighPass,
                BassFilterMode::BandPass,
            ] {
                let mut params = resonant_test_params(slope, mode, 1.0);
                params.filter_drive = 1.0;
                let out = render_bass(params, 22_050);
                assert!(
                    out.iter().all(|x| x.is_finite() && x.abs() < 10.0),
                    "unstable output for {slope:?} {mode:?}: max={:?}",
                    out.iter().fold(0.0_f32, |a, b| a.max(b.abs()))
                );
            }
        }
    }

    /// The sub bypasses the filter entirely: identical settings except a
    /// wide-open vs nearly-shut cutoff (plus max resonance) must produce the
    /// exact same signal.
    #[test]
    fn sub_bypasses_the_filter() {
        let mut closed = resonant_test_params(BassFilterSlope::S24dB, BassFilterMode::LowPass, 1.0);
        closed.osc_a_level = 0.0;
        closed.sub_enabled = true;
        closed.sub_level = 1.0;
        closed.base_cutoff_hz = 20.0;

        let mut open = closed;
        open.base_cutoff_hz = 8_000.0;
        open.resonance = 0.0;

        let a = render_bass(closed, 22_050);
        let b = render_bass(open, 22_050);
        assert_eq!(a, b, "sub-only output changed with cutoff/resonance");
    }

    /// A single pole cannot form a resonant pair, so the 6 dB slope is
    /// deliberately flat — resonance must not blow it up either.
    #[test]
    fn resonance_does_not_destabilize_one_pole() {
        let out = render_bass(
            resonant_test_params(BassFilterSlope::S6dB, BassFilterMode::LowPass, 1.0),
            22_050,
        );
        assert!(out.iter().all(|x| x.is_finite() && x.abs() < 10.0));
    }

    /// A powered-off filter envelope must not reach the mod matrix either:
    /// routing a disabled envelope is exactly the same as no routing at all.
    /// The flat filter LUT reads 1.0, so a leak would push full resonance.
    #[test]
    fn disabled_filter_env_is_inert_as_mod_source() {
        for (source, enabled_field) in [
            (ModSource::EnvFilter1, false),
            (ModSource::EnvFilter2, false),
        ] {
            let mut params =
                resonant_test_params(BassFilterSlope::S24dB, BassFilterMode::LowPass, 0.2);
            params.filter_enabled = enabled_field;
            params.filter_2_enabled = enabled_field;
            params.mod_matrix[0] = ModRouting {
                source,
                dest: ModDest::Resonance,
                amount: 1.0,
            };
            let routed = render_bass(params, 22_050);

            params.mod_matrix = [ModRouting::default(); BASS_MOD_SLOT_COUNT];
            let unrouted = render_bass(params, 22_050);

            assert_eq!(
                routed, unrouted,
                "{source:?} still modulated resonance while its envelope was powered off"
            );
        }

        // And the mirror: with the pill on, the same routing must modulate.
        let mut params =
            resonant_test_params(BassFilterSlope::S24dB, BassFilterMode::LowPass, 0.2);
        params.filter_2_enabled = true;
        params.mod_matrix[0] = ModRouting {
            source: ModSource::EnvFilter2,
            dest: ModDest::Resonance,
            amount: 1.0,
        };
        let routed = render_bass(params, 22_050);
        params.mod_matrix = [ModRouting::default(); BASS_MOD_SLOT_COUNT];
        let unrouted = render_bass(params, 22_050);
        assert_ne!(
            routed, unrouted,
            "powered-on ENV 2 failed to modulate resonance"
        );
    }

    /// Powered-off kick envelopes must contribute nothing: AMP off behaves
    /// exactly like a flat-1 LUT, PITCH off like a flat-0 LUT.
    #[test]
    fn disabled_kick_envelopes_are_inert() {
        const SAMPLES: usize = 4_000;
        let params = |amp_env_enabled, pitch_env_enabled| VoiceParams {
            level: 1.0,
            keytrack_enabled: false,
            tuning_scale: 1.0,
            note_length_ms: 1_000.0,
            pitch_hz: 55.0,
            waveform: Waveform::Sine,
            amp_env_enabled,
            pitch_env_enabled,
        };
        let render = |params: VoiceParams,
                      amp_lut: &[f32; CURVE_LUT_SIZE],
                      pitch_lut: &[f32; CURVE_LUT_SIZE]| {
            let mut voice = KickVoice::default();
            voice.trigger(0.0, true, false);
            (0..SAMPLES)
                .map(|_| voice.next_sample(params, amp_lut, pitch_lut))
                .collect::<Vec<_>>()
        };

        let ones = [1.0_f32; CURVE_LUT_SIZE];
        let zeroes = [0.0_f32; CURVE_LUT_SIZE];
        // A falling envelope — the defaults decay from 1 to 0.
        let mut decay = [0.0_f32; CURVE_LUT_SIZE];
        for (index, value) in decay.iter_mut().enumerate() {
            *value = 1.0 - index as f32 / (CURVE_LUT_SIZE as f32 - 1.0);
        }

        // AMP off == flat-1 amp LUT, whatever the real curve says.
        let off = render(params(false, true), &decay, &zeroes);
        let flat = render(params(true, true), &ones, &zeroes);
        assert_eq!(off, flat, "disabled AMP envelope still shaped the signal");

        // PITCH off == flat-0 pitch LUT, whatever the real curve says.
        let off = render(params(true, false), &ones, &decay);
        let flat = render(params(true, true), &ones, &zeroes);
        assert_eq!(
            off, flat,
            "disabled PITCH envelope still swept the pitch"
        );

        // Mirror: powered on, the same curves must actually drive the sound.
        let driven = render(params(true, true), &decay, &decay);
        assert_ne!(driven, flat, "powered-on envelopes failed to modulate");
    }

    /// Glide is an exponential slew: an overlapping note-on leaves the pitch
    /// mid-travel, converging on the target after a few time constants —
    /// never snapping. This is the 303-style constant-time portamento.
    #[test]
    fn glide_slews_overlapping_notes() {
        const GLIDE_MS: f32 = 50.0;
        let params = BassVoiceParams {
            glide_ms: GLIDE_MS,
            ..resonant_test_params(BassFilterSlope::S6dB, BassFilterMode::LowPass, 0.0)
        };
        let amp_lut = [1.0_f32; CURVE_LUT_SIZE];
        let filter_lut = [1.0_f32; CURVE_LUT_SIZE];
        let lfos = [LfoParams::default(); BASS_LFO_COUNT];
        let mut voice = BassVoice::default();
        voice.set_sample_rate(44_100.0);

        // First note snaps (nothing was sounding).
        voice.note_on(55.0, 1.0, 0.0, true, true, &lfos, GLIDE_MS, false);
        assert_eq!(voice.glide_hz, 55.0);
        for _ in 0..4_410 {
            voice.next_sample(params, &amp_lut, &filter_lut, &filter_lut);
        }

        // Overlapping note glides from 55 toward 110.
        voice.note_on(110.0, 1.0, 0.0, false, true, &lfos, GLIDE_MS, false);
        assert!(
            voice.glide_hz < 60.0,
            "glide should start near the old pitch, got {}",
            voice.glide_hz
        );
        // ~3τ (150 ms) should cover >95% of the interval.
        for _ in 0..6_615 {
            voice.next_sample(params, &amp_lut, &filter_lut, &filter_lut);
        }
        assert!(
            voice.glide_hz > 107.0,
            "glide should be near target after ~3τ, got {}",
            voice.glide_hz
        );
    }

    /// Auto glide restricts the slew to overlapping (legato) notes — a
    /// note that retriggers the envelope snaps to pitch instead.
    #[test]
    fn glide_auto_snaps_on_retrigger() {
        const GLIDE_MS: f32 = 50.0;
        let lfos = [LfoParams::default(); BASS_LFO_COUNT];
        let mut voice = BassVoice::default();
        voice.set_sample_rate(44_100.0);
        voice.note_on(55.0, 1.0, 0.0, true, true, &lfos, GLIDE_MS, true);

        // Retriggered note: the previous note is released and the new one
        // cuts its tail (legato off) — snap, not glide.
        voice.note_off();
        voice.note_on(110.0, 1.0, 0.0, true, false, &lfos, GLIDE_MS, true);
        assert_eq!(voice.glide_hz, 110.0);

        // Overlapping note (legato steal): still glides in Auto mode.
        voice.note_on(220.0, 1.0, 0.0, false, true, &lfos, GLIDE_MS, true);
        assert!(
            voice.glide_hz < 115.0,
            "auto glide should still slew on overlap, got {}",
            voice.glide_hz
        );
    }
}
