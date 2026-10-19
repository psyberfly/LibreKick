use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui::{self, Align, Layout, Pos2, Rect, Vec2};

use super::{menu_bar, nav_menu};
use crate::ui::state::BezierUiState;
use crate::LibreKickParams;

/// Renders the main scaffold: menu bar on top, nav menu on the right
/// (natural width), page content on the left (remaining width).
pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    params: &LibreKickParams,
    setter: &ParamSetter,
    history_action_applied: &mut bool,
    page_content: impl FnOnce(&mut egui::Ui, &mut BezierUiState),
) {
    let section_gap = (10.0 * ui_scale).max(8.0);

    let available = ui.available_size_before_wrap();
    let section_height = available.y.max(320.0 * ui_scale);

    // Fixed nav menu width (scaled with the rest of the UI).
    let menu_width = 130.0 * ui_scale;

    // Cap the page width so the nav menu always fits inside the window.
    // Page contents can grow their min_rect beyond the allocated width (egui
    // allocates what is actually used), which would otherwise push the nav
    // column off the right edge of the window.
    let page_width =
        (1500.0 * ui_scale).min((available.x - menu_width - section_gap).max(320.0 * ui_scale));

    menu_bar::render(
        ui,
        ui_scale,
        state,
        params,
        setter,
        page_width,
        history_action_applied,
    );

    // Both columns are placed at explicit rects rather than flowing in a
    // horizontal layout: page contents can report a min_rect wider than the
    // width they were given, which in a flow layout would shove the nav
    // column off the right edge of the window. With fixed rects the page can
    // no longer displace the nav, and `page_width + gap + menu_width` always
    // stays inside the width we were given.
    let origin = ui.cursor().min;
    let page_rect = Rect::from_min_size(origin, Vec2::new(page_width, section_height));
    let menu_rect = Rect::from_min_size(
        Pos2::new(page_rect.right() + section_gap, origin.y),
        Vec2::new(menu_width, section_height),
    );

    let mut page_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(page_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    page_ui.set_clip_rect(page_rect.intersect(ui.clip_rect()));
    page_ui.group(|ui| {
        page_content(ui, state);
    });

    let mut menu_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(menu_rect)
            .layout(Layout::top_down(Align::Min)),
    );
    nav_menu::render(&mut menu_ui, ui_scale, &mut state.active_page);

    // Claim the whole row so anything after the scaffold starts below it.
    ui.allocate_rect(page_rect.union(menu_rect), egui::Sense::hover());
}
