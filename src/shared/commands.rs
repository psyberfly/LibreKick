use crate::common::logger::LOGGER;
use crate::config;
use crate::interface::{BassCommand, KickCommand};

use super::state::SharedStateHandle;

pub fn apply_kick_command(shared: &SharedStateHandle, command: KickCommand) {
    let app_cfg = config::app_config();
    if let Ok(mut state) = shared.lock() {
        match command {
            KickCommand::SetAmpLut(lut) => state.amp_lut = lut,
            KickCommand::SetPitchLut(lut) => state.pitch_lut = lut,
            KickCommand::SetKeytrackEnabled(enabled) => state.keytrack_enabled = enabled,
            KickCommand::SetNoteLengthMs(ms) => {
                state.note_length_ms = ms.clamp(0.0, app_cfg.note_length_max_ms);
            }
            KickCommand::SetWaveform(waveform) => state.kick_oscillator_waveform = waveform,
            KickCommand::SetRetrigger(value) => state.kick_retrigger = value,
            KickCommand::SetLegatoVoiceSteal(value) => state.kick_legato_voice_steal = value,
            KickCommand::RequestTrigger => {
                LOGGER.entering("shared::request_trigger", "{}".to_owned());
                state.trigger_counter = state.trigger_counter.wrapping_add(1);
                LOGGER.debug(format!("trigger_counter={}", state.trigger_counter));
                LOGGER.leaving("shared::request_trigger");
            }
        }
    } else {
        LOGGER.error("shared::apply_kick_command failed to acquire state lock");
    }
}

pub fn apply_bass_command(shared: &SharedStateHandle, command: BassCommand) {
    if let Ok(mut state) = shared.lock() {
        match command {
            BassCommand::SetAmpLut(lut) => state.bass_amp_lut = lut,
            BassCommand::SetFilterLut(lut) => state.bass_filter_lut = lut,
            BassCommand::SetNoteLengthMs(ms) => {
                state.bass_note_length_ms = ms.clamp(1.0, 1000.0);
            }
            BassCommand::SetCutoffHz(hz) => {
                state.bass_cutoff_hz = hz.clamp(20.0, 8_000.0);
            }
            BassCommand::SetFilterMode(mode) => state.bass_filter_mode = mode,
            BassCommand::SetPitchHz(hz) => {
                state.bass_pitch_hz = hz.clamp(20.0, 2_000.0);
            }
            BassCommand::SetRetrigger(value) => state.bass_retrigger = value,
            BassCommand::SetLegatoVoiceSteal(value) => state.bass_legato_voice_steal = value,
            BassCommand::SetWaveform(waveform) => state.bass_oscillator_waveform = waveform,
        }
    } else {
        LOGGER.error("shared::apply_bass_command failed to acquire state lock");
    }
}
