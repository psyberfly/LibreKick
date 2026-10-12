use nih_plug_egui::egui::{self, Color32, FontId, Stroke, Vec2};

use crate::config;

const RESIZE_CORNER_VISUAL_SIZE: f32 = 20.0;
const RESIZE_CORNER_HIT_RADIUS: f32 = 30.0;
const RESIZE_SIDE_HIT_RADIUS: f32 = 16.0;

/// Retro-CRT phosphor palette: saturated neon orange on near-black, tuned so
/// text and trim read like a fluorescent tube behind dark bezel glass.
pub(crate) struct Theme;

impl Theme {
    pub(crate) fn app_font_family(self) -> egui::FontFamily {
        egui::FontFamily::Name("Open Sans Adwaita Bold Italica".into())
    }

    /// Hot phosphor orange, the primary accent.
    pub(crate) fn brand_orange(self) -> Color32 {
        phosphor_color()
    }

    /// Slightly red-shifted core used for the brightest glow point.
    pub(crate) fn brand_hot_core(self) -> Color32 {
        Color32::from_rgb(255, 122, 20)
    }

    pub(crate) fn brand_glow_inner(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 138, 20, 96)
    }

    pub(crate) fn brand_glow_outer(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 110, 16, 42)
    }

    /// Near-black panel face.
    pub(crate) fn panel_bg(self) -> Color32 {
        background_color()
    }

    /// Inset bezel behind graphs and displays — one step above pure black.
    pub(crate) fn graph_bg(self) -> Color32 {
        Color32::from_rgb(8, 8, 9)
    }

    /// Recessed screen area inside a panel.
    pub(crate) fn screen_bg(self) -> Color32 {
        Color32::from_rgb(12, 10, 8)
    }

    pub(crate) fn post_length_tint(self) -> Color32 {
        Color32::from_rgba_unmultiplied(0, 0, 0, 78)
    }

    /// Dim amber outline for bezels and inactive controls.
    pub(crate) fn graph_border(self) -> Color32 {
        Color32::from_rgb(66, 44, 22)
    }

    /// Faint amber grid line inside displays.
    pub(crate) fn grid_line(self) -> Color32 {
        Color32::from_rgb(38, 28, 18)
    }

    pub(crate) fn note_length_fill(self) -> Color32 {
        accent_color()
    }

    pub(crate) fn note_length_stroke(self) -> Color32 {
        Color32::from_rgb(70, 36, 8)
    }

    /// Phosphor-fluorescent label text.
    pub(crate) fn axis_title(self) -> Color32 {
        text_color()
    }

    /// Dimmed phosphor for secondary text and tick labels.
    pub(crate) fn axis_tick(self) -> Color32 {
        Color32::from_rgb(196, 128, 48)
    }

    pub(crate) fn waveform_midline(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 150, 60, 45)
    }

    /// Phosphor trace for waveform displays.
    pub(crate) fn waveform_trace(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 150, 40, 220)
    }

    /// Soft halo drawn behind phosphor traces and knob arcs.
    pub(crate) fn trace_glow(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 120, 20, 70)
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

    /// Bright phosphor ring drawn around node dots.
    pub(crate) fn node_ring(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 180, 90, 200)
    }

    pub(crate) fn bubble_bg(self) -> Color32 {
        Color32::from_rgba_unmultiplied(14, 10, 6, 235)
    }

    pub(crate) fn bubble_border(self) -> Color32 {
        Color32::from_rgb(96, 60, 20)
    }

    pub(crate) fn bubble_text(self) -> Color32 {
        text_color()
    }

    /// Lit control: dim amber fill rather than a flat red.
    pub(crate) fn active_button_bg(self) -> Color32 {
        Color32::from_rgb(120, 56, 8)
    }

    pub(crate) fn active_button_hover(self) -> Color32 {
        Color32::from_rgb(150, 72, 12)
    }

    pub(crate) fn active_button_border(self) -> Color32 {
        phosphor_color()
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
        (egui::TextStyle::Heading, themed_font(26.25 * scale)),
        (egui::TextStyle::Body, themed_font(17.5 * scale)),
        (egui::TextStyle::Monospace, FontId::monospace(16.25 * scale)),
        (egui::TextStyle::Button, themed_font(17.5 * scale)),
        (egui::TextStyle::Small, themed_font(13.75 * scale)),
    ]
    .into();
    ui.ctx().set_style(style.clone());
    ui.set_style(style);
}

/// Applies resize handles, selection, and widget colors for the current scale.
///
/// The stock egui widgets are forced into the CRT scheme too — near-black
/// fills, dim amber outlines, phosphor interaction states — so checkboxes,
/// combo boxes and menus match the custom-drawn panels.
pub(crate) fn apply_widget_style(ui: &mut egui::Ui, ui_scale: f32) {
    let style = ui.style_mut();
    style.interaction.resize_grab_radius_corner =
        (RESIZE_CORNER_HIT_RADIUS * ui_scale).max(24.0);
    style.interaction.resize_grab_radius_side =
        (RESIZE_SIDE_HIT_RADIUS * ui_scale).max(12.0);
    style.visuals.resize_corner_size = (RESIZE_CORNER_VISUAL_SIZE * ui_scale).max(16.0);

    // Dark-mode chrome.
    style.visuals.dark_mode = true;
    style.visuals.panel_fill = background_color();
    style.visuals.window_fill = background_color();
    style.visuals.faint_bg_color = APP_THEME.graph_bg();
    style.visuals.extreme_bg_color = APP_THEME.screen_bg();
    style.visuals.code_bg_color = APP_THEME.graph_bg();
    style.visuals.window_stroke = Stroke::new(1.0, APP_THEME.graph_border());
    style.visuals.widgets.noninteractive.bg_stroke =
        Stroke::new(1.0, APP_THEME.grid_line());
    style.visuals.widgets.noninteractive.fg_stroke.color = text_color();
    style.visuals.widgets.inactive.bg_fill = APP_THEME.screen_bg();
    style.visuals.widgets.inactive.weak_bg_fill = APP_THEME.screen_bg();
    style.visuals.widgets.inactive.bg_stroke =
        Stroke::new(1.0, APP_THEME.graph_border());
    style.visuals.widgets.inactive.fg_stroke.color = APP_THEME.axis_tick();

    style.visuals.selection.bg_fill = APP_THEME.active_button_bg();
    style.visuals.selection.stroke = Stroke::new(1.0, phosphor_color());
    style.visuals.widgets.active.bg_fill = APP_THEME.active_button_bg();
    style.visuals.widgets.active.weak_bg_fill = APP_THEME.active_button_bg();
    style.visuals.widgets.active.bg_stroke =
        Stroke::new(1.0, APP_THEME.active_button_border());
    style.visuals.widgets.active.fg_stroke.color = phosphor_color();
    style.visuals.widgets.hovered.bg_fill = APP_THEME.active_button_hover();
    style.visuals.widgets.hovered.weak_bg_fill = APP_THEME.active_button_hover();
    style.visuals.widgets.hovered.bg_stroke =
        Stroke::new(1.0, APP_THEME.active_button_border());
    style.visuals.widgets.hovered.fg_stroke.color = phosphor_color();
}

/// Draws the retro-CRT finish over a rect: faint scanlines plus a soft
/// phosphor rim. Cheap enough to paint once per panel.
pub(crate) fn crt_overlay(
    painter: &egui::Painter,
    rect: egui::Rect,
    corner_radius: f32,
) {
    const SCANLINE_GAP: f32 = 3.0;
    let clip = painter.with_clip_rect(rect);
    let scanline = Color32::from_rgba_unmultiplied(0, 0, 0, 26);
    let mut y = rect.top() + SCANLINE_GAP;
    while y < rect.bottom() {
        clip.hline(rect.left()..=rect.right(), y, Stroke::new(1.0, scanline));
        y += SCANLINE_GAP;
    }
    // Phosphor bloom hugging the bezel: a translucent inner rim stroke.
    painter.rect_stroke(
        rect.shrink(1.0),
        corner_radius,
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 140, 30, 26)),
        egui::StrokeKind::Inside,
    );
}

pub fn background_color() -> Color32 {
    Color32::from_rgb(5, 5, 6)
}

/// Bright phosphor orange — fluorescent text, lit accents, active arcs.
pub fn phosphor_color() -> Color32 {
    Color32::from_rgb(255, 140, 20)
}

/// Softer amber for body text so it reads as glowing rather than white.
pub fn text_color() -> Color32 {
    Color32::from_rgb(255, 176, 80)
}

/// Secondary display color (envelope/pitch), kept inside the amber family.
pub fn pitch_env_color() -> Color32 {
    Color32::from_rgb(255, 196, 60)
}

pub fn amp_env_color() -> Color32 {
    Color32::from_rgb(255, 140, 20)
}

pub fn weight_color() -> Color32 {
    Color32::from_rgb(255, 200, 120)
}

pub fn start_weight_color() -> Color32 {
    Color32::from_rgb(255, 220, 120)
}

pub fn end_weight_color() -> Color32 {
    Color32::from_rgb(255, 90, 20)
}

pub fn edge_color() -> Color32 {
    Color32::from_rgb(255, 180, 40)
}

pub fn pitch_edge_color() -> Color32 {
    Color32::from_rgb(255, 210, 120)
}

/// Legacy name kept for existing call sites; now the phosphor accent.
pub fn accent_color() -> Color32 {
    phosphor_color()
}

pub fn axis_title_color() -> Color32 {
    text_color()
}

