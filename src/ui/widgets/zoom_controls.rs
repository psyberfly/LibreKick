use nih_plug_egui::egui;
use crate::config::AppConfig;

/// Renders zoom controls: - button, zoom percentage label, + button
/// Returns true if zoom value changed
pub fn render(
    ui: &mut egui::Ui,
    zoom_percent: &mut f32,
    config: &AppConfig,
) -> bool {
    let mut changed = false;
    
    if ui.button("-").clicked() {
        *zoom_percent = (*zoom_percent - config.waveform_zoom_step_percent).clamp(
            config.waveform_zoom_min_percent,
            config.waveform_zoom_max_percent,
        );
        changed = true;
    }
    
    ui.label(format!("Zoom {:.0}%", zoom_percent));
    
    if ui.button("+").clicked() {
        *zoom_percent = (*zoom_percent + config.waveform_zoom_step_percent).clamp(
            config.waveform_zoom_min_percent,
            config.waveform_zoom_max_percent,
        );
        changed = true;
    }
    
    changed
}
