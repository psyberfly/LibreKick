use nih_plug_egui::egui::{
    self, Align2, Color32, Pos2, RichText, Sense, Stroke, Vec2,
};

use crate::ui::state::{ArrangeNote, BezierUiState};
use crate::ui::theme::{accent_color, APP_THEME};

use super::{colors, BASS_NOTES, TOTAL_ROWS};

/// Renders the MIDI channel: a piano-roll style grid with a bass octave
/// (12 rows) on top and a single kick lane at the bottom.
///
/// Interactions:
/// - Double-click empty slot: add a note; double-click a note: remove it
/// - Right-click a bass note: toggle between Note 1 (blue) and Note 2 (green)
/// - Shift+right-click a note: remove it
/// - Drag a note: move it (snapped to beat slots, no overlap)
/// - Drag empty space: horizontal scroll
pub(super) fn render(ui: &mut egui::Ui, ui_scale: f32, state: &mut BezierUiState) {
    ui.group(|ui| {
        ui.label(RichText::new("MIDI Channel").strong());
        ui.add_space(4.0 * ui_scale);

        // Zoom controls
        ui.add(crate::ui::helpers::slider_fine_step(
            ui,
            egui::Slider::new(&mut state.midi_channel_zoom, 0.25..=4.0)
                .text("Zoom")
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                .custom_parser(|s| {
                    s.trim_end_matches('%').parse::<f64>().ok().map(|v| v / 100.0)
                }),
            0.25,
        ));

        ui.add_space(8.0 * ui_scale);

        // MIDI channel visualization area
        let channel_height = 200.0 * ui_scale;
        let available_width = ui.available_width();
        let keyboard_width = 56.0 * ui_scale;

        let row_height = channel_height / TOTAL_ROWS as f32;

        // Note names for bass octave (top to bottom: B to C)
        const NOTE_NAMES: [&str; 12] = [
            "B", "A#", "A", "G#", "G", "F#", "F", "E", "D#", "D", "C#", "C",
        ];
        // Black keys in an octave (top-to-bottom order matches NOTE_NAMES)
        const BLACK_KEYS: [bool; 12] = [
            false, true, false, true, false, true, false, false, true, false, true, false,
        ];

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available_width, channel_height),
            Sense::click_and_drag(),
        );

        let painter = ui.painter_at(rect);

        // Draw outer background and border
        painter.rect_filled(rect, 4.0, colors::background());
        painter.rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, colors::border()),
            egui::StrokeKind::Inside,
        );

        draw_keyboard_strip(
            &painter,
            rect,
            keyboard_width,
            row_height,
            ui_scale,
            &NOTE_NAMES,
            &BLACK_KEYS,
        );

        // === Right bar grid ===
        let grid_rect = egui::Rect::from_min_max(
            Pos2::new(rect.left() + keyboard_width, rect.top()),
            rect.max,
        );
        let grid_width = grid_rect.width();

        // Calculate visible bars based on zoom
        let total_bars = state.num_bars;
        let visible_bars = (total_bars / state.midi_channel_zoom).max(1.0);
        let bar_width = grid_width / visible_bars;
        let scroll_offset = state.midi_channel_scroll_offset;
        let note_len_bars = state.note_size.bars();

        draw_row_separators(&painter, rect, grid_rect, row_height);
        draw_bar_grid(
            &painter,
            grid_rect,
            ui_scale,
            total_bars,
            visible_bars,
            bar_width,
            scroll_offset,
            note_len_bars,
        );

        // Helper: find note index at a pointer position.
        let note_at = |pos: Pos2, notes: &[ArrangeNote]| -> Option<usize> {
            if !grid_rect.contains(pos) {
                return None;
            }
            let row = ((pos.y - rect.top()) / row_height) as usize;
            if row >= TOTAL_ROWS {
                return None;
            }
            let bar_pos = scroll_offset + (pos.x - grid_rect.left()) / bar_width;
            notes.iter().position(|n| {
                n.row == row && bar_pos >= n.bar_pos && bar_pos < n.bar_pos + note_len_bars
            })
        };

        handle_interactions(
            ui,
            &response,
            state,
            grid_rect,
            rect,
            row_height,
            bar_width,
            scroll_offset,
            total_bars,
            visible_bars,
            note_len_bars,
            &note_at,
        );

        draw_notes(
            &painter,
            state,
            grid_rect,
            rect,
            row_height,
            bar_width,
            scroll_offset,
            note_len_bars,
        );

        draw_scroll_indicator(
            &painter,
            rect,
            grid_rect,
            grid_width,
            total_bars,
            visible_bars,
            scroll_offset,
        );
    });
}

/// Draws the left keyboard strip: bass octave rows + kick lane.
fn draw_keyboard_strip(
    painter: &egui::Painter,
    rect: egui::Rect,
    keyboard_width: f32,
    row_height: f32,
    ui_scale: f32,
    note_names: &[&str; 12],
    black_keys: &[bool; 12],
) {
    let keyboard_rect =
        egui::Rect::from_min_size(rect.min, Vec2::new(keyboard_width, rect.height()));
    painter.rect_filled(keyboard_rect, 0.0, Color32::from_rgb(14, 16, 19));

    // Bass octave rows (top section)
    for i in 0..BASS_NOTES {
        let row_top = rect.top() + i as f32 * row_height;
        let row_rect = egui::Rect::from_min_size(
            Pos2::new(rect.left(), row_top),
            Vec2::new(keyboard_width, row_height),
        );

        // Black keys get darker background
        if black_keys[i] {
            painter.rect_filled(row_rect, 0.0, Color32::from_rgb(8, 9, 11));
        }

        // Row separator line
        painter.line_segment(
            [
                Pos2::new(rect.left(), row_top),
                Pos2::new(rect.left() + keyboard_width, row_top),
            ],
            Stroke::new(0.5, colors::row_line()),
        );

        // Note label
        painter.text(
            row_rect.center(),
            Align2::CENTER_CENTER,
            note_names[i],
            egui::FontId::proportional(9.0 * ui_scale),
            if black_keys[i] {
                Color32::from_rgb(140, 145, 152)
            } else {
                APP_THEME.axis_tick()
            },
        );
    }

    // Kick row (bottom-most)
    let kick_top = rect.top() + BASS_NOTES as f32 * row_height;
    let kick_rect = egui::Rect::from_min_size(
        Pos2::new(rect.left(), kick_top),
        Vec2::new(keyboard_width, row_height),
    );
    painter.rect_filled(kick_rect, 0.0, Color32::from_rgb(26, 20, 20));
    painter.line_segment(
        [
            Pos2::new(rect.left(), kick_top),
            Pos2::new(rect.left() + keyboard_width, kick_top),
        ],
        Stroke::new(1.0, accent_color()),
    );
    painter.text(
        kick_rect.center(),
        Align2::CENTER_CENTER,
        "KICK",
        egui::FontId::proportional(8.0 * ui_scale),
        accent_color(),
    );

    // Keyboard / grid separator
    painter.line_segment(
        [
            Pos2::new(rect.left() + keyboard_width, rect.top()),
            Pos2::new(rect.left() + keyboard_width, rect.bottom()),
        ],
        Stroke::new(1.0, colors::border()),
    );
}

/// Draws horizontal row separators across the grid area.
fn draw_row_separators(
    painter: &egui::Painter,
    rect: egui::Rect,
    grid_rect: egui::Rect,
    row_height: f32,
) {
    for i in 0..=TOTAL_ROWS {
        let y = rect.top() + i as f32 * row_height;
        let is_kick_separator = i == BASS_NOTES;
        painter.line_segment(
            [Pos2::new(grid_rect.left(), y), Pos2::new(grid_rect.right(), y)],
            if is_kick_separator {
                Stroke::new(1.0, accent_color())
            } else {
                Stroke::new(0.5, colors::row_line())
            },
        );
    }
}

/// Draws bar lines, bar numbers, beat subdivisions, and note-size slot lines.
#[allow(clippy::too_many_arguments)]
fn draw_bar_grid(
    painter: &egui::Painter,
    grid_rect: egui::Rect,
    ui_scale: f32,
    total_bars: f32,
    visible_bars: f32,
    bar_width: f32,
    scroll_offset: f32,
    note_len_bars: f32,
) {
    let first_visible_bar = scroll_offset.floor() as i32;
    let last_visible_bar = (scroll_offset + visible_bars).ceil() as i32;

    // Bar lines + numbers
    for bar in first_visible_bar..=last_visible_bar {
        if bar < 0 || bar as f32 > total_bars {
            continue;
        }
        let x = grid_rect.left() + ((bar as f32 - scroll_offset) * bar_width);
        if x >= grid_rect.left() && x <= grid_rect.right() {
            painter.line_segment(
                [Pos2::new(x, grid_rect.top()), Pos2::new(x, grid_rect.bottom())],
                Stroke::new(1.0, colors::bar_line()),
            );

            if (bar as f32) < total_bars {
                painter.text(
                    Pos2::new(x + bar_width / 2.0, grid_rect.top() + 6.0),
                    Align2::CENTER_TOP,
                    format!("{}", bar + 1),
                    egui::FontId::proportional(10.0 * ui_scale),
                    APP_THEME.axis_tick(),
                );
            }
        }
    }

    // Beat subdivisions (4 beats per bar for 4/4 time)
    let beats_per_bar = 4;
    for bar in first_visible_bar..last_visible_bar {
        if bar < 0 || bar as f32 >= total_bars {
            continue;
        }
        for beat in 1..beats_per_bar {
            let x = grid_rect.left()
                + ((bar as f32 - scroll_offset) * bar_width)
                + (beat as f32 * bar_width / beats_per_bar as f32);
            if x >= grid_rect.left() && x <= grid_rect.right() {
                painter.line_segment(
                    [Pos2::new(x, grid_rect.top()), Pos2::new(x, grid_rect.bottom())],
                    Stroke::new(0.5, colors::beat_line()),
                );
            }
        }
    }

    // Note-size slot lines when finer than a beat (e.g. 1/16 inside a beat)
    let slots_per_beat = (0.25 / note_len_bars).round() as i32;
    if slots_per_beat > 1 {
        let slot_color = Color32::from_rgb(38, 43, 50);
        let first_slot = (scroll_offset / note_len_bars).floor() as i32;
        let last_slot = ((scroll_offset + visible_bars) / note_len_bars).ceil() as i32;
        for slot in first_slot..=last_slot {
            let slot_bars = slot as f32 * note_len_bars;
            if slot_bars <= 0.0 || slot_bars >= total_bars {
                continue;
            }
            // Skip positions already covered by bar/beat lines
            let on_beat = (slot_bars / 0.25).fract().abs() < 1e-4;
            if on_beat {
                continue;
            }
            let x = grid_rect.left() + (slot_bars - scroll_offset) * bar_width;
            if x >= grid_rect.left() && x <= grid_rect.right() {
                painter.line_segment(
                    [Pos2::new(x, grid_rect.top()), Pos2::new(x, grid_rect.bottom())],
                    Stroke::new(0.5, slot_color),
                );
            }
        }
    }
}

/// Handles note add/remove/move and empty-space scrolling.
#[allow(clippy::too_many_arguments)]
fn handle_interactions(
    ui: &egui::Ui,
    response: &egui::Response,
    state: &mut BezierUiState,
    grid_rect: egui::Rect,
    rect: egui::Rect,
    row_height: f32,
    bar_width: f32,
    scroll_offset: f32,
    total_bars: f32,
    visible_bars: f32,
    note_len_bars: f32,
    note_at: &impl Fn(Pos2, &[ArrangeNote]) -> Option<usize>,
) {
    // Right-click toggles a bass note between Note 1 and Note 2;
    // Shift+right-click removes a note.
    if response.secondary_clicked() {
        if let Some(pointer_pos) = response.interact_pointer_pos() {
            if let Some(index) = note_at(pointer_pos, &state.midi_notes) {
                let shift = ui.input(|i| i.modifiers.shift);
                if shift {
                    state.midi_notes.remove(index);
                } else if state.midi_notes[index].row < BASS_NOTES {
                    let note = &mut state.midi_notes[index];
                    note.slot = if note.slot == 0 { 1 } else { 0 };
                }
            }
        }
    }

    // Drag start: grab a note if the pointer is over one
    if response.drag_started() {
        if let Some(pointer_pos) = response.interact_pointer_pos() {
            state.dragging_note = note_at(pointer_pos, &state.midi_notes);
        }
    }

    // Dragging: move the grabbed note, or scroll if empty space
    if response.dragged() {
        if let Some(pointer_pos) = response.interact_pointer_pos() {
            if let Some(index) = state.dragging_note {
                if index < state.midi_notes.len() {
                    // Move note: position follows pointer, snapped to beat slots
                    let new_row = ((pointer_pos.y - rect.top()) / row_height) as usize;
                    let new_bar_pos = scroll_offset
                        + (pointer_pos.x - grid_rect.left()) / bar_width
                        - note_len_bars / 2.0;
                    // Snap to the nearest note-size slot
                    let snapped = (new_bar_pos / note_len_bars).round() * note_len_bars;
                    let snapped = snapped.clamp(0.0, (total_bars - note_len_bars).max(0.0));
                    let target_row = if new_row < TOTAL_ROWS {
                        new_row
                    } else {
                        state.midi_notes[index].row
                    };
                    // Don't drop onto a slot occupied by another note
                    let occupied = state.midi_notes.iter().enumerate().any(|(i, n)| {
                        i != index && n.row == target_row && n.bar_pos == snapped
                    });
                    if !occupied {
                        state.midi_notes[index].row = target_row;
                        state.midi_notes[index].bar_pos = snapped;
                    }
                }
            } else if pointer_pos.x > grid_rect.left() {
                // Empty-space drag scrolls horizontally
                let drag_delta = response.drag_delta().x;
                state.midi_channel_scroll_offset = (scroll_offset - drag_delta / bar_width)
                    .clamp(0.0, (total_bars - visible_bars).max(0.0));
            }
        }
    }

    // Release: stop dragging
    if !ui.input(|i| i.pointer.primary_down()) {
        state.dragging_note = None;
    }

    // Double-click in grid area adds a note; on an existing note removes it
    if response.double_clicked() {
        if let Some(pointer_pos) = response.interact_pointer_pos() {
            if grid_rect.contains(pointer_pos) {
                if let Some(index) = note_at(pointer_pos, &state.midi_notes) {
                    state.midi_notes.remove(index);
                } else {
                    let row = ((pointer_pos.y - rect.top()) / row_height) as usize;
                    if row < TOTAL_ROWS {
                        let bar_pos =
                            scroll_offset + (pointer_pos.x - grid_rect.left()) / bar_width;
                        // Snap to the note-size slot that was clicked
                        let snapped = (bar_pos / note_len_bars).floor() * note_len_bars;
                        if snapped >= 0.0 && snapped < total_bars {
                            state.midi_notes.push(ArrangeNote {
                                row,
                                bar_pos: snapped,
                                slot: 0,
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Draws all placed notes (kick = orange, bass = blue).
fn draw_notes(
    painter: &egui::Painter,
    state: &BezierUiState,
    grid_rect: egui::Rect,
    rect: egui::Rect,
    row_height: f32,
    bar_width: f32,
    scroll_offset: f32,
    note_len_bars: f32,
) {
    let note_width = bar_width * note_len_bars;
    for (index, note) in state.midi_notes.iter().enumerate() {
        let x = grid_rect.left() + (note.bar_pos - scroll_offset) * bar_width;
        let y = rect.top() + note.row as f32 * row_height;
        let note_rect = egui::Rect::from_min_size(
            Pos2::new(x, y + 1.0),
            Vec2::new(note_width, row_height - 2.0),
        );
        if note_rect.right() >= grid_rect.left() && note_rect.left() <= grid_rect.right() {
            let mut color = if note.row == BASS_NOTES {
                colors::kick_note()
            } else if note.slot == 1 {
                colors::bass_note_alt()
            } else {
                colors::bass_note()
            };
            // Brighten the note being dragged
            if state.dragging_note == Some(index) {
                color = color.linear_multiply(1.3);
            }
            painter.rect_filled(note_rect, 2.0, color);
        }
    }
}

/// Draws the horizontal scroll indicator bar when content overflows.
fn draw_scroll_indicator(
    painter: &egui::Painter,
    rect: egui::Rect,
    grid_rect: egui::Rect,
    grid_width: f32,
    total_bars: f32,
    visible_bars: f32,
    scroll_offset: f32,
) {
    if total_bars > visible_bars {
        let scroll_ratio = scroll_offset / (total_bars - visible_bars).max(1.0);
        let indicator_width = grid_width * (visible_bars / total_bars);
        let indicator_x = grid_rect.left() + scroll_ratio * (grid_width - indicator_width);

        painter.rect_filled(
            egui::Rect::from_min_size(
                Pos2::new(indicator_x, rect.bottom() - 4.0),
                Vec2::new(indicator_width, 4.0),
            ),
            2.0,
            accent_color(),
        );
    }
}
