use nih_plug_egui::egui;
use crate::config::AppConfig;
use crate::ui::helpers::slider_fine_step;

/// Renders a zoom slider.
/// Returns true if zoom value changed
pub fn render(
    ui: &mut egui::Ui,
    zoom_percent: &mut f32,
    config: &AppConfig,
) -> bool {
    let slider = egui::Slider::new(
        zoom_percent,
        config.waveform_zoom_min_percent..=config.waveform_zoom_max_percent,
    )
    .text("Zoom")
    .suffix("%");
    let slider = slider_fine_step(ui, slider, config.waveform_zoom_step_percent as f64);
    ui.add(slider).changed()
}
