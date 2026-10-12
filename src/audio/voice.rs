use std::f32::consts::TAU;

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
    if count == 1 {
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
    hp_prev_in: [f32; BassFilterSlope::S24dB.poles()],
    hp_prev_out: [f32; BassFilterSlope::S24dB.poles()],
    lp_prev_out: [f32; BassFilterSlope::S24dB.poles()],
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
            hp_prev_in: [0.0; BassFilterSlope::S24dB.poles()],
            hp_prev_out: [0.0; BassFilterSlope::S24dB.poles()],
            lp_prev_out: [0.0; BassFilterSlope::S24dB.poles()],
        }
    }
}


impl KickVoice {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
        self.oscillator.set_sample_rate(self.sample_rate);
    }

    pub fn trigger(&mut self, phase_offset: f32, retrigger: bool, legato_voice_steal: bool) {
        self.trigger_with_velocity(1.0, phase_offset, retrigger, legato_voice_steal);
    }

    pub fn is_active(&self) -> bool {
        self.oscillator.is_active()
    }

    pub fn trigger_with_velocity(&mut self, velocity: f32, phase_offset: f32, retrigger: bool, legato_voice_steal: bool) {
        if !self.oscillator.note_on(retrigger, legato_voice_steal, phase_offset) {
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
        if !self.oscillator.note_on(retrigger, legato_voice_steal, phase_offset) {
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
        let lut_index = ((normalized_time * (CURVE_LUT_SIZE as f32 - 1.0)).round() as usize)
            .min(CURVE_LUT_SIZE - 1);

        let amp_curve = amp_lut[lut_index].clamp(0.0, 1.0);
        let pitch_curve = pitch_lut[lut_index].clamp(0.0, 1.0);
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
        let frequency = (base_hz * pitch_ratio * params.tuning_scale.max(0.5))
            .clamp(20.0, 20_000.0);

        let sample = self
            .oscillator
            .render_sample(params.waveform, frequency, amplitude);

        self.time_seconds += 1.0 / self.sample_rate;
        if self.time_seconds >= note_length_seconds || normalized_time >= 1.0 || amplitude < 0.0005 {
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
    }

    pub fn note_on(
        &mut self,
        note_hz: f32,
        velocity: f32,
        phase_offset: f32,
        retrigger: bool,
        legato_voice_steal: bool,
        lfos: &[LfoParams; BASS_LFO_COUNT],
    ) {
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
        self.velocity = velocity.clamp(0.0, 1.0);
    }

    pub fn note_off(&mut self) {
        for voice in self.osc_a.iter_mut().chain(self.osc_b.iter_mut()) {
            voice.note_off();
        }
        self.sub.note_off();
        self.reset_filter_state();
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
        let lut_index = ((normalized_time * (CURVE_LUT_SIZE as f32 - 1.0)).round() as usize)
            .min(CURVE_LUT_SIZE - 1);

        let amp_env = bass_amp_lut[lut_index].clamp(0.0, 1.0);
        let cutoff_env_1 = bass_filter_lut[lut_index].clamp(0.0, 1.0);
        let cutoff_env_2 = bass_filter_2_lut[lut_index].clamp(0.0, 1.0);

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
            }
        }

        let amplitude = (params.level.clamp(0.0, 1.0)
            * velocity
            * amp_env
            * (1.0 + modulation.level))
            .clamp(0.0, 4.0);
        let frequency = (self.note_hz
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

        if params.sub_enabled {
            let sub_hz = (frequency * octave_ratio(params.sub_octave)).clamp(20.0, 20_000.0);
            mix += self
                .sub
                .render_sample(params.sub_waveform, sub_hz, 1.0)
                * (params.sub_level + modulation.sub_mix).clamp(0.0, 1.0);
        }

        if params.noise_enabled {
            mix += self.next_noise() * (params.noise_level + modulation.noise_mix).clamp(0.0, 1.0);
        }

        let raw = mix * amplitude;

        let base_cutoff = params.base_cutoff_hz.clamp(20.0, 8_000.0);
        let cutoff_hz = (base_cutoff
            * (0.25 + cutoff_env * 0.75)
            * octaves_ratio(modulation.cutoff_octaves))
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
        let lp_alpha = dt / (rc + dt);

        let poles = params.filter_slope.poles();
        let drive = (params.filter_drive + modulation.filter_drive).clamp(0.0, 1.0);
        let driven = if drive > 0.0 {
            (raw * (1.0 + drive * 4.0)).tanh()
        } else {
            raw
        };

        let mut hp = driven;
        for i in 0..poles {
            let y = hp_alpha * (self.hp_prev_out[i] + hp - self.hp_prev_in[i]);
            self.hp_prev_in[i] = hp;
            self.hp_prev_out[i] = y;
            hp = y;
        }

        let filtered = match params.filter_mode {
            BassFilterMode::LowPass => {
                let mut x = driven;
                for i in 0..poles {
                    x = self.lp_prev_out[i] + lp_alpha * (x - self.lp_prev_out[i]);
                    self.lp_prev_out[i] = x;
                }
                x
            }
            BassFilterMode::HighPass => hp,
            BassFilterMode::BandPass => {
                let mut x = hp;
                for i in 0..poles {
                    x = self.lp_prev_out[i] + lp_alpha * (x - self.lp_prev_out[i]);
                    self.lp_prev_out[i] = x;
                }
                x
            }
        };

        self.time_seconds += dt;
        if self.time_seconds >= note_length_seconds || normalized_time >= 1.0 || amplitude < 0.0005 {
            self.note_off();
        }

        filtered
    }
}
