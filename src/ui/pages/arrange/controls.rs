use nih_plug_egui::egui::{self, RichText};

use crate::ui::helpers::slider_fine_step;
use crate::ui::state::{BezierUiState, NoteSize};

/// Renders the transport controls row: Tempo, Time Signature, and Bars.
pub(super) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    daw_tempo: Option<f64>,
) {
    ui.horizontal(|ui| {
        // Tempo section
        ui.group(|ui| {
            ui.label(RichText::new("Tempo").strong());
            ui.add_space(4.0 * ui_scale);

            // Determine which tempo to display
            let display_tempo = if state.use_daw_tempo {
                if let Some(tempo) = daw_tempo {
                    state.manual_tempo = tempo as f32;
                    tempo as f32
                } else {
                    state.manual_tempo
                }
            } else {
                state.manual_tempo
            };

            // Editable tempo slider
            let mut tempo_value = display_tempo;
            let slider_enabled = !state.use_daw_tempo || daw_tempo.is_none();

            ui.add_enabled(
                slider_enabled,
                slider_fine_step(
                    ui,
                    egui::Slider::new(&mut tempo_value, 20.0..=300.0)
                        .text("BPM"),
                    1.0,
                ),
            );

            if tempo_value != display_tempo {
                state.manual_tempo = tempo_value;
            }

            ui.add_space(4.0 * ui_scale);

            // Toggle button for DAW tempo lock
            if daw_tempo.is_some() {
                ui.checkbox(&mut state.use_daw_tempo, "Use from DAW");
            }
        });

        // Time Signature section
        ui.group(|ui| {
            ui.label(RichText::new("Time Signature").strong());
            ui.add_space(4.0 * ui_scale);

            // Hard-coded 4/4 time signature (not editable for now)
            ui.label(RichText::new("4/4").size(24.0));
        });

        // Bars section
        ui.group(|ui| {
            ui.label(RichText::new("Bars").strong());
            ui.add_space(4.0 * ui_scale);

            // Editable bars slider (0.25-8 in quarter-bar steps; shift = 1 bar)
            ui.add(slider_fine_step(
                ui,
                egui::Slider::new(&mut state.num_bars, 0.25..=8.0)
                    .step_by(0.25)
                    .text("bars"),
                1.0,
            ));
        });

        // Note Size section
        ui.group(|ui| {
            ui.label(RichText::new("Note Size").strong());
            ui.add_space(4.0 * ui_scale);

            // Slider over the note-size options (1/16 .. 1)
            let mut index = NoteSize::ALL
                .iter()
                .position(|s| *s == state.note_size)
                .unwrap_or(2);
            let response = ui.add(
                egui::Slider::new(&mut index, 0..=(NoteSize::ALL.len() - 1))
                    .show_value(false),
            );
            ui.label(RichText::new(state.note_size.label()).size(16.0));

            if response.changed() {
                state.note_size = NoteSize::ALL[index];
            }
        });
    });
}
