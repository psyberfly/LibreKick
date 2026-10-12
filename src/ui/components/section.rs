use nih_plug_egui::egui::{self, Align2, Color32, Sense, Stroke, Vec2};

use crate::ui::theme::{
    accent_color, crt_overlay, phosphor_color, text_color, themed_font, APP_THEME,
};

/// Header bar for a Serum-style module panel: a title on the left plus an
/// optional enable toggle, followed by the module body.
pub(crate) struct SectionHeader<'a> {
    pub(crate) title: &'a str,
    /// When present, draws an on/off pill on the left of the title.
    pub(crate) enabled: Option<&'a mut bool>,
    pub(crate) min_height: f32,
}

pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    header: SectionHeader<'_>,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let frame = egui::Frame::new()
        .fill(APP_THEME.screen_bg())
        .stroke(Stroke::new(1.0, APP_THEME.graph_border()))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same((12.0 * ui_scale) as i8));

    let inner = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        // Minimum 12px (scaled) between controls inside a module panel.
        let spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = egui::Vec2::new(
            spacing.x.max(12.0 * ui_scale),
            spacing.y.max(12.0 * ui_scale),
        );
        if header.min_height > 0.0 {
            ui.set_min_height(header.min_height);
        }

        ui.horizontal(|ui| {
            if let Some(enabled) = header.enabled {
                if power_pill(ui, ui_scale, *enabled).clicked() {
                    *enabled = !*enabled;
                }
                ui.add_space(5.0 * ui_scale);
            }
            // Small phosphor tick before the title, like a lit module tag.
            let tick_size = Vec2::new(3.0 * ui_scale, 11.0 * ui_scale);
            let (tick_rect, _) = ui.allocate_exact_size(tick_size, Sense::hover());
            ui.painter().rect_filled(tick_rect, 1.0, phosphor_color());
            ui.add_space(4.0 * ui_scale);
            ui.label(
                egui::RichText::new(header.title)
                    .font(themed_font(12.0 * ui_scale))
                    .color(text_color())
                    .strong(),
            );
        });

        ui.add_space(4.0 * ui_scale);
        // Phosphor divider instead of the stock grey separator.
        let (line_rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), 1.0),
            Sense::hover(),
        );
        ui.painter().hline(
            line_rect.x_range(),
            line_rect.center().y,
            Stroke::new(1.0, APP_THEME.graph_border()),
        );
        ui.add_space(4.0 * ui_scale);

        add_contents(ui);
    });

    // CRT finish over the panel once the frame knows its real rect.
    crt_overlay(ui.painter(), inner.response.rect, 4.0);
}

/// Small round power indicator used to enable/disable a module.
pub(crate) fn power_pill(ui: &mut egui::Ui, ui_scale: f32, on: bool) -> egui::Response {
    let size = Vec2::splat(13.0 * ui_scale);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let center = rect.center();
    let radius = rect.width() * 0.5;

    painter.circle_filled(
        center,
        radius,
        if on {
            APP_THEME.brand_hot_core()
        } else {
            APP_THEME.graph_bg()
        },
    );
    painter.circle_stroke(
        center,
        radius,
        Stroke::new(1.0, APP_THEME.graph_border()),
    );
    if on {
        // Halo ring plus hot centre — reads as a lit phosphor lamp.
        painter.circle_stroke(
            center,
            radius + 1.5 * ui_scale,
            Stroke::new(1.0, APP_THEME.trace_glow()),
        );
        painter.circle_filled(center, radius * 0.38, Color32::WHITE);
    }

    response.on_hover_text(if on { "On" } else { "Off" })
}

/// Row of mutually exclusive text buttons, Serum's segmented selector.
/// Returns true when the selection changed.
pub(crate) fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    ui_scale: f32,
    current: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0 * ui_scale;
        for (value, label) in options {
            let selected = *current == *value;
            let text_size = 10.0 * ui_scale;
            let galley = ui.painter().layout_no_wrap(
                (*label).to_owned(),
                themed_font(text_size),
                Color32::WHITE,
            );
            let size = Vec2::new(
                galley.size().x + 10.0 * ui_scale,
                18.0 * ui_scale,
            );
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            let painter = ui.painter_at(rect);
            painter.rect_filled(
                rect,
                2.0,
                if selected {
                    APP_THEME.active_button_bg()
                } else {
                    APP_THEME.graph_bg()
                },
            );
            painter.rect_stroke(
                rect,
                2.0,
                Stroke::new(
                    1.0,
                    if selected {
                        accent_color()
                    } else {
                        APP_THEME.grid_line()
                    },
                ),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                *label,
                themed_font(text_size),
                if selected {
                    phosphor_color()
                } else {
                    APP_THEME.axis_tick()
                },
            );
            if response.clicked() && !selected {
                *current = *value;
                changed = true;
            }
        }
    });
    changed
}
