use nih_plug_egui::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use nih_plug::prelude::ParamSetter;

use crate::{config, shared, LibreKickParams};
use crate::ui::components::{
    oscillator_panel, panel, waveform_preview as shared_waveform_preview,
};
use crate::ui::helpers::{
    axis_x_label, axis_y_label, constrain_curve_points, curve_lut, effective_waveform_zoom,
    envelope_value_linear, normalize_segment_bends, point_value_label,
    to_normalized_with_note_end, to_screen_with_note_end, waveform_preview_points,
};
use crate::ui::state::{
    BezierUiState, CurveKind, EditorSnapshot, NOTE_LENGTH_MAX_SLIDER_MAX_MS,
    NOTE_LENGTH_MAX_SLIDER_MIN_MS,
};
use crate::ui::theme::{self as ui_theme, apply_ui_text_scale, themed_font, APP_THEME};

const AXIS_SUBDIVISIONS: usize = 10;
const SHIFT_LOCK_X_FREEZE_AFTER_VERTICAL_RELEASE_SECONDS: f64 = 0.250;
const SHIFT_LOCK_X_REENGAGE_HORIZONTAL_PIXELS: f32 = 4.0;
const EDGE_BEND_HIT_RADIUS_PIXELS: f32 = 14.0;

pub(crate) fn render(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    add_contents(ui);
}

pub(crate) fn render_controls(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    app_cfg: &config::AppConfig,
    shared_for_ui: &shared::SharedStateHandle,
    params: &LibreKickParams,
    setter: &ParamSetter,
    history_action_applied: &mut bool,
) {
    ui.add_space(8.0 * ui_scale);
    ui.heading("Kick");
    ui.label(
        egui::RichText::new("Kick drum voice page")
            .italics()
            .small(),
    );
    ui.separator();

    ui.add_space(8.0 * ui_scale);
    panel::render(ui, "Oscillator", ui_scale, 150.0 * ui_scale, |ui| {
        oscillator_panel::render(
            ui,
            ui_scale,
            oscillator_panel::OscillatorPanelModel {
                waveform: &mut state.kick_oscillator_waveform,
                retrigger: &mut state.kick_retrigger,
                legato_voice_steal: &mut state.kick_legato_voice_steal,
                pitch_hz: Some(&mut state.kick_pitch_hz),
                note_length_ms: Some(&mut state.note_length_ms),
                level: Some((&params.kick_level, setter)),
            },
        );
    });
    ui.add_space(8.0 * ui_scale);

    shared::set_kick_oscillator_waveform(shared_for_ui, state.kick_oscillator_waveform);
    shared::set_kick_retrigger(shared_for_ui, state.kick_retrigger);
    shared::set_kick_legato_voice_steal(shared_for_ui, state.kick_legato_voice_steal);
    shared::set_kick_pitch_hz(shared_for_ui, state.kick_pitch_hz);

    ui.horizontal(|ui| {
        ui.label("Curve:");
        ui.selectable_value(&mut state.active_curve, CurveKind::Amplitude, "Amplitude");
        ui.selectable_value(&mut state.active_curve, CurveKind::Pitch, "Pitch");
        ui.separator();
        ui.checkbox(&mut state.keytrack_enabled, "Keytrack");
        ui.separator();
        if ui.button("Trigger").clicked() {
            shared::request_trigger(shared_for_ui);
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
            state.note_length_max_ms = state
                .note_length_max_ms
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
                *history_action_applied |= state.redo();
            }
            if undo_clicked {
                *history_action_applied |= state.undo();
            }
        });
        ui.add_space(8.0);
    });
    ui.add_space(8.0);
}

/// Renders the kick envelope curve editor graph (axes, note-length handle,
/// point drag/select, shift-lock, segment bends, waveform preview) and commits
/// undo history for edits made this frame.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_editor(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    app_cfg: &config::AppConfig,
    shared_for_ui: &shared::SharedStateHandle,
    snapshot_before: &EditorSnapshot,
    history_action_applied: &mut bool,
    cut_shortcut: bool,
    delete_shortcut: bool,
) {
    let mut point_dragging_this_frame = false;
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
        if graph_response.hovered() && !ui.ctx().wants_keyboard_input() {
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
                axis_y_label(state.active_curve, f, state.kick_pitch_hz),
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
        let mut edge_bend_drag_segment = state.edge_bend_drag_segment;
        let mut edge_bend_drag_start_pointer_y = state.edge_bend_drag_start_pointer_y;
        let mut edge_bend_drag_start_value = state.edge_bend_drag_start_value;
        let mut bend_hover_segment: Option<usize> = None;
        let mut bend_hover_point: Option<Pos2> = None;
        let mut bend_hover_value: Option<f32> = None;
        let mut bend_hover_polyline: Vec<Pos2> = Vec::new();
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
            let curve = state.active_curve_mut();
            let points = &mut curve.points;
            let bends = &mut curve.bends;
            normalize_segment_bends(points, bends);
            if edge_bend_drag_segment.is_some_and(|segment| segment >= bends.len()) {
                edge_bend_drag_segment = None;
                edge_bend_drag_start_pointer_y = None;
                edge_bend_drag_start_value = 0.0;
            }
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
                        if i == 0 || i + 1 == points.len() {
                            new_point.x = points[i].x;
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

            let bend_modifier_down = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
            let pointer_primary_down = ui.input(|i| i.pointer.primary_down());
            let pointer_pos = ui
                .input(|i| i.pointer.interact_pos())
                .filter(|pos| graph_rect.contains(*pos));

            if !pointer_primary_down {
                edge_bend_drag_segment = None;
                edge_bend_drag_start_pointer_y = None;
            }

            if bend_modifier_down {
                if let Some(pointer_pos) = pointer_pos {
                    let mut best_segment: Option<(usize, f32, Pos2, Vec<Pos2>)> = None;
                    for seg_idx in 0..points.len().saturating_sub(1) {
                        let left_norm = points[seg_idx];
                        let right_norm = points[seg_idx + 1];
                        let left_screen =
                            to_screen_with_note_end(left_norm, graph_rect, note_end_display_t);
                        let right_screen =
                            to_screen_with_note_end(right_norm, graph_rect, note_end_display_t);
                        let sample_count =
                            ((left_screen.distance(right_screen) / 10.0).ceil() as usize).clamp(8, 48);

                        let mut curve_polyline = Vec::with_capacity(sample_count + 1);
                        let mut prev_point = to_screen_with_note_end(
                            Pos2::new(
                                left_norm.x,
                                envelope_value_linear(points, bends, left_norm.x),
                            ),
                            graph_rect,
                            note_end_display_t,
                        );
                        curve_polyline.push(prev_point);
                        let mut closest_point = prev_point;
                        let mut distance = f32::INFINITY;

                        for step in 1..=sample_count {
                            let local_t = step as f32 / sample_count as f32;
                            let sample_t = egui::lerp(left_norm.x..=right_norm.x, local_t);
                            let sample_point = to_screen_with_note_end(
                                Pos2::new(
                                    sample_t,
                                    envelope_value_linear(points, bends, sample_t),
                                ),
                                graph_rect,
                                note_end_display_t,
                            );
                            curve_polyline.push(sample_point);

                            let ab = sample_point - prev_point;
                            let ap = pointer_pos - prev_point;
                            let denom = ab.dot(ab).max(f32::EPSILON);
                            let proj_t = (ap.dot(ab) / denom).clamp(0.0, 1.0);
                            let projected = prev_point + ab * proj_t;
                            let seg_distance = projected.distance(pointer_pos);
                            if seg_distance < distance {
                                distance = seg_distance;
                                closest_point = projected;
                            }

                            prev_point = sample_point;
                        }

                        if distance <= EDGE_BEND_HIT_RADIUS_PIXELS {
                            if let Some((_, best_distance, _, _)) = best_segment {
                                if distance < best_distance {
                                    best_segment =
                                        Some((seg_idx, distance, closest_point, curve_polyline));
                                }
                            } else {
                                best_segment =
                                    Some((seg_idx, distance, closest_point, curve_polyline));
                            }
                        }
                    }

                    if let Some((seg_idx, _distance, closest, hover_polyline)) = best_segment {
                        bend_hover_segment = Some(seg_idx);
                        bend_hover_point = Some(closest);
                        bend_hover_value = bends.get(seg_idx).copied();
                        bend_hover_polyline = hover_polyline;
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);

                        if pointer_primary_down
                            && !shift_down
                            && !point_dragging_this_frame
                            && edge_bend_drag_segment.is_none()
                        {
                            edge_bend_drag_segment = Some(seg_idx);
                            edge_bend_drag_start_pointer_y = Some(pointer_pos.y);
                            edge_bend_drag_start_value = bends.get(seg_idx).copied().unwrap_or(0.0);
                        }
                    }
                }
            }

            if bend_modifier_down && pointer_primary_down && !shift_down {
                if let (Some(seg_idx), Some(pointer_pos)) = (edge_bend_drag_segment, pointer_pos) {
                    if seg_idx < bends.len() {
                        let start_y = edge_bend_drag_start_pointer_y.unwrap_or(pointer_pos.y);
                        let delta = (start_y - pointer_pos.y)
                            / (graph_rect.height() * 0.45).max(f32::EPSILON);
                        let bend = (edge_bend_drag_start_value + delta).clamp(-1.0, 1.0);
                        bends[seg_idx] = bend;
                        bend_hover_segment = Some(seg_idx);
                        bend_hover_point = Some(pointer_pos);
                        bend_hover_value = Some(bend);
                        let left_norm = points[seg_idx];
                        let right_norm = points[seg_idx + 1];
                        let left_screen =
                            to_screen_with_note_end(left_norm, graph_rect, note_end_display_t);
                        let right_screen =
                            to_screen_with_note_end(right_norm, graph_rect, note_end_display_t);
                        let sample_count =
                            ((left_screen.distance(right_screen) / 10.0).ceil() as usize).clamp(8, 48);
                        let mut hover_polyline = Vec::with_capacity(sample_count + 1);
                        for step in 0..=sample_count {
                            let local_t = step as f32 / sample_count as f32;
                            let sample_t = egui::lerp(left_norm.x..=right_norm.x, local_t);
                            hover_polyline.push(to_screen_with_note_end(
                                Pos2::new(
                                    sample_t,
                                    envelope_value_linear(points, bends, sample_t),
                                ),
                                graph_rect,
                                note_end_display_t,
                            ));
                        }
                        bend_hover_polyline = hover_polyline;
                        point_dragging_this_frame = true;
                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
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
                                && idx > 0
                                && idx + 1 < points.len()
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
                        if !bends.is_empty() {
                            let bend_idx = idx.saturating_sub(1).min(bends.len() - 1);
                            bends.remove(bend_idx);
                        }
                    }
                    normalize_segment_bends(points, bends);
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
                if !bends.is_empty() {
                    let bend_idx = remove_index.saturating_sub(1).min(bends.len() - 1);
                    bends.remove(bend_idx);
                }
                normalize_segment_bends(points, bends);
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
                        let bend_index = index.saturating_sub(1).min(bends.len());
                        bends.insert(bend_index, 0.0);
                        normalize_segment_bends(points, bends);
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
        state.edge_bend_drag_segment = edge_bend_drag_segment;
        state.edge_bend_drag_start_pointer_y = edge_bend_drag_start_pointer_y;
        state.edge_bend_drag_start_value = edge_bend_drag_start_value;
        let active_points = state.active_curve().points.clone();
        let active_bends = state.active_curve().bends.clone();
        let tuning_a4_hz = state.tuning_standard.a4_hz();
        shared::set_keytrack_enabled(&shared_for_ui, state.keytrack_enabled);
        state.note_length_ms = note_end_ms.clamp(0.0, max_note_length_ms);
        shared::set_note_length_ms(&shared_for_ui, state.note_length_ms);

        let amplitude_lut = curve_lut(&state.amplitude_curve.points, &state.amplitude_curve.bends);
        let pitch_lut = curve_lut(&state.pitch_curve.points, &state.pitch_curve.bends);
        shared::set_curve_lut(&shared_for_ui, shared::CurveKind::Amplitude, amplitude_lut);
        shared::set_curve_lut(&shared_for_ui, shared::CurveKind::Pitch, pitch_lut);

        let waveform_points = waveform_preview_points(
            graph_rect,
            &state.amplitude_curve.points,
            &state.amplitude_curve.bends,
            &state.pitch_curve.points,
            &state.pitch_curve.bends,
            tuning_a4_hz,
            state.kick_pitch_hz,
            state.note_length_ms,
            max_note_length_ms,
            state.waveform_zoom_percent,
            adaptive_zoom_factor,
        );

        shared_waveform_preview::draw(
            &painter,
            graph_rect,
            &waveform_points,
            APP_THEME.waveform_midline(),
            APP_THEME.waveform_trace(),
        );

        let screen_points: Vec<Pos2> = active_points
            .iter()
            .map(|point| to_screen_with_note_end(*point, graph_rect, note_end_display_t))
            .collect();

        let curve_draw_points: Vec<Pos2> = (0..=160)
            .map(|step| {
                let t = step as f32 / 160.0;
                let y = envelope_value_linear(&active_points, &active_bends, t);
                to_screen_with_note_end(Pos2::new(t, y), graph_rect, note_end_display_t)
            })
            .collect();

        for line in curve_draw_points.windows(2) {
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

        if bend_hover_segment.is_some() && bend_hover_polyline.len() > 1 {
            for line in bend_hover_polyline.windows(2) {
                painter.line_segment(
                    [line[0], line[1]],
                    Stroke::new(4.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60)),
                );
            }
        }

        if let (Some(point), Some(value)) = (bend_hover_point, bend_hover_value) {
            let label = format!("{:+.0}%", value * 100.0);
            let bubble_width = (label.len() as f32 * 7.0 * ui_scale + 14.0 * ui_scale)
                .max(52.0 * ui_scale);
            let bubble_height = 20.0 * ui_scale;
            let bubble_rect = Rect::from_min_size(
                point + Vec2::new(10.0 * ui_scale, -bubble_height * 0.5),
                Vec2::new(bubble_width, bubble_height),
            );
            painter.rect_filled(bubble_rect, bubble_height * 0.5, APP_THEME.bubble_bg());
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

        for (i, point) in screen_points.iter().enumerate() {
            let color = if i == 0 {
                APP_THEME.start_endpoint_point()
            } else if i + 1 == screen_points.len() {
                APP_THEME.end_endpoint_point()
            } else if state.selected_points.contains(&i) {
                APP_THEME.selected_point()
            } else {
                APP_THEME.control_point()
            };
            painter.circle_filled(*point, 6.0, color);
            painter.circle_stroke(*point, 7.0, Stroke::new(1.0, APP_THEME.point_outline()));
            painter.circle_stroke(*point, 10.5, Stroke::new(1.5, APP_THEME.node_ring()));

            if shift_down && shift_snap_candidate == Some(i) {
                painter.circle_stroke(
                    *point,
                    14.0,
                    Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 72, 72, 180)),
                );
            }

            if shift_down && state.shift_locked_point == Some(i) {
                painter.circle_stroke(
                    *point,
                    15.0,
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
                let label = point_value_label(active_kind, value_point, tuning_a4_hz, state.kick_pitch_hz);
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

    if !*history_action_applied && state.point_drag_snapshot.is_none() {
        state.commit_history_if_changed(snapshot_before);
    }
}
