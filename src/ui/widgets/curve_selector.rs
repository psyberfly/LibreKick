use nih_plug_egui::egui;
use crate::ui::state::CurveKind;

/// Renders curve selector radio buttons: Amplitude | Pitch
/// Returns true if selection changed
pub fn render(
    ui: &mut egui::Ui,
    active_curve: &mut CurveKind,
) -> bool {
    let before = *active_curve;
    
    ui.label("Curve:");
    ui.selectable_value(active_curve, CurveKind::Amplitude, "Amplitude");
    ui.selectable_value(active_curve, CurveKind::Pitch, "Pitch");
    
    *active_curve != before
}
