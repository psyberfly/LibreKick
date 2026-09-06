use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg::{LineCap, Paint, Path};

const PRIMARY_TEXT_COLOR: Color = Color::rgb(245, 160, 88);

#[derive(Debug, Clone, Copy, PartialEq, Data)]
pub(crate) struct SignalPoint {
    pub x: f32,
    pub y: f32,
}

fn sparkline_char(value: f32) -> char {
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let idx = (value.clamp(0.0, 1.0) * (LEVELS.len() as f32 - 1.0)).round() as usize;
    LEVELS[idx]
}

impl SignalPoint {
    pub(crate) fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SignalShaperTheme {
    pub panel_background: Color,
    pub graph_background: Color,
    pub border: Color,
    pub curve: Color,
    pub point: Color,
    pub selected_point: Color,
    pub text: Color,
}

impl Default for SignalShaperTheme {
    fn default() -> Self {
        Self {
            panel_background: Color::rgb(14, 17, 20),
            graph_background: Color::rgb(19, 23, 27),
            border: Color::rgb(70, 76, 84),
            curve: Color::rgb(245, 136, 78),
            point: Color::rgb(235, 108, 62),
            selected_point: Color::rgb(255, 198, 70),
            text: PRIMARY_TEXT_COLOR,
        }
    }
}

pub(crate) type AxisValueFormatter = fn(f32) -> String;

#[derive(Clone, Copy)]
pub(crate) struct SignalShaperAxis {
    pub x_name: &'static str,
    pub y_name: &'static str,
    pub x_formatter: AxisValueFormatter,
    pub y_formatter: AxisValueFormatter,
}

#[derive(Clone, Copy)]
pub(crate) struct SignalShaperConfig {
    pub title: &'static str,
    pub axis: SignalShaperAxis,
    pub sample_steps: usize,
    pub edge_bend_hit_radius_pixels: f32,
    pub theme: SignalShaperTheme,
}

impl SignalShaperConfig {
    pub(crate) fn with_theme(mut self, theme: SignalShaperTheme) -> Self {
        self.theme = theme;
        self
    }
}

impl Default for SignalShaperConfig {
    fn default() -> Self {
        Self {
            title: "Signal Shaper",
            axis: SignalShaperAxis {
                x_name: "Time",
                y_name: "Value",
                x_formatter: default_percent_label,
                y_formatter: default_percent_label,
            },
            sample_steps: 160,
            edge_bend_hit_radius_pixels: 14.0,
            theme: SignalShaperTheme::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Data)]
pub(crate) struct SignalCurve {
    pub points: Vec<SignalPoint>,
    pub bends: Vec<f32>,
}

impl SignalCurve {
    pub(crate) fn normalized_default() -> Self {
        Self {
            points: vec![
                SignalPoint::new(0.0, 1.0),
                SignalPoint::new(0.12, 0.94),
                SignalPoint::new(0.42, 0.24),
                SignalPoint::new(1.0, 0.0),
            ],
            bends: vec![0.0; 3],
        }
    }
}

const SHAPER_HISTORY_LIMIT: usize = 32;

#[derive(Debug, Clone, PartialEq, Data)]
pub(crate) struct SignalShaperState {
    pub curve: SignalCurve,
    pub selected_point: Option<usize>,
    pub history: Vec<SignalCurve>,
    pub future: Vec<SignalCurve>,
}

impl SignalShaperState {
    pub(crate) fn new(curve: SignalCurve) -> Self {
        Self {
            curve: curve.clone(),
            selected_point: Some(1),
            history: vec![curve],
            future: Vec::new(),
        }
    }

    fn push_history(&mut self) {
        if self.history.last() != Some(&self.curve) {
            self.history.push(self.curve.clone());
            if self.history.len() > SHAPER_HISTORY_LIMIT {
                self.history.remove(0);
            }
        }
        self.future.clear();
    }

    pub(crate) fn apply(&mut self, event: SignalShaperEvent) {
        match event {
            SignalShaperEvent::SelectPoint(index) => {
                if index < self.curve.points.len() {
                    self.selected_point = Some(index);
                }
            }
            SignalShaperEvent::AddPoint(point) => {
                self.push_history();
                let points = &mut self.curve.points;
                if points.len() < 2 {
                    return;
                }

                let insert_index = points
                    .iter()
                    .position(|existing| existing.x > point.x)
                    .unwrap_or(points.len() - 1)
                    .max(1)
                    .min(points.len() - 1);

                points.insert(insert_index, clamp_unit_point(point));
                self.curve
                    .bends
                    .insert(insert_index.saturating_sub(1).min(self.curve.bends.len()), 0.0);
                normalize_segment_bends(&self.curve.points, &mut self.curve.bends);
                constrain_curve_points(&mut self.curve.points);
                self.selected_point = Some(insert_index);
            }
            SignalShaperEvent::MovePoint { index, point } => {
                if index >= self.curve.points.len() {
                    return;
                }

                let mut next = clamp_unit_point(point);
                if index == 0 || index + 1 == self.curve.points.len() {
                    next.x = self.curve.points[index].x;
                }
                self.curve.points[index] = next;
                constrain_curve_points(&mut self.curve.points);
                self.selected_point = Some(index);
            }
            SignalShaperEvent::RemovePoint(index) => {
                if index == 0 || index + 1 >= self.curve.points.len() {
                    return;
                }
                self.push_history();

                self.curve.points.remove(index);
                if !self.curve.bends.is_empty() {
                    let bend_index = index.saturating_sub(1).min(self.curve.bends.len() - 1);
                    self.curve.bends.remove(bend_index);
                }
                normalize_segment_bends(&self.curve.points, &mut self.curve.bends);
                constrain_curve_points(&mut self.curve.points);
                self.selected_point = Some(index.saturating_sub(1).min(self.curve.points.len().saturating_sub(1)));
            }
            SignalShaperEvent::SetSegmentBend { segment, bend } => {
                if let Some(target) = self.curve.bends.get_mut(segment) {
                    *target = bend.clamp(-1.0, 1.0);
                }
            }
            SignalShaperEvent::Commit => self.push_history(),
            SignalShaperEvent::Undo => {
                if let Some(prev) = self.history.pop() {
                    self.future.push(self.curve.clone());
                    self.curve = prev;
                    self.selected_point = Some(1)
                        .filter(|i| *i < self.curve.points.len());
                }
            }
            SignalShaperEvent::Redo => {
                if let Some(next) = self.future.pop() {
                    self.push_history();
                    self.curve = next;
                    self.selected_point = Some(1)
                        .filter(|i| *i < self.curve.points.len());
                }
            }
        }
    }

    pub(crate) fn sampled_curve(&self, sample_steps: usize) -> Vec<SignalPoint> {
        let steps = sample_steps.max(1);
        let mut sampled = Vec::with_capacity(steps + 1);
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let y = envelope_value_linear(&self.curve.points, &self.curve.bends, t);
            sampled.push(SignalPoint::new(t, y));
        }
        sampled
    }

    pub(crate) fn selected_label(&self, config: &SignalShaperConfig) -> String {
        if let Some(index) = self.selected_point.and_then(|i| self.curve.points.get(i)) {
            let x = (config.axis.x_formatter)(index.x);
            let y = (config.axis.y_formatter)(index.y);
            format!("{} {}, {} {}", config.axis.x_name, x, config.axis.y_name, y)
        } else {
            "No point selected".to_owned()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SignalShaperEvent {
    SelectPoint(usize),
    AddPoint(SignalPoint),
    MovePoint { index: usize, point: SignalPoint },
    RemovePoint(usize),
    SetSegmentBend { segment: usize, bend: f32 },
    Commit,
    Undo,
    Redo,
}

struct SignalShaperGraph {
    config: SignalShaperConfig,
    state: SignalShaperState,
    secondary: Option<SignalShaperState>,
    secondary_color: Color,
    map_event: fn(SignalShaperEvent) -> crate::ui_vizia::AppScaffoldEvent,
    drag_point_index: Option<usize>,
    drag_bend_segment: Option<usize>,
}

impl SignalShaperGraph {
    fn new(
        cx: &mut Context,
        config: SignalShaperConfig,
        state: SignalShaperState,
        secondary: Option<SignalShaperState>,
        secondary_color: Color,
        map_event: fn(SignalShaperEvent) -> crate::ui_vizia::AppScaffoldEvent,
    ) -> Handle<Self> {
        Self {
            config,
            state,
            secondary,
            secondary_color,
            map_event,
            drag_point_index: None,
            drag_bend_segment: None,
        }
        .build(cx, |_| {})
    }

    fn to_normalized(&self, bounds: BoundingBox, x: f32, y: f32) -> SignalPoint {
        SignalPoint::new(
            ((x - bounds.x) / bounds.w.max(f32::EPSILON)).clamp(0.0, 1.0),
            ((bounds.y + bounds.h - y) / bounds.h.max(f32::EPSILON)).clamp(0.0, 1.0),
        )
    }

    fn to_screen(&self, bounds: BoundingBox, p: SignalPoint) -> (f32, f32) {
        (
            bounds.x + p.x.clamp(0.0, 1.0) * bounds.w,
            bounds.y + (1.0 - p.y.clamp(0.0, 1.0)) * bounds.h,
        )
    }

    fn nearest_point(&self, bounds: BoundingBox, x: f32, y: f32, radius: f32) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (idx, point) in self.state.curve.points.iter().enumerate() {
            let (sx, sy) = self.to_screen(bounds, *point);
            let distance = ((sx - x).powi(2) + (sy - y).powi(2)).sqrt();
            if distance <= radius {
                match best {
                    Some((_, best_distance)) if distance >= best_distance => {}
                    _ => best = Some((idx, distance)),
                }
            }
        }
        best.map(|(idx, _)| idx)
    }

    fn nearest_segment(&self, bounds: BoundingBox, x: f32, y: f32, radius: f32) -> Option<usize> {
        let points = &self.state.curve.points;
        if points.len() < 2 {
            return None;
        }
        let mut best: Option<(usize, f32)> = None;
        for seg_idx in 0..points.len().saturating_sub(1) {
            let (x1, y1) = self.to_screen(bounds, points[seg_idx]);
            let (x2, y2) = self.to_screen(bounds, points[seg_idx + 1]);
            let abx = x2 - x1;
            let aby = y2 - y1;
            let apx = x - x1;
            let apy = y - y1;
            let denom = (abx * abx + aby * aby).max(f32::EPSILON);
            let t = ((apx * abx + apy * aby) / denom).clamp(0.0, 1.0);
            let cx = x1 + t * abx;
            let cy = y1 + t * aby;
            let distance = ((cx - x).powi(2) + (cy - y).powi(2)).sqrt();
            if distance <= radius {
                match best {
                    Some((_, best_distance)) if distance >= best_distance => {}
                    _ => best = Some((seg_idx, distance)),
                }
            }
        }
        best.map(|(idx, _)| idx)
    }
}

impl View for SignalShaperGraph {
    fn element(&self) -> Option<&'static str> {
        Some("signalshaper-graph")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, _| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                let bounds = cx.bounds();
                let px = cx.mouse().left.pos_down.0;
                let py = cx.mouse().left.pos_down.1;
                let ctrl_down = cx.modifiers().contains(Modifiers::CTRL)
                    || cx.modifiers().contains(Modifiers::LOGO);

                if ctrl_down {
                    if let Some(seg_idx) = self.nearest_segment(bounds, px, py, 14.0) {
                        self.drag_bend_segment = Some(seg_idx);
                        cx.capture();
                    }
                } else if let Some(point_idx) = self.nearest_point(bounds, px, py, 12.0) {
                    self.drag_point_index = Some(point_idx);
                    cx.emit((self.map_event)(SignalShaperEvent::SelectPoint(point_idx)));
                    cx.capture();
                }
            }

            WindowEvent::MouseDown(MouseButton::Right) => {
                let bounds = cx.bounds();
                let px = cx.mouse().right.pos_down.0;
                let py = cx.mouse().right.pos_down.1;
                if let Some(point_idx) = self.nearest_point(bounds, px, py, 12.0) {
                    if point_idx > 0 && point_idx + 1 < self.state.curve.points.len() {
                        cx.emit((self.map_event)(SignalShaperEvent::RemovePoint(point_idx)));
                    }
                }
            }

            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let bounds = cx.bounds();
                let px = cx.mouse().left.pos_down.0;
                let py = cx.mouse().left.pos_down.1;
                let point = self.to_normalized(bounds, px, py);
                cx.emit((self.map_event)(SignalShaperEvent::AddPoint(point)));
            }

            WindowEvent::MouseMove(x, y) => {
                let bounds = cx.bounds();
                if let Some(point_idx) = self.drag_point_index {
                    let point = self.to_normalized(bounds, *x, *y);
                    cx.emit((self.map_event)(SignalShaperEvent::MovePoint {
                        index: point_idx,
                        point,
                    }));
                } else if let Some(segment) = self.drag_bend_segment {
                    let center_y = bounds.y + bounds.h * 0.5;
                    let bend = ((center_y - *y) / (bounds.h * 0.5).max(f32::EPSILON)).clamp(-1.0, 1.0);
                    cx.emit((self.map_event)(SignalShaperEvent::SetSegmentBend {
                        segment,
                        bend,
                    }));
                }
            }

            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_point_index.is_some() || self.drag_bend_segment.is_some() {
                    cx.emit((self.map_event)(SignalShaperEvent::Commit));
                }
                self.drag_point_index = None;
                self.drag_bend_segment = None;
                cx.release();
            }

            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();

        let mut bg_path = Path::new();
        bg_path.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.fill_path(&bg_path, &Paint::color(self.config.theme.graph_background.into()));

        if let Some(secondary) = &self.secondary {
            let mut sampled = Vec::with_capacity(self.config.sample_steps + 1);
            for step in 0..=self.config.sample_steps {
                let t = step as f32 / self.config.sample_steps.max(1) as f32;
                let y = envelope_value_linear(&secondary.curve.points, &secondary.curve.bends, t);
                sampled.push(self.to_screen(bounds, SignalPoint::new(t, y)));
            }
            if let Some((first_x, first_y)) = sampled.first().copied() {
                let mut curve_path = Path::new();
                curve_path.move_to(first_x, first_y);
                for (x, y) in sampled.iter().skip(1).copied() {
                    curve_path.line_to(x, y);
                }
                let mut paint = Paint::color(self.secondary_color.into());
                paint.set_line_width(1.5);
                paint.set_line_cap(LineCap::Round);
                canvas.stroke_path(&curve_path, &paint);
            }
        }

        let mut sampled = Vec::with_capacity(self.config.sample_steps + 1);
        for step in 0..=self.config.sample_steps {
            let t = step as f32 / self.config.sample_steps.max(1) as f32;
            let y = envelope_value_linear(&self.state.curve.points, &self.state.curve.bends, t);
            sampled.push(self.to_screen(bounds, SignalPoint::new(t, y)));
        }

        if let Some((first_x, first_y)) = sampled.first().copied() {
            let mut curve_path = Path::new();
            curve_path.move_to(first_x, first_y);
            for (x, y) in sampled.iter().skip(1).copied() {
                curve_path.line_to(x, y);
            }
            let mut curve_paint = Paint::color(self.config.theme.curve.into());
            curve_paint.set_line_width(2.0);
            curve_paint.set_line_cap(LineCap::Round);
            canvas.stroke_path(&curve_path, &curve_paint);
        }

        let selected = self.state.selected_point;
        for (idx, point) in self.state.curve.points.iter().enumerate() {
            let (x, y) = self.to_screen(bounds, *point);
            let color = if selected == Some(idx) {
                self.config.theme.selected_point
            } else {
                self.config.theme.point
            };

            let mut point_path = Path::new();
            point_path.circle(x, y, 4.5);
            canvas.fill_path(&point_path, &Paint::color(color.into()));
        }

        let mut border_path = Path::new();
        border_path.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        let mut border_paint = Paint::color(self.config.theme.border.into());
        border_paint.set_line_width(1.0);
        canvas.stroke_path(&border_path, &border_paint);
    }
}

pub(crate) fn build_signal_shaper(
    cx: &mut Context,
    config: SignalShaperConfig,
    state: &SignalShaperState,
    secondary: Option<SignalShaperState>,
    secondary_color: Color,
    map_event: fn(SignalShaperEvent) -> crate::ui_vizia::AppScaffoldEvent,
) {
    let sampled = state.sampled_curve(config.sample_steps);
    let sampled_count = sampled.len();
    let selected_label = state.selected_label(&config);
    let theme = config.theme;

    let sample_stride = (sampled.len() / 8).max(1);
    let sample_preview = sampled
        .iter()
        .step_by(sample_stride)
        .take(8)
        .map(|point| format!("{:.2}", point.y))
        .collect::<Vec<_>>()
        .join(" · ");

    let point_preview = state
        .curve
        .points
        .iter()
        .take(5)
        .enumerate()
        .map(|(index, point)| {
            format!(
                "p{index}: {} {}",
                (config.axis.x_formatter)(point.x),
                (config.axis.y_formatter)(point.y)
            )
        })
        .collect::<Vec<_>>()
        .join("    ");

    let selected_index = state
        .selected_point
        .filter(|&idx| idx < state.curve.points.len())
        .unwrap_or(0);
    let selected_point = state
        .curve
        .points
        .get(selected_index)
        .copied()
        .unwrap_or(SignalPoint::new(0.0, 0.0));
    let prev_index = selected_index.saturating_sub(1);
    let next_index = (selected_index + 1).min(state.curve.points.len().saturating_sub(1));
    let selected_segment = if selected_index + 1 < state.curve.points.len() {
        selected_index
    } else {
        selected_index.saturating_sub(1)
    }
    .min(state.curve.bends.len().saturating_sub(1));
    let selected_bend = state.curve.bends.get(selected_segment).copied().unwrap_or(0.0);

    let sparkline = sampled
        .iter()
        .step_by((sampled.len() / 48).max(1))
        .map(|point| sparkline_char(point.y))
        .collect::<String>();

    VStack::new(cx, |cx| {
        Label::new(cx, config.title).font_size(16.0).color(theme.text);

        HStack::new(cx, |cx| {
            Label::new(cx, config.axis.x_name).font_size(12.0).color(theme.text);
            Label::new(cx, config.axis.y_name)
                .font_size(12.0)
                .left(Pixels(12.0))
                .color(theme.text);
        })
        .height(Auto);

        VStack::new(cx, |cx| {
            Label::new(cx, config.title).font_size(12.0).color(theme.text);
            SignalShaperGraph::new(
                cx,
                config,
                state.clone(),
                secondary,
                secondary_color,
                map_event,
            )
            .height(Pixels(220.0))
            .width(Stretch(1.0));
            Label::new(cx, &sparkline).font_size(13.0).color(theme.curve);
            Label::new(cx, &point_preview).font_size(11.0).color(theme.text);
            Label::new(cx, &format!("samples: {sample_preview}"))
                .font_size(11.0)
                .color(theme.text);
        })
        .width(Stretch(1.0))
        .child_left(Pixels(8.0))
        .child_right(Pixels(8.0))
        .child_top(Pixels(8.0))
        .child_bottom(Pixels(8.0))
        .background_color(theme.graph_background)
        .border_width(Pixels(1.0))
        .border_color(theme.border);

        HStack::new(cx, |cx| {
            Label::new(cx, "Point").color(theme.text);
            Button::new(
                cx,
                move |cx| cx.emit(map_event(SignalShaperEvent::SelectPoint(prev_index))),
                |cx| Label::new(cx, "<"),
            )
            .class("control-button");
            Label::new(cx, &format!("{}", selected_index)).color(theme.text);
            Button::new(
                cx,
                move |cx| cx.emit(map_event(SignalShaperEvent::SelectPoint(next_index))),
                |cx| Label::new(cx, ">"),
            )
            .class("control-button");
            Button::new(
                cx,
                move |cx| {
                    cx.emit(map_event(SignalShaperEvent::AddPoint(SignalPoint::new(
                        ((selected_point.x + 0.08).min(0.99)).max(0.01),
                        selected_point.y,
                    ))))
                },
                |cx| Label::new(cx, "+ Point"),
            )
            .class("control-button");
            Button::new(
                cx,
                move |cx| cx.emit(map_event(SignalShaperEvent::RemovePoint(selected_index))),
                |cx| Label::new(cx, "- Point"),
            )
            .class("control-button");
        })
        .class("control-row");

        HStack::new(cx, |cx| {
            Label::new(cx, "Move").color(theme.text);
            Button::new(
                cx,
                move |cx| {
                    cx.emit(map_event(SignalShaperEvent::MovePoint {
                        index: selected_index,
                        point: SignalPoint::new(selected_point.x - 0.01, selected_point.y),
                    }))
                },
                |cx| Label::new(cx, "X-"),
            )
            .class("control-button");
            Button::new(
                cx,
                move |cx| {
                    cx.emit(map_event(SignalShaperEvent::MovePoint {
                        index: selected_index,
                        point: SignalPoint::new(selected_point.x + 0.01, selected_point.y),
                    }))
                },
                |cx| Label::new(cx, "X+"),
            )
            .class("control-button");
            Button::new(
                cx,
                move |cx| {
                    cx.emit(map_event(SignalShaperEvent::MovePoint {
                        index: selected_index,
                        point: SignalPoint::new(selected_point.x, selected_point.y - 0.01),
                    }))
                },
                |cx| Label::new(cx, "Y-"),
            )
            .class("control-button");
            Button::new(
                cx,
                move |cx| {
                    cx.emit(map_event(SignalShaperEvent::MovePoint {
                        index: selected_index,
                        point: SignalPoint::new(selected_point.x, selected_point.y + 0.01),
                    }))
                },
                |cx| Label::new(cx, "Y+"),
            )
            .class("control-button");
        })
        .class("control-row");

        HStack::new(cx, |cx| {
            Button::new(
                cx,
                move |cx| cx.emit(map_event(SignalShaperEvent::Undo)),
                |cx| Label::new(cx, "Undo"),
            )
            .class("control-button");
            Button::new(
                cx,
                move |cx| cx.emit(map_event(SignalShaperEvent::Redo)),
                |cx| Label::new(cx, "Redo"),
            )
            .class("control-button");
        })
        .class("control-row");

        HStack::new(cx, |cx| {
            Label::new(cx, "Bend").color(theme.text);
            Label::new(cx, &format!("seg {}", selected_segment)).color(theme.text);
            Button::new(
                cx,
                move |cx| {
                    cx.emit(map_event(SignalShaperEvent::SetSegmentBend {
                        segment: selected_segment,
                        bend: selected_bend - 0.1,
                    }))
                },
                |cx| Label::new(cx, "-"),
            )
            .class("control-button");
            Label::new(cx, &format!("{selected_bend:.2}")).color(theme.text);
            Button::new(
                cx,
                move |cx| {
                    cx.emit(map_event(SignalShaperEvent::SetSegmentBend {
                        segment: selected_segment,
                        bend: selected_bend + 0.1,
                    }))
                },
                |cx| Label::new(cx, "+"),
            )
            .class("control-button");
        })
        .class("control-row");

        Label::new(cx, &selected_label).font_size(12.0).color(theme.text);
        Label::new(
            cx,
            &format!(
                "points: {}  bends: {}  sampled: {}  bend-hit-radius: {:.0}px",
                state.curve.points.len(),
                state.curve.bends.len(),
                sampled_count,
                config.edge_bend_hit_radius_pixels
            ),
        )
        .font_size(11.0)
        .color(theme.text);
    })
    .class("signal-shaper")
    .width(Stretch(1.0))
    .background_color(theme.panel_background);
}

pub(crate) fn default_percent_label(normalized: f32) -> String {
    format!("{:.0}%", normalized.clamp(0.0, 1.0) * 100.0)
}

pub(crate) fn normalize_segment_bends(points: &[SignalPoint], bends: &mut Vec<f32>) {
    let target_len = points.len().saturating_sub(1);
    bends.truncate(target_len);
    if bends.len() < target_len {
        bends.resize(target_len, 0.0);
    }
    for bend in bends.iter_mut() {
        *bend = bend.clamp(-1.0, 1.0);
    }
}

pub(crate) fn constrain_curve_points(points: &mut [SignalPoint]) {
    if points.len() < 2 {
        return;
    }

    points[0].x = 0.0;
    points[0].y = points[0].y.clamp(0.0, 1.0);
    let last = points.len() - 1;
    points[last].x = 1.0;
    points[last].y = points[last].y.clamp(0.0, 1.0);

    for i in 1..last {
        let prev_x = points[i - 1].x;
        let next_x = points[i + 1].x;
        points[i].x = points[i].x.clamp(prev_x + 0.001, next_x - 0.001);
        points[i].y = points[i].y.clamp(0.0, 1.0);
    }
}

pub(crate) fn envelope_value_linear(points: &[SignalPoint], bends: &[f32], t: f32) -> f32 {
    if points.is_empty() {
        return 0.0;
    }

    let t = t.clamp(0.0, 1.0);
    if t <= points[0].x {
        return points[0].y;
    }
    for seg in 0..points.len().saturating_sub(1) {
        let left = points[seg];
        let right = points[seg + 1];
        if t <= right.x {
            let width = (right.x - left.x).max(f32::EPSILON);
            let local_t = ((t - left.x) / width).clamp(0.0, 1.0);
            let bend = bends.get(seg).copied().unwrap_or(0.0);
            let curved_t = bend_local_t(local_t, bend);
            return left.y + (right.y - left.y) * curved_t;
        }
    }

    points.last().map_or(0.0, |point| point.y)
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

fn clamp_unit_point(point: SignalPoint) -> SignalPoint {
    SignalPoint {
        x: point.x.clamp(0.0, 1.0),
        y: point.y.clamp(0.0, 1.0),
    }
}
