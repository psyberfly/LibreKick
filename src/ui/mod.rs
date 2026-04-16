use std::sync::Arc;

use nih_plug::prelude::Editor;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2},
    resizable_window::ResizableWindow,
    EguiState,
};

const MIN_POINT_GAP_X: f32 = 0.01;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CurveKind {
    Amplitude,
    Pitch,
}

struct Curve {
    points: Vec<Pos2>,
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
    selected_point: Option<usize>,
}

impl Default for BezierUiState {
    fn default() -> Self {
        Self {
            amplitude_curve: Curve::default_amplitude(),
            pitch_curve: Curve::default_pitch(),
            active_curve: CurveKind::Amplitude,
            selected_point: Some(1),
        }
    }
}

impl BezierUiState {
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

fn to_normalized(point: Pos2, rect: Rect) -> Pos2 {
    let x = ((point.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
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

pub fn create_testing_editor(editor_state: Arc<EguiState>) -> Option<Box<dyn Editor>> {
    let resizable_state = editor_state.clone();

    create_egui_editor(
        editor_state,
        BezierUiState::default(),
        |_ctx, _state| {},
        move |_ctx, _setter, state| {
            ResizableWindow::new("kick-plugin-resize")
                .min_size(Vec2::new(520.0, 320.0))
                .show(_ctx, &resizable_state, |ui| {
                ui.heading("Kick Curve Editor (Prototype)");
                ui.horizontal(|ui| {
                    ui.label("Curve:");
                    ui.selectable_value(&mut state.active_curve, CurveKind::Amplitude, "Amplitude");
                    ui.selectable_value(&mut state.active_curve, CurveKind::Pitch, "Pitch");
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
                let graph_rect = outer_rect.shrink2(Vec2::new(20.0, 20.0));

                let painter = ui.painter_at(outer_rect);
                painter.rect_filled(outer_rect, 4.0, Color32::from_rgb(10, 12, 14));
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

                let active_kind = state.active_curve;
                let mut selected_point = state.selected_point;

                {
                    let points = &mut state.active_curve_mut().points;
                    constrain_curve_points(points);
                    let mut remove_point_index: Option<usize> = None;

                    for i in 0..points.len() {
                        let screen_point = to_screen(points[i], graph_rect);
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
                            if ui
                                .add_enabled(can_remove_here, egui::Button::new("Remove point"))
                                .clicked()
                            {
                                remove_point_index = Some(i);
                                ui.close_menu();
                            }
                        });

                        if response.dragged() {
                            if let Some(pointer_pos) = response.interact_pointer_pos() {
                                points[i] = to_normalized(pointer_pos, graph_rect);
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
                            let new_point = to_normalized(pointer_pos, graph_rect);
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

                let screen_points: Vec<Pos2> = active_points
                    .iter()
                    .map(|point| to_screen(*point, graph_rect))
                    .collect();

                for line in screen_points.windows(2) {
                    painter.line_segment([line[0], line[1]], Stroke::new(1.0, Color32::from_rgb(90, 150, 190)));
                }

                let mut previous = to_screen(bezier_point(&active_points, 0.0), graph_rect);
                for step in 1..=220 {
                    let t = step as f32 / 220.0;
                    let next = to_screen(bezier_point(&active_points, t), graph_rect);
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
                });
        },
    )
}
