use std::f32::consts::TAU;

use crate::shared::CURVE_LUT_SIZE;

#[derive(Clone, Copy)]
pub struct VoiceParams {
    pub decay_ms: f32,
    pub base_freq_hz: f32,
    pub pitch_drop_hz: f32,
    pub level: f32,
}

pub struct KickVoice {
    sample_rate: f32,
    active: bool,
    phase: f32,
    time_seconds: f32,
}

impl Default for KickVoice {
    fn default() -> Self {
        Self {
            sample_rate: 44_100.0,
            active: false,
            phase: 0.0,
            time_seconds: 0.0,
        }
    }
}

impl KickVoice {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
    }

    pub fn trigger(&mut self) {
        self.active = true;
        self.phase = 0.0;
        self.time_seconds = 0.0;
    }

    pub fn next_sample(
        &mut self,
        params: VoiceParams,
        amp_lut: &[f32; CURVE_LUT_SIZE],
        pitch_lut: &[f32; CURVE_LUT_SIZE],
    ) -> f32 {
        if !self.active {
            return 0.0;
        }

        let duration_seconds = (params.decay_ms * 0.001).max(0.02);
        let normalized_time = (self.time_seconds / duration_seconds).clamp(0.0, 1.0);
        let lut_index = ((normalized_time * (CURVE_LUT_SIZE as f32 - 1.0)).round() as usize)
            .min(CURVE_LUT_SIZE - 1);

        let amp_curve = amp_lut[lut_index].clamp(0.0, 1.0);
        let pitch_curve = pitch_lut[lut_index].clamp(0.0, 1.0);

        let amplitude = params.level.clamp(0.0, 1.0) * amp_curve;
        let frequency = (params.base_freq_hz + params.pitch_drop_hz * pitch_curve).max(20.0);

        self.phase += TAU * frequency / self.sample_rate;
        if self.phase >= TAU {
            self.phase -= TAU;
        }

        let sample = self.phase.sin() * amplitude;

        self.time_seconds += 1.0 / self.sample_rate;
        if normalized_time >= 1.0 || amplitude < 0.0005 {
            self.active = false;
        }

        sample
    }
}
