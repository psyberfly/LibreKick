use nih_plug_egui::egui::{self, RichText};

use crate::ui::state::BezierUiState;
use crate::{config, patches};

use super::super::state::TuningStandard;

pub(crate) fn render(ui: &mut egui::Ui, ui_scale: f32, state: &mut BezierUiState) {
    ui.add_space(8.0 * ui_scale);
    ui.heading("Settings");
    ui.label(
        RichText::new("Global instrument settings")
            .italics()
            .small(),
    );
    ui.separator();

    ui.group(|ui| {
        ui.label(RichText::new("Tuning").strong());
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.tuning_standard, TuningStandard::A440, "A=440");
            ui.selectable_value(&mut state.tuning_standard, TuningStandard::A432, "A=432");
        });
        ui.label(format!("Current A4: {:.1} Hz", state.tuning_standard.a4_hz()));
        ui.label("Applies globally to kick and bass note display/frequency scaling.");
    });

    ui.add_space(8.0 * ui_scale);
    ui.group(|ui| {
        ui.label(RichText::new("Display").strong());
        ui.horizontal(|ui| {
            ui.label("UI scale");
            ui.add(
                egui::Slider::new(&mut state.display_scale, 0.5..=2.0)
                    .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                    .custom_parser(|s| {
                        s.trim_end_matches('%').parse::<f64>().ok().map(|v| v / 100.0)
                    }),
            );
        });
        ui.label("Multiplies the automatic window-based scaling.");
    });

    ui.add_space(8.0 * ui_scale);
    ui.group(|ui| {
        ui.label(RichText::new("Persistence").strong());
        if ui.button("Save settings").clicked() {
            let settings = patches::UiSettingsData {
                tuning_a4_hz: state.tuning_standard.a4_hz(),
                display_scale: state.display_scale,
            };
            state.patch_status = Some(match patches::save_settings(&settings) {
                Ok(()) => "Settings saved. They will be applied on next startup.".to_owned(),
                Err(error) => format!("Failed to save settings: {error}"),
            });
        }
        ui.label(
            RichText::new(format!("Saved to {}", config::patches_dir())).small(),
        );
        if let Some(status) = &state.patch_status {
            ui.label(RichText::new(status).small().italics());
        }
    });
}
