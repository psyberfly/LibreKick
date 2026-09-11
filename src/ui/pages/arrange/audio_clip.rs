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

        // Zoom controls
        ui.add(crate::ui::helpers::slider_fine_step(
            ui,
            egui::Slider::new(&mut state.clip_zoom, 1.0..=4.0)
                .text("Zoom")
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                .custom_parser(|s| {
                    s.trim_end_matches('%').parse::<f64>().ok().map(|v| v / 100.0)
                }),
            0.25,
        ));

        ui.add_space(4.0 * ui_scale);

        let clip_height = 400.0 * ui_scale;
        let available_width = ui.available_width();
        let axis_height = 18.0 * ui_scale;

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available_width, clip_height),
            Sense::drag(),
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

        // Waveform area excludes the bottom axis strip
        let wave_rect = egui::Rect::from_min_max(
            rect.min,
            Pos2::new(rect.right(), rect.bottom() - axis_height),
        );

        // Visible window in bars; drag scrolls horizontally
        let visible_bars = (state.num_bars / state.clip_zoom)
            .min(state.num_bars)
            .max(0.0625);
        let max_scroll = (state.num_bars - visible_bars).max(0.0);
        if response.dragged() {
            let bar_width = wave_rect.width() / visible_bars;
            state.clip_scroll_offset = (state.clip_scroll_offset
                - response.drag_delta().x / bar_width)
                .clamp(0.0, max_scroll);
        }
        let scroll_offset = state.clip_scroll_offset.clamp(0.0, max_scroll);

        draw_grid(
            &painter,
            wave_rect,
            state.num_bars,
            visible_bars,
            scroll_offset,
            state.note_size.bars(),
        );

        update_preview(state, shared_snapshot, params, tempo, bar_ms, total_ms);

        // Draw only the visible slice of the rendered clip
        draw_waveform(
            &painter,
            wave_rect,
            sample_window(
                &state.clip_preview_bass,
                scroll_offset,
                visible_bars,
                state.num_bars,
            ),
            colors::bass_note(),
        );
        draw_waveform(
            &painter,
            wave_rect,
            sample_window(
                &state.clip_preview_kick,
                scroll_offset,
                visible_bars,
                state.num_bars,
            ),
            colors::kick_note(),
        );

        draw_note_markers(
            &painter,
            rect,
            wave_rect,
            ui_scale,
            state,
            bar_ms,
            visible_bars,
            scroll_offset,
        );

        draw_time_axis(
            &painter,
            rect,
            wave_rect,
            ui_scale,
            bar_ms,
            state.num_bars,
            visible_bars,
            scroll_offset,
        );

        draw_scroll_indicator(
            &painter,
            rect,
            state.num_bars,
            visible_bars,
            scroll_offset,
        );
    });
}

/// Returns the slice of `samples` covering the visible bar window.
fn sample_window<'a>(
    samples: &'a [f32],
    scroll_offset: f32,
    visible_bars: f32,
    num_bars: f32,
) -> &'a [f32] {
    if samples.is_empty() || num_bars <= 0.0 {
        return &[];
    }
    let start = ((scroll_offset / num_bars) * samples.len() as f32) as usize;
    let end = (((scroll_offset + visible_bars) / num_bars) * samples.len() as f32).ceil() as usize;
    let start = start.min(samples.len());
    &samples[start..end.min(samples.len()).max(start)]
}

/// Draws bar boundary and beat subdivision lines across the visible window
/// of the waveform area. For narrow windows (<= 1 bar), note-size slot lines
/// are also drawn.
fn draw_grid(
    painter: &egui::Painter,
    wave_rect: egui::Rect,
    num_bars: f32,
    visible_bars: f32,
    scroll_offset: f32,
    note_len_bars: f32,
) {
    if num_bars <= 0.0 || visible_bars <= 0.0 {
        return;
    }
    let bars_to_x = |bars: f32| {
        wave_rect.left() + (bars - scroll_offset) / visible_bars * wave_rect.width()
    };

    let first_beat = (scroll_offset * 4.0).floor().max(0.0) as i32;
    let last_beat = ((scroll_offset + visible_bars) * 4.0).ceil() as i32;
    for beat in first_beat..=last_beat {
        let beat_bars = beat as f32 * 0.25;
        if beat_bars > num_bars {
            break;
        }
        let x = bars_to_x(beat_bars);
        if x < wave_rect.left() - 1.0 || x > wave_rect.right() + 1.0 {
            continue;
        }
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

    // Note-size slot lines for narrow windows (<= 1 bar) when finer than a beat
    if visible_bars <= 1.0 && note_len_bars < 0.25 {
        let slot_color = Color32::from_rgb(38, 43, 50);
        let first_slot = (scroll_offset / note_len_bars).floor() as i32;
        let last_slot = ((scroll_offset + visible_bars) / note_len_bars).ceil() as i32;
        for slot in first_slot..=last_slot {
            let slot_bars = slot as f32 * note_len_bars;
            if slot_bars <= 0.0 || slot_bars >= num_bars {
                continue;
            }
            // Skip positions already covered by beat/bar lines
            let on_beat = (slot_bars / 0.25).fract().abs() < 1e-4;
            if on_beat {
                continue;
            }
            let x = bars_to_x(slot_bars);
            if x >= wave_rect.left() && x <= wave_rect.right() {
                painter.line_segment(
                    [Pos2::new(x, wave_rect.top()), Pos2::new(x, wave_rect.bottom())],
                    Stroke::new(0.5, slot_color),
                );
            }
        }
    }
}

/// Draws the horizontal scroll indicator when the clip overflows the view.
fn draw_scroll_indicator(
    painter: &egui::Painter,
    rect: egui::Rect,
    num_bars: f32,
    visible_bars: f32,
    scroll_offset: f32,
) {
    if num_bars > visible_bars {
        let scroll_ratio = scroll_offset / (num_bars - visible_bars).max(1.0);
        let indicator_width = rect.width() * (visible_bars / num_bars);
        let indicator_x = rect.left() + scroll_ratio * (rect.width() - indicator_width);
        painter.rect_filled(
            egui::Rect::from_min_size(
                Pos2::new(indicator_x, rect.bottom() - 2.0),
                Vec2::new(indicator_width, 2.0),
            ),
            1.0,
            crate::ui::theme::accent_color(),
        );
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
    let bass_levels = [params.bass1_level.value(), params.bass2_level.value()];

    let preview_key = preview_hash(state, shared_snapshot, tempo, kick_level, bass_levels);

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
            slot: n.slot,
            start_seconds: (n.bar_pos as f64 * bar_seconds) as f32,
        })
        .collect();
    let (kick, bass) = audio::render_arrangement_preview(
        &specs,
        total_seconds,
        CLIP_PREVIEW_RATE,
        shared_snapshot,
        kick_level,
        bass_levels,
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
    bass_levels: [f32; 2],
) -> u64 {
    let mut hasher = DefaultHasher::new();
    for note in &state.midi_notes {
        note.row.hash(&mut hasher);
        note.bar_pos.to_bits().hash(&mut hasher);
        note.slot.hash(&mut hasher);
    }
    state.num_bars.to_bits().hash(&mut hasher);
    (tempo as u64).hash(&mut hasher);
    kick_level.to_bits().hash(&mut hasher);
    for level in bass_levels {
        level.to_bits().hash(&mut hasher);
    }
    shared_snapshot.keytrack_enabled.hash(&mut hasher);
    shared_snapshot.note_length_ms.to_bits().hash(&mut hasher);
    shared_snapshot.kick_pitch_hz.to_bits().hash(&mut hasher);
    shared_snapshot.kick_phase_deg.to_bits().hash(&mut hasher);
    (shared_snapshot.kick_oscillator_waveform as u8).hash(&mut hasher);
    shared_snapshot.kick_retrigger.hash(&mut hasher);
    shared_snapshot.kick_legato_voice_steal.hash(&mut hasher);
    for bass in shared_snapshot.bass.iter() {
        bass.note_length_ms.to_bits().hash(&mut hasher);
        bass.keytrack_enabled.hash(&mut hasher);
        bass.cutoff_hz.to_bits().hash(&mut hasher);
        bass.pitch_hz.to_bits().hash(&mut hasher);
        bass.phase_deg.to_bits().hash(&mut hasher);
        (bass.filter_mode as u8).hash(&mut hasher);
        (bass.filter_slope as u8).hash(&mut hasher);
        bass.filter_drive.to_bits().hash(&mut hasher);
        (bass.oscillator_waveform as u8).hash(&mut hasher);
        bass.retrigger.hash(&mut hasher);
        bass.legato_voice_steal.hash(&mut hasher);
        for v in bass.amp_lut.iter().chain(bass.filter_lut.iter()) {
            v.to_bits().hash(&mut hasher);
        }
    }
    for v in shared_snapshot
        .amp_lut
        .iter()
        .chain(shared_snapshot.pitch_lut.iter())
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
    visible_bars: f32,
    scroll_offset: f32,
) {
    if state.num_bars <= 0.0 || visible_bars <= 0.0 {
        return;
    }
    for note in &state.midi_notes {
        let t = (note.bar_pos - scroll_offset) / visible_bars;
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

/// Draws the bottom time axis with ms labels on each visible beat marker
/// line. Labels show absolute clip time; edge labels are clamped so they
/// don't overflow.
fn draw_time_axis(
    painter: &egui::Painter,
    rect: egui::Rect,
    wave_rect: egui::Rect,
    ui_scale: f32,
    bar_ms: f64,
    num_bars: f32,
    visible_bars: f32,
    scroll_offset: f32,
) {
    if num_bars <= 0.0 || visible_bars <= 0.0 {
        return;
    }
    let first_beat = (scroll_offset * 4.0).floor().max(0.0) as i32;
    let last_beat = ((scroll_offset + visible_bars) * 4.0)
        .ceil()
        .min(num_bars * 4.0) as i32;
    for beat in first_beat..=last_beat {
        let beat_bars = beat as f32 * 0.25;
        let f = (beat_bars - scroll_offset) / visible_bars;
        let x = egui::lerp(rect.left()..=rect.right(), f);
        let time_ms = beat_bars as f64 * bar_ms;

        // Tick mark
        painter.line_segment(
            [
                Pos2::new(x, wave_rect.bottom()),
                Pos2::new(x, rect.bottom() - 4.0 * ui_scale),
            ],
            Stroke::new(1.0, APP_THEME.grid_line()),
        );

        // ms label - clamp edge labels so they don't overflow
        let (label_pos, label_align) = if f <= 0.0 {
            (
                Pos2::new(rect.left() + 2.0 * ui_scale, rect.bottom() - 2.0 * ui_scale),
                Align2::LEFT_BOTTOM,
            )
        } else if f >= 1.0 {
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
