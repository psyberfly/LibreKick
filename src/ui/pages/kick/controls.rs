use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui;

use crate::{config, shared, LibreKickParams};
use crate::ui::components::{oscillator_panel, panel};
use crate::ui::state::{BezierUiState, NOTE_LENGTH_MAX_SLIDER_MAX_MS, NOTE_LENGTH_MAX_SLIDER_MIN_MS};
use crate::ui::widgets::{zoom_controls, curve_selector};

/// Renders the kick page control panel with oscillator settings, curve selector,
/// trigger button, note length slider, and zoom controls.
pub(super) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    app_cfg: &config::AppConfig,
    shared_for_ui: &shared::SharedStateHandle,
    params: &LibreKickParams,
    setter: &ParamSetter,
) {
    ui.add_space(8.0 * ui_scale);
    ui.heading("Kick");
    ui.separator();

    ui.add_space(8.0 * ui_scale);
    panel::render(ui, "Oscillator", ui_scale, 150.0 * ui_scale, |ui| {
        oscillator_panel::render(
            ui,
            ui_scale,
            oscillator_panel::OscillatorPanelModel {
                waveform: &mut state.kick_oscillator_waveform,
                retrigger: &mut state.kick_retrigger,
                legato_voice_steal: &mut state.kick_legato_voice_steal,
                pitch_hz: Some(&mut state.kick_pitch_hz),
                phase_deg: Some(&mut state.kick_phase_deg),
                note_length_ms: Some(&mut state.note_length_ms),
                level: Some((&params.kick_level, setter)),
            },
        );
    });
    ui.add_space(8.0 * ui_scale);

    // Sync UI state to shared state for DSP
    state.sync_to_shared(shared_for_ui);

    ui.horizontal(|ui| {
        curve_selector::render(ui, &mut state.active_curve);
        ui.separator();
        ui.checkbox(&mut state.keytrack_enabled, "Keytrack");
        ui.separator();
        if ui.button("Trigger").clicked() {
            shared::request_trigger(shared_for_ui);
        }
        ui.separator();
        ui.label("Max Note Length");
        let max_length_changed = ui
            .add(crate::ui::helpers::slider_fine_step(
                ui,
                egui::Slider::new(
                    &mut state.note_length_max_ms,
                    NOTE_LENGTH_MAX_SLIDER_MIN_MS..=NOTE_LENGTH_MAX_SLIDER_MAX_MS,
                )
                .text("ms"),
                1.0,
            ))
            .changed();
        if max_length_changed {
            state.note_length_max_ms = state
                .note_length_max_ms
                .clamp(NOTE_LENGTH_MAX_SLIDER_MIN_MS, NOTE_LENGTH_MAX_SLIDER_MAX_MS);
            state.note_length_ms = state.note_length_ms.clamp(0.0, state.note_length_max_ms);
        }
        ui.separator();
        zoom_controls::render(ui, &mut state.waveform_zoom_percent, app_cfg);
    });
    ui.add_space(8.0);
}
