use std::sync::Arc;

use nih_plug::prelude::Editor;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2},
    resizable_window::ResizableWindow,
    EguiState,
};

use crate::shared;

const MIN_POINT_GAP_X: f32 = 0.01;
const WAVEFORM_PREVIEW_DURATION_SECONDS: f32 = 1.0;
const WAVEFORM_PREVIEW_OVERSAMPLE: usize = 8;
const NOTE_LENGTH_MAX_MS: f32 = 1000.0;
const WAVEFORM_ZOOM_MIN_PERCENT: f32 = 1.0;
const WAVEFORM_ZOOM_MAX_PERCENT: f32 = 200.0;
const WAVEFORM_ZOOM_STEP_PERCENT: f32 = 5.0;
const HISTORY_STACK_CAP: usize = 200;
const RESIZE_CORNER_VISUAL_SIZE: f32 = 20.0;
const RESIZE_CORNER_HIT_RADIUS: f32 = 30.0;
const RESIZE_SIDE_HIT_RADIUS: f32 = 16.0;
const AXIS_SUBDIVISIONS: usize = 10;
const AMP_DB_FLOOR: f32 = -30.0;
const BASE_EDITOR_WIDTH: f32 = 760.0;
const BASE_EDITOR_HEIGHT: f32 = 420.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CurveKind {
    Amplitude,
    Pitch,
}

fn ui_scale_from_size(size: Vec2) -> f32 {
    let scale_x = size.x / BASE_EDITOR_WIDTH;
    let scale_y = size.y / BASE_EDITOR_HEIGHT;
    ((scale_x + scale_y) * 0.5).clamp(0.8, 2.2)
}

fn apply_ui_text_scale(ui: &mut egui::Ui, scale: f32) {
    let mut style = ui.style().as_ref().clone();
    style.text_styles = [
        (egui::TextStyle::Heading, FontId::proportional(21.0 * scale)),
        (egui::TextStyle::Body, FontId::proportional(14.0 * scale)),
        (egui::TextStyle::Monospace, FontId::monospace(13.0 * scale)),
        (egui::TextStyle::Button, FontId::proportional(14.0 * scale)),
        (egui::TextStyle::Small, FontId::proportional(11.0 * scale)),
    ]
    .into();
    ui.ctx().set_style(style.clone());
    ui.set_style(style);
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

fn axis_x_label(normalized: f32) -> String {
    if normalized <= 0.0 {
        "0s".to_owned()
    } else {
        format!("{:.0}ms", normalized * 1000.0)
    }
}

fn waveform_preview_columns(
    graph_rect: Rect,
    amplitude_points: &[Pos2],
    pitch_points: &[Pos2],
    tuning_a4_hz: f32,
    note_length_ms: f32,
    waveform_zoom_percent: f32,
) -> Vec<[Pos2; 2]> {
    let pixel_width = graph_rect.width().max(1.0) as usize;
    let note_length_t = (note_length_ms / NOTE_LENGTH_MAX_MS).clamp(0.0, 1.0);
    let zoom = (waveform_zoom_percent / 100.0).clamp(
        WAVEFORM_ZOOM_MIN_PERCENT / 100.0,
        WAVEFORM_ZOOM_MAX_PERCENT / 100.0,
    );
    let source_length_t = (note_length_t / zoom).min(note_length_t);
    let display_length_t = (note_length_t * zoom).min(note_length_t);
    let active_pixel_width = ((pixel_width as f32 * display_length_t).round() as usize).min(pixel_width);
    if active_pixel_width == 0 {
        return Vec::new();
    }

    let total_steps = active_pixel_width * WAVEFORM_PREVIEW_OVERSAMPLE;
    let dt = WAVEFORM_PREVIEW_DURATION_SECONDS / total_steps as f32;
    let tuning_scale = tuning_a4_hz / 440.0;
    let mut phase = 0.0_f32;

    let mut col_min = vec![f32::MAX; active_pixel_width];
    let mut col_max = vec![f32::MIN; active_pixel_width];

    for step in 0..=total_steps {
        let t = (step as f32 / total_steps as f32) * source_length_t;
        let amp = bezier_point(amplitude_points, t).y.clamp(0.0, 1.0);
        let pitch = bezier_point(pitch_points, t).y;
        let hz = (pitch_hz_from_normalized(pitch) * tuning_scale).clamp(20.0, 22050.0);
        phase = (phase + std::f32::consts::TAU * hz * dt).rem_euclid(std::f32::consts::TAU);

        let sample = phase.sin() * amp;
        let y = (0.5 + sample * 0.46).clamp(0.0, 1.0);

        let x_t = (t * zoom).min(display_length_t);
        let col = ((x_t * pixel_width as f32) as usize).min(active_pixel_width - 1);
        col_min[col] = col_min[col].min(y);
        col_max[col] = col_max[col].max(y);
    }

    (0..active_pixel_width)
        .map(|col| {
            let y_min = if col_min[col] == f32::MAX { 0.5 } else { col_min[col] };
            let y_max = if col_max[col] == f32::MIN { 0.5 } else { col_max[col] };
            let x = (col as f32 + 0.5) / pixel_width as f32;
            [
                to_screen(Pos2::new(x, y_min), graph_rect),
                to_screen(Pos2::new(x, y_max), graph_rect),
            ]
        })
        .collect()
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
    waveform_zoom_percent: f32,
    selected_point: Option<usize>,
    undo_stack: Vec<EditorSnapshot>,
    redo_stack: Vec<EditorSnapshot>,
    point_drag_snapshot: Option<EditorSnapshot>,
}

impl Default for BezierUiState {
    fn default() -> Self {
        Self {
            amplitude_curve: Curve::default_amplitude(),
            pitch_curve: Curve::default_pitch(),
            active_curve: CurveKind::Amplitude,
            tuning_standard: TuningStandard::A440,
            note_length_ms: NOTE_LENGTH_MAX_MS,
            waveform_zoom_percent: 100.0,
            selected_point: Some(1),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            point_drag_snapshot: None,
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

fn bezier_point(points: &[Pos2], t: f32) -> Pos2 {
    if points.is_empty() {
        return Pos2::new(0.0, 0.0);
    }

    let mut work = points.to_vec();
    let n = work.len();
    for level in 1..n {
        for i in 0..(n - level) {
            let x = egui::lerp(work[i].x..=work[i + 1].x, t);
            let y = egui::lerp(work[i].y..=work[i + 1].y, t);
            work[i] = Pos2::new(x, y);
        }
    }

    work[0]
}

fn curve_lut(points: &[Pos2]) -> [f32; shared::CURVE_LUT_SIZE] {
    let mut lut = [0.0; shared::CURVE_LUT_SIZE];

    for (i, value) in lut.iter_mut().enumerate() {
        let t = i as f32 / (shared::CURVE_LUT_SIZE as f32 - 1.0);
        *value = bezier_point(points, t).y.clamp(0.0, 1.0);
    }

    lut
}

fn amplitude_db(value: f32) -> f32 {
    let min_amp = 10.0_f32.powf(AMP_DB_FLOOR / 20.0);
    (20.0 * value.max(min_amp).log10()).clamp(AMP_DB_FLOOR, 0.0)
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
            let hz = pitch_hz_from_normalized(point.y) * (tuning_a4_hz / 440.0);
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
    let resizable_state = editor_state.clone();
    let shared_for_ui = shared_state.clone();

    create_egui_editor(
        editor_state,
        BezierUiState::default(),
        |_ctx, _state| {},
        move |_ctx, _setter, state| {
            ResizableWindow::new("kick-plugin-resize")
                .min_size(Vec2::new(520.0, 320.0))
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
                let mut point_dragging_this_frame = false;
                {
                    let style = ui.style_mut();
                    style.interaction.resize_grab_radius_corner =
                        (RESIZE_CORNER_HIT_RADIUS * ui_scale).max(24.0);
                    style.interaction.resize_grab_radius_side =
                        (RESIZE_SIDE_HIT_RADIUS * ui_scale).max(12.0);
                    style.visuals.resize_corner_size =
                        (RESIZE_CORNER_VISUAL_SIZE * ui_scale).max(16.0);
                }
                ui.scope(|ui| {
                apply_ui_text_scale(ui, ui_scale);
                ui.add_space(6.0 * ui_scale);
                ui.heading("Kick Curve Editor (Prototype)");
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
                    if ui.button("-").clicked() {
                        state.waveform_zoom_percent =
                            (state.waveform_zoom_percent - WAVEFORM_ZOOM_STEP_PERCENT)
                                .clamp(WAVEFORM_ZOOM_MIN_PERCENT, WAVEFORM_ZOOM_MAX_PERCENT);
                    }
                    ui.label(format!("Zoom {:.0}%", state.waveform_zoom_percent));
                    if ui.button("+").clicked() {
                        state.waveform_zoom_percent =
                            (state.waveform_zoom_percent + WAVEFORM_ZOOM_STEP_PERCENT)
                                .clamp(WAVEFORM_ZOOM_MIN_PERCENT, WAVEFORM_ZOOM_MAX_PERCENT);
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
                                WAVEFORM_ZOOM_MIN_PERCENT,
                                WAVEFORM_ZOOM_MAX_PERCENT,
                            );
                    }
                }
                let left_axis_padding = (62.0 * ui_scale).clamp(52.0, 120.0);
                let bottom_axis_padding = (52.0 * ui_scale).clamp(40.0, 110.0);
                let top_axis_padding = (30.0 * ui_scale).clamp(22.0, 56.0);
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
                painter.rect_filled(outer_rect, 4.0, Color32::from_rgb(10, 12, 14));
                painter.rect_filled(graph_rect, 4.0, Color32::from_rgb(16, 19, 22));

                let mut note_length_ms = state.note_length_ms.clamp(0.0, NOTE_LENGTH_MAX_MS);
                let mut note_length_norm = (note_length_ms / NOTE_LENGTH_MAX_MS).clamp(0.0, 1.0);

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
                        note_length_ms = note_length_norm * NOTE_LENGTH_MAX_MS;
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
                        Color32::from_rgba_unmultiplied(0, 0, 0, 78),
                    );
                }

                painter.rect_stroke(
                    graph_rect,
                    4.0,
                    Stroke::new(1.0, Color32::from_rgb(90, 95, 102)),
                    egui::StrokeKind::Inside,
                );

                for i in 0..=AXIS_SUBDIVISIONS {
                    let f = i as f32 / AXIS_SUBDIVISIONS as f32;
                    let x = egui::lerp(graph_rect.left()..=graph_rect.right(), f);

                    painter.line_segment(
                        [Pos2::new(x, graph_rect.top()), Pos2::new(x, graph_rect.bottom())],
                        Stroke::new(1.0, Color32::from_rgb(34, 39, 45)),
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
                    Color32::from_rgb(220, 64, 64),
                    Stroke::new(1.0, Color32::from_rgb(70, 18, 18)),
                ));
                painter.text(
                    Pos2::new(note_length_x, outer_rect.bottom() - bottom_axis_padding * 0.62),
                    Align2::CENTER_BOTTOM,
                    format!("{:.0}ms", note_length_ms),
                    FontId::proportional(10.0 * ui_scale),
                    Color32::from_rgb(220, 64, 64),
                );

                for i in 0..=AXIS_SUBDIVISIONS {
                    let f = i as f32 / AXIS_SUBDIVISIONS as f32;
                    let y = egui::lerp(graph_rect.bottom()..=graph_rect.top(), f);

                    painter.line_segment(
                        [Pos2::new(graph_rect.left(), y), Pos2::new(graph_rect.right(), y)],
                        Stroke::new(1.0, Color32::from_rgb(34, 39, 45)),
                    );
                }

                painter.text(
                    Pos2::new(graph_rect.left(), outer_rect.top() + top_axis_padding * 0.35),
                    Align2::LEFT_BOTTOM,
                    match state.active_curve {
                        CurveKind::Amplitude => "Amount (dB)",
                        CurveKind::Pitch => "Pitch (Hz)",
                    },
                    FontId::proportional(12.0 * ui_scale),
                    Color32::from_rgb(185, 191, 198),
                );
                painter.text(
                    Pos2::new(graph_rect.right(), outer_rect.bottom() - bottom_axis_padding * 0.2),
                    Align2::RIGHT_TOP,
                    "Length",
                    FontId::proportional(12.0 * ui_scale),
                    Color32::from_rgb(185, 191, 198),
                );

                for i in 0..=AXIS_SUBDIVISIONS {
                    let f = i as f32 / AXIS_SUBDIVISIONS as f32;
                    let x = egui::lerp(graph_rect.left()..=graph_rect.right(), f);
                    painter.text(
                        Pos2::new(x, graph_rect.bottom() + bottom_axis_padding * 0.08),
                        Align2::CENTER_TOP,
                        axis_x_label(f),
                        FontId::proportional(10.0 * ui_scale),
                        Color32::from_rgb(165, 171, 178),
                    );
                }

                for i in 0..=AXIS_SUBDIVISIONS {
                    let f = i as f32 / AXIS_SUBDIVISIONS as f32;
                    let y = egui::lerp(graph_rect.bottom()..=graph_rect.top(), f);
                    painter.text(
                        Pos2::new(graph_rect.left() - left_axis_padding * 0.12, y),
                        Align2::RIGHT_CENTER,
                        axis_y_label(state.active_curve, f),
                        FontId::proportional(10.0 * ui_scale),
                        Color32::from_rgb(165, 171, 178),
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

                    ui.horizontal(|ui| {
                        if ui.button("Add Point").clicked() {
                            let (left, right, insert_index) = if let Some(selected) = selected_point {
                                if selected + 1 < points.len() {
                                    (points[selected], points[selected + 1], selected + 1)
                                } else {
                                    (points[selected - 1], points[selected], selected)
                                }
                            } else {
                                (
                                    points[0],
                                    *points.last().unwrap_or(&Pos2::new(1.0, 0.0)),
                                    points.len() - 1,
                                )
                            };

                            let new_point =
                                Pos2::new((left.x + right.x) * 0.5, (left.y + right.y) * 0.5);
                            points.insert(insert_index, new_point);
                            constrain_curve_points(points);
                            selected_point = Some(insert_index);
                        }

                        let can_remove = matches!(
                            selected_point,
                            Some(index) if index > 0 && index + 1 < points.len()
                        );
                        if ui
                            .add_enabled(can_remove, egui::Button::new("Remove Selected"))
                            .clicked()
                        {
                            if let Some(selected) = selected_point {
                                points.remove(selected);
                                constrain_curve_points(points);
                                selected_point = Some(
                                    selected
                                        .saturating_sub(1)
                                        .min(points.len() - 2)
                                        .max(1),
                                );
                            }
                        }
                    });
                }

                state.selected_point = selected_point;
                let active_points = state.active_curve().points.clone();
                let tuning_a4_hz = state.tuning_standard.a4_hz();
                shared::set_tuning_a4_hz(&shared_for_ui, tuning_a4_hz);
                state.note_length_ms = note_length_ms.clamp(0.0, NOTE_LENGTH_MAX_MS);
                shared::set_note_length_ms(&shared_for_ui, state.note_length_ms);

                let amplitude_lut = curve_lut(&state.amplitude_curve.points);
                let pitch_lut = curve_lut(&state.pitch_curve.points);
                shared::set_curve_lut(&shared_for_ui, shared::CurveKind::Amplitude, amplitude_lut);
                shared::set_curve_lut(&shared_for_ui, shared::CurveKind::Pitch, pitch_lut);

                let waveform_cols = waveform_preview_columns(
                    graph_rect,
                    &state.amplitude_curve.points,
                    &state.pitch_curve.points,
                    tuning_a4_hz,
                    state.note_length_ms,
                    state.waveform_zoom_percent,
                );

                if let (Some(first), Some(last)) = (waveform_cols.first(), waveform_cols.last()) {
                    let mid_y = graph_rect.center().y;
                    painter.line_segment(
                        [Pos2::new(first[0].x, mid_y), Pos2::new(last[0].x, mid_y)],
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 128, 136, 45)),
                    );
                }

                for [a, b] in &waveform_cols {
                    painter.line_segment(
                        [*a, *b],
                        Stroke::new(1.0, Color32::from_rgba_unmultiplied(180, 206, 232, 110)),
                    );
                }

                let screen_points: Vec<Pos2> = active_points
                    .iter()
                    .map(|point| to_screen_with_note_length(*point, graph_rect, note_length_norm))
                    .collect();

                for line in screen_points.windows(2) {
                    painter.line_segment([line[0], line[1]], Stroke::new(1.0, Color32::from_rgb(90, 150, 190)));
                }

                let mut previous =
                    to_screen_with_note_length(bezier_point(&active_points, 0.0), graph_rect, note_length_norm);
                for step in 1..=220 {
                    let t = step as f32 / 220.0;
                    let next = to_screen_with_note_length(
                        bezier_point(&active_points, t),
                        graph_rect,
                        note_length_norm,
                    );
                    painter.line_segment(
                        [previous, next],
                        Stroke::new(
                            2.0,
                            if active_kind == CurveKind::Amplitude {
                                Color32::from_rgb(72, 210, 170)
                            } else {
                                Color32::from_rgb(249, 122, 122)
                            },
                        ),
                    );
                    previous = next;
                }

                for (i, point) in screen_points.iter().enumerate() {
                    let color = if i == 0 || i + 1 == screen_points.len() {
                        Color32::from_rgb(220, 64, 64)
                    } else if Some(i) == state.selected_point {
                        Color32::from_rgb(255, 234, 122)
                    } else {
                        Color32::from_rgb(112, 182, 255)
                    };
                    painter.circle_filled(*point, 6.0, color);
                    painter.circle_stroke(*point, 7.0, Stroke::new(1.0, Color32::BLACK));

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
                            Color32::from_rgba_unmultiplied(24, 28, 33, 220),
                        );
                        painter.rect_stroke(
                            bubble_rect,
                            bubble_height * 0.5,
                            Stroke::new(1.0, Color32::from_rgb(90, 95, 102)),
                            egui::StrokeKind::Inside,
                        );
                        painter.text(
                            bubble_rect.center(),
                            Align2::CENTER_CENTER,
                            label,
                            FontId::proportional(11.0 * ui_scale),
                            Color32::from_rgb(224, 230, 238),
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
