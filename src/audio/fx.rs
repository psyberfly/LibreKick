use std::f32::consts::TAU;

/// Comb filter delay lengths in samples at 44.1 kHz, from the classic
/// Schroeder/Freeverb tuning. Scaled to the running sample rate.
const COMB_TUNING: [usize; 4] = [1116, 1188, 1277, 1356];
/// All-pass diffuser lengths in samples at 44.1 kHz.
const ALLPASS_TUNING: [usize; 2] = [556, 441];
const TUNING_SAMPLE_RATE: f32 = 44_100.0;

/// Longest delay the delay line must be able to hold, in milliseconds.
const MAX_DELAY_MS: f32 = 1_000.0;
/// Longest chorus delay, in milliseconds.
const MAX_CHORUS_MS: f32 = 40.0;

/// Settings for the bass FX rack. Every section is bypassed by default so a
/// patch without FX produces bit-identical output to the dry voice.
#[derive(Clone, Copy)]
pub struct BassFxParams {
    pub drive_enabled: bool,
    /// 0..1, how hard the signal is pushed into the saturator.
    pub drive_amount: f32,
    pub drive_mix: f32,
    pub chorus_enabled: bool,
    pub chorus_rate_hz: f32,
    pub chorus_depth: f32,
    pub chorus_mix: f32,
    pub delay_enabled: bool,
    pub delay_time_ms: f32,
    pub delay_feedback: f32,
    pub delay_mix: f32,
    pub reverb_enabled: bool,
    pub reverb_size: f32,
    pub reverb_damping: f32,
    pub reverb_mix: f32,
}

impl Default for BassFxParams {
    fn default() -> Self {
        Self {
            drive_enabled: false,
            drive_amount: 0.3,
            drive_mix: 1.0,
            chorus_enabled: false,
            chorus_rate_hz: 0.8,
            chorus_depth: 0.5,
            chorus_mix: 0.4,
            delay_enabled: false,
            delay_time_ms: 250.0,
            delay_feedback: 0.35,
            delay_mix: 0.3,
            reverb_enabled: false,
            reverb_size: 0.5,
            reverb_damping: 0.5,
            reverb_mix: 0.25,
        }
    }
}

fn lerp(dry: f32, wet: f32, mix: f32) -> f32 {
    let mix = mix.clamp(0.0, 1.0);
    dry * (1.0 - mix) + wet * mix
}

/// Circular delay line with fractional read, used by chorus and delay.
struct DelayLine {
    buffer: Vec<f32>,
    write: usize,
}

impl DelayLine {
    fn new() -> Self {
        Self {
            buffer: vec![0.0; 1],
            write: 0,
        }
    }

    fn resize(&mut self, len: usize) {
        self.buffer = vec![0.0; len.max(2)];
        self.write = 0;
    }

    fn clear(&mut self) {
        self.buffer.iter_mut().for_each(|sample| *sample = 0.0);
        self.write = 0;
    }

    fn push(&mut self, value: f32) {
        self.buffer[self.write] = value;
        self.write = (self.write + 1) % self.buffer.len();
    }

    /// Reads `delay_samples` behind the write head, interpolating between
    /// neighbours so modulated delay times do not zipper.
    fn read(&self, delay_samples: f32) -> f32 {
        let len = self.buffer.len();
        let max_delay = (len - 1) as f32;
        let delay = delay_samples.clamp(1.0, max_delay);
        let mut position = self.write as f32 - delay;
        if position < 0.0 {
            position += len as f32;
        }
        let index = position.floor();
        let frac = position - index;
        let i0 = (index as usize) % len;
        let i1 = (i0 + 1) % len;
        self.buffer[i0] * (1.0 - frac) + self.buffer[i1] * frac
    }
}

/// Lowpass-damped comb filter, the reverb's energy store.
struct Comb {
    buffer: Vec<f32>,
    index: usize,
    damp_store: f32,
}

impl Comb {
    fn new() -> Self {
        Self {
            buffer: vec![0.0; 1],
            index: 0,
            damp_store: 0.0,
        }
    }

    fn resize(&mut self, len: usize) {
        self.buffer = vec![0.0; len.max(1)];
        self.index = 0;
        self.damp_store = 0.0;
    }

    fn clear(&mut self) {
        self.buffer.iter_mut().for_each(|sample| *sample = 0.0);
        self.index = 0;
        self.damp_store = 0.0;
    }

    fn process(&mut self, input: f32, feedback: f32, damping: f32) -> f32 {
        let output = self.buffer[self.index];
        self.damp_store = output * (1.0 - damping) + self.damp_store * damping;
        self.buffer[self.index] = input + self.damp_store * feedback;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

/// All-pass diffuser, smears comb output into a denser tail.
struct AllPass {
    buffer: Vec<f32>,
    index: usize,
}

impl AllPass {
    fn new() -> Self {
        Self {
            buffer: vec![0.0; 1],
            index: 0,
        }
    }

    fn resize(&mut self, len: usize) {
        self.buffer = vec![0.0; len.max(1)];
        self.index = 0;
    }

    fn clear(&mut self) {
        self.buffer.iter_mut().for_each(|sample| *sample = 0.0);
        self.index = 0;
    }

    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        let output = buffered - input;
        self.buffer[self.index] = input + buffered * 0.5;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

/// Serial FX chain applied to the bass bus: drive, chorus, delay, reverb.
pub struct BassFx {
    sample_rate: f32,
    chorus: DelayLine,
    chorus_phase: f32,
    delay: DelayLine,
    combs: [Comb; COMB_TUNING.len()],
    allpasses: [AllPass; ALLPASS_TUNING.len()],
}

impl Default for BassFx {
    fn default() -> Self {
        let mut fx = Self {
            sample_rate: TUNING_SAMPLE_RATE,
            chorus: DelayLine::new(),
            chorus_phase: 0.0,
            delay: DelayLine::new(),
            combs: std::array::from_fn(|_| Comb::new()),
            allpasses: std::array::from_fn(|_| AllPass::new()),
        };
        fx.set_sample_rate(TUNING_SAMPLE_RATE);
        fx
    }
}

impl BassFx {
    /// Reallocates every buffer for the new rate. Called from prepare, never
    /// from the audio callback.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
        let scale = self.sample_rate / TUNING_SAMPLE_RATE;

        self.chorus
            .resize((MAX_CHORUS_MS * 0.001 * self.sample_rate).ceil() as usize + 4);
        self.delay
            .resize((MAX_DELAY_MS * 0.001 * self.sample_rate).ceil() as usize + 4);

        for (comb, tuning) in self.combs.iter_mut().zip(COMB_TUNING) {
            comb.resize((tuning as f32 * scale).round() as usize);
        }
        for (allpass, tuning) in self.allpasses.iter_mut().zip(ALLPASS_TUNING) {
            allpass.resize((tuning as f32 * scale).round() as usize);
        }
    }

    pub fn reset(&mut self) {
        self.chorus.clear();
        self.chorus_phase = 0.0;
        self.delay.clear();
        self.combs.iter_mut().for_each(Comb::clear);
        self.allpasses.iter_mut().for_each(AllPass::clear);
    }

    pub fn process(&mut self, input: f32, params: &BassFxParams) -> f32 {
        let mut signal = input;

        if params.drive_enabled {
            let amount = params.drive_amount.clamp(0.0, 1.0);
            let gain = 1.0 + amount * 24.0;
            // Pull the level back down so turning drive up changes tone
            // rather than simply getting louder.
            let compensation = 1.0 / (1.0 + amount * 3.0);
            let driven = (signal * gain).tanh() * compensation;
            signal = lerp(signal, driven, params.drive_mix);
        }

        if params.chorus_enabled {
            self.chorus_phase += params.chorus_rate_hz.max(0.0) / self.sample_rate;
            if self.chorus_phase >= 1.0 {
                self.chorus_phase -= 1.0;
            }
            let lfo = (self.chorus_phase * TAU).sin();
            let centre_ms = 14.0;
            let depth_ms = 8.0 * params.chorus_depth.clamp(0.0, 1.0);
            let delay_samples = (centre_ms + lfo * depth_ms) * 0.001 * self.sample_rate;
            let wet = self.chorus.read(delay_samples);
            self.chorus.push(signal);
            signal = lerp(signal, wet, params.chorus_mix);
        }

        if params.delay_enabled {
            let delay_samples =
                params.delay_time_ms.clamp(1.0, MAX_DELAY_MS) * 0.001 * self.sample_rate;
            let wet = self.delay.read(delay_samples);
            let feedback = params.delay_feedback.clamp(0.0, 0.95);
            self.delay.push(signal + wet * feedback);
            signal = lerp(signal, wet, params.delay_mix);
        }

        if params.reverb_enabled {
            let feedback = 0.70 + params.reverb_size.clamp(0.0, 1.0) * 0.28;
            let damping = params.reverb_damping.clamp(0.0, 1.0) * 0.4;
            let mut wet = 0.0;
            for comb in self.combs.iter_mut() {
                wet += comb.process(signal, feedback, damping);
            }
            wet /= self.combs.len() as f32;
            for allpass in self.allpasses.iter_mut() {
                wet = allpass.process(wet);
            }
            signal = lerp(signal, wet, params.reverb_mix);
        }

        signal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With every section bypassed the rack must be a pure pass-through, so
    /// patches without FX sound exactly as they did before the rack existed.
    #[test]
    fn bypassed_rack_is_transparent() {
        let mut fx = BassFx::default();
        let params = BassFxParams::default();
        for step in 0..256 {
            let input = (step as f32 * 0.05).sin();
            assert_eq!(fx.process(input, &params), input);
        }
    }

    /// A decaying delay tail must stay finite rather than running away.
    #[test]
    fn delay_feedback_stays_bounded() {
        let mut fx = BassFx::default();
        let params = BassFxParams {
            delay_enabled: true,
            delay_time_ms: 10.0,
            delay_feedback: 0.95,
            delay_mix: 1.0,
            ..BassFxParams::default()
        };
        let mut peak = 0.0_f32;
        for step in 0..100_000 {
            let input = if step < 64 { 1.0 } else { 0.0 };
            peak = peak.max(fx.process(input, &params).abs());
        }
        assert!(
            peak.is_finite() && peak < 20.0,
            "delay tail blew up: {peak}"
        );
    }

    /// Reverb must also settle rather than oscillate without bound.
    #[test]
    fn reverb_tail_stays_bounded() {
        let mut fx = BassFx::default();
        let params = BassFxParams {
            reverb_enabled: true,
            reverb_size: 1.0,
            reverb_damping: 0.0,
            reverb_mix: 1.0,
            ..BassFxParams::default()
        };
        let mut peak = 0.0_f32;
        for step in 0..100_000 {
            let input = if step < 64 { 1.0 } else { 0.0 };
            peak = peak.max(fx.process(input, &params).abs());
        }
        assert!(
            peak.is_finite() && peak < 20.0,
            "reverb tail blew up: {peak}"
        );
    }
}
