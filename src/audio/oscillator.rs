use std::f32::consts::TAU;

use crate::shared::Waveform;

#[derive(Clone, Copy)]
pub struct Oscillator {
    sample_rate: f32,
    phase: f32,
    active: bool,
}

impl Default for Oscillator {
    fn default() -> Self {
        Self {
            sample_rate: 44_100.0,
            phase: 0.0,
            active: false,
        }
    }
}

impl Oscillator {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
    }

    pub fn note_on(&mut self, retrigger: bool, legato_voice_steal: bool) -> bool {
        if self.active && !legato_voice_steal {
            return false;
        }

        let was_active = self.active;
        self.active = true;
        if retrigger || !was_active {
            self.phase = 0.0;
        }
        true
    }

    pub fn note_off(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn render_sample(&mut self, waveform: Waveform, frequency_hz: f32, amplitude: f32) -> f32 {
        if !self.active {
            return 0.0;
        }

        let frequency_hz = frequency_hz.clamp(20.0, 20_000.0);
        self.phase += frequency_hz / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        match waveform {
            Waveform::Saw => (2.0 * self.phase - 1.0) * amplitude,
            Waveform::Square => {
                if self.phase < 0.5 {
                    amplitude
                } else {
                    -amplitude
                }
            }
            Waveform::Sine => (self.phase * TAU).sin() * amplitude,
        }
    }
}
