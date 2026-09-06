use crate::config;
use crate::interface::{BassFilterMode, BassSnapshot, KickSnapshot, Waveform};

use super::state::{default_curve_luts, SharedStateHandle};

pub fn kick_snapshot(shared: &SharedStateHandle) -> KickSnapshot {
    let app_cfg = config::app_config();
    if let Ok(state) = shared.lock() {
        return KickSnapshot {
            amp_lut: state.amp_lut,
            pitch_lut: state.pitch_lut,
            keytrack_enabled: state.keytrack_enabled,
            note_length_ms: state.note_length_ms,
            waveform: state.kick_oscillator_waveform,
            retrigger: state.kick_retrigger,
            legato_voice_steal: state.kick_legato_voice_steal,
            trigger_counter: state.trigger_counter,
        };
    }

    let (amp_lut, pitch_lut, _, _) = default_curve_luts();

    KickSnapshot {
        amp_lut,
        pitch_lut,
        keytrack_enabled: false,
        note_length_ms: app_cfg.note_length_max_ms,
        waveform: Waveform::Sine,
        retrigger: true,
        legato_voice_steal: true,
        trigger_counter: 0,
    }
}

pub fn bass_snapshot(shared: &SharedStateHandle) -> BassSnapshot {
    let app_cfg = config::app_config();
    if let Ok(state) = shared.lock() {
        return BassSnapshot {
            amp_lut: state.bass_amp_lut,
            filter_lut: state.bass_filter_lut,
            bass_note_length_ms: state.bass_note_length_ms,
            bass_cutoff_hz: state.bass_cutoff_hz,
            bass_pitch_hz: state.bass_pitch_hz,
            retrigger: state.bass_retrigger,
            legato_voice_steal: state.bass_legato_voice_steal,
            bass_filter_mode: state.bass_filter_mode,
            waveform: state.bass_oscillator_waveform,
        };
    }

    let (_, _, bass_amp_lut, bass_filter_lut) = default_curve_luts();

    BassSnapshot {
        amp_lut: bass_amp_lut,
        filter_lut: bass_filter_lut,
        bass_note_length_ms: app_cfg.note_length_max_ms,
        bass_cutoff_hz: 120.0,
        bass_pitch_hz: 55.0,
        retrigger: true,
        legato_voice_steal: false,
        bass_filter_mode: BassFilterMode::LowPass,
        waveform: Waveform::Saw,
    }
}
