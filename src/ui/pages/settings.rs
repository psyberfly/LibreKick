use nih_plug_egui::egui::{self, RichText};

use crate::ui::state::BezierUiState;

use super::super::TuningStandard;

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
}
