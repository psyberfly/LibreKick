use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use nih_plug_egui::egui::{self, Align2, Color32, Pos2, RichText, Sense, Stroke, Vec2};

use crate::audio::{self, ArrangeNoteSpec};
use crate::shared::SharedSnapshot;
use crate::ui::helpers::axis_x_label;
use crate::ui::state::BezierUiState;
use crate::ui::theme::{themed_font, APP_THEME};
use crate::LibreKickParams;

use super::{colors, BASS_NOTES, CLIP_PREVIEW_RATE};

/// Renders the Audio Clip preview: an offline-rendered waveform of the
/// arrangement's MIDI notes through the kick/bass voices, with a time axis
/// in milliseconds computed from the current tempo and bar count.
pub(super) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    shared_snapshot: &SharedSnapshot,
    daw_tempo: Option<f64>,
    params: &LibreKickParams,
) {
    ui.group(|ui| {
        ui.label(RichText::new("Audio Clip").strong());
        ui.add_space(4.0 * ui_scale);

        let clip_height = 400.0 * ui_scale;
        let available_width = ui.available_width();
        let axis_height = 18.0 * ui_scale;

        let (rect, _response) = ui.allocate_exact_size(
            Vec2::new(available_width, clip_height),
            Sense::hover(),
        );

        let painter = ui.painter_at(rect);

        // Draw background and border
        painter.rect_filled(rect, 4.0, colors::background());
        painter.rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, colors::border()),
            egui::StrokeKind::Inside,
        );

        // Calculate total clip duration in ms from tempo and bars
        // 4/4 time: bar_ms = 4 beats * (60000 / BPM)
        let tempo = if state.use_daw_tempo {
            daw_tempo.unwrap_or(state.manual_tempo as f64)
        } else {
            state.manual_tempo as f64
        };
        let beats_per_bar = 4.0;
        let bar_ms = beats_per_bar * (60_000.0 / tempo.max(1.0));
        let total_ms = bar_ms * state.num_bars as f64;
        let total_beats = (state.num_bars * beats_per_bar as f32).round() as u32;

        // Waveform area excludes the bottom axis strip
        let wave_rect = egui::Rect::from_min_max(
            rect.min,
            Pos2::new(rect.right(), rect.bottom() - axis_height),
        );

        draw_grid(
            &painter,
            wave_rect,
            total_beats,
            state.num_bars,
            state.note_size.bars(),
        );

        update_preview(state, shared_snapshot, params, tempo, bar_ms, total_ms);

        draw_waveform(&painter, wave_rect, &state.clip_preview_bass, colors::bass_note());
        draw_waveform(&painter, wave_rect, &state.clip_preview_kick, colors::kick_note());

        draw_note_markers(&painter, rect, wave_rect, ui_scale, state, bar_ms);

        draw_time_axis(&painter, rect, wave_rect, ui_scale, total_ms, total_beats);
    });
}

/// Draws bar boundary and beat subdivision lines across the waveform area.
/// `total_beats` covers fractional bars too (0.25 bar = 1 beat).
/// For clips of one bar or less, note-size slot lines are also drawn.
fn draw_grid(
    painter: &egui::Painter,
    wave_rect: egui::Rect,
    total_beats: u32,
    num_bars: f32,
    note_len_bars: f32,
) {
    if total_beats == 0 {
        return;
    }
    for beat in 0..=total_beats {
        let t = beat as f32 / total_beats as f32;
        let x = egui::lerp(wave_rect.left()..=wave_rect.right(), t);
        let is_bar_line = beat % 4 == 0;
        painter.line_segment(
            [Pos2::new(x, wave_rect.top()), Pos2::new(x, wave_rect.bottom())],
            if is_bar_line {
                Stroke::new(1.0, colors::bar_line())
            } else {
                Stroke::new(0.5, colors::beat_line())
            },
        );
    }

    // Note-size slot lines for short clips (<= 1 bar) when finer than a beat
    if num_bars <= 1.0 && note_len_bars < 0.25 {
        let slot_color = Color32::from_rgb(38, 43, 50);
        let total_slots = (num_bars / note_len_bars).round() as i32;
        for slot in 1..total_slots {
            let slot_bars = slot as f32 * note_len_bars;
            // Skip positions already covered by beat/bar lines
            let on_beat = (slot_bars / 0.25).fract().abs() < 1e-4;
            if on_beat {
                continue;
            }
            let t = slot_bars / num_bars;
            let x = egui::lerp(wave_rect.left()..=wave_rect.right(), t);
            painter.line_segment(
                [Pos2::new(x, wave_rect.top()), Pos2::new(x, wave_rect.bottom())],
                Stroke::new(0.5, slot_color),
            );
        }
    }
}

/// Re-renders the clip preview when any input (notes, tempo, bars, or
/// instrument parameters) has changed since the last render.
fn update_preview(
    state: &mut BezierUiState,
    shared_snapshot: &SharedSnapshot,
    params: &LibreKickParams,
    tempo: f64,
    bar_ms: f64,
    total_ms: f64,
) {
    let kick_level = params.kick_level.value();
    let bass_level = params.bass_level.value();

    let preview_key = preview_hash(state, shared_snapshot, tempo, kick_level, bass_level);

    if preview_key == state.clip_preview_key {
        return;
    }

    let bar_seconds = bar_ms / 1000.0;
    let total_seconds = (total_ms / 1000.0) as f32;
    let specs: Vec<ArrangeNoteSpec> = state
        .midi_notes
        .iter()
        .map(|n| ArrangeNoteSpec {
            is_kick: n.row == BASS_NOTES,
            // Row 0 = B (+11 semitones) .. row 11 = C (+0)
            semitone: (BASS_NOTES - 1 - n.row.min(BASS_NOTES - 1)) as i32,
            start_seconds: (n.bar_pos as f64 * bar_seconds) as f32,
        })
        .collect();
    let (kick, bass) = audio::render_arrangement_preview(
        &specs,
        total_seconds,
        CLIP_PREVIEW_RATE,
        shared_snapshot,
        kick_level,
        bass_level,
    );
    state.clip_preview_kick = kick;
    state.clip_preview_bass = bass;
    state.clip_preview_key = preview_key;
}

/// Computes a hash over every input that affects the rendered waveform.
fn preview_hash(
    state: &BezierUiState,
    shared_snapshot: &SharedSnapshot,
    tempo: f64,
    kick_level: f32,
    bass_level: f32,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    for note in &state.midi_notes {
        note.row.hash(&mut hasher);
        note.bar_pos.to_bits().hash(&mut hasher);
    }
    state.num_bars.to_bits().hash(&mut hasher);
    (tempo as u64).hash(&mut hasher);
    kick_level.to_bits().hash(&mut hasher);
    bass_level.to_bits().hash(&mut hasher);
    shared_snapshot.keytrack_enabled.hash(&mut hasher);
    shared_snapshot.note_length_ms.to_bits().hash(&mut hasher);
    shared_snapshot.kick_pitch_hz.to_bits().hash(&mut hasher);
    (shared_snapshot.kick_oscillator_waveform as u8).hash(&mut hasher);
    shared_snapshot.kick_retrigger.hash(&mut hasher);
    shared_snapshot.kick_legato_voice_steal.hash(&mut hasher);
    shared_snapshot.bass_note_length_ms.to_bits().hash(&mut hasher);
    shared_snapshot.bass_cutoff_hz.to_bits().hash(&mut hasher);
    shared_snapshot.bass_pitch_hz.to_bits().hash(&mut hasher);
    (shared_snapshot.bass_filter_mode as u8).hash(&mut hasher);
    (shared_snapshot.bass_oscillator_waveform as u8).hash(&mut hasher);
    shared_snapshot.bass_retrigger.hash(&mut hasher);
    shared_snapshot.bass_legato_voice_steal.hash(&mut hasher);
    for v in shared_snapshot
        .amp_lut
        .iter()
        .chain(shared_snapshot.pitch_lut.iter())
        .chain(shared_snapshot.bass_amp_lut.iter())
        .chain(shared_snapshot.bass_filter_lut.iter())
    {
        v.to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

/// Draws the waveform as per-pixel min/max columns with connected edges so
/// steep sections stay continuous instead of breaking into dashed segments.
fn draw_waveform(
    painter: &egui::Painter,
    wave_rect: egui::Rect,
    samples: &[f32],
    wave_color: Color32,
) {
    if samples.is_empty() {
        return;
    }
    let mid_y = wave_rect.center().y;
    let half_height = wave_rect.height() * 0.45;
    let width = wave_rect.width().max(1.0) as usize;

    // Per-pixel min/max envelope
    let mut tops = Vec::with_capacity(width);
    let mut bottoms = Vec::with_capacity(width);
    for px in 0..width {
        let start = px * samples.len() / width;
        let end = ((px + 1) * samples.len() / width).max(start + 1);
        let slice = &samples[start..end.min(samples.len())];
        let (mut lo, mut hi) = (0.0_f32, 0.0_f32);
        for &s in slice {
            lo = lo.min(s);
            hi = hi.max(s);
        }
        let x = wave_rect.left() + px as f32;
        tops.push(Pos2::new(x, mid_y - hi.clamp(-1.0, 1.0) * half_height));
        bottoms.push(Pos2::new(x, mid_y - lo.clamp(-1.0, 1.0) * half_height));
    }

    // Column fills
    for px in 0..width {
        painter.line_segment(
            [tops[px], bottoms[px]],
            Stroke::new(1.0, wave_color),
        );
    }
    // Connect adjacent column edges so the envelope has no gaps
    for px in 0..width.saturating_sub(1) {
        painter.line_segment(
            [tops[px], tops[px + 1]],
            Stroke::new(1.0, wave_color),
        );
        painter.line_segment(
            [bottoms[px], bottoms[px + 1]],
            Stroke::new(1.0, wave_color),
        );
    }

    // Center line
    painter.line_segment(
        [
            Pos2::new(wave_rect.left(), mid_y),
            Pos2::new(wave_rect.right(), mid_y),
        ],
        Stroke::new(0.5, colors::beat_line()),
    );
}

/// Draws a marker line and ms time label at each note's start position.
fn draw_note_markers(
    painter: &egui::Painter,
    rect: egui::Rect,
    wave_rect: egui::Rect,
    ui_scale: f32,
    state: &BezierUiState,
    bar_ms: f64,
) {
    if state.num_bars <= 0.0 {
        return;
    }
    for note in &state.midi_notes {
        let t = note.bar_pos / state.num_bars;
        if !(0.0..=1.0).contains(&t) {
            continue;
        }
        let x = egui::lerp(wave_rect.left()..=wave_rect.right(), t);
        let time_ms = note.bar_pos as f64 * bar_ms;
        let color = if note.row == BASS_NOTES {
            colors::kick_note()
        } else {
            colors::bass_note()
        };

        // Marker line below the label at the top of the waveform area
        painter.line_segment(
            [
                Pos2::new(x, wave_rect.top() + 12.0 * ui_scale),
                Pos2::new(x, wave_rect.top() + 20.0 * ui_scale),
            ],
            Stroke::new(1.0, color),
        );

        // ms label just above the marker, clamped inside the clip bounds
        let label = axis_x_label(time_ms as f32);
        let (label_pos, label_align) = if t <= 0.0 {
            (
                Pos2::new(rect.left() + 2.0 * ui_scale, wave_rect.top() + 2.0 * ui_scale),
                Align2::LEFT_TOP,
            )
        } else if t >= 1.0 {
            (
                Pos2::new(rect.right() - 2.0 * ui_scale, wave_rect.top() + 2.0 * ui_scale),
                Align2::RIGHT_TOP,
            )
        } else {
            (
                Pos2::new(x, wave_rect.top() + 2.0 * ui_scale),
                Align2::CENTER_TOP,
            )
        };
        painter.text(
            label_pos,
            label_align,
            label,
            themed_font(9.0 * ui_scale),
            color,
        );
    }
}

/// Draws the bottom time axis with ms labels on each beat marker line.
/// First and last labels are edge-clamped so they don't overflow.
fn draw_time_axis(
    painter: &egui::Painter,
    rect: egui::Rect,
    wave_rect: egui::Rect,
    ui_scale: f32,
    total_ms: f64,
    total_beats: u32,
) {
    if total_beats == 0 {
        return;
    }
    for beat in 0..=total_beats {
        let f = beat as f32 / total_beats as f32;
        let x = egui::lerp(rect.left()..=rect.right(), f);
        let time_ms = f as f64 * total_ms;

        // Tick mark
        painter.line_segment(
            [
                Pos2::new(x, wave_rect.bottom()),
                Pos2::new(x, rect.bottom() - 4.0 * ui_scale),
            ],
            Stroke::new(1.0, APP_THEME.grid_line()),
        );

        // ms label - clamp edge labels so they don't overflow
        let (label_pos, label_align) = if beat == 0 {
            (
                Pos2::new(rect.left() + 2.0 * ui_scale, rect.bottom() - 2.0 * ui_scale),
                Align2::LEFT_BOTTOM,
            )
        } else if beat == total_beats {
            (
                Pos2::new(rect.right() - 2.0 * ui_scale, rect.bottom() - 2.0 * ui_scale),
                Align2::RIGHT_BOTTOM,
            )
        } else {
            (Pos2::new(x, rect.bottom() - 2.0 * ui_scale), Align2::CENTER_BOTTOM)
        };
        painter.text(
            label_pos,
            label_align,
            axis_x_label(time_ms as f32),
            themed_font(10.0 * ui_scale),
            APP_THEME.axis_tick(),
        );
    }
}
