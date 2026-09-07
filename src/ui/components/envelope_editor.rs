use nih_plug_egui::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::ui::{
    helpers::{constrain_curve_points, envelope_value_linear, normalize_segment_bends},
    state::Curve,
    theme::{themed_font, APP_THEME},
};

pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    id_prefix: &str,
    title: &str,
    curve: &mut Curve,
    selected_point: &mut Option<usize>,
) -> bool {
    const EDGE_BEND_HIT_RADIUS_PIXELS: f32 = 14.0;

    // Reports whether a point/bend drag gesture happened this frame so the
    // caller can stash a pre-drag snapshot for undo history.
    let mut drag_active = false;

    ui.group(|ui| {
        ui.label(title);
        let desired_height = (180.0 * ui_scale).max(140.0);
        let desired_width = ui.available_width().max(180.0 * ui_scale);
        let (outer_rect, response) =
            ui.allocate_exact_size(Vec2::new(desired_width, desired_height), Sense::click_and_drag());

        let graph_rect = outer_rect.shrink2(Vec2::new(12.0 * ui_scale, 16.0 * ui_scale));
        let painter = ui.painter_at(outer_rect);
        painter.rect_filled(outer_rect, 4.0, Color32::from_rgb(14, 17, 20));
        painter.rect_filled(graph_rect, 4.0, Color32::from_rgb(19, 23, 27));
        painter.rect_stroke(
            graph_rect,
            4.0,
            Stroke::new(1.0, Color32::from_rgb(70, 76, 84)),
            egui::StrokeKind::Inside,
        );

        let points = &mut curve.points;
        let bends = &mut curve.bends;
        normalize_segment_bends(points, bends);
        constrain_curve_points(points);

        let to_screen = |p: Pos2| {
            Pos2::new(
                egui::lerp(graph_rect.left()..=graph_rect.right(), p.x),
                egui::lerp(graph_rect.bottom()..=graph_rect.top(), p.y),
            )
        };
        let to_normalized = |p: Pos2| {
            Pos2::new(
                ((p.x - graph_rect.left()) / graph_rect.width()).clamp(0.0, 1.0),
                ((graph_rect.bottom() - p.y) / graph_rect.height()).clamp(0.0, 1.0),
            )
        };

        let mut sampled = Vec::with_capacity(161);
        for step in 0..=160 {
            let t = step as f32 / 160.0;
            let y = envelope_value_linear(points, bends, t);
            sampled.push(to_screen(Pos2::new(t, y)));
        }
        for line in sampled.windows(2) {
            painter.line_segment(
                [line[0], line[1]],
                Stroke::new(2.0, Color32::from_rgb(245, 136, 78)),
            );
        }

        if response.double_clicked() {
            if let Some(pointer) = response.interact_pointer_pos().filter(|p| graph_rect.contains(*p)) {
                let new_point = to_normalized(pointer);
                let insert_index = points
                    .iter()
                    .position(|p| p.x > new_point.x)
                    .unwrap_or(points.len() - 1)
                    .max(1)
                    .min(points.len() - 1);
                points.insert(insert_index, new_point);
                bends.insert(insert_index.saturating_sub(1).min(bends.len()), 0.0);
                normalize_segment_bends(points, bends);
                constrain_curve_points(points);
                *selected_point = Some(insert_index);
            }
        }

        let mut remove_index: Option<usize> = None;
        for i in 0..points.len() {
            let screen = to_screen(points[i]);
            let hit_rect = Rect::from_center_size(screen, Vec2::splat(30.0));
            let point_response = ui.interact(
                hit_rect,
                ui.make_persistent_id((id_prefix, title, i)),
                Sense::click_and_drag(),
            );

            if point_response.clicked() {
                *selected_point = Some(i);
            }
            if point_response.secondary_clicked() && i > 0 && i + 1 < points.len() {
                remove_index = Some(i);
            }
            if point_response.dragged() {
                if let Some(pointer) = point_response.interact_pointer_pos() {
                    let mut next = to_normalized(pointer);
                    if i == 0 || i + 1 == points.len() {
                        next.x = points[i].x;
                    }
                    points[i] = next;
                    constrain_curve_points(points);
                    *selected_point = Some(i);
                    drag_active = true;
                }
            }

            let color = if *selected_point == Some(i) {
                Color32::from_rgb(255, 198, 70)
            } else {
                Color32::from_rgb(235, 108, 62)
            };
            painter.circle_filled(screen, 4.5, color);
            painter.circle_stroke(screen, 5.5, Stroke::new(1.0, Color32::BLACK));
            painter.circle_stroke(screen, 9.5, Stroke::new(1.5, APP_THEME.node_ring()));
        }

        if let Some(idx) = remove_index {
            points.remove(idx);
            if !bends.is_empty() {
                let bend_idx = idx.saturating_sub(1).min(bends.len() - 1);
                bends.remove(bend_idx);
            }
            normalize_segment_bends(points, bends);
            constrain_curve_points(points);
            *selected_point = Some(idx.saturating_sub(1).max(1).min(points.len().saturating_sub(2)));
        }

        // --- Edge bending (Ctrl/Cmd + drag on a segment) ---
        // Drag state persists across frames in egui temp data, keyed per editor.
        let bend_drag_id = egui::Id::new((id_prefix, "edge_bend_drag"));
        let (mut drag_segment, mut drag_start_y, mut drag_start_value) = ui
            .ctx()
            .data_mut(|d| d.get_temp::<(Option<usize>, Option<f32>, f32)>(bend_drag_id))
            .unwrap_or((None, None, 0.0));

        let bend_modifier_down = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let pointer_primary_down = ui.input(|i| i.pointer.primary_down());
        let pointer_pos = ui
            .input(|i| i.pointer.interact_pos())
            .filter(|pos| graph_rect.contains(*pos));

        if !pointer_primary_down {
            drag_segment = None;
            drag_start_y = None;
        }
        if drag_segment.is_some_and(|segment| segment >= bends.len()) {
            drag_segment = None;
            drag_start_y = None;
            drag_start_value = 0.0;
        }

        let mut bend_hover_segment: Option<usize> = None;
        let mut bend_hover_point: Option<Pos2> = None;
        let mut bend_hover_value: Option<f32> = None;
        let mut bend_hover_polyline: Vec<Pos2> = Vec::new();

        // Hit-test against the actual (bent) curve by sampling each segment.
        if bend_modifier_down {
            if let Some(pointer_pos) = pointer_pos {
                let mut best_segment: Option<(usize, f32, Pos2, Vec<Pos2>)> = None;
                for seg_idx in 0..points.len().saturating_sub(1) {
                    let left_norm = points[seg_idx];
                    let right_norm = points[seg_idx + 1];
                    let left_screen = to_screen(left_norm);
                    let right_screen = to_screen(right_norm);
                    let sample_count = ((left_screen.distance(right_screen) / 10.0).ceil()
                        as usize)
                        .clamp(8, 48);

                    let mut curve_polyline = Vec::with_capacity(sample_count + 1);
                    let mut prev_point = to_screen(Pos2::new(
                        left_norm.x,
                        envelope_value_linear(points, bends, left_norm.x),
                    ));
                    curve_polyline.push(prev_point);
                    let mut closest_point = prev_point;
                    let mut distance = f32::INFINITY;

                    for step in 1..=sample_count {
                        let local_t = step as f32 / sample_count as f32;
                        let sample_t = egui::lerp(left_norm.x..=right_norm.x, local_t);
                        let sample_point = to_screen(Pos2::new(
                            sample_t,
                            envelope_value_linear(points, bends, sample_t),
                        ));
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
                        let replace = best_segment
                            .as_ref()
                            .is_none_or(|(_, best_distance, _, _)| distance < *best_distance);
                        if replace {
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
                        && !drag_active
                        && drag_segment.is_none()
                    {
                        drag_segment = Some(seg_idx);
                        drag_start_y = Some(pointer_pos.y);
                        drag_start_value = bends.get(seg_idx).copied().unwrap_or(0.0);
                    }
                }
            }
        }

        // Apply the drag as a delta from the value at drag start.
        if bend_modifier_down && pointer_primary_down {
            if let (Some(seg_idx), Some(pointer_pos)) = (drag_segment, pointer_pos) {
                if seg_idx < bends.len() {
                    let start_y = drag_start_y.unwrap_or(pointer_pos.y);
                    let delta = (start_y - pointer_pos.y)
                        / (graph_rect.height() * 0.45).max(f32::EPSILON);
                    let bend = (drag_start_value + delta).clamp(-1.0, 1.0);
                    bends[seg_idx] = bend;
                    bend_hover_segment = Some(seg_idx);
                    bend_hover_point = Some(pointer_pos);
                    bend_hover_value = Some(bend);

                    let left_norm = points[seg_idx];
                    let right_norm = points[seg_idx + 1];
                    let left_screen = to_screen(left_norm);
                    let right_screen = to_screen(right_norm);
                    let sample_count = ((left_screen.distance(right_screen) / 10.0).ceil()
                        as usize)
                        .clamp(8, 48);
                    let mut hover_polyline = Vec::with_capacity(sample_count + 1);
                    for step in 0..=sample_count {
                        let local_t = step as f32 / sample_count as f32;
                        let sample_t = egui::lerp(left_norm.x..=right_norm.x, local_t);
                        hover_polyline.push(to_screen(Pos2::new(
                            sample_t,
                            envelope_value_linear(points, bends, sample_t),
                        )));
                    }
                    bend_hover_polyline = hover_polyline;
                    drag_active = true;
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                }
            }
        }

        ui.ctx().data_mut(|d| {
            d.insert_temp(bend_drag_id, (drag_segment, drag_start_y, drag_start_value));
        });

        // Hover highlight + bend value bubble.
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

        if let Some(sel) = selected_point.and_then(|i| points.get(i).copied()) {
            painter.text(
                Pos2::new(graph_rect.left() + 6.0, graph_rect.top() + 6.0),
                Align2::LEFT_TOP,
                format!("t={:.2} v={:.2}", sel.x, sel.y),
                egui::FontId::proportional(10.0 * ui_scale),
                Color32::from_rgb(180, 188, 198),
            );
        }
    });

    drag_active
}
