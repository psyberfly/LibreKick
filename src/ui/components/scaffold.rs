use nih_plug_egui::egui::{self, Align, Layout, Vec2};

use super::nav_menu;
use crate::ui::state::BezierUiState;

/// Renders the main scaffold: nav menu on the right (natural width),
/// page content on the left (remaining width).
pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    page_content: impl FnOnce(&mut egui::Ui, &mut BezierUiState),
) {
    let section_gap = (10.0 * ui_scale).max(8.0);
    let available = ui.available_size_before_wrap();
    let section_height = available.y.max(320.0 * ui_scale);

    // Fixed nav menu width (scaled with the rest of the UI).
    let menu_width = 100.0 * ui_scale;

    ui.horizontal(|ui| {
        // Page content on the left — fixed width.
        let page_width = 1500.0 * ui_scale;
        ui.allocate_ui_with_layout(
            Vec2::new(page_width, section_height),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_max_width(page_width);
                ui.group(|ui| {
                    page_content(ui, state);
                });
            },
        );

        ui.add_space(section_gap);

        // Nav menu on the right — fixed width.
        ui.allocate_ui_with_layout(
            Vec2::new(menu_width, section_height),
            Layout::top_down(Align::Min),
            |ui| {
                nav_menu::render(ui, ui_scale, &mut state.active_page);
            },
        );
    });
}
