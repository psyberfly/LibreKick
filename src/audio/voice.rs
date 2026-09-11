use std::f32::consts::TAU;

use crate::shared::{BassFilterMode, BassFilterSlope, Waveform, CURVE_LUT_SIZE};

use super::oscillator::Oscillator;

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
    pub waveform: Waveform,
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
    oscillator: Oscillator,
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
            oscillator: Oscillator::default(),
            time_seconds: 0.0,
            note_hz: 55.0,
            velocity: 1.0,
            hp_prev_in: [0.0; BassFilterSlope::S24dB.poles()],
            hp_prev_out: [0.0; BassFilterSlope::S24dB.poles()],
            lp_prev_out: [0.0; BassFilterSlope::S24dB.poles()],
        }
    }
}

fn pitch_curve_to_hz(value: f32) -> f32 {
    let min_hz = 20.0_f32;
    let max_hz = 20_000.0_f32;
    min_hz * (max_hz / min_hz).powf(value.clamp(0.0, 1.0))
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
        // The pitch envelope is a ratio above the base pitch: 1x at the
        // bottom of the curve up to 1000x at the top (same log range as
        // pitch_curve_to_hz, normalized by its 20 Hz minimum).
        let pitch_ratio = pitch_curve_to_hz(pitch_curve) / 20.0;
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
        self.oscillator.set_sample_rate(self.sample_rate);
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
    ) {
        if !self.oscillator.note_on(retrigger, legato_voice_steal, phase_offset) {
            return;
        }

        self.time_seconds = 0.0;
        self.note_hz = note_hz.max(20.0);
        self.velocity = velocity.clamp(0.0, 1.0);
    }

    pub fn note_off(&mut self) {
        self.oscillator.note_off();
        self.reset_filter_state();
    }

    pub fn is_active(&self) -> bool {
        self.oscillator.is_active()
    }

    pub fn next_sample(
        &mut self,
        params: BassVoiceParams,
        bass_amp_lut: &[f32; CURVE_LUT_SIZE],
        bass_filter_lut: &[f32; CURVE_LUT_SIZE],
    ) -> f32 {
        if !self.oscillator.is_active() {
            return 0.0;
        }

        let note_length_seconds = (params.note_length_ms * 0.001).clamp(0.001, 2.0);
        let normalized_time = (self.time_seconds / note_length_seconds).clamp(0.0, 1.0);
        let lut_index = ((normalized_time * (CURVE_LUT_SIZE as f32 - 1.0)).round() as usize)
            .min(CURVE_LUT_SIZE - 1);

        let amp_env = bass_amp_lut[lut_index].clamp(0.0, 1.0);
        let cutoff_env = bass_filter_lut[lut_index].clamp(0.0, 1.0);

        let amplitude = params.level.clamp(0.0, 1.0) * self.velocity * amp_env;
        let frequency = (self.note_hz * params.tuning_scale.max(0.5)).clamp(20.0, 20_000.0);

        let raw = self
            .oscillator
            .render_sample(params.waveform, frequency, amplitude);

        let base_cutoff = params.base_cutoff_hz.clamp(20.0, 8_000.0);
        let cutoff_hz = (base_cutoff * (0.25 + cutoff_env * 0.75)).clamp(20.0, 8_000.0);
        let dt = 1.0 / self.sample_rate;
        let rc = 1.0 / (TAU * cutoff_hz.max(20.0));
        let hp_alpha = rc / (rc + dt);
        let lp_alpha = dt / (rc + dt);

        let poles = params.filter_slope.poles();
        let drive = params.filter_drive.clamp(0.0, 1.0);
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
