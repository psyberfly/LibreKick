pub const CURVE_LUT_SIZE: usize = 256;
pub const OSCILLOSCOPE_BUFFER_SIZE: usize = 2048;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CurveKind {
    Amplitude,
    Pitch,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Waveform {
    Saw,
    Square,
    Sine,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BassFilterMode {
    LowPass,
    HighPass,
    BandPass,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OscilloscopeSignal {
    Kick,
    Bass,
    Sum,
}

#[derive(Clone, PartialEq)]
pub struct OscilloscopeSnapshot {
    pub kick: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    pub bass: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    pub sum: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    pub len: usize,
    pub sequence: u64,
}

pub use crate::audio::devices::bass::{BassCommand, BassSnapshot};
pub use crate::audio::devices::kick::{KickCommand, KickSnapshot};

pub trait AudioEnginePort {
    fn kick_snapshot(&self) -> KickSnapshot;
    fn bass_snapshot(&self) -> BassSnapshot;
    fn publish_oscilloscope_signal_block(&self, signal: OscilloscopeSignal, samples: &[f32]);
    fn commit_oscilloscope_frame(&self);
}

pub trait UiEnginePort {
    fn apply_kick_command(&self, command: KickCommand);
    fn apply_bass_command(&self, command: BassCommand);
    fn oscilloscope_snapshot(&self) -> OscilloscopeSnapshot;
}
