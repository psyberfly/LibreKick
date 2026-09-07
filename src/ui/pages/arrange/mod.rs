// Arrange page - modular structure
//
// Components:
// - controls: Tempo, time signature, and bars controls
// - midi_channel: Piano-roll style note grid with kick/bass lanes
// - audio_clip: Offline-rendered waveform preview of the arrangement

mod audio_clip;
mod controls;
mod midi_channel;

use nih_plug_egui::egui;

use nih_plug::prelude::ParamSetter;

use crate::ui::state::BezierUiState;
use crate::{shared, LibreKickParams};

/// Preview sample rate for the offline-rendered clip waveform — shared with
/// the instrument page previews so all rendered waveforms are identical.
pub(super) const CLIP_PREVIEW_RATE: f32 = crate::audio::PREVIEW_SAMPLE_RATE;

/// MIDI channel layout: bass octave (12 rows) on top, kick (1 row) at bottom.
pub(super) const BASS_NOTES: usize = 12;
pub(super) const KICK_NOTES: usize = 1;
pub(super) const TOTAL_ROWS: usize = BASS_NOTES + KICK_NOTES;

/// Shared grid colors for the arrange page views.
pub(super) mod colors {
    use nih_plug_egui::egui::Color32;

    pub(super) fn bar_line() -> Color32 {
        Color32::from_rgb(80, 88, 98)
    }

    pub(super) fn beat_line() -> Color32 {
        Color32::from_rgb(52, 58, 66)
    }

    pub(super) fn row_line() -> Color32 {
        Color32::from_rgb(48, 54, 62)
    }

    pub(super) fn background() -> Color32 {
        Color32::from_rgb(20, 24, 28)
    }

    pub(super) fn border() -> Color32 {
        Color32::from_rgb(90, 95, 102)
    }

    pub(super) fn kick_note() -> Color32 {
        Color32::from_rgb(245, 160, 88)
    }

    pub(super) fn bass_note() -> Color32 {
        Color32::from_rgb(80, 160, 245)
    }
}

pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    shared_for_ui: &shared::SharedStateHandle,
    params: &LibreKickParams,
    _setter: &ParamSetter,
) {
    ui.add_space(8.0 * ui_scale);
    ui.heading("Arrange");
    ui.separator();

    ui.add_space(16.0 * ui_scale);

    // Get tempo from shared state
    let shared_snapshot = shared::snapshot(shared_for_ui);
    let daw_tempo = shared_snapshot.tempo;

    controls::render(ui, ui_scale, state, daw_tempo);

    ui.add_space(16.0 * ui_scale);

    midi_channel::render(ui, ui_scale, state);

    ui.add_space(16.0 * ui_scale);

    audio_clip::render(
        ui,
        ui_scale,
        state,
        &shared_snapshot,
        daw_tempo,
        params,
    );
}
