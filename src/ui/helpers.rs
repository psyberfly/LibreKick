use nih_plug::prelude::{FloatParam, ParamSetter};
use nih_plug_egui::egui::{self, Pos2, Rect};

use crate::{config, shared};

use super::{state::CurveKind, AMP_DB_FLOOR, MIN_POINT_GAP_X};

pub(super) fn axis_y_label(kind: CurveKind, normalized: f32, base_pitch_hz: f32) -> String {
    match kind {
        CurveKind::Amplitude => {
            let db = AMP_DB_FLOOR + normalized.clamp(0.0, 1.0) * (0.0 - AMP_DB_FLOOR);
            format!("{db:.0} dB")
        }
        CurveKind::Pitch => {
            let hz = base_pitch_hz * pitch_ratio_from_normalized(normalized);
            if hz >= 1000.0 {
                format!("{:.1}k", hz / 1000.0)
            } else {
                format!("{hz:.0}")
            }
        }
    }
}

/// Applies a fixed `step_by` to a slider while Shift is held, enabling
/// fine adjustment in discrete units (e.g. 1 Hz, 1 ms, 1 bar).
pub(crate) fn slider_fine_step<'a>(
    ui: &egui::Ui,
    slider: egui::Slider<'a>,
    step: f64,
) -> egui::Slider<'a> {
    if ui.input(|i| i.modifiers.shift) {
        slider.step_by(step)
    } else {
        slider
    }
}

/// Renders a slider bound to a float parameter, sending proper
/// begin/set/end parameter gestures so DAW automation stays in sync.
/// Shift+drag adjusts in steps of 0.01.
pub(crate) fn float_param_slider(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &FloatParam,
    label: &str,
) -> egui::Response {
    let mut value = param.value();
    let mut slider = egui::Slider::new(&mut value, 0.0..=1.0);
    if !label.is_empty() {
        slider = slider.text(label);
    }
    let slider = slider_fine_step(ui, slider, 0.01);
    let response = ui.add(slider);
    if response.drag_started() {
        setter.begin_set_parameter(param);
    }
    if response.changed() {
        setter.set_parameter(param, value);
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    response
}

/// Polls keyboard shortcuts for the curve editor.
/// Returns (undo, redo, cut, delete).
pub(super) fn poll_editor_shortcuts(ui: &egui::Ui) -> (bool, bool, bool, bool) {
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

    (undo_shortcut, redo_shortcut, cut_shortcut, delete_shortcut)
}

pub(super) fn axis_x_label(time_ms: f32) -> String {
    if time_ms <= 0.0 {
        "0ms".to_owned()
    } else {
        format!("{time_ms:.0}ms")
    }
}

pub(super) fn effective_waveform_zoom(waveform_zoom_percent: f32, adaptive_zoom_factor: f32) -> f32 {
    let app_cfg = config::app_config();
    let user_zoom = (waveform_zoom_percent / 100.0).clamp(
        app_cfg.waveform_zoom_min_percent / 100.0,
        app_cfg.waveform_zoom_max_percent / 100.0,
    );
    (user_zoom * adaptive_zoom_factor.max(f32::EPSILON)).max(f32::EPSILON)
}

/// Maps real rendered voice samples to screen points for the instrument
/// waveform preview. `samples` must be the actual post-envelope/post-filter
/// output of the voice (see `audio::render_kick_preview` /
/// `audio::render_bass_preview`) covering `0..note_end_ms`.
pub(super) fn waveform_preview_points(
    graph_rect: Rect,
    samples: &[f32],
    sample_rate: f32,
    note_end_ms: f32,
    max_note_length_ms: f32,
    waveform_zoom_percent: f32,
    adaptive_zoom_factor: f32,
) -> Vec<Pos2> {
    let note_end_t = (note_end_ms / max_note_length_ms.max(f32::EPSILON)).clamp(0.0, 1.0);
    let zoom = effective_waveform_zoom(waveform_zoom_percent, adaptive_zoom_factor);
    let display_length_t = (note_end_t * zoom).clamp(0.0, 1.0);
    if samples.is_empty() || display_length_t <= 0.0 {
        return Vec::new();
    }

    let note_seconds = (note_end_ms * 0.001).max(f32::EPSILON);
    samples
        .iter()
        .enumerate()
        .map(|(i, &sample)| {
            let t_seconds = i as f32 / sample_rate.max(1.0);
            let note_progress_t = (t_seconds / note_seconds).clamp(0.0, 1.0);
            let x = note_progress_t * display_length_t;
            let y = (0.5 + sample * 0.46).clamp(0.0, 1.0);
            to_screen(Pos2::new(x, y), graph_rect)
        })
        .collect()
}

pub(super) fn to_screen(point: Pos2, rect: Rect) -> Pos2 {
    Pos2::new(
        rect.left() + point.x * rect.width(),
        rect.bottom() - point.y * rect.height(),
    )
}

pub(super) fn to_screen_with_note_end(point: Pos2, rect: Rect, note_end_display_t: f32) -> Pos2 {
    let note_end_display_t = note_end_display_t.clamp(0.0, 1.0);
    Pos2::new(
        rect.left() + point.x * note_end_display_t * rect.width(),
        rect.bottom() - point.y * rect.height(),
    )
}

pub(super) fn to_normalized_with_note_end(point: Pos2, rect: Rect, note_end_display_t: f32) -> Pos2 {
    let note_end_display_t = note_end_display_t.clamp(0.0, 1.0).max(f32::EPSILON);
    let x = ((point.x - rect.left()) / (rect.width() * note_end_display_t)).clamp(0.0, 1.0);
    let y = ((rect.bottom() - point.y) / rect.height()).clamp(0.0, 1.0);
    Pos2::new(x, y)
}

pub(super) fn normalize_segment_bends(points: &[Pos2], bends: &mut Vec<f32>) {
    let target_len = points.len().saturating_sub(1);
    bends.truncate(target_len);
    if bends.len() < target_len {
        bends.resize(target_len, 0.0);
    }
    for bend in bends.iter_mut() {
        *bend = bend.clamp(-1.0, 1.0);
    }
}

fn bend_local_t(local_t: f32, bend: f32) -> f32 {
    let local_t = local_t.clamp(0.0, 1.0);
    let bend = bend.clamp(-1.0, 1.0);
    if bend.abs() <= f32::EPSILON {
        return local_t;
    }

    if bend > 0.0 {
        local_t.powf(1.0 + bend * 3.0)
    } else {
        1.0 - (1.0 - local_t).powf(1.0 + (-bend) * 3.0)
    }
}

pub(super) fn envelope_value_linear(points: &[Pos2], bends: &[f32], t: f32) -> f32 {
    if points.is_empty() {
        return 0.0;
    }

    let t = t.clamp(0.0, 1.0);
    if t <= points[0].x {
        return points[0].y.clamp(0.0, 1.0);
    }

    for (segment_idx, pair) in points.windows(2).enumerate() {
        let left = pair[0];
        let right = pair[1];
        if t <= right.x {
            let span = (right.x - left.x).max(f32::EPSILON);
            let local_t = ((t - left.x) / span).clamp(0.0, 1.0);
            let bent_t = bend_local_t(local_t, bends.get(segment_idx).copied().unwrap_or(0.0));
            return egui::lerp(left.y..=right.y, bent_t).clamp(0.0, 1.0);
        }
    }

    points.last().map_or(0.0, |p| p.y).clamp(0.0, 1.0)
}

pub(super) fn amplitude_floor_linear() -> f32 {
    10.0_f32.powf(AMP_DB_FLOOR / 20.0)
}

pub(super) fn curve_lut(points: &[Pos2], bends: &[f32]) -> [f32; shared::CURVE_LUT_SIZE] {
    let mut lut = [0.0; shared::CURVE_LUT_SIZE];

    for (i, value) in lut.iter_mut().enumerate() {
        let t = i as f32 / (shared::CURVE_LUT_SIZE as f32 - 1.0);
        *value = envelope_value_linear(points, bends, t);
    }

    lut
}

pub(super) fn amplitude_db(value: f32) -> f32 {
    (20.0 * value.max(amplitude_floor_linear()).log10()).clamp(AMP_DB_FLOOR, 0.0)
}

pub(super) fn pitch_hz_from_normalized(value: f32) -> f32 {
    let min_hz = 20.0_f32;
    let max_hz = 20_000.0_f32;
    min_hz * (max_hz / min_hz).powf(value.clamp(0.0, 1.0))
}

/// Pitch envelope value as a multiplier above the base pitch: 1x at the
/// bottom of the curve, 1000x at the top.
pub(super) fn pitch_ratio_from_normalized(value: f32) -> f32 {
    pitch_hz_from_normalized(value) / 20.0
}

pub(crate) fn note_name_from_hz(hz: f32, tuning_a4_hz: f32) -> String {
    const NOTE_NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    let midi_note = (69.0 + 12.0 * (hz / tuning_a4_hz.max(1.0)).log2()).round() as i32;
    let octave = midi_note / 12 - 1;
    let note_idx = midi_note.rem_euclid(12) as usize;

    format!("{}{}", NOTE_NAMES[note_idx], octave)
}

pub(super) fn point_value_label(
    kind: CurveKind,
    point: Pos2,
    tuning_a4_hz: f32,
    base_pitch_hz: f32,
) -> String {
    match kind {
        CurveKind::Amplitude => format!("{:.1} dB", amplitude_db(point.y)),
        CurveKind::Pitch => {
            let hz = base_pitch_hz
                * pitch_ratio_from_normalized(point.y)
                * (tuning_a4_hz / config::app_config().default_tuning_a4_hz.max(f32::EPSILON));
            let note = note_name_from_hz(hz, tuning_a4_hz);
            format!("{} {:.1}Hz", note, hz)
        }
    }
}

pub(super) fn constrain_curve_points(points: &mut [Pos2]) {
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
