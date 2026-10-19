use std::f32::consts::TAU;

use crate::shared::Waveform;

#[derive(Clone, Copy)]
pub struct Oscillator {
    sample_rate: f32,
    phase: f32,
    active: bool,
    /// PolyBLEP edge correction on saw/square. On by default.
    anti_aliasing: bool,
}

impl Default for Oscillator {
    fn default() -> Self {
        Self {
            sample_rate: 44_100.0,
            phase: 0.0,
            active: false,
            anti_aliasing: true,
        }
    }
}

impl Oscillator {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
    }

    /// Enables or disables PolyBLEP edge correction on saw and square.
    /// Default is on; off is available as a raw, deliberately aliased flavour.
    pub fn set_anti_aliasing(&mut self, enabled: bool) {
        self.anti_aliasing = enabled;
    }

    /// `phase_offset` is the oscillator start phase in cycles (0.0-1.0),
    /// applied whenever the phase is reset (retrigger or first note-on).
    pub fn note_on(
        &mut self,
        retrigger: bool,
        legato_voice_steal: bool,
        phase_offset: f32,
    ) -> bool {
        if self.active && !legato_voice_steal {
            return false;
        }

        let was_active = self.active;
        self.active = true;
        if retrigger || !was_active {
            self.phase = phase_offset.rem_euclid(1.0);
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

        // Phase increment in cycles, needed by the PolyBLEP edge corrections.
        let dt = frequency_hz / self.sample_rate;
        let aa = self.anti_aliasing;
        let blep = |t: f32| if aa { poly_blep(t, dt) } else { 0.0 };

        let value = match waveform {
            // Naive waveforms have infinite harmonics that fold back above
            // Nyquist (aliasing). PolyBLEP subtracts a polynomial correction
            // at each discontinuity, band-limiting the edge.
            Waveform::Saw => 2.0 * self.phase - 1.0 - blep(self.phase),
            Waveform::Square => {
                let naive = if self.phase < 0.5 { 1.0 } else { -1.0 };
                naive + blep(self.phase) - blep((self.phase + 0.5).fract())
            }
            // Sine has no harmonics above the fundamental, so no correction.
            Waveform::Sine => (self.phase * TAU).sin(),
        };
        value * amplitude
    }
}

/// Polynomial BLEP residual: the error between a naive step edge and a
/// band-limited one, evaluated near a discontinuity. `t` is phase in cycles
/// measured from the edge; `dt` is the per-sample phase increment. Returns 0
/// away from edges so the correction only touches the transition samples.
fn poly_blep(t: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Single-bin DFT magnitude at `hz`.
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

    /// A 4096 Hz saw's 7th harmonic sits at 28672 Hz, above Nyquist, and folds
    /// back to 15428 Hz — a bin no true harmonic occupies. Without PolyBLEP
    /// that bin carries the full folded harmonic; with it, most is removed.
    #[test]
    fn poly_blep_suppresses_aliased_foldback() {
        const SR: f32 = 44_100.0;
        const F0: f32 = 4_096.0;
        const N: usize = 44_100;

        let mut osc = Oscillator::default();
        osc.set_sample_rate(SR);
        osc.note_on(true, true, 0.0);
        let blep: Vec<f32> = (0..N)
            .map(|_| osc.render_sample(Waveform::Saw, F0, 1.0))
            .collect();

        // Reference: the same phase sequence rendered naively.
        let mut naive = Vec::with_capacity(N);
        let mut phase = 0.0f32;
        for _ in 0..N {
            phase += F0 / SR;
            if phase >= 1.0 {
                phase -= 1.0;
            }
            naive.push(2.0 * phase - 1.0);
        }

        const ALIAS_BIN_HZ: f32 = 15_428.0;
        let blep_mag = goertzel(&blep[1_000..], ALIAS_BIN_HZ, SR);
        let naive_mag = goertzel(&naive[1_000..], ALIAS_BIN_HZ, SR);
        assert!(
            naive_mag > 0.02,
            "test setup wrong: no aliased content at {ALIAS_BIN_HZ} Hz ({naive_mag})"
        );
        assert!(
            blep_mag < naive_mag * 0.5,
            "PolyBLEP did not suppress foldback: blep={blep_mag}, naive={naive_mag}"
        );
    }

    /// With AA off the render must be the exact naive waveform — no residual
    /// PolyBLEP correction may leak into the raw path.
    #[test]
    fn aa_off_renders_exact_naive_waveform() {
        const SR: f32 = 44_100.0;
        for hz in [55.0_f32, 440.0, 4_096.0] {
            // Each render_sample call advances the phase, so every waveform
            // gets its own oscillator and mirrored naive phase.
            for waveform in [Waveform::Saw, Waveform::Square] {
                let mut osc = Oscillator::default();
                osc.set_sample_rate(SR);
                osc.set_anti_aliasing(false);
                osc.note_on(true, true, 0.0);

                let mut phase = 0.0f32;
                for _ in 0..2_000 {
                    phase += hz / SR;
                    if phase >= 1.0 {
                        phase -= 1.0;
                    }
                    let naive = match waveform {
                        Waveform::Saw => 2.0 * phase - 1.0,
                        Waveform::Square => {
                            if phase < 0.5 {
                                1.0
                            } else {
                                -1.0
                            }
                        }
                        Waveform::Sine => (phase * TAU).sin(),
                    };
                    assert_eq!(osc.render_sample(waveform, hz, 1.0), naive);
                }
            }
        }
    }

    /// Edge corrections must never push the output far outside ±1 even when
    /// the two square corrections overlap at high frequencies.
    #[test]
    fn poly_blep_output_stays_bounded() {
        for hz in [55.0_f32, 1_000.0, 8_000.0, 15_000.0] {
            let mut osc = Oscillator::default();
            osc.set_sample_rate(44_100.0);
            osc.note_on(true, true, 0.0);
            for waveform in [Waveform::Saw, Waveform::Square] {
                for _ in 0..10_000 {
                    let x = osc.render_sample(waveform, hz, 1.0);
                    assert!(
                        x.is_finite() && x.abs() < 1.5,
                        "{waveform:?} @ {hz} Hz = {x}"
                    );
                }
            }
        }
    }
}
