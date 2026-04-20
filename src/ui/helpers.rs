use std::f32::consts::TAU;

use nih_plug_egui::egui::{self, Pos2, Rect};

use crate::{config, shared};

use super::{
    CurveKind, AMP_DB_FLOOR, MIN_POINT_GAP_X, WAVEFORM_PREVIEW_DURATION_SECONDS,
    WAVEFORM_PREVIEW_MAX_CYCLES_PER_PIXEL,
};

pub(super) fn axis_y_label(kind: CurveKind, normalized: f32) -> String {
    match kind {
        CurveKind::Amplitude => {
            let db = AMP_DB_FLOOR + normalized.clamp(0.0, 1.0) * (0.0 - AMP_DB_FLOOR);
            format!("{db:.0} dB")
        }
        CurveKind::Pitch => {
            let hz = pitch_hz_from_normalized(normalized);
            if hz >= 1000.0 {
                format!("{:.1}k", hz / 1000.0)
            } else {
                format!("{hz:.0}")
            }
        }
    }
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

pub(super) fn waveform_preview_points(
    graph_rect: Rect,
    amplitude_points: &[Pos2],
    pitch_points: &[Pos2],
    tuning_a4_hz: f32,
    note_end_ms: f32,
    max_note_length_ms: f32,
    waveform_zoom_percent: f32,
    adaptive_zoom_factor: f32,
) -> Vec<Pos2> {
    let app_cfg = config::app_config();
    let pixel_width = graph_rect.width().max(1.0) as usize;
    let note_end_t = (note_end_ms / max_note_length_ms.max(f32::EPSILON)).clamp(0.0, 1.0);
    let zoom = effective_waveform_zoom(waveform_zoom_percent, adaptive_zoom_factor);
    let display_length_t = (note_end_t * zoom).clamp(0.0, 1.0);
    let active_pixel_width = ((pixel_width as f32 * display_length_t).round() as usize).min(pixel_width);
    if active_pixel_width == 0 {
        return Vec::new();
    }

    let source_seconds = (WAVEFORM_PREVIEW_DURATION_SECONDS * note_end_t.max(f32::EPSILON)).max(0.001);
    let max_display_hz =
        ((active_pixel_width as f32 / source_seconds) * WAVEFORM_PREVIEW_MAX_CYCLES_PER_PIXEL).max(5.0);
    let tuning_scale = tuning_a4_hz / app_cfg.default_tuning_a4_hz.max(f32::EPSILON);
    let mut phase = 0.0_f32;

    let mut previous_time_t = 0.0_f32;
    (0..active_pixel_width)
        .map(|col| {
            let x = (col as f32 + 0.5) / pixel_width as f32;
            let x_t = x.min(display_length_t);
            let note_progress_t = (x_t / display_length_t.max(f32::EPSILON)).clamp(0.0, 1.0);
            let time_t = note_progress_t * note_end_t;

            let amp = envelope_value_amplitude_db(amplitude_points, note_progress_t);
            let pitch = envelope_value_linear(pitch_points, note_progress_t);
            let hz = (pitch_hz_from_normalized(pitch) * tuning_scale)
                .clamp(20.0, 22050.0)
                .min(max_display_hz);

            let dt = ((time_t - previous_time_t).max(0.0)) * WAVEFORM_PREVIEW_DURATION_SECONDS;
            phase = (phase + TAU * hz * dt).rem_euclid(TAU);
            previous_time_t = time_t;

            let sample = phase.sin() * amp;
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

pub(super) fn envelope_value_linear(points: &[Pos2], t: f32) -> f32 {
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

pub(super) fn amplitude_floor_linear() -> f32 {
    10.0_f32.powf(AMP_DB_FLOOR / 20.0)
}

pub(super) fn amplitude_db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db.clamp(AMP_DB_FLOOR, 0.0) / 20.0)
}

pub(super) fn envelope_value_amplitude_db(points: &[Pos2], t: f32) -> f32 {
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

pub(super) fn curve_lut(points: &[Pos2]) -> [f32; shared::CURVE_LUT_SIZE] {
    let mut lut = [0.0; shared::CURVE_LUT_SIZE];

    for (i, value) in lut.iter_mut().enumerate() {
        let t = i as f32 / (shared::CURVE_LUT_SIZE as f32 - 1.0);
        *value = envelope_value_linear(points, t);
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

pub(super) fn note_name_from_hz(hz: f32, tuning_a4_hz: f32) -> String {
    const NOTE_NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    let midi_note = (69.0 + 12.0 * (hz / tuning_a4_hz.max(1.0)).log2()).round() as i32;
    let octave = midi_note / 12 - 1;
    let note_idx = midi_note.rem_euclid(12) as usize;

    format!("{}{}", NOTE_NAMES[note_idx], octave)
}

pub(super) fn point_value_label(kind: CurveKind, point: Pos2, tuning_a4_hz: f32) -> String {
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
