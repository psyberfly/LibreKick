use std::sync::Arc;

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

use crate::{config, shared};

const MIN_POINT_GAP_X: f32 = 0.01;
const WAVEFORM_PREVIEW_DURATION_SECONDS: f32 = 1.0;
const WAVEFORM_PREVIEW_MAX_CYCLES_PER_PIXEL: f32 = 0.3;
const HISTORY_STACK_CAP: usize = 200;
const RESIZE_CORNER_VISUAL_SIZE: f32 = 20.0;
const RESIZE_CORNER_HIT_RADIUS: f32 = 30.0;
const RESIZE_SIDE_HIT_RADIUS: f32 = 16.0;
const AXIS_SUBDIVISIONS: usize = 10;
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
        Color32::from_rgb(10, 12, 14)
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
        Color32::from_rgb(220, 64, 64)
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
        Color32::from_rgb(220, 64, 64)
    }

    fn selected_point(self) -> Color32 {
        Color32::from_rgb(255, 214, 122)
    }

    fn control_point(self) -> Color32 {
        Color32::from_rgb(245, 160, 88)
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
    let image_bytes = include_bytes!("../media/logo.png");
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

fn axis_y_label(kind: CurveKind, normalized: f32) -> String {
    match kind {
        CurveKind::Amplitude => {
            let db = AMP_DB_FLOOR + normalized.clamp(0.0, 1.0) * (0.0 - AMP_DB_FLOOR);
            format!("{:.0} dB", db)
        }
        CurveKind::Pitch => {
            let hz = pitch_hz_from_normalized(normalized);
            if hz >= 1000.0 {
                format!("{:.1}k", hz / 1000.0)
            } else {
                format!("{:.0}", hz)
            }
        }
    }
}

fn axis_x_label(normalized: f32, note_length_max_ms: f32) -> String {
    if normalized <= 0.0 {
        "0s".to_owned()
    } else {
        format!("{:.0}ms", normalized * note_length_max_ms)
    }
}

fn waveform_preview_points(
    graph_rect: Rect,
    amplitude_points: &[Pos2],
    pitch_points: &[Pos2],
    tuning_a4_hz: f32,
    note_length_ms: f32,
    note_length_max_ms: f32,
    waveform_zoom_percent: f32,
    adaptive_zoom_factor: f32,
) -> Vec<Pos2> {
    let app_cfg = config::app_config();
    let pixel_width = graph_rect.width().max(1.0) as usize;
    let note_length_t = (note_length_ms / note_length_max_ms.max(f32::EPSILON)).clamp(0.0, 1.0);
    let user_zoom = (waveform_zoom_percent / 100.0).clamp(
        app_cfg.waveform_zoom_min_percent / 100.0,
        app_cfg.waveform_zoom_max_percent / 100.0,
    );
    let zoom = (user_zoom * adaptive_zoom_factor.max(f32::EPSILON)).max(f32::EPSILON);
    let source_length_t = (note_length_t / zoom).min(note_length_t);
    let display_length_t = (note_length_t * zoom).min(note_length_t);
    let active_pixel_width = ((pixel_width as f32 * display_length_t).round() as usize).min(pixel_width);
    if active_pixel_width == 0 {
        return Vec::new();
    }

    let source_seconds = (WAVEFORM_PREVIEW_DURATION_SECONDS * source_length_t).max(0.001);
    let max_display_hz = ((active_pixel_width as f32 / source_seconds)
        * WAVEFORM_PREVIEW_MAX_CYCLES_PER_PIXEL)
        .max(5.0);
    let tuning_scale = tuning_a4_hz / app_cfg.default_tuning_a4_hz.max(f32::EPSILON);
    let mut phase = 0.0_f32;

    let mut previous_t = 0.0_f32;
    let raw_points: Vec<Pos2> = (0..active_pixel_width)
        .map(|col| {
            let x = (col as f32 + 0.5) / pixel_width as f32;
            let x_t = x.min(display_length_t);
            let t = (x_t / zoom).clamp(0.0, source_length_t);

            let amp = envelope_value_amplitude_db(amplitude_points, t);
            let pitch = envelope_value_linear(pitch_points, t);
            let hz = (pitch_hz_from_normalized(pitch) * tuning_scale)
                .clamp(20.0, 22050.0)
                .min(max_display_hz);

            let dt = ((t - previous_t).max(0.0)) * WAVEFORM_PREVIEW_DURATION_SECONDS;
            phase = (phase + std::f32::consts::TAU * hz * dt).rem_euclid(std::f32::consts::TAU);
            previous_t = t;

            let sample = phase.sin() * amp;
            let y = (0.5 + sample * 0.46).clamp(0.0, 1.0);
            to_screen(Pos2::new(x, y), graph_rect)
        })
        .collect();

    raw_points
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

#[derive(Clone, PartialEq)]
struct Curve {
    points: Vec<Pos2>,
}

#[derive(Clone, PartialEq)]
struct EditorSnapshot {
    amplitude_curve: Curve,
    pitch_curve: Curve,
    active_curve: CurveKind,
    tuning_standard: TuningStandard,
    note_length_ms: f32,
    note_length_max_ms: f32,
    waveform_zoom_percent: f32,
    selected_point: Option<usize>,
}

impl Curve {
    fn default_amplitude() -> Self {
        Self {
            points: vec![
                Pos2::new(0.0, 1.0),
                Pos2::new(0.12, 0.94),
                Pos2::new(0.42, 0.24),
                Pos2::new(1.0, 0.0),
            ],
        }
    }

    fn default_pitch() -> Self {
        Self {
            points: vec![
                Pos2::new(0.0, 1.0),
                Pos2::new(0.08, 0.98),
                Pos2::new(0.30, 0.30),
                Pos2::new(1.0, 0.08),
            ],
        }
    }
}

struct BezierUiState {
    amplitude_curve: Curve,
    pitch_curve: Curve,
    active_curve: CurveKind,
    tuning_standard: TuningStandard,
    note_length_ms: f32,
    note_length_max_ms: f32,
    base_note_length_max_ms: f32,
    waveform_zoom_percent: f32,
    selected_point: Option<usize>,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    point_drag_snapshot: Option<EditorSnapshot>,
    brand_logo: Option<TextureHandle>,
    show_help_popup: bool,
}

impl Default for BezierUiState {
    fn default() -> Self {
        let note_length_max_ms = config::app_config().note_length_max_ms;
        Self {
            amplitude_curve: Curve::default_amplitude(),
            pitch_curve: Curve::default_pitch(),
            active_curve: CurveKind::Amplitude,
            tuning_standard: TuningStandard::A432,
            note_length_ms: note_length_max_ms,
            note_length_max_ms,
            base_note_length_max_ms: note_length_max_ms,
            waveform_zoom_percent: 100.0,
            selected_point: Some(1),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            point_drag_snapshot: None,
            brand_logo: None,
            show_help_popup: false,
        }
    }
}

impl BezierUiState {
    fn push_bounded_snapshot(stack: &mut Vec<EditorSnapshot>, snapshot: EditorSnapshot) {
        stack.push(snapshot);
        let overflow = stack.len().saturating_sub(HISTORY_STACK_CAP);
        if overflow > 0 {
            stack.drain(0..overflow);
        }
    }

    fn push_undo_snapshot(&mut self, snapshot: EditorSnapshot) {
        if self.snapshot() != snapshot {
            Self::push_bounded_snapshot(&mut self.undo_stack, snapshot);
            self.redo_stack.clear();
        }
    }

    fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            amplitude_curve: self.amplitude_curve.clone(),
            pitch_curve: self.pitch_curve.clone(),
            active_curve: self.active_curve,
            tuning_standard: self.tuning_standard,
            note_length_ms: self.note_length_ms,
            note_length_max_ms: self.note_length_max_ms,
            waveform_zoom_percent: self.waveform_zoom_percent,
            selected_point: self.selected_point,
        }
    }

    fn apply_snapshot(&mut self, snapshot: EditorSnapshot) {
        self.amplitude_curve = snapshot.amplitude_curve;
        self.pitch_curve = snapshot.pitch_curve;
        self.active_curve = snapshot.active_curve;
        self.tuning_standard = snapshot.tuning_standard;
        self.note_length_ms = snapshot.note_length_ms;
        self.note_length_max_ms = snapshot.note_length_max_ms;
        self.waveform_zoom_percent = snapshot.waveform_zoom_percent;
        self.selected_point = snapshot.selected_point;
    }

    fn commit_history_if_changed(&mut self, before: &EditorSnapshot) {
        self.push_undo_snapshot(before.clone());
    }

    fn undo(&mut self) -> bool {
        if let Some(snapshot) = self.undo_stack.pop() {
            let current = self.snapshot();
            Self::push_bounded_snapshot(&mut self.redo_stack, current);
            self.apply_snapshot(snapshot);
            return true;
        }
        false
    }

    fn redo(&mut self) -> bool {
        if let Some(snapshot) = self.redo_stack.pop() {
            let current = self.snapshot();
            Self::push_bounded_snapshot(&mut self.undo_stack, current);
            self.apply_snapshot(snapshot);
            return true;
        }
        false
    }

    fn active_curve(&self) -> &Curve {
        match self.active_curve {
            CurveKind::Amplitude => &self.amplitude_curve,
            CurveKind::Pitch => &self.pitch_curve,
        }
    }

    fn active_curve_mut(&mut self) -> &mut Curve {
        match self.active_curve {
            CurveKind::Amplitude => &mut self.amplitude_curve,
            CurveKind::Pitch => &mut self.pitch_curve,
        }
    }
}

fn to_screen(point: Pos2, rect: Rect) -> Pos2 {
    Pos2::new(
        rect.left() + point.x * rect.width(),
        rect.bottom() - point.y * rect.height(),
    )
}

fn to_screen_with_note_length(point: Pos2, rect: Rect, note_length_t: f32) -> Pos2 {
    let note_length_t = note_length_t.clamp(0.0, 1.0);
    Pos2::new(
        rect.left() + point.x * note_length_t * rect.width(),
        rect.bottom() - point.y * rect.height(),
    )
}

fn to_normalized_with_note_length(point: Pos2, rect: Rect, note_length_t: f32) -> Pos2 {
    let note_length_t = note_length_t.clamp(0.0, 1.0).max(f32::EPSILON);
    let x = ((point.x - rect.left()) / (rect.width() * note_length_t)).clamp(0.0, 1.0);
    let y = ((rect.bottom() - point.y) / rect.height()).clamp(0.0, 1.0);
    Pos2::new(x, y)
}

fn envelope_value_linear(points: &[Pos2], t: f32) -> f32 {
    if points.is_empty() {
        return 0.0;
    }

    let t = t.clamp(0.0, 1.0);
    if t <= points[0].x {
        return points[0].y.clamp(0.0, 1.0);
    }

    for pair in points.windows(2) {
        let left = pair[0];
        let right = pair[1];
        if t <= right.x {
            let span = (right.x - left.x).max(f32::EPSILON);
            let local_t = ((t - left.x) / span).clamp(0.0, 1.0);
            return egui::lerp(left.y..=right.y, local_t).clamp(0.0, 1.0);
        }
    }

    points.last().map_or(0.0, |p| p.y).clamp(0.0, 1.0)
}

fn amplitude_floor_linear() -> f32 {
    10.0_f32.powf(AMP_DB_FLOOR / 20.0)
}

fn amplitude_db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db.clamp(AMP_DB_FLOOR, 0.0) / 20.0)
}

fn envelope_value_amplitude_db(points: &[Pos2], t: f32) -> f32 {
    if points.is_empty() {
        return 0.0;
    }

    let t = t.clamp(0.0, 1.0);
    let point_db = |y: f32| amplitude_db(y);

    if t <= points[0].x {
        return points[0].y.clamp(0.0, 1.0);
    }

    for pair in points.windows(2) {
        let left = pair[0];
        let right = pair[1];
        if t <= right.x {
            let span = (right.x - left.x).max(f32::EPSILON);
            let local_t = ((t - left.x) / span).clamp(0.0, 1.0);
            let interpolated_db = egui::lerp(point_db(left.y)..=point_db(right.y), local_t);
            return amplitude_db_to_linear(interpolated_db).clamp(0.0, 1.0);
        }
    }

    points.last().map_or(0.0, |p| p.y).clamp(0.0, 1.0)
}

fn curve_lut(points: &[Pos2]) -> [f32; shared::CURVE_LUT_SIZE] {
    let mut lut = [0.0; shared::CURVE_LUT_SIZE];

    for (i, value) in lut.iter_mut().enumerate() {
        let t = i as f32 / (shared::CURVE_LUT_SIZE as f32 - 1.0);
        *value = envelope_value_linear(points, t);
    }

    lut
}

fn amplitude_db(value: f32) -> f32 {
    (20.0 * value.max(amplitude_floor_linear()).log10()).clamp(AMP_DB_FLOOR, 0.0)
}

fn pitch_hz_from_normalized(value: f32) -> f32 {
    let min_hz = 20.0_f32;
    let max_hz = 20_000.0_f32;
    min_hz * (max_hz / min_hz).powf(value.clamp(0.0, 1.0))
}

fn note_name_from_hz(hz: f32, tuning_a4_hz: f32) -> String {
    const NOTE_NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    let midi_note = (69.0 + 12.0 * (hz / tuning_a4_hz.max(1.0)).log2()).round() as i32;
    let octave = midi_note / 12 - 1;
    let note_idx = midi_note.rem_euclid(12) as usize;

    format!("{}{}", NOTE_NAMES[note_idx], octave)
}

fn point_value_label(kind: CurveKind, point: Pos2, tuning_a4_hz: f32) -> String {
    match kind {
        CurveKind::Amplitude => format!("{:.1} dB", amplitude_db(point.y)),
        CurveKind::Pitch => {
            let hz = pitch_hz_from_normalized(point.y)
                * (tuning_a4_hz / config::app_config().default_tuning_a4_hz.max(f32::EPSILON));
            let note = note_name_from_hz(hz, tuning_a4_hz);
            format!("{} {:.1}Hz", note, hz)
        }
    }
}

fn constrain_curve_points(points: &mut [Pos2]) {
    if points.len() < 2 {
        return;
    }

    points[0].x = 0.0;
    points[0].y = points[0].y.clamp(0.0, 1.0);

    let last = points.len() - 1;
    points[last].x = 1.0;
    points[last].y = points[last].y.clamp(0.0, 1.0);

    for i in 1..last {
        let min_x = (points[i - 1].x + MIN_POINT_GAP_X).clamp(0.0, 1.0);
        let max_x = (points[i + 1].x - MIN_POINT_GAP_X).clamp(0.0, 1.0);
        points[i].x = points[i].x.clamp(min_x.min(max_x), max_x.max(min_x));
        points[i].y = points[i].y.clamp(0.0, 1.0);
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
                    let modifier_down = i.modifiers.ctrl || i.modifiers.command;
                    (
                        modifier_down && i.key_pressed(egui::Key::Z),
                        modifier_down && i.key_pressed(egui::Key::Y),
                    )
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
                    if ui.button("Trigger").clicked() {
                        shared::request_trigger(&shared_for_ui);
                    }
                    ui.separator();
                    ui.label("Max Length");
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
                    Sense::click(),
                );
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

                let note_length_max_ms = state.note_length_max_ms.max(f32::EPSILON);
                let mut note_length_ms = state.note_length_ms.clamp(0.0, note_length_max_ms);
                let mut note_length_norm = (note_length_ms / note_length_max_ms).clamp(0.0, 1.0);

                let mut note_length_x = egui::lerp(graph_rect.left()..=graph_rect.right(), note_length_norm);
                let length_handle_center = Pos2::new(
                    note_length_x,
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
                        note_length_x = pointer_pos.x.clamp(graph_rect.left(), graph_rect.right());
                        note_length_norm =
                            ((note_length_x - graph_rect.left()) / graph_rect.width()).clamp(0.0, 1.0);
                        note_length_ms = note_length_norm * note_length_max_ms;
                    }
                }

                if note_length_x < graph_rect.right() {
                    let shaded_rect = Rect::from_min_max(
                        Pos2::new(note_length_x, graph_rect.top()),
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
                    Pos2::new(note_length_x - triangle_half_w, triangle_top_y),
                    Pos2::new(note_length_x + triangle_half_w, triangle_top_y),
                    Pos2::new(note_length_x, triangle_top_y + triangle_h),
                ];
                painter.add(egui::Shape::convex_polygon(
                    triangle_points,
                    APP_THEME.note_length_fill(),
                    Stroke::new(1.0, APP_THEME.note_length_stroke()),
                ));
                painter.text(
                    Pos2::new(note_length_x, outer_rect.bottom() - bottom_axis_padding * 0.62),
                    Align2::CENTER_BOTTOM,
                    format!("{:.0}ms", note_length_ms),
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
                    APP_THEME.axis_title(),
                );
                painter.text(
                    Pos2::new(graph_rect.right(), outer_rect.bottom() - bottom_axis_padding * 0.2),
                    Align2::RIGHT_TOP,
                    "Length",
                    themed_font(12.0 * ui_scale),
                    APP_THEME.axis_title(),
                );

                for i in 0..=AXIS_SUBDIVISIONS {
                    let f = i as f32 / AXIS_SUBDIVISIONS as f32;
                    let x = egui::lerp(graph_rect.left()..=graph_rect.right(), f);
                    painter.text(
                        Pos2::new(x, graph_rect.bottom() + bottom_axis_padding * 0.08),
                        Align2::CENTER_TOP,
                        axis_x_label(f, note_length_max_ms),
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

                {
                    let points = &mut state.active_curve_mut().points;
                    constrain_curve_points(points);
                    let mut remove_point_index: Option<usize> = None;

                    for i in 0..points.len() {
                        let screen_point = to_screen_with_note_length(points[i], graph_rect, note_length_norm);
                        let hit_rect = Rect::from_center_size(screen_point, Vec2::splat(18.0));
                        let response = ui.interact(
                            hit_rect,
                            ui.make_persistent_id(("bezier-control", active_kind as u8, i)),
                            Sense::click_and_drag(),
                        );

                        if response.clicked() {
                            selected_point = Some(i);
                        }

                        if response.secondary_clicked() {
                            selected_point = Some(i);
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
                            point_dragging_this_frame = true;
                            if let Some(pointer_pos) = response.interact_pointer_pos() {
                                points[i] = to_normalized_with_note_length(
                                    pointer_pos,
                                    graph_rect,
                                    note_length_norm,
                                );
                                selected_point = Some(i);
                                constrain_curve_points(points);
                            }
                        }
                    }

                    if let Some(remove_index) = remove_point_index {
                        points.remove(remove_index);
                        constrain_curve_points(points);
                        selected_point = Some(
                            remove_index
                                .saturating_sub(1)
                                .min(points.len() - 2)
                                .max(1),
                        );
                    }

                    if graph_response.double_clicked() {
                        if let Some(pointer_pos) = graph_response.interact_pointer_pos() {
                            if pointer_pos.x <= note_length_x {
                                let new_point = to_normalized_with_note_length(
                                    pointer_pos,
                                    graph_rect,
                                    note_length_norm,
                                );
                                let insert_index = points
                                    .iter()
                                    .position(|p| p.x > new_point.x)
                                    .unwrap_or(points.len() - 1);
                                let index = insert_index.max(1).min(points.len() - 1);
                                points.insert(index, new_point);
                                constrain_curve_points(points);
                                selected_point = Some(index);
                            }
                        }
                    }

                }

                state.selected_point = selected_point;
                let active_points = state.active_curve().points.clone();
                let tuning_a4_hz = state.tuning_standard.a4_hz();
                shared::set_tuning_a4_hz(&shared_for_ui, tuning_a4_hz);
                state.note_length_ms = note_length_ms.clamp(0.0, note_length_max_ms);
                shared::set_note_length_ms(&shared_for_ui, state.note_length_ms);
                let adaptive_zoom_factor =
                    state.base_note_length_max_ms.max(f32::EPSILON) / note_length_max_ms;

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
                    note_length_max_ms,
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
                    .map(|point| to_screen_with_note_length(*point, graph_rect, note_length_norm))
                    .collect();

                for line in screen_points.windows(2) {
                    painter.line_segment(
                        [line[0], line[1]],
                        Stroke::new(
                            2.0,
                            Color32::from_rgb(255, 255, 0),
                        ),
                    );
                }

                for (i, point) in screen_points.iter().enumerate() {
                    let color = if i == 0 || i + 1 == screen_points.len() {
                        APP_THEME.endpoint_point()
                    } else if Some(i) == state.selected_point {
                        APP_THEME.selected_point()
                    } else {
                        APP_THEME.control_point()
                    };
                    painter.circle_filled(*point, 6.0, color);
                    painter.circle_stroke(*point, 7.0, Stroke::new(1.0, APP_THEME.point_outline()));

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
                ui.label("Click/drag points to edit. Double-click graph to add point.");
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
