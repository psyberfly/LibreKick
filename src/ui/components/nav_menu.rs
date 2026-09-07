use nih_plug_egui::egui::{self, RichText};

use crate::ui::state::UiPage;
use crate::ui::theme as ui_theme;

pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    active_page: &mut UiPage,
) {
    ui.group(|ui| {
        ui.add_space(8.0 * ui_scale);
        ui.label(
            RichText::new("Sections")
                .strong()
                .color(ui_theme::axis_title_color()),
        );
        ui.separator();
        ui.selectable_value(active_page, UiPage::Kick, "Kick");
        ui.selectable_value(active_page, UiPage::Bass, "Bass");
        ui.selectable_value(active_page, UiPage::Arrange, "Arrange");
        ui.selectable_value(active_page, UiPage::Settings, "Settings");
        ui.selectable_value(active_page, UiPage::Oscilloscope, "Oscilloscope");
        ui.selectable_value(active_page, UiPage::Logs, "Logs");
    });
}
