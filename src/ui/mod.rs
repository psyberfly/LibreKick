use std::sync::Arc;

mod helpers;
mod state;
mod theme;

use nih_plug::prelude::Editor;
use nih_plug_egui::{
    create_egui_editor,
    egui::{
        self, Align2, Color32, ColorImage, FontId, Pos2, Rect, RichText, Sense, Stroke,
        TextureHandle, TextureOptions, Vec2,
    },
    resizable_window::ResizableWindow,
    EguiState,
};

use crate::{config, patches, shared};

use self::helpers::{
    axis_x_label, axis_y_label, constrain_curve_points, curve_lut, effective_waveform_zoom,
    point_value_label, to_normalized_with_note_end, to_screen_with_note_end, waveform_preview_points,
};
use self::state::BezierUiState;
use self::theme as ui_theme;

const MIN_POINT_GAP_X: f32 = 0.01;
const WAVEFORM_PREVIEW_DURATION_SECONDS: f32 = 1.0;
const WAVEFORM_PREVIEW_MAX_CYCLES_PER_PIXEL: f32 = 0.3;
const HISTORY_STACK_CAP: usize = 200;
const RESIZE_CORNER_VISUAL_SIZE: f32 = 20.0;
const RESIZE_CORNER_HIT_RADIUS: f32 = 30.0;
const RESIZE_SIDE_HIT_RADIUS: f32 = 16.0;
const AXIS_SUBDIVISIONS: usize = 10;
const SHIFT_LOCK_X_FREEZE_AFTER_VERTICAL_RELEASE_SECONDS: f64 = 0.250;
const SHIFT_LOCK_X_REENGAGE_HORIZONTAL_PIXELS: f32 = 4.0;
const AMP_DB_FLOOR: f32 = -30.0;
const NOTE_LENGTH_MAX_SLIDER_MIN_MS: f32 = 100.0;
const NOTE_LENGTH_MAX_SLIDER_MAX_MS: f32 = 5000.0;
struct Theme;

impl Theme {
    fn app_font_family(self) -> egui::FontFamily {
        egui::FontFamily::Name("Open Sans Adwaita Bold Italica".into())
    }

    fn brand_orange(self) -> Color32 {
        Color32::from_rgb(242, 134, 52)
    }

    fn brand_hot_core(self) -> Color32 {
        Color32::from_rgb(255, 104, 74)
    }

    fn brand_glow_inner(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 138, 60, 96)
    }

    fn brand_glow_outer(self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 120, 42, 42)
    }

    fn panel_bg(self) -> Color32 {
        ui_theme::background_color()
    }

    fn graph_bg(self) -> Color32 {
        Color32::from_rgb(16, 19, 22)
    }

    fn post_length_tint(self) -> Color32 {
        Color32::from_rgba_unmultiplied(0, 0, 0, 78)
    }

    fn graph_border(self) -> Color32 {
        Color32::from_rgb(90, 95, 102)
    }

    fn grid_line(self) -> Color32 {
        Color32::from_rgb(34, 39, 45)
    }

    fn note_length_fill(self) -> Color32 {
        ui_theme::accent_color()
    }

    fn note_length_stroke(self) -> Color32 {
        Color32::from_rgb(70, 18, 18)
    }

    fn axis_title(self) -> Color32 {
        Color32::from_rgb(185, 191, 198)
    }

    fn axis_tick(self) -> Color32 {
        Color32::from_rgb(165, 171, 178)
    }

    fn waveform_midline(self) -> Color32 {
        Color32::from_rgba_unmultiplied(120, 128, 136, 45)
    }

    fn waveform_trace(self) -> Color32 {
        Color32::from_rgba_unmultiplied(245, 170, 112, 125)
    }

    fn endpoint_point(self) -> Color32 {
        ui_theme::weight_color()
    }

    fn selected_point(self) -> Color32 {
        ui_theme::weight_color()
    }

    fn control_point(self) -> Color32 {
        ui_theme::weight_color()
    }

    fn point_outline(self) -> Color32 {
        Color32::BLACK
    }

    fn bubble_bg(self) -> Color32 {
        Color32::from_rgba_unmultiplied(24, 28, 33, 220)
    }

    fn bubble_border(self) -> Color32 {
        Color32::from_rgb(90, 95, 102)
    }

    fn bubble_text(self) -> Color32 {
        Color32::from_rgb(224, 230, 238)
    }

    fn active_button_bg(self) -> Color32 {
        Color32::from_rgb(188, 54, 54)
    }

    fn active_button_hover(self) -> Color32 {
        Color32::from_rgb(220, 72, 72)
    }

    fn active_button_border(self) -> Color32 {
        Color32::from_rgb(96, 30, 30)
    }
}

const APP_THEME: Theme = Theme;

fn themed_font(size: f32) -> FontId {
    FontId::new(size, APP_THEME.app_font_family())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CurveKind {
    Amplitude,
    Pitch,
}

fn ui_scale_from_size(size: Vec2) -> f32 {
    let ui_cfg = config::ui_config();
    let scale_x = size.x / ui_cfg.base_editor_width;
    let scale_y = size.y / ui_cfg.base_editor_height;
    ((scale_x + scale_y) * 0.5).clamp(0.8, 2.2)
}

fn apply_ui_text_scale(ui: &mut egui::Ui, scale: f32) {
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

fn brand_logo_texture(ctx: &egui::Context) -> Option<TextureHandle> {
    let image_bytes = include_bytes!("../assets/logo.png");
    let image = image::load_from_memory_with_format(image_bytes, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let [width, height] = [image.width() as usize, image.height() as usize];
    let pixels = image.into_raw();
    let color_image = ColorImage::from_rgba_unmultiplied([width, height], &pixels);

    Some(ctx.load_texture(
        "librekick-brand-logo",
        color_image,
        TextureOptions::LINEAR,
    ))
}

fn title_logo_size(ui: &egui::Ui, scale: f32) -> Vec2 {
    let font = themed_font((26.0 * scale).max(18.0));
    let text_size = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap("LibreKick".to_owned(), font, APP_THEME.brand_orange())
            .size()
    });
    text_size + Vec2::new(10.0 * scale, 6.0 * scale)
}

fn brand_title_logo(ui: &mut egui::Ui, logo: Option<&TextureHandle>, scale: f32) {
    let target_size = title_logo_size(ui, scale);

    if let Some(logo) = logo {
        let image_size = logo.size_vec2();
        if image_size.x > 0.0 && image_size.y > 0.0 {
            let fit = (target_size.x / image_size.x).min(target_size.y / image_size.y);
            let draw_size = image_size * fit;
            ui.add(egui::Image::new(logo).fit_to_exact_size(draw_size));
            return;
        }
    }

    glowing_brand_label(ui, "LibreKick", scale);
}

fn glowing_brand_label(ui: &mut egui::Ui, text: &str, scale: f32) {
    let font = themed_font((26.0 * scale).max(18.0));
    let text_size = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font.clone(), APP_THEME.brand_orange())
            .size()
    });
    let padding = Vec2::new(10.0 * scale, 6.0 * scale);
    let (rect, _) = ui.allocate_exact_size(text_size + padding, Sense::hover());
    let text_pos = Pos2::new(rect.left() + 4.0 * scale, rect.center().y - text_size.y * 0.5);
    let painter = ui.painter();

    let outer_offsets = [
        Vec2::new(-3.0, 0.0),
        Vec2::new(3.0, 0.0),
        Vec2::new(0.0, -3.0),
        Vec2::new(0.0, 3.0),
        Vec2::new(-2.0, -2.0),
        Vec2::new(2.0, -2.0),
        Vec2::new(-2.0, 2.0),
        Vec2::new(2.0, 2.0),
    ];
    for offset in outer_offsets {
        painter.text(
            text_pos + offset * scale,
            Align2::LEFT_TOP,
            text,
            font.clone(),
            APP_THEME.brand_glow_outer(),
        );
    }

    let inner_offsets = [
        Vec2::new(-1.2, 0.0),
        Vec2::new(1.2, 0.0),
        Vec2::new(0.0, -1.2),
        Vec2::new(0.0, 1.2),
    ];
    for offset in inner_offsets {
        painter.text(
            text_pos + offset * scale,
            Align2::LEFT_TOP,
            text,
            font.clone(),
            APP_THEME.brand_glow_inner(),
        );
    }

    painter.text(
        text_pos,
        Align2::LEFT_TOP,
        text,
        font.clone(),
        APP_THEME.brand_hot_core(),
    );
    painter.text(
        text_pos + Vec2::new(0.0, -0.2 * scale),
        Align2::LEFT_TOP,
        text,
        font,
        APP_THEME.brand_orange(),
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TuningStandard {
    A440,
    A432,
}

impl TuningStandard {
    fn a4_hz(self) -> f32 {
        match self {
            TuningStandard::A440 => 440.0,
            TuningStandard::A432 => 432.0,
        }
    }
}

fn tuning_standard_from_a4_hz(hz: f32) -> TuningStandard {
    if (hz - 432.0).abs() <= (hz - 440.0).abs() {
        TuningStandard::A432
    } else {
        TuningStandard::A440
    }
}

pub fn create_testing_editor(
    editor_state: Arc<EguiState>,
    shared_state: shared::SharedStateHandle,
) -> Option<Box<dyn Editor>> {
    let ui_cfg = config::ui_config();
    let resizable_state = editor_state.clone();
    let shared_for_ui = shared_state.clone();

    create_egui_editor(
        editor_state,
        BezierUiState::default(),
        |_ctx, state| {
            let mut fonts = egui::FontDefinitions::default();
            let app_family = APP_THEME.app_font_family();
            if let Some(fallbacks) = fonts.families.get(&egui::FontFamily::Proportional).cloned() {
                fonts.families.insert(app_family, fallbacks);
            }
            _ctx.set_fonts(fonts);
            state.brand_logo = brand_logo_texture(_ctx);
        },
        move |_ctx, _setter, state| {
            ResizableWindow::new("kick-plugin-resize")
                .min_size(Vec2::new(ui_cfg.min_editor_width, ui_cfg.min_editor_height))
                .show(_ctx, &resizable_state, |ui| {
                let snapshot_before = state.snapshot();
                let mut history_action_applied = false;

                let (undo_shortcut, redo_shortcut) = ui.input(|i| {
                    let mut undo = false;
                    let mut redo = false;

                    for event in &i.events {
                        if let egui::Event::Key {
                            key,
                            pressed,
                            modifiers,
                            ..
                        } = event
                        {
                            if !*pressed {
                                continue;
                            }

                            let modifier_down = modifiers.ctrl || modifiers.command;
                            if !modifier_down {
                                continue;
                            }

                            if *key == egui::Key::Z {
                                if modifiers.shift {
                                    redo = true;
                                } else {
                                    undo = true;
                                }
                            } else if *key == egui::Key::Y {
                                redo = true;
                            }
                        }
                    }

                    (undo, redo)
                });
                let (cut_shortcut, delete_shortcut) = ui.input(|i| {
                    let mut cut = false;
                    let mut delete = false;

                    for event in &i.events {
                        match event {
                            egui::Event::Cut => {
                                cut = true;
                            }
                            egui::Event::Key {
                                key,
                                pressed,
                                modifiers,
                                ..
                            } if *pressed => {
                                if *key == egui::Key::X && (modifiers.ctrl || modifiers.command) {
                                    cut = true;
                                }
                                if *key == egui::Key::Delete || *key == egui::Key::Backspace {
                                    delete = true;
                                }
                            }
                            _ => {}
                        }
                    }

                    (cut, delete)
                });
                if undo_shortcut {
                    history_action_applied |= state.undo();
                }
                if redo_shortcut {
                    history_action_applied |= state.redo();
                }

                let ui_scale = ui_scale_from_size(ui.available_size_before_wrap());
                let app_cfg = config::app_config();
                let mut point_dragging_this_frame = false;
                {
                    let style = ui.style_mut();
                    style.interaction.resize_grab_radius_corner =
                        (RESIZE_CORNER_HIT_RADIUS * ui_scale).max(24.0);
                    style.interaction.resize_grab_radius_side =
                        (RESIZE_SIDE_HIT_RADIUS * ui_scale).max(12.0);
                    style.visuals.resize_corner_size =
                        (RESIZE_CORNER_VISUAL_SIZE * ui_scale).max(16.0);
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
                ui.scope(|ui| {
                apply_ui_text_scale(ui, ui_scale);
                ui.add_space(6.0 * ui_scale);
                ui.horizontal(|ui| {
                    brand_title_logo(ui, state.brand_logo.as_ref(), ui_scale);
                    ui.add_space(6.0 * ui_scale);
                    ui.label(
                        RichText::new("Prototype")
                            .strong()
                            .color(APP_THEME.axis_title()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new("?").min_size(Vec2::new(18.0 * ui_scale, 18.0 * ui_scale)))
                            .clicked()
                        {
                            state.show_help_popup = true;
                        }

                        ui.vertical(|ui| {
                            ui.menu_button("Patches", |ui| {
                                apply_ui_text_scale(ui, ui_scale);
                                ui.set_min_width(300.0 * ui_scale);
                                ui.label(format!("Dir: {}", config::patches_dir()));
                                ui.label(format!("Current: {}", state.selected_patch_indicator_text()));
                                ui.separator();
                                ui.label("Load Patch");

                                let patch_names = state.available_patches.clone();
                                if patch_names.is_empty() {
                                    ui.label("No patches found.");
                                } else {
                                    for patch_name in patch_names {
                                        if ui.button(&patch_name).clicked() {
                                            let before = state.snapshot();
                                            match patches::load_patch(&patch_name) {
                                                Ok(patch) => {
                                                    state.apply_patch_data(patch);
                                                    state.mark_patch_clean(patch_name.clone());
                                                    state.commit_history_if_changed(&before);
                                                    state.patch_status =
                                                        Some(format!("Loaded patch: {patch_name}"));
                                                    ui.close_menu();
                                                }
                                                Err(error) => {
                                                    state.patch_status =
                                                        Some(format!("Failed to load patch: {error}"));
                                                    ui.close_menu();
                                                }
                                            }
                                        }
                                    }
                                }

                                ui.separator();
                                ui.label("Save Patch");
                                ui.text_edit_singleline(&mut state.new_patch_name);
                                let can_save_patch = !state.new_patch_name.trim().is_empty();
                                if ui
                                    .add_enabled(can_save_patch, egui::Button::new("Save"))
                                    .clicked()
                                {
                                    let patch_name = state.new_patch_name.trim().to_owned();
                                    let patch_data = state.to_patch_data(patch_name.clone());
                                    match patches::save_patch(&patch_data) {
                                        Ok(()) => {
                                            state.mark_patch_clean(patch_name.clone());
                                            state.patch_status = Some(format!("Saved patch: {patch_name}"));
                                            state.refresh_patch_list();
                                        }
                                        Err(error) => {
                                            state.patch_status =
                                                Some(format!("Failed to save patch: {error}"));
                                        }
                                    }
                                }

                                if ui.button("Set current as default").clicked() {
                                    let selected_name = state.selected_patch_name.clone();
                                    match selected_name {
                                        Some(name) if !state.is_selected_patch_dirty() => {
                                            match patches::set_default_patch_name(&name) {
                                                Ok(()) => {
                                                    state.default_patch_name = Some(name.clone());
                                                    state.patch_status =
                                                        Some(format!("Default patch set: {name}"));
                                                }
                                                Err(error) => {
                                                    state.patch_status = Some(format!(
                                                        "Failed to set default patch: {error}"
                                                    ));
                                                }
                                            }
                                        }
                                        _ => {
                                            state.patch_status = Some(
                                                "Save this patch first, then set it as default."
                                                    .to_owned(),
                                            );
                                        }
                                    }
                                }

                                if ui.button("Refresh List").clicked() {
                                    state.refresh_patch_list();
                                }

                                if let Some(status) = &state.patch_status {
                                    ui.separator();
                                    ui.label(status);
                                }
                            });

                            ui.label(
                                RichText::new(state.selected_patch_indicator_text())
                                    .small()
                                    .color(APP_THEME.axis_tick()),
                            );
                        });
                    });
                });

                if state.show_help_popup {
                    egui::Window::new("Help")
                        .anchor(Align2::RIGHT_TOP, Vec2::new(-10.0 * ui_scale, 10.0 * ui_scale))
                        .collapsible(false)
                        .resizable(false)
                        .open(&mut state.show_help_popup)
                        .show(ui.ctx(), |ui| {
                            ui.label("Mouse controls:");
                            ui.label("- Drag points to shape the selected envelope.");
                            ui.label("- Double-click inside the graph to add a point.");
                            ui.label("- Right-click a point to remove it.");
                            ui.add_space(4.0 * ui_scale);
                            ui.label("Envelope basics:");
                            ui.label("- Amplitude envelope controls volume over time.");
                            ui.label("- Pitch envelope controls pitch over time.");
                        });
                }

                ui.add_space(8.0 * ui_scale);
                ui.horizontal(|ui| {
                    ui.label("Curve:");
                    ui.selectable_value(&mut state.active_curve, CurveKind::Amplitude, "Amplitude");
                    ui.selectable_value(&mut state.active_curve, CurveKind::Pitch, "Pitch");
                    ui.separator();
                    ui.label("Tuning:");
                    ui.selectable_value(&mut state.tuning_standard, TuningStandard::A440, "A=440");
                    ui.selectable_value(&mut state.tuning_standard, TuningStandard::A432, "A=432");
                    ui.separator();
                    ui.checkbox(&mut state.keytrack_enabled, "Keytrack");
                    ui.separator();
                    if ui.button("Trigger").clicked() {
                        shared::request_trigger(&shared_for_ui);
                    }
                    ui.separator();
                    ui.label("Max Note Length");
                    let max_length_changed = ui
                        .add(
                            egui::Slider::new(
                                &mut state.note_length_max_ms,
                                NOTE_LENGTH_MAX_SLIDER_MIN_MS..=NOTE_LENGTH_MAX_SLIDER_MAX_MS,
                            )
                            .text("ms")
                            .step_by(1.0),
                        )
                        .changed();
                    if max_length_changed {
                        state.note_length_max_ms = state.note_length_max_ms
                            .clamp(NOTE_LENGTH_MAX_SLIDER_MIN_MS, NOTE_LENGTH_MAX_SLIDER_MAX_MS);
                        state.note_length_ms = state.note_length_ms.clamp(0.0, state.note_length_max_ms);
                    }
                    ui.separator();
                    if ui.button("-").clicked() {
                        state.waveform_zoom_percent =
                            (state.waveform_zoom_percent - app_cfg.waveform_zoom_step_percent).clamp(
                                app_cfg.waveform_zoom_min_percent,
                                app_cfg.waveform_zoom_max_percent,
                            );
                    }
                    ui.label(format!("Zoom {:.0}%", state.waveform_zoom_percent));
                    if ui.button("+").clicked() {
                        state.waveform_zoom_percent =
                            (state.waveform_zoom_percent + app_cfg.waveform_zoom_step_percent).clamp(
                                app_cfg.waveform_zoom_min_percent,
                                app_cfg.waveform_zoom_max_percent,
                            );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0 * ui_scale);
                        let redo_clicked = ui
                            .add_enabled(!state.redo_stack.is_empty(), egui::Button::new(">"))
                            .on_hover_ui(|ui| {
                                apply_ui_text_scale(ui, ui_scale);
                                ui.label("Redo (Ctrl/Cmd + Y)");
                            })
                            .clicked();
                        let undo_clicked = ui
                            .add_enabled(!state.undo_stack.is_empty(), egui::Button::new("<"))
                            .on_hover_ui(|ui| {
                                apply_ui_text_scale(ui, ui_scale);
                                ui.label("Undo (Ctrl/Cmd + Z)");
                            })
                            .clicked();
                        if redo_clicked {
                            history_action_applied |= state.redo();
                        }
                        if undo_clicked {
                            history_action_applied |= state.undo();
                        }
                    });
                });
                ui.add_space(8.0);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {

                let available = ui.available_size_before_wrap();
                let graph_width = available.x.max(280.0);
                let graph_height = available.y.max(220.0);
                let (outer_rect, graph_response) = ui.allocate_exact_size(
                    Vec2::new(graph_width, graph_height),
                    Sense::click_and_drag(),
                );
                if graph_response.clicked()
                    || graph_response.drag_started_by(egui::PointerButton::Primary)
                    || graph_response.drag_started_by(egui::PointerButton::Secondary)
                {
                    graph_response.request_focus();
                }
                let graph_has_focus = graph_response.has_focus();
                if graph_response.hovered() {
                    graph_response.request_focus();
                }
                let shift_down = ui.input(|i| i.modifiers.shift);
                if shift_down && (graph_response.hovered() || graph_has_focus) {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Crosshair);
                }
                if graph_response.hovered() {
                    let (modifier_down, scroll_y) =
                        ui.input(|i| ((i.modifiers.ctrl || i.modifiers.command), i.raw_scroll_delta.y));
                    if modifier_down && scroll_y.abs() > f32::EPSILON {
                        state.waveform_zoom_percent =
                            (state.waveform_zoom_percent + scroll_y * 0.08).clamp(
                                app_cfg.waveform_zoom_min_percent,
                                app_cfg.waveform_zoom_max_percent,
                            );
                    }
                }
                if !shift_down {
                    state.shift_locked_point = None;
                    state.shift_lock_x_freeze_until_seconds = 0.0;
                    state.shift_lock_require_horizontal_reengage = false;
                    state.shift_lock_reengage_anchor_screen_x = None;
                }
                let left_axis_padding = (62.0 * ui_scale).clamp(52.0, 120.0);
                let bottom_axis_padding = (52.0 * ui_scale).clamp(40.0, 110.0);
                let top_axis_padding = (50.0 * ui_scale).clamp(38.0, 88.0);
                let right_axis_padding = (24.0 * ui_scale).clamp(18.0, 48.0);
                let graph_rect = Rect::from_min_max(
                    Pos2::new(
                        outer_rect.left() + left_axis_padding,
                        outer_rect.top() + top_axis_padding,
                    ),
                    Pos2::new(
                        outer_rect.right() - right_axis_padding,
                        outer_rect.bottom() - bottom_axis_padding,
                    ),
                );

                let painter = ui.painter_at(outer_rect);
                painter.rect_filled(outer_rect, 4.0, APP_THEME.panel_bg());
                painter.rect_filled(graph_rect, 4.0, APP_THEME.graph_bg());

                let max_note_length_ms = state.note_length_max_ms.max(f32::EPSILON);
                let mut note_end_ms = state.note_length_ms.clamp(0.0, max_note_length_ms);
                let mut note_end_t = (note_end_ms / max_note_length_ms).clamp(0.0, 1.0);
                let adaptive_zoom_factor =
                    state.base_note_length_max_ms.max(f32::EPSILON) / max_note_length_ms;
                let waveform_zoom = effective_waveform_zoom(state.waveform_zoom_percent, adaptive_zoom_factor);
                let mut note_end_display_t = (note_end_t * waveform_zoom).clamp(0.0, 1.0);

                let mut note_end_x =
                    egui::lerp(graph_rect.left()..=graph_rect.right(), note_end_display_t);
                let length_handle_center = Pos2::new(
                    note_end_x,
                    graph_rect.bottom() + bottom_axis_padding * 0.34,
                );
                let length_handle_size = Vec2::new(18.0 * ui_scale, (bottom_axis_padding * 0.55).max(18.0));
                let length_handle_rect = Rect::from_center_size(length_handle_center, length_handle_size);
                let length_response = ui.interact(
                    length_handle_rect,
                    ui.make_persistent_id("note-length-handle"),
                    Sense::click_and_drag(),
                );

                if length_response.hovered() || length_response.dragged() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                }

                if length_response.dragged() {
                    if let Some(pointer_pos) = length_response.interact_pointer_pos() {
                        note_end_x = pointer_pos.x.clamp(graph_rect.left(), graph_rect.right());
                        note_end_display_t =
                            ((note_end_x - graph_rect.left()) / graph_rect.width()).clamp(0.0, 1.0);
                        note_end_t = (note_end_display_t / waveform_zoom).clamp(0.0, 1.0);
                        note_end_ms = note_end_t * max_note_length_ms;
                    }
                }

                if note_end_x < graph_rect.right() {
                    let shaded_rect = Rect::from_min_max(
                        Pos2::new(note_end_x, graph_rect.top()),
                        Pos2::new(graph_rect.right(), graph_rect.bottom()),
                    );
                    painter.rect_filled(
                        shaded_rect,
                        0.0,
                        APP_THEME.post_length_tint(),
                    );
                }

                painter.rect_stroke(
                    graph_rect,
                    4.0,
                    Stroke::new(1.0, APP_THEME.graph_border()),
                    egui::StrokeKind::Inside,
                );

                for i in 0..=AXIS_SUBDIVISIONS {
                    let f = i as f32 / AXIS_SUBDIVISIONS as f32;
                    let x = egui::lerp(graph_rect.left()..=graph_rect.right(), f);

                    painter.line_segment(
                        [Pos2::new(x, graph_rect.top()), Pos2::new(x, graph_rect.bottom())],
                        Stroke::new(1.0, APP_THEME.grid_line()),
                    );
                }

                let triangle_half_w = (8.0 * ui_scale).max(6.0);
                let triangle_h = (10.0 * ui_scale).max(8.0);
                let triangle_top_y = graph_rect.bottom() + 2.0 * ui_scale;
                let triangle_points = vec![
                    Pos2::new(note_end_x - triangle_half_w, triangle_top_y),
                    Pos2::new(note_end_x + triangle_half_w, triangle_top_y),
                    Pos2::new(note_end_x, triangle_top_y + triangle_h),
                ];
                painter.add(egui::Shape::convex_polygon(
                    triangle_points,
                    APP_THEME.note_length_fill(),
                    Stroke::new(1.0, APP_THEME.note_length_stroke()),
                ));
                painter.text(
                    Pos2::new(note_end_x, outer_rect.bottom() - bottom_axis_padding * 0.62),
                    Align2::CENTER_BOTTOM,
                    format!("{:.0}ms", note_end_ms),
                    themed_font(10.0 * ui_scale),
                    APP_THEME.note_length_fill(),
                );

                for i in 0..=AXIS_SUBDIVISIONS {
                    let f = i as f32 / AXIS_SUBDIVISIONS as f32;
                    let y = egui::lerp(graph_rect.bottom()..=graph_rect.top(), f);

                    painter.line_segment(
                        [Pos2::new(graph_rect.left(), y), Pos2::new(graph_rect.right(), y)],
                        Stroke::new(1.0, APP_THEME.grid_line()),
                    );
                }

                painter.text(
                    Pos2::new(graph_rect.left(), outer_rect.top() + top_axis_padding * 0.35),
                    Align2::LEFT_BOTTOM,
                    match state.active_curve {
                        CurveKind::Amplitude => "Amount (dB)",
                        CurveKind::Pitch => "Pitch (Hz)",
                    },
                    themed_font(12.0 * ui_scale),
                    match state.active_curve {
                        CurveKind::Amplitude => ui_theme::amp_env_color(),
                        CurveKind::Pitch => ui_theme::pitch_env_color(),
                    },
                );
                painter.text(
                    Pos2::new(graph_rect.right(), outer_rect.bottom() - bottom_axis_padding * 0.2),
                    Align2::RIGHT_TOP,
                    "Time",
                    themed_font(12.0 * ui_scale),
                    APP_THEME.axis_title(),
                );

                for i in 0..=AXIS_SUBDIVISIONS {
                    let f = i as f32 / AXIS_SUBDIVISIONS as f32;
                    let x = egui::lerp(graph_rect.left()..=graph_rect.right(), f);
                    painter.text(
                        Pos2::new(x, graph_rect.bottom() + bottom_axis_padding * 0.08),
                        Align2::CENTER_TOP,
                        axis_x_label((f / waveform_zoom) * max_note_length_ms),
                        themed_font(10.0 * ui_scale),
                        APP_THEME.axis_tick(),
                    );
                }

                for i in 0..=AXIS_SUBDIVISIONS {
                    let f = i as f32 / AXIS_SUBDIVISIONS as f32;
                    let y = egui::lerp(graph_rect.bottom()..=graph_rect.top(), f);
                    painter.text(
                        Pos2::new(graph_rect.left() - left_axis_padding * 0.12, y),
                        Align2::RIGHT_CENTER,
                        axis_y_label(state.active_curve, f),
                        themed_font(10.0 * ui_scale),
                        APP_THEME.axis_tick(),
                    );
                }

                let active_kind = state.active_curve;
                let mut selected_point = state.selected_point;
                let mut selected_points = state.selected_points.clone();
                let mut selection_drag_start = state.selection_drag_start;
                let mut selection_drag_current = state.selection_drag_current;
                let mut shift_locked_point = state.shift_locked_point;
                let mut shift_lock_x_freeze_until_seconds = state.shift_lock_x_freeze_until_seconds;
                let mut shift_lock_require_horizontal_reengage =
                    state.shift_lock_require_horizontal_reengage;
                let mut shift_lock_reengage_anchor_screen_x = state.shift_lock_reengage_anchor_screen_x;
                let mut shift_snap_candidate: Option<usize> = None;
                let mut remove_selected_requested = graph_has_focus && (cut_shortcut || delete_shortcut);

                let curve_point_count = state.active_curve().points.len();
                shift_locked_point = shift_locked_point.filter(|&idx| idx < curve_point_count);
                if shift_locked_point.is_none() {
                    shift_lock_x_freeze_until_seconds = 0.0;
                    shift_lock_require_horizontal_reengage = false;
                    shift_lock_reengage_anchor_screen_x = None;
                }
                selected_points.retain(|&idx| idx < curve_point_count);
                if selected_points.is_empty() {
                    if let Some(idx) = selected_point.filter(|idx| *idx < curve_point_count) {
                        selected_points.push(idx);
                    }
                }
                if shift_down {
                    if let Some(idx) = shift_locked_point {
                        selected_point = Some(idx);
                        selected_points.clear();
                        selected_points.push(idx);
                    }
                }

                graph_response.context_menu(|ui| {
                    apply_ui_text_scale(ui, ui_scale);
                    let can_remove_selected = selected_points
                        .iter()
                        .any(|&idx| idx > 0 && idx + 1 < curve_point_count);
                    if ui
                        .add_enabled(can_remove_selected, egui::Button::new("Remove selected points"))
                        .clicked()
                    {
                        remove_selected_requested = true;
                        ui.close_menu();
                    }
                });

                {
                    let points = &mut state.active_curve_mut().points;
                    constrain_curve_points(points);
                    let mut remove_point_index: Option<usize> = None;

                    for i in 0..points.len() {
                        let screen_point = to_screen_with_note_end(points[i], graph_rect, note_end_display_t);
                        let hit_rect = Rect::from_center_size(screen_point, Vec2::splat(54.0));
                        let response = ui.interact(
                            hit_rect,
                            ui.make_persistent_id(("bezier-control", active_kind as u8, i)),
                            Sense::click_and_drag(),
                        );

                        if response.clicked() {
                            graph_response.request_focus();
                            selected_point = Some(i);
                            selected_points.clear();
                            selected_points.push(i);
                            if shift_down {
                                shift_locked_point = Some(i);
                                shift_lock_x_freeze_until_seconds = 0.0;
                                shift_lock_require_horizontal_reengage = false;
                                shift_lock_reengage_anchor_screen_x = None;
                            }
                        }

                        if response.secondary_clicked() {
                            graph_response.request_focus();
                            selected_point = Some(i);
                            if !selected_points.contains(&i) {
                                selected_points.clear();
                                selected_points.push(i);
                            }
                            if shift_down {
                                shift_locked_point = Some(i);
                                shift_lock_x_freeze_until_seconds = 0.0;
                                shift_lock_require_horizontal_reengage = false;
                                shift_lock_reengage_anchor_screen_x = None;
                            }
                        }

                        let can_remove_here = i > 0 && i + 1 < points.len();
                        response.context_menu(|ui| {
                            apply_ui_text_scale(ui, ui_scale);
                            if ui
                                .add_enabled(can_remove_here, egui::Button::new("Remove point"))
                                .clicked()
                            {
                                remove_point_index = Some(i);
                                ui.close_menu();
                            }
                        });

                        if response.dragged() {
                            graph_response.request_focus();
                            if shift_down && shift_locked_point == Some(i) {
                                continue;
                            }
                            point_dragging_this_frame = true;
                            if let Some(pointer_pos) = response.interact_pointer_pos() {
                                let mut new_point = to_normalized_with_note_end(
                                    pointer_pos,
                                    graph_rect,
                                    note_end_display_t,
                                );
                                if shift_down {
                                    new_point.y = points[i].y;
                                }
                                points[i] = new_point;
                                selected_point = Some(i);
                                selected_points.clear();
                                selected_points.push(i);
                                if shift_down {
                                    shift_locked_point = Some(i);
                                    shift_lock_x_freeze_until_seconds = 0.0;
                                    shift_lock_require_horizontal_reengage = false;
                                    shift_lock_reengage_anchor_screen_x = None;
                                }
                                constrain_curve_points(points);
                            }
                        }
                    }

                    if shift_down {
                        let pointer_primary_down = ui.input(|i| i.pointer.primary_down());
                        let pointer_primary_released =
                            ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary));
                        let pointer_primary_clicked = ui.input(|i| i.pointer.primary_clicked());
                        let pointer_pos = ui.input(|i| i.pointer.hover_pos());

                        if let Some(pointer_pos) = pointer_pos.filter(|pos| graph_rect.contains(*pos)) {
                            if shift_locked_point.is_none() {
                                let snap_assist_radius = 30.0_f32;
                                let mut best: Option<(usize, f32)> = None;
                                for (idx, point) in points.iter().enumerate() {
                                    let screen =
                                        to_screen_with_note_end(*point, graph_rect, note_end_display_t);
                                    let distance = screen.distance(pointer_pos);
                                    if distance <= snap_assist_radius {
                                        if let Some((_, best_distance)) = best {
                                            if distance < best_distance {
                                                best = Some((idx, distance));
                                            }
                                        } else {
                                            best = Some((idx, distance));
                                        }
                                    }
                                }

                                if let Some((idx, _)) = best {
                                    shift_snap_candidate = Some(idx);
                                    if pointer_primary_clicked {
                                        shift_locked_point = Some(idx);
                                        shift_lock_x_freeze_until_seconds = 0.0;
                                        shift_lock_require_horizontal_reengage = false;
                                        shift_lock_reengage_anchor_screen_x = None;
                                        selected_point = Some(idx);
                                        selected_points.clear();
                                        selected_points.push(idx);
                                    }
                                }
                            }

                            if let Some(idx) = shift_locked_point.filter(|&idx| idx < points.len()) {
                                let locked_screen_x =
                                    to_screen_with_note_end(points[idx], graph_rect, note_end_display_t).x;
                                let virtual_pointer_pos = Pos2::new(locked_screen_x, pointer_pos.y);
                                let mapped_point = to_normalized_with_note_end(
                                    virtual_pointer_pos,
                                    graph_rect,
                                    note_end_display_t,
                                );
                                let mut new_point = points[idx];
                                let now_seconds = ui.input(|i| i.time);
                                if pointer_primary_released {
                                    shift_lock_x_freeze_until_seconds =
                                        now_seconds + SHIFT_LOCK_X_FREEZE_AFTER_VERTICAL_RELEASE_SECONDS;
                                    shift_lock_require_horizontal_reengage = true;
                                    shift_lock_reengage_anchor_screen_x = Some(locked_screen_x);
                                }
                                if pointer_primary_down {
                                    new_point.y = mapped_point.y;
                                    shift_lock_require_horizontal_reengage = true;
                                    shift_lock_reengage_anchor_screen_x = Some(locked_screen_x);
                                } else {
                                    if shift_lock_require_horizontal_reengage {
                                        if let Some(anchor_x) = shift_lock_reengage_anchor_screen_x {
                                            if (pointer_pos.x - anchor_x).abs()
                                                >= SHIFT_LOCK_X_REENGAGE_HORIZONTAL_PIXELS
                                            {
                                                shift_lock_require_horizontal_reengage = false;
                                                shift_lock_reengage_anchor_screen_x = None;
                                            }
                                        } else {
                                            shift_lock_reengage_anchor_screen_x = Some(pointer_pos.x);
                                        }
                                    }

                                    if now_seconds >= shift_lock_x_freeze_until_seconds
                                        && !shift_lock_require_horizontal_reengage
                                    {
                                        new_point.x = mapped_point.x;
                                    }
                                }
                                if (new_point.x - points[idx].x).abs() > f32::EPSILON
                                    || (new_point.y - points[idx].y).abs() > f32::EPSILON
                                {
                                    points[idx] = new_point;
                                    point_dragging_this_frame = true;
                                    selected_point = Some(idx);
                                    selected_points.clear();
                                    selected_points.push(idx);
                                    shift_locked_point = Some(idx);
                                    constrain_curve_points(points);
                                }
                            }
                        }
                    }

                    if graph_response.drag_started_by(egui::PointerButton::Primary)
                        && !point_dragging_this_frame
                        && !shift_down
                    {
                        if let Some(pointer_pos) = graph_response.interact_pointer_pos() {
                            if graph_rect.contains(pointer_pos) {
                                selection_drag_start = Some(pointer_pos);
                                selection_drag_current = Some(pointer_pos);
                                selected_points.clear();
                                selected_point = None;
                            }
                        }
                    }

                    let pointer_primary_down = ui.input(|i| i.pointer.primary_down());
                    if selection_drag_start.is_some() && pointer_primary_down && !point_dragging_this_frame {
                        if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                            selection_drag_current = Some(pointer_pos);
                        }
                    }

                    if selection_drag_start.is_some() && !pointer_primary_down {
                        if let (Some(start), Some(end)) = (selection_drag_start, selection_drag_current) {
                            let selection_rect = Rect::from_two_pos(start, end).intersect(graph_rect);
                            if selection_rect.width() > 1.0 || selection_rect.height() > 1.0 {
                                selected_points = points
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(idx, point)| {
                                        let screen_point =
                                            to_screen_with_note_end(*point, graph_rect, note_end_display_t);
                                        if selection_rect.contains(screen_point) {
                                            Some(idx)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                selected_point = selected_points.first().copied();
                            }
                        }
                        selection_drag_start = None;
                        selection_drag_current = None;
                    }

                    if remove_selected_requested {
                        let mut remove_indices: Vec<usize> = selected_points
                            .iter()
                            .copied()
                            .filter(|&idx| idx > 0 && idx + 1 < points.len())
                            .collect();
                        remove_indices.sort_unstable();
                        remove_indices.dedup();

                        if !remove_indices.is_empty() {
                            for idx in remove_indices.into_iter().rev() {
                                points.remove(idx);
                            }
                            constrain_curve_points(points);
                            selected_points.clear();
                            if points.len() > 1 {
                                let fallback = 1.min(points.len() - 1);
                                selected_point = Some(fallback);
                                selected_points.push(fallback);
                            } else {
                                selected_point = Some(0);
                                selected_points.push(0);
                            }
                        }
                    }

                    if let Some(remove_index) = remove_point_index {
                        points.remove(remove_index);
                        constrain_curve_points(points);
                        let fallback =
                            remove_index
                                .saturating_sub(1)
                                .min(points.len() - 2)
                                .max(1);
                        selected_point = Some(fallback);
                        selected_points.clear();
                        selected_points.push(fallback);
                    }

                    if graph_response.double_clicked() {
                        if let Some(pointer_pos) = graph_response.interact_pointer_pos() {
                            if pointer_pos.x <= note_end_x {
                                let new_point = to_normalized_with_note_end(
                                    pointer_pos,
                                    graph_rect,
                                    note_end_display_t,
                                );
                                let insert_index = points
                                    .iter()
                                    .position(|p| p.x > new_point.x)
                                    .unwrap_or(points.len() - 1);
                                let index = insert_index.max(1).min(points.len() - 1);
                                points.insert(index, new_point);
                                constrain_curve_points(points);
                                selected_point = Some(index);
                                selected_points.clear();
                                selected_points.push(index);
                            }
                        }
                    }

                }

                state.selected_point = selected_point;
                state.selected_points = selected_points;
                state.selection_drag_start = selection_drag_start;
                state.selection_drag_current = selection_drag_current;
                state.shift_locked_point = shift_locked_point;
                state.shift_lock_x_freeze_until_seconds = shift_lock_x_freeze_until_seconds;
                state.shift_lock_require_horizontal_reengage = shift_lock_require_horizontal_reengage;
                state.shift_lock_reengage_anchor_screen_x = shift_lock_reengage_anchor_screen_x;
                let active_points = state.active_curve().points.clone();
                let tuning_a4_hz = state.tuning_standard.a4_hz();
                shared::set_tuning_a4_hz(&shared_for_ui, tuning_a4_hz);
                shared::set_keytrack_enabled(&shared_for_ui, state.keytrack_enabled);
                state.note_length_ms = note_end_ms.clamp(0.0, max_note_length_ms);
                shared::set_note_length_ms(&shared_for_ui, state.note_length_ms);

                let amplitude_lut = curve_lut(&state.amplitude_curve.points);
                let pitch_lut = curve_lut(&state.pitch_curve.points);
                shared::set_curve_lut(&shared_for_ui, shared::CurveKind::Amplitude, amplitude_lut);
                shared::set_curve_lut(&shared_for_ui, shared::CurveKind::Pitch, pitch_lut);

                let waveform_points = waveform_preview_points(
                    graph_rect,
                    &state.amplitude_curve.points,
                    &state.pitch_curve.points,
                    tuning_a4_hz,
                    state.note_length_ms,
                    max_note_length_ms,
                    state.waveform_zoom_percent,
                    adaptive_zoom_factor,
                );

                if let (Some(first), Some(last)) = (waveform_points.first(), waveform_points.last()) {
                    let mid_y = graph_rect.center().y;
                    painter.line_segment(
                        [Pos2::new(first.x, mid_y), Pos2::new(last.x, mid_y)],
                        Stroke::new(1.0, APP_THEME.waveform_midline()),
                    );
                }

                for line in waveform_points.windows(2) {
                    painter.line_segment(
                        [line[0], line[1]],
                        Stroke::new(1.0, APP_THEME.waveform_trace()),
                    );
                }

                let screen_points: Vec<Pos2> = active_points
                    .iter()
                    .map(|point| to_screen_with_note_end(*point, graph_rect, note_end_display_t))
                    .collect();

                for line in screen_points.windows(2) {
                    painter.line_segment(
                        [line[0], line[1]],
                        Stroke::new(
                            match active_kind {
                                CurveKind::Amplitude => 2.0,
                                CurveKind::Pitch => 3.5,
                            },
                            match active_kind {
                                CurveKind::Amplitude => ui_theme::edge_color(),
                                CurveKind::Pitch => ui_theme::pitch_edge_color(),
                            },
                        ),
                    );
                }

                for (i, point) in screen_points.iter().enumerate() {
                    let color = if i == 0 || i + 1 == screen_points.len() {
                        APP_THEME.endpoint_point()
                    } else if state.selected_points.contains(&i) {
                        APP_THEME.selected_point()
                    } else {
                        APP_THEME.control_point()
                    };
                    painter.circle_filled(*point, 6.0, color);
                    painter.circle_stroke(*point, 7.0, Stroke::new(1.0, APP_THEME.point_outline()));

                    if shift_down && shift_snap_candidate == Some(i) {
                        painter.circle_stroke(
                            *point,
                            11.0,
                            Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 72, 72, 180)),
                        );
                    }

                    if shift_down && state.shift_locked_point == Some(i) {
                        painter.circle_stroke(
                            *point,
                            12.0,
                            Stroke::new(2.0, ui_theme::accent_color()),
                        );
                        let cross_len = 8.0;
                        painter.line_segment(
                            [
                                Pos2::new(point.x - cross_len, point.y),
                                Pos2::new(point.x + cross_len, point.y),
                            ],
                            Stroke::new(1.6, APP_THEME.selected_point()),
                        );
                        painter.line_segment(
                            [
                                Pos2::new(point.x, point.y - cross_len),
                                Pos2::new(point.x, point.y + cross_len),
                            ],
                            Stroke::new(1.6, APP_THEME.selected_point()),
                        );
                    }

                    if let Some(value_point) = active_points.get(i).copied() {
                        let label = point_value_label(active_kind, value_point, tuning_a4_hz);
                        let bubble_width = (label.len() as f32 * 7.0 * ui_scale + 14.0 * ui_scale)
                            .max(56.0 * ui_scale);
                        let bubble_height = 20.0 * ui_scale;
                        let bubble_min =
                            Pos2::new(point.x + 10.0 * ui_scale, point.y - bubble_height * 0.5);
                        let bubble_rect = Rect::from_min_size(
                            bubble_min,
                            Vec2::new(bubble_width, bubble_height),
                        );

                        painter.rect_filled(
                            bubble_rect,
                            bubble_height * 0.5,
                            APP_THEME.bubble_bg(),
                        );
                        painter.rect_stroke(
                            bubble_rect,
                            bubble_height * 0.5,
                            Stroke::new(1.0, APP_THEME.bubble_border()),
                            egui::StrokeKind::Inside,
                        );
                        painter.text(
                            bubble_rect.center(),
                            Align2::CENTER_CENTER,
                            label,
                            themed_font(11.0 * ui_scale),
                            APP_THEME.bubble_text(),
                        );
                    }
                }
                if let (Some(start), Some(current)) =
                    (state.selection_drag_start, state.selection_drag_current)
                {
                    let selection_rect = Rect::from_two_pos(start, current).intersect(graph_rect);
                    painter.rect_filled(
                        selection_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(255, 200, 0, 36),
                    );
                    painter.rect_stroke(
                        selection_rect,
                        0.0,
                        Stroke::new(1.0, APP_THEME.selected_point()),
                        egui::StrokeKind::Inside,
                    );
                }
                if let Some(selected) = selected_point {
                    if let Some(point) = active_points.get(selected) {
                        ui.label(format!(
                            "Selected P{}: time={:.3}, amount={:.3}",
                            selected, point.x, point.y
                        ));
                    }
                } else {
                    ui.label("No point selected.");
                }
                if state.selected_points.len() > 1 {
                    ui.label(format!("{} points selected.", state.selected_points.len()));
                }
                ui.label(
                    "Click/drag points to edit. Drag box to multi-select. Delete/Backspace/Ctrl(Cmd)+X removes selected points.",
                );
                    });
                if point_dragging_this_frame && state.point_drag_snapshot.is_none() {
                    state.point_drag_snapshot = Some(snapshot_before.clone());
                }
                let pointer_primary_down = ui.input(|i| i.pointer.primary_down());
                if !point_dragging_this_frame && !pointer_primary_down {
                    if let Some(drag_start_snapshot) = state.point_drag_snapshot.take() {
                        state.push_undo_snapshot(drag_start_snapshot);
                    }
                }

                if !history_action_applied && state.point_drag_snapshot.is_none() {
                    state.commit_history_if_changed(&snapshot_before);
                }
                });
                });
        },
    )
}
