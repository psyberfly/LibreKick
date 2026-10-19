use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui;

use crate::ui::components::{knob, section};
use crate::ui::helpers::note_name_from_hz;
use crate::ui::state::{
    BezierUiState, CurveKind, NOTE_LENGTH_MAX_SLIDER_MAX_MS, NOTE_LENGTH_MAX_SLIDER_MIN_MS,
};
use crate::{config, shared, LibreKickParams};

/// Kick page controls in the unified panel spec: a styled OSC section with
/// segmented waveform buttons, rotary knobs for every continuous value, and
/// a checkbox row — matching the bass page layout.
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

    let kick_knob = knob::KnobStyle::panel();

    section::render(
        ui,
        ui_scale,
        section::SectionHeader {
            title: "OSC",
            enabled: None,
            min_height: 0.0,
        },
        |ui| {
            section::segmented(
                ui,
                ui_scale,
                &mut state.kick_oscillator_waveform,
                &[
                    (shared::Waveform::Sine, "SIN"),
                    (shared::Waveform::Saw, "SAW"),
                    (shared::Waveform::Square, "SQR"),
                ],
            );

            ui.add_space(6.0 * ui_scale);
            let dial = knob::fit_knob_diameter(ui, 5, ui_scale);
            let style = |diameter: f32| knob::KnobStyle {
                diameter,
                ..kick_knob
            };
            ui.horizontal(|ui| {
                knob::param_knob(
                    ui,
                    ui_scale,
                    "LEVEL",
                    &params.kick_level,
                    setter,
                    style(dial),
                );

                let note = note_name_from_hz(state.kick_pitch_hz, state.tuning_standard.a4_hz());
                let pitch_display = format!("{:.0}Hz {note}", state.kick_pitch_hz);
                knob::log_range_knob(
                    ui,
                    ui_scale,
                    "PITCH",
                    &mut state.kick_pitch_hz,
                    20.0,
                    2_000.0,
                    &pitch_display,
                    style(dial),
                );

                let phase_display = format!("{:.0}\u{00b0}", state.kick_phase_deg);
                knob::range_knob(
                    ui,
                    ui_scale,
                    "PHASE",
                    &mut state.kick_phase_deg,
                    0.0,
                    360.0,
                    &phase_display,
                    style(dial),
                );

                let length_display = format!("{:.0} ms", state.note_length_ms);
                if knob::range_knob(
                    ui,
                    ui_scale,
                    "LENGTH",
                    &mut state.note_length_ms,
                    1.0,
                    1_000.0,
                    &length_display,
                    style(dial),
                ) {
                    state.note_length_ms =
                        state.note_length_ms.clamp(0.0, state.note_length_max_ms);
                }

                let max_display = format!("{:.0} ms", state.note_length_max_ms);
                if knob::range_knob(
                    ui,
                    ui_scale,
                    "MAX LEN",
                    &mut state.note_length_max_ms,
                    NOTE_LENGTH_MAX_SLIDER_MIN_MS,
                    NOTE_LENGTH_MAX_SLIDER_MAX_MS,
                    &max_display,
                    style(dial),
                ) {
                    state.note_length_max_ms = state
                        .note_length_max_ms
                        .clamp(NOTE_LENGTH_MAX_SLIDER_MIN_MS, NOTE_LENGTH_MAX_SLIDER_MAX_MS);
                    state.note_length_ms =
                        state.note_length_ms.clamp(0.0, state.note_length_max_ms);
                }
            });

            ui.add_space(6.0 * ui_scale);
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.kick_retrigger, "Retrig");
                ui.checkbox(&mut state.kick_legato_voice_steal, "Legato");
                ui.checkbox(&mut state.keytrack_enabled, "Keytrack");
                ui.checkbox(&mut state.kick_length_override, "Sync")
                    .on_hover_text(
                        "Gate the kick to one 16th note at the Arrange tempo — \
                         live DAW tempo when synced, else the manual tempo — \
                         instead of the fixed LENGTH",
                    );
            });
        },
    );

    // Sync UI state to shared state for DSP
    state.sync_to_shared(shared_for_ui);

    ui.add_space(8.0 * ui_scale);
    ui.horizontal(|ui| {
        // Curve selector — segmented like the module buttons.
        ui.label("Curve");
        section::segmented(
            ui,
            ui_scale,
            &mut state.active_curve,
            &[(CurveKind::Amplitude, "AMP"), (CurveKind::Pitch, "PITCH")],
        );
        // Per-envelope power — a powered-off envelope leaves the signal
        // untouched (AMP flat at full level, PITCH flat at base pitch).
        for (enabled, label) in [
            (&mut state.amp_env_enabled, "AMP"),
            (&mut state.pitch_env_enabled, "PITCH"),
        ] {
            if section::power_pill(ui, ui_scale, *enabled).clicked() {
                *enabled = !*enabled;
            }
            ui.label(label);
        }
        ui.separator();
        if ui.button("Trigger").clicked() {
            shared::request_trigger(shared_for_ui);
        }
        ui.separator();
        // Waveform zoom as a small dial — same spec, smaller footprint.
        let zoom_display = format!("{:.0}%", state.waveform_zoom_percent);
        knob::range_knob(
            ui,
            ui_scale,
            "ZOOM",
            &mut state.waveform_zoom_percent,
            app_cfg.waveform_zoom_min_percent,
            app_cfg.waveform_zoom_max_percent,
            &zoom_display,
            knob::KnobStyle {
                diameter: 30.0,
                ..knob::KnobStyle::panel()
            },
        );
    });
    ui.add_space(8.0);
}
