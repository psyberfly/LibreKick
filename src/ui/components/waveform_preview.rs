use nih_plug_egui::egui::{Color32, Painter, Pos2, Rect, Stroke};

pub(crate) fn draw(
    painter: &Painter,
    graph_rect: Rect,
    waveform_points: &[Pos2],
    midline_color: Color32,
    trace_color: Color32,
) {
    if let (Some(first), Some(last)) = (waveform_points.first(), waveform_points.last()) {
        let mid_y = graph_rect.center().y;
        painter.line_segment(
            [Pos2::new(first.x, mid_y), Pos2::new(last.x, mid_y)],
            Stroke::new(1.0, midline_color),
        );
    }

    for line in waveform_points.windows(2) {
        painter.line_segment([line[0], line[1]], Stroke::new(1.0, trace_color));
    }
}
