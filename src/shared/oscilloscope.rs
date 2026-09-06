use crate::interface::{OscilloscopeSignal, OscilloscopeSnapshot, OSCILLOSCOPE_BUFFER_SIZE};

use super::state::SharedStateHandle;

pub fn publish_oscilloscope_signal_block(
    shared: &SharedStateHandle,
    signal: OscilloscopeSignal,
    samples: &[f32],
) {
    if let Ok(mut state) = shared.lock() {
        let len = samples.len().min(OSCILLOSCOPE_BUFFER_SIZE);
        let target = match signal {
            OscilloscopeSignal::Kick => &mut state.osc_kick,
            OscilloscopeSignal::Bass => &mut state.osc_bass,
            OscilloscopeSignal::Sum => &mut state.osc_sum,
        };
        target[..len].copy_from_slice(&samples[..len]);
        state.osc_len = state.osc_len.max(len);
    }
}

pub fn commit_oscilloscope_frame(shared: &SharedStateHandle) {
    if let Ok(mut state) = shared.lock() {
        state.osc_len = state.osc_len.min(OSCILLOSCOPE_BUFFER_SIZE);
        state.osc_sequence = state.osc_sequence.wrapping_add(1);
    }
}

pub fn oscilloscope_snapshot(shared: &SharedStateHandle) -> OscilloscopeSnapshot {
    if let Ok(state) = shared.lock() {
        return OscilloscopeSnapshot {
            kick: state.osc_kick,
            bass: state.osc_bass,
            sum: state.osc_sum,
            len: state.osc_len,
            sequence: state.osc_sequence,
        };
    }

    OscilloscopeSnapshot {
        kick: [0.0; OSCILLOSCOPE_BUFFER_SIZE],
        bass: [0.0; OSCILLOSCOPE_BUFFER_SIZE],
        sum: [0.0; OSCILLOSCOPE_BUFFER_SIZE],
        len: 0,
        sequence: 0,
    }
}
