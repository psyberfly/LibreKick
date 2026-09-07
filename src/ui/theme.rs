use nih_plug_egui::egui::{self, Color32, FontId, Stroke, Vec2};

use crate::config;

const RESIZE_CORNER_VISUAL_SIZE: f32 = 20.0;
const RESIZE_CORNER_HIT_RADIUS: f32 = 30.0;
const RESIZE_SIDE_HIT_RADIUS: f32 = 16.0;

pub(crate) struct Theme;

impl Theme {
    pub(crate) fn app_font_family(self) -> egui::FontFamily {
        egui::FontFamily::Name("Open Sans Adwaita Bold Italica".into())
    }

    pub(crate) fn brand_orange(self) -> Color32 {
        Color32::from_rgb(242, 134, 52)
    }

    pub(crate) fn brand_hot_core(self) -> Color32 {
        Color32::from_rgb(255, 104, 74)
    }

    pub(crate) fn brand_glow_inner(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 138, 60, 96)
    }

    pub(crate) fn brand_glow_outer(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 120, 42, 42)
    }

    pub(crate) fn panel_bg(self) -> Color32 {
        background_color()
    }

    pub(crate) fn graph_bg(self) -> Color32 {
        Color32::from_rgb(16, 19, 22)
    }

    pub(crate) fn post_length_tint(self) -> Color32 {
        Color32::from_rgba_unmultiplied(0, 0, 0, 78)
    }

    pub(crate) fn graph_border(self) -> Color32 {
        Color32::from_rgb(90, 95, 102)
    }

    pub(crate) fn grid_line(self) -> Color32 {
        Color32::from_rgb(34, 39, 45)
    }

    pub(crate) fn note_length_fill(self) -> Color32 {
        accent_color()
    }

    pub(crate) fn note_length_stroke(self) -> Color32 {
        Color32::from_rgb(70, 18, 18)
    }

    pub(crate) fn axis_title(self) -> Color32 {
        Color32::from_rgb(185, 191, 198)
    }

    pub(crate) fn axis_tick(self) -> Color32 {
        Color32::from_rgb(165, 171, 178)
    }

    pub(crate) fn waveform_midline(self) -> Color32 {
        Color32::from_rgba_unmultiplied(120, 128, 136, 45)
    }

    pub(crate) fn waveform_trace(self) -> Color32 {
        Color32::from_rgba_unmultiplied(245, 170, 112, 125)
    }

    pub(crate) fn start_endpoint_point(self) -> Color32 {
        start_weight_color()
    }

    pub(crate) fn end_endpoint_point(self) -> Color32 {
        end_weight_color()
    }

    pub(crate) fn selected_point(self) -> Color32 {
        weight_color()
    }

    pub(crate) fn control_point(self) -> Color32 {
        weight_color()
    }

    pub(crate) fn point_outline(self) -> Color32 {
        Color32::BLACK
    }

    /// Off-white concentric ring drawn around node dots.
    pub(crate) fn node_ring(self) -> Color32 {
        Color32::from_rgb(228, 230, 224)
    }

    pub(crate) fn bubble_bg(self) -> Color32 {
        Color32::from_rgba_unmultiplied(24, 28, 33, 220)
    }

    pub(crate) fn bubble_border(self) -> Color32 {
        Color32::from_rgb(90, 95, 102)
    }

    pub(crate) fn bubble_text(self) -> Color32 {
        Color32::from_rgb(224, 230, 238)
    }

    pub(crate) fn active_button_bg(self) -> Color32 {
        Color32::from_rgb(188, 54, 54)
    }

    pub(crate) fn active_button_hover(self) -> Color32 {
        Color32::from_rgb(220, 72, 72)
    }

    pub(crate) fn active_button_border(self) -> Color32 {
        Color32::from_rgb(96, 30, 30)
    }
}

pub(crate) const APP_THEME: Theme = Theme;

pub(crate) fn themed_font(size: f32) -> FontId {
    FontId::new(size, APP_THEME.app_font_family())
}

pub(crate) fn ui_scale_from_size(size: Vec2) -> f32 {
    let ui_cfg = config::ui_config();
    let scale_x = size.x / ui_cfg.base_editor_width;
    let scale_y = size.y / ui_cfg.base_editor_height;
    ((scale_x + scale_y) * 0.5).clamp(0.8, 2.2)
}

pub(crate) fn apply_ui_text_scale(ui: &mut egui::Ui, scale: f32) {
    let mut style = ui.style().as_ref().clone();
    style.text_styles = [
        (egui::TextStyle::Heading, themed_font(21.0 * scale)),
        (egui::TextStyle::Body, themed_font(14.0 * scale)),
        (egui::TextStyle::Monospace, FontId::monospace(13.0 * scale)),
        (egui::TextStyle::Button, themed_font(14.0 * scale)),
        (egui::TextStyle::Small, themed_font(11.0 * scale)),
    ]
    .into();
    ui.ctx().set_style(style.clone());
    ui.set_style(style);
}

/// Applies resize handles, selection, and widget colors for the current scale.
pub(crate) fn apply_widget_style(ui: &mut egui::Ui, ui_scale: f32) {
    let style = ui.style_mut();
    style.interaction.resize_grab_radius_corner =
        (RESIZE_CORNER_HIT_RADIUS * ui_scale).max(24.0);
    style.interaction.resize_grab_radius_side =
        (RESIZE_SIDE_HIT_RADIUS * ui_scale).max(12.0);
    style.visuals.resize_corner_size = (RESIZE_CORNER_VISUAL_SIZE * ui_scale).max(16.0);
    style.visuals.selection.bg_fill = APP_THEME.active_button_bg();
    style.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    style.visuals.widgets.active.bg_fill = APP_THEME.active_button_bg();
    style.visuals.widgets.active.weak_bg_fill = APP_THEME.active_button_bg();
    style.visuals.widgets.active.bg_stroke =
        Stroke::new(1.0, APP_THEME.active_button_border());
    style.visuals.widgets.active.fg_stroke.color = Color32::WHITE;
    style.visuals.widgets.hovered.bg_fill = APP_THEME.active_button_hover();
    style.visuals.widgets.hovered.weak_bg_fill = APP_THEME.active_button_hover();
    style.visuals.widgets.hovered.bg_stroke =
        Stroke::new(1.0, APP_THEME.active_button_border());
    style.visuals.widgets.hovered.fg_stroke.color = Color32::WHITE;
}

pub fn background_color() -> Color32 {
    Color32::from_rgb(10, 12, 14)
}

pub fn pitch_env_color() -> Color32 {
    Color32::from_rgb(0, 220, 220)
}

pub fn amp_env_color() -> Color32 {
    Color32::from_rgb(245, 160, 88)
}

pub fn weight_color() -> Color32 {
    Color32::from_rgb(245, 222, 179)
}

pub fn start_weight_color() -> Color32 {
    Color32::from_rgb(80, 200, 120)
}

pub fn end_weight_color() -> Color32 {
    Color32::from_rgb(220, 64, 64)
}

pub fn edge_color() -> Color32 {
    Color32::from_rgb(255, 255, 0)
}

pub fn pitch_edge_color() -> Color32 {
    Color32::from_rgb(135, 206, 235)
}

pub fn accent_color() -> Color32 {
    Color32::from_rgb(220, 64, 64)
}

pub fn axis_title_color() -> Color32 {
    Color32::from_rgb(185, 191, 198)
}

