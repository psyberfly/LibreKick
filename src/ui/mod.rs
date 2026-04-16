use std::sync::Arc;

use nih_plug::prelude::Editor;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2},
    EguiState,
};

struct BezierUiState {
    control_points: [Pos2; 4],
}

impl Default for BezierUiState {
    fn default() -> Self {
        Self {
            control_points: [
                Pos2::new(0.0, 1.0),
                Pos2::new(0.15, 0.95),
                Pos2::new(0.42, 0.20),
                Pos2::new(1.0, 0.0),
            ],
        }
    }
}

fn to_screen(point: Pos2, rect: Rect) -> Pos2 {
    Pos2::new(
        rect.left() + point.x * rect.width(),
        rect.bottom() - point.y * rect.height(),
    )
}

fn to_normalized(point: Pos2, rect: Rect) -> Pos2 {
    let x = ((point.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    let y = ((rect.bottom() - point.y) / rect.height()).clamp(0.0, 1.0);
    Pos2::new(x, y)
}

fn cubic_bezier(points: [Pos2; 4], t: f32) -> Pos2 {
    let one_minus_t = 1.0 - t;
    let one_minus_t2 = one_minus_t * one_minus_t;
    let one_minus_t3 = one_minus_t2 * one_minus_t;
    let t2 = t * t;
    let t3 = t2 * t;

    let x = one_minus_t3 * points[0].x
        + 3.0 * one_minus_t2 * t * points[1].x
        + 3.0 * one_minus_t * t2 * points[2].x
        + t3 * points[3].x;
    let y = one_minus_t3 * points[0].y
        + 3.0 * one_minus_t2 * t * points[1].y
        + 3.0 * one_minus_t * t2 * points[2].y
        + t3 * points[3].y;

    Pos2::new(x, y)
}

pub fn create_testing_editor(editor_state: Arc<EguiState>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        editor_state,
        BezierUiState::default(),
        |_ctx, _state| {},
        |_ctx, _setter, state| {
            egui::CentralPanel::default().show(_ctx, |ui| {
                ui.heading("Kick Curve Editor (Prototype)");
                ui.add_space(8.0);

                let graph_width = (ui.available_width() - 8.0).max(280.0);
                let graph_height = (ui.available_height() - 12.0).max(220.0);
                let (outer_rect, _) = ui.allocate_exact_size(
                    Vec2::new(graph_width, graph_height),
                    Sense::hover(),
                );
                let graph_rect = outer_rect.shrink2(Vec2::new(20.0, 20.0));

                let painter = ui.painter_at(outer_rect);
                painter.rect_filled(graph_rect, 4.0, Color32::from_rgb(16, 19, 22));
                painter.rect_stroke(
                    graph_rect,
                    4.0,
                    Stroke::new(1.0, Color32::from_rgb(90, 95, 102)),
                    egui::StrokeKind::Inside,
                );

                let grid_divisions = 8;
                for i in 0..=grid_divisions {
                    let f = i as f32 / grid_divisions as f32;
                    let x = egui::lerp(graph_rect.left()..=graph_rect.right(), f);
                    let y = egui::lerp(graph_rect.bottom()..=graph_rect.top(), f);

                    painter.line_segment(
                        [Pos2::new(x, graph_rect.top()), Pos2::new(x, graph_rect.bottom())],
                        Stroke::new(1.0, Color32::from_rgb(34, 39, 45)),
                    );
                    painter.line_segment(
                        [Pos2::new(graph_rect.left(), y), Pos2::new(graph_rect.right(), y)],
                        Stroke::new(1.0, Color32::from_rgb(34, 39, 45)),
                    );
                }

                painter.text(
                    Pos2::new(graph_rect.left(), graph_rect.top() - 16.0),
                    Align2::LEFT_BOTTOM,
                    "Amount",
                    FontId::proportional(12.0),
                    Color32::from_rgb(185, 191, 198),
                );
                painter.text(
                    Pos2::new(graph_rect.right(), graph_rect.bottom() + 16.0),
                    Align2::RIGHT_TOP,
                    "Time",
                    FontId::proportional(12.0),
                    Color32::from_rgb(185, 191, 198),
                );

                state.control_points[0].x = 0.0;
                state.control_points[3].x = 1.0;

                for point in &mut state.control_points {
                    point.x = point.x.clamp(0.0, 1.0);
                    point.y = point.y.clamp(0.0, 1.0);
                }

                for i in 0..state.control_points.len() {
                    let screen_point = to_screen(state.control_points[i], graph_rect);
                    let hit_rect = Rect::from_center_size(screen_point, Vec2::splat(18.0));
                    let response = ui.interact(
                        hit_rect,
                        ui.make_persistent_id(("bezier-control", i)),
                        Sense::drag(),
                    );

                    if response.dragged() {
                        if let Some(pointer_pos) = response.interact_pointer_pos() {
                            state.control_points[i] = to_normalized(pointer_pos, graph_rect);
                        }
                    }
                }

                let screen_points = state
                    .control_points
                    .map(|point| to_screen(point, graph_rect));

                painter.line_segment(
                    [screen_points[0], screen_points[1]],
                    Stroke::new(1.0, Color32::from_rgb(90, 150, 190)),
                );
                painter.line_segment(
                    [screen_points[2], screen_points[3]],
                    Stroke::new(1.0, Color32::from_rgb(90, 150, 190)),
                );

                let mut previous = to_screen(cubic_bezier(state.control_points, 0.0), graph_rect);
                for step in 1..=160 {
                    let t = step as f32 / 160.0;
                    let next = to_screen(cubic_bezier(state.control_points, t), graph_rect);
                    painter.line_segment(
                        [previous, next],
                        Stroke::new(2.0, Color32::from_rgb(72, 210, 170)),
                    );
                    previous = next;
                }

                for (i, point) in screen_points.iter().enumerate() {
                    let color = if i == 0 || i == 3 {
                        Color32::from_rgb(242, 170, 73)
                    } else {
                        Color32::from_rgb(112, 182, 255)
                    };
                    painter.circle_filled(*point, 6.0, color);
                    painter.circle_stroke(*point, 7.0, Stroke::new(1.0, Color32::BLACK));
                }
                painter.circle_filled(
                    to_screen(state.control_points[0], graph_rect),
                    6.5,
                    Color32::from_rgb(242, 170, 73),
                );
                painter.circle_filled(
                    to_screen(state.control_points[3], graph_rect),
                    6.5,
                    Color32::from_rgb(242, 170, 73),
                );

                ui.label("Drag the 4 points to shape the curve.");
                });
        },
    )
}
