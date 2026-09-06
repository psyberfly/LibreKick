mod commands;
mod oscilloscope;
mod snapshot;
mod state;

use crate::interface::{BassCommand, BassSnapshot, KickCommand, KickSnapshot, OscilloscopeSignal, OscilloscopeSnapshot};

pub use crate::interface::Waveform;


pub use oscilloscope::{commit_oscilloscope_frame, oscilloscope_snapshot, publish_oscilloscope_signal_block};
pub use snapshot::{bass_snapshot, kick_snapshot};
pub use state::{new_shared_state, SharedStateHandle};

impl crate::interface::AudioEnginePort for SharedStateHandle {
    fn kick_snapshot(&self) -> KickSnapshot {
        kick_snapshot(self)
    }

    fn bass_snapshot(&self) -> BassSnapshot {
        bass_snapshot(self)
    }

    fn publish_oscilloscope_signal_block(&self, signal: OscilloscopeSignal, samples: &[f32]) {
        publish_oscilloscope_signal_block(self, signal, samples);
    }

    fn commit_oscilloscope_frame(&self) {
        commit_oscilloscope_frame(self);
    }
}

impl crate::interface::UiEnginePort for SharedStateHandle {
    fn apply_kick_command(&self, command: KickCommand) {
        commands::apply_kick_command(self, command);
    }

    fn apply_bass_command(&self, command: BassCommand) {
        commands::apply_bass_command(self, command);
    }

    fn oscilloscope_snapshot(&self) -> OscilloscopeSnapshot {
        oscilloscope_snapshot(self)
    }
}
