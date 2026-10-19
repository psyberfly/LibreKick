use nih_plug_egui::egui::{Color32, Painter, Pos2, Rect, Stroke};

use super::energy_heatmap::{self, EnergyMap};

/// How far the fading trail reaches back from the playhead, in points.
const TRAIL_LENGTH: f32 = 86.0;

/// What the preview needs in order to animate the signal.
pub(crate) struct LiveSignal<'a> {
    /// Normalised playback position through the note, 0..1.
    ///
    /// This is the one value that cannot come from the signal: a static
    /// buffer has no notion of "now", so the time cursor has to come from
    /// the transport. Everything drawn *at* that position is measured.
    pub phase: f32,
    /// Fraction of the graph width the note spans at the current zoom.
    pub display_length_t: f32,
    /// Analysis of the very same samples the trace was drawn from.
    pub map: &'a EnergyMap,
}

/// Animates the signal over its own waveform, in real time.
///
/// Every visual property is generated from the analysed samples, so the
/// result is a structural picture of the sound rather than a readout of
/// parameter values:
///
/// - each trail segment is coloured by the spectral centroid measured at
///   that segment's own point in time, so the colour gradient along the
///   trail *is* the signal's changing tone,
/// - opacity follows measured energy, so quiet passages fade out,
/// - the dot's size follows measured energy at the cursor.
///
/// Drawn on top of the static trace from [`draw`], so the underlying
/// waveform stays visible and only the moving region is recoloured.
pub(crate) fn draw_live_signal(
    painter: &Painter,
    graph_rect: Rect,
    waveform_points: &[Pos2],
    live: &LiveSignal,
    ui_scale: f32,
) {
    if waveform_points.len() < 2 {
        return;
    }

    let phase = live.phase.clamp(0.0, 1.0);
    let span = live.display_length_t.clamp(0.0, 1.0);
    if span <= 0.0 {
        return;
    }
    let head_x = graph_rect.left() + phase * span * graph_rect.width();

    // Normalised time of a screen x, for looking up the analysis. The trace
    // spans `span` of the graph width, so this inverts the same mapping the
    // waveform points were built with.
    let time_at_x =
        |x: f32| ((x - graph_rect.left()) / (graph_rect.width() * span)).clamp(0.0, 1.0);

    let signal_color_at =
        |t: f32| energy_heatmap::signal_color(live.map.brightness_at(t), live.map.level_at(t));

    // Trail: recolour the stretch of trace behind the playhead. Each segment
    // takes the colour of the signal at its own instant, so the trail shows
    // how the tone evolved, not just where the cursor is. Squaring the fade
    // keeps the glow tight to the head.
    let trail = (TRAIL_LENGTH * ui_scale).max(1.0);
    for line in waveform_points.windows(2) {
        let x = line[1].x;
        if x > head_x || x < head_x - trail {
            continue;
        }
        let fade = ((x - (head_x - trail)) / trail).clamp(0.0, 1.0);
        painter.line_segment(
            [line[0], line[1]],
            Stroke::new(
                1.5 * ui_scale,
                signal_color_at(time_at_x(x)).gamma_multiply(fade * fade),
            ),
        );
    }

    let head_time = time_at_x(head_x);
    let head_color = signal_color_at(head_time);
    let head_level = live.map.level_at(head_time);

    painter.vline(
        head_x,
        graph_rect.y_range(),
        Stroke::new(1.0, head_color.gamma_multiply(0.4)),
    );

    // Dot rides the actual trace, so it tracks the rendered output rather
    // than floating over it, and swells with the measured energy there.
    if let Some(dot_y) = trace_y_at(waveform_points, head_x) {
        let dot = Pos2::new(head_x, dot_y);
        let radius = (2.0 + 2.6 * head_level) * ui_scale;
        painter.circle_filled(dot, radius * 2.6, head_color.gamma_multiply(0.28));
        painter.circle_filled(dot, radius, head_color);
    }
}

/// Height of the drawn trace at `x`, interpolated within the segment that
/// straddles it. Returns `None` when `x` falls outside the trace.
fn trace_y_at(points: &[Pos2], x: f32) -> Option<f32> {
    let first = points.first()?;
    let last = points.last()?;
    if x <= first.x {
        return Some(first.y);
    }
    if x >= last.x {
        return Some(last.y);
    }

    for line in points.windows(2) {
        let (a, b) = (line[0], line[1]);
        if x >= a.x && x <= b.x {
            let width = b.x - a.x;
            if width <= f32::EPSILON {
                return Some(a.y);
            }
            let t = (x - a.x) / width;
            return Some(a.y + (b.y - a.y) * t);
        }
    }
    None
}

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
