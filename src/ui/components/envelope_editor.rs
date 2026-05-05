use nih_plug_egui::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Vec2};

use crate::ui::{
    helpers::{constrain_curve_points, envelope_value_linear, normalize_segment_bends},
    state::Curve,
};

pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    id_prefix: &str,
    title: &str,
    curve: &mut Curve,
    selected_point: &mut Option<usize>,
) {
    const EDGE_BEND_HIT_RADIUS_PIXELS: f32 = 14.0;

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
            let hit_rect = Rect::from_center_size(screen, Vec2::splat(24.0));
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
                }
            }

            let color = if *selected_point == Some(i) {
                Color32::from_rgb(255, 198, 70)
            } else {
                Color32::from_rgb(235, 108, 62)
            };
            painter.circle_filled(screen, 4.5, color);
            painter.circle_stroke(screen, 5.5, Stroke::new(1.0, Color32::BLACK));
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

        let modifier_down = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        if modifier_down {
            let pointer = ui.input(|i| i.pointer.hover_pos());
            if let Some(pointer) = pointer.filter(|p| graph_rect.contains(*p)) {
                let mut best: Option<(usize, f32)> = None;
                for seg in 0..points.len().saturating_sub(1) {
                    let left = to_screen(points[seg]);
                    let right = to_screen(points[seg + 1]);
                    let ab = right - left;
                    let ap = pointer - left;
                    let denom = ab.dot(ab).max(f32::EPSILON);
                    let t = (ap.dot(ab) / denom).clamp(0.0, 1.0);
                    let closest = left + ab * t;
                    let distance = closest.distance(pointer);
                    if distance <= EDGE_BEND_HIT_RADIUS_PIXELS {
                        if let Some((_, best_distance)) = best {
                            if distance < best_distance {
                                best = Some((seg, distance));
                            }
                        } else {
                            best = Some((seg, distance));
                        }
                    }
                }

                if let Some((seg, _)) = best {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                    if ui.input(|i| i.pointer.primary_down()) {
                        let left = to_screen(points[seg]);
                        let right = to_screen(points[seg + 1]);
                        let midpoint_y = (left.y + right.y) * 0.5;
                        bends[seg] = ((midpoint_y - pointer.y) / (graph_rect.height() * 0.5))
                            .clamp(-1.0, 1.0);
                    }
                }
            }
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
}
