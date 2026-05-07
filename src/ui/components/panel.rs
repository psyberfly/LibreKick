use nih_plug_egui::egui::{self, RichText};

pub(crate) fn render(
    ui: &mut egui::Ui,
    title: &str,
    ui_scale: f32,
    min_height: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(min_height.max(120.0 * ui_scale));
        ui.label(RichText::new(title).strong());
        ui.separator();
        add_contents(ui);
    });
}
