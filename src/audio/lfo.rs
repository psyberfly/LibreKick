use std::f32::consts::TAU;

use crate::shared::{LfoParams, LfoShape};

/// Seed for the sample-and-hold random shape, so a retriggered LFO is
/// deterministic and patches sound the same on every replay.
const RANDOM_SEED: u32 = 0x9E37_79B9;

/// One low-frequency oscillator. Phase is tracked as 0..1 and advanced per
/// sample, so changing rate mid-note glides instead of jumping.
pub struct Lfo {
    sample_rate: f32,
    phase: f32,
    random_state: u32,
    /// Current sample-and-hold output, refreshed when the phase wraps.
    random_value: f32,
}

impl Default for Lfo {
    fn default() -> Self {
        Self {
            sample_rate: 44_100.0,
            phase: 0.0,
            random_state: RANDOM_SEED,
            random_value: 0.0,
        }
    }
}

impl Lfo {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
    }

    /// Restarts the phase at the configured offset. Called on note-on for
    /// retriggering LFOs; free-running ones keep their phase.
    pub fn retrigger(&mut self, params: &LfoParams) {
        self.phase = (params.phase_deg / 360.0).fract().clamp(0.0, 1.0);
        self.random_state = RANDOM_SEED;
        self.random_value = Self::next_random(&mut self.random_state);
    }

    fn next_random(state: &mut u32) -> f32 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *state = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Advances one sample and returns the output in -1..1, already scaled by
    /// depth. A disabled LFO outputs silence so routings referencing it go
    /// inert without needing to be unassigned.
    pub fn next_sample(&mut self, params: &LfoParams, beats_per_second: f32) -> f32 {
        if !params.enabled {
            return 0.0;
        }

        let frequency = if params.tempo_sync {
            // `rate_beats` is the cycle length in beats, so a 1-beat cycle at
            // 120 BPM (2 beats/sec) runs at 2 Hz.
            beats_per_second.max(0.0) / params.rate_beats.max(0.0625)
        } else {
            params.rate_hz.max(0.0)
        };

        let shape_value = self.shape_value(params.shape);

        self.phase += frequency / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
            // Sample-and-hold steps exactly once per cycle.
            self.random_value = Self::next_random(&mut self.random_state);
        }

        shape_value * params.depth.clamp(0.0, 1.0)
    }

    fn shape_value(&self, shape: LfoShape) -> f32 {
        match shape {
            // Sample-and-hold is stateful, so it cannot come from the
            // stateless drawing helper.
            LfoShape::Random => self.random_value,
            // Everything else shares one implementation with the UI so the
            // animated dot always sits exactly on the drawn curve.
            other => shape_at(other, self.phase),
        }
    }

    /// Current phase, 0..1, for the UI playhead.
    pub fn phase(&self) -> f32 {
        self.phase
    }
}

/// Evaluates an LFO shape at an arbitrary phase without advancing state.
/// The UI uses this to draw the shape outline in the LFO display.
pub fn shape_at(shape: LfoShape, phase: f32) -> f32 {
    let phase = phase.rem_euclid(1.0);
    match shape {
        LfoShape::Sine => (phase * TAU).sin(),
        LfoShape::Triangle => {
            if phase < 0.5 {
                phase * 4.0 - 1.0
            } else {
                3.0 - phase * 4.0
            }
        }
        LfoShape::SawUp => phase * 2.0 - 1.0,
        LfoShape::SawDown => 1.0 - phase * 2.0,
        LfoShape::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        // Random has no drawable outline; callers render it as a flat line.
        LfoShape::Random => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(shape: LfoShape) -> LfoParams {
        LfoParams {
            enabled: true,
            shape,
            rate_hz: 1.0,
            depth: 1.0,
            phase_deg: 0.0,
            tempo_sync: false,
            rate_beats: 1.0,
            retrigger: true,
        }
    }

    /// A disabled LFO must contribute nothing, so unused routings are inert.
    #[test]
    fn disabled_lfo_is_silent() {
        let mut lfo = Lfo::default();
        let mut settings = params(LfoShape::Sine);
        settings.enabled = false;
        for _ in 0..1000 {
            assert_eq!(lfo.next_sample(&settings, 2.0), 0.0);
        }
    }

    /// Every shape must stay inside -1..1 across a full cycle, otherwise
    /// modulation could push destinations far out of range.
    #[test]
    fn shapes_stay_in_range() {
        for shape in LfoShape::ALL {
            let mut lfo = Lfo::default();
            lfo.set_sample_rate(1000.0);
            let settings = params(shape);
            for _ in 0..4000 {
                let value = lfo.next_sample(&settings, 2.0);
                assert!(
                    (-1.0..=1.0).contains(&value),
                    "{shape:?} produced {value} outside -1..1"
                );
            }
        }
    }

    /// Depth must scale the output linearly.
    #[test]
    fn depth_scales_output() {
        let mut full = Lfo::default();
        let mut half = Lfo::default();
        full.set_sample_rate(1000.0);
        half.set_sample_rate(1000.0);
        let full_params = params(LfoShape::SawUp);
        let half_params = LfoParams {
            depth: 0.5,
            ..full_params
        };
        for _ in 0..500 {
            let a = full.next_sample(&full_params, 2.0);
            let b = half.next_sample(&half_params, 2.0);
            assert!((a * 0.5 - b).abs() < 1e-6);
        }
    }

    /// Tempo sync must track the host tempo: a one-beat cycle at 2 beats per
    /// second runs at 2 Hz. Asserted at mid-cycle positions, which is robust
    /// to float accumulation error at the wrap boundary.
    #[test]
    fn tempo_sync_follows_host() {
        let mut lfo = Lfo::default();
        lfo.set_sample_rate(1000.0);
        let settings = LfoParams {
            tempo_sync: true,
            rate_beats: 1.0,
            ..params(LfoShape::SawUp)
        };

        // 2 Hz at 1000 samples/sec: half a cycle after 250 samples.
        for _ in 0..250 {
            lfo.next_sample(&settings, 2.0);
        }
        assert!(
            (lfo.phase() - 0.5).abs() < 0.02,
            "phase {} not ~0.5 at 2 beats/sec",
            lfo.phase()
        );

        // 4 beats/sec doubles the rate: another half cycle in 125 samples,
        // wrapping the phase back to ~0.
        for _ in 0..125 {
            lfo.next_sample(&settings, 4.0);
        }
        let phase = lfo.phase();
        assert!(
            phase < 0.05 || phase > 0.95,
            "phase {phase} not ~0 after wrap at 4 beats/sec"
        );
    }

    /// The drawing helper must agree with the running oscillator at the
    /// phases the UI samples, so the dot sits on the drawn curve.
    #[test]
    fn shape_at_matches_running_lfo() {
        for shape in [
            LfoShape::Sine,
            LfoShape::Triangle,
            LfoShape::SawUp,
            LfoShape::SawDown,
            LfoShape::Square,
        ] {
            let mut lfo = Lfo::default();
            lfo.set_sample_rate(1000.0);
            let settings = params(shape);
            for _ in 0..997 {
                let phase = lfo.phase();
                let running = lfo.next_sample(&settings, 2.0);
                let drawn = shape_at(shape, phase);
                assert!(
                    (running - drawn).abs() < 1e-5,
                    "{shape:?} mismatch at phase {phase}: {running} vs {drawn}"
                );
            }
        }
    }
}
