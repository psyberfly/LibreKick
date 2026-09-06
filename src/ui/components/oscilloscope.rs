use nih_plug_egui::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Vec2};

pub(crate) struct OscilloscopeSettings {
    pub(crate) zoom_x: f32,
    pub(crate) zoom_y: f32,
}

pub(crate) struct OscilloscopeTrace<'a> {
    pub(crate) label: &'a str,
    pub(crate) color: Color32,
    pub(crate) samples: &'a [f32],
    pub(crate) visible: bool,
}

pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    settings: &OscilloscopeSettings,
    traces: &[OscilloscopeTrace<'_>],
) {
    let min_height = (260.0 * ui_scale).max(180.0);
    let (outer_rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(300.0 * ui_scale), min_height),
        Sense::hover(),
    );
    let graph_rect = outer_rect.shrink2(Vec2::new(14.0 * ui_scale, 16.0 * ui_scale));
    let painter = ui.painter_at(outer_rect);

    painter.rect_filled(outer_rect, 4.0, Color32::from_rgb(16, 19, 22));
    painter.rect_filled(graph_rect, 4.0, Color32::from_rgb(20, 24, 28));
    painter.rect_stroke(
        graph_rect,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(90, 95, 102)),
        egui::StrokeKind::Inside,
    );

    draw_grid(&painter, graph_rect, ui_scale);

    for trace in traces {
        if !trace.visible || trace.samples.len() < 2 {
            continue;
        }
        draw_trace(&painter, graph_rect, trace.samples, trace.color, settings);
    }

    painter.text(
        graph_rect.left_top() + Vec2::new(8.0 * ui_scale, 8.0 * ui_scale),
        Align2::LEFT_TOP,
        "Realtime Oscilloscope",
        egui::FontId::proportional(11.0 * ui_scale),
        Color32::from_rgb(185, 191, 198),
    );

    let mut legend_x = graph_rect.left() + 8.0 * ui_scale;
    let legend_y = graph_rect.bottom() - 14.0 * ui_scale;
    for trace in traces {
        if !trace.visible {
            continue;
        }
        let text = trace.label;
        let swatch_rect = Rect::from_min_size(
            Pos2::new(legend_x, legend_y - 8.0 * ui_scale),
            Vec2::new(10.0 * ui_scale, 10.0 * ui_scale),
        );
        painter.rect_filled(swatch_rect, 2.0, trace.color);
        painter.text(
            Pos2::new(swatch_rect.right() + 6.0 * ui_scale, legend_y - 3.0 * ui_scale),
            Align2::LEFT_TOP,
            text,
            egui::FontId::proportional(10.0 * ui_scale),
            Color32::from_rgb(185, 191, 198),
        );
        legend_x += (text.len() as f32 * 7.0 + 36.0) * ui_scale;
    }
}

fn draw_grid(painter: &egui::Painter, graph_rect: Rect, ui_scale: f32) {
    let grid_color = Color32::from_rgba_unmultiplied(120, 130, 140, 30);
    for i in 1..8 {
        let t = i as f32 / 8.0;
        let x = egui::lerp(graph_rect.x_range(), t);
        painter.line_segment(
            [Pos2::new(x, graph_rect.top()), Pos2::new(x, graph_rect.bottom())],
            Stroke::new(1.0, grid_color),
        );
    }

    for i in 1..4 {
        let t = i as f32 / 4.0;
        let y = egui::lerp(graph_rect.y_range(), t);
        painter.line_segment(
            [Pos2::new(graph_rect.left(), y), Pos2::new(graph_rect.right(), y)],
            Stroke::new(1.0, grid_color),
        );
    }

    let mid_y = egui::lerp(graph_rect.y_range(), 0.5);
    painter.line_segment(
        [Pos2::new(graph_rect.left(), mid_y), Pos2::new(graph_rect.right(), mid_y)],
        Stroke::new((1.5 * ui_scale).max(1.0), Color32::from_rgba_unmultiplied(170, 180, 190, 80)),
    );
}

fn draw_trace(
    painter: &egui::Painter,
    graph_rect: Rect,
    samples: &[f32],
    color: Color32,
    settings: &OscilloscopeSettings,
) {
    let zoom_x = settings.zoom_x.clamp(1.0, 16.0);
    let zoom_y = settings.zoom_y.clamp(0.25, 4.0);
    let visible_count = ((samples.len() as f32) / zoom_x).round() as usize;
    let visible_count = visible_count.max(2).min(samples.len());
    let start = samples.len().saturating_sub(visible_count);
    let window = &samples[start..];

    if window.len() < 2 {
        return;
    }

    for i in 0..(window.len() - 1) {
        let t0 = i as f32 / (window.len() - 1) as f32;
        let t1 = (i + 1) as f32 / (window.len() - 1) as f32;
        let y0 = (window[i] * zoom_y).clamp(-1.2, 1.2);
        let y1 = (window[i + 1] * zoom_y).clamp(-1.2, 1.2);
        let p0 = Pos2::new(
            egui::lerp(graph_rect.x_range(), t0),
            egui::lerp(graph_rect.y_range(), (1.0 - ((y0 + 1.0) * 0.5)).clamp(0.0, 1.0)),
        );
        let p1 = Pos2::new(
            egui::lerp(graph_rect.x_range(), t1),
            egui::lerp(graph_rect.y_range(), (1.0 - ((y1 + 1.0) * 0.5)).clamp(0.0, 1.0)),
        );
        painter.line_segment([p0, p1], Stroke::new(1.5, color));
    }
}
