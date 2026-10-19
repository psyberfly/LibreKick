use nih_plug::prelude::{FloatParam, Param, ParamSetter};
use nih_plug_egui::egui::{self, Align2, Color32, Pos2, Sense, Stroke, Vec2};

use crate::ui::theme::{accent_color, pitch_env_color, themed_font, APP_THEME};

/// Sweep of the knob arc, centred on straight-up. Serum knobs leave a gap at
/// the bottom, so the usable travel is 270 degrees.
const SWEEP_DEGREES: f32 = 270.0;
const START_DEGREES: f32 = 90.0 + (360.0 - SWEEP_DEGREES) * 0.5;

/// How many screen pixels of vertical drag correspond to the full range.
const DRAG_PIXELS_FULL_RANGE: f32 = 180.0;
const DRAG_PIXELS_FINE_RANGE: f32 = 900.0;

#[derive(Clone, Copy)]
pub(crate) struct KnobStyle {
    pub(crate) diameter: f32,
    pub(crate) arc_color: Color32,
    /// Signed normalized modulation offset (-1..1). When Some, a Serum-style
    /// ring arc is drawn from the set value toward the modulated value.
    pub(crate) mod_arc: Option<f32>,
    /// Font size (unscaled points) for the label above and readout below.
    pub(crate) font: f32,
    /// Label colour override; `None` keeps the dimmed tick colour.
    pub(crate) label_color: Option<Color32>,
}

impl Default for KnobStyle {
    fn default() -> Self {
        Self {
            diameter: 38.0,
            arc_color: accent_color(),
            mod_arc: None,
            font: 10.0,
            label_color: None,
        }
    }
}

impl KnobStyle {
    /// The unified front-panel spec: bigger, brighter captions so dense
    /// module panels stay readable. All instrument pages use this.
    pub(crate) fn panel() -> Self {
        Self {
            font: 12.5,
            label_color: Some(APP_THEME.axis_title()),
            ..Self::default()
        }
    }
}

/// Knob diameter (unscaled points) that spreads `count` knobs evenly across
/// the row's available width so module panels do not sit half-empty. The
/// knob widget cell is `diameter` wide; the cap keeps sparse panels (one or
/// two knobs) from inflating into oversized dials that read as dead space.
pub(crate) fn fit_knob_diameter(ui: &egui::Ui, count: usize, ui_scale: f32) -> f32 {
    let count = count.max(1) as f32;
    let gap = ui.spacing().item_spacing.x;
    let cell = (ui.available_width() - gap * (count - 1.0)).max(0.0) / count;
    (cell / ui_scale).clamp(38.0, 52.0)
}

/// Draws a Serum-style rotary knob bound to a normalized `0.0..=1.0` value.
///
/// `label` is drawn above the knob and `display` below it. Returns true when
/// the value changed this frame.
pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    label: &str,
    value: &mut f32,
    display: &str,
    style: KnobStyle,
) -> bool {
    let diameter = style.diameter * ui_scale;
    // Text rows track the configured font so bigger labels keep their
    // breathing room instead of clipping into the dial.
    let label_height = (style.font + 3.0) * ui_scale;
    let value_height = (style.font + 3.0) * ui_scale;
    let total = Vec2::new(
        diameter.max(44.0 * ui_scale),
        label_height + diameter + value_height + 4.0 * ui_scale,
    );

    let (rect, response) = ui.allocate_exact_size(total, Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    let mut changed = false;
    if response.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        let span = if fine {
            DRAG_PIXELS_FINE_RANGE
        } else {
            DRAG_PIXELS_FULL_RANGE
        };
        let delta = -response.drag_delta().y / (span * ui_scale);
        if delta != 0.0 {
            *value = (*value + delta).clamp(0.0, 1.0);
            changed = true;
        }
    }
    if response.double_clicked() {
        *value = 0.5;
        changed = true;
    }

    let center = Pos2::new(rect.center().x, rect.top() + label_height + diameter * 0.5);
    let radius = diameter * 0.5;
    let track_radius = radius - 2.0 * ui_scale;

    painter.text(
        Pos2::new(rect.center().x, rect.top()),
        Align2::CENTER_TOP,
        label,
        themed_font(style.font * ui_scale),
        style.label_color.unwrap_or_else(|| APP_THEME.axis_tick()),
    );

    painter.circle_filled(center, radius, APP_THEME.screen_bg());
    painter.circle_stroke(center, radius, Stroke::new(1.0, APP_THEME.graph_border()));

    // Hand-drawn pencil shading, sparingly along the right rim: a few short
    // sketch arcs plus sparse radial hatch ticks read as pencil work rather
    // than a clean gradient, and make the dial look raised.
    let pencil = APP_THEME.pencil_shade();
    let rim = radius - 1.0 * ui_scale;
    for &(from, to, dr, w) in &[
        (-36.0f32, 4.0f32, 0.0, 1.4),
        (12.0f32, 46.0f32, -1.6, 1.1),
        (24.0f32, 42.0f32, -3.0, 0.9),
    ] {
        draw_arc_degrees(
            &painter,
            center + Vec2::new(0.5, 0.4) * ui_scale,
            rim + dr * ui_scale,
            from,
            to,
            Stroke::new(w * ui_scale, pencil),
        );
    }
    for &(deg, inner) in &[
        (-28.0f32, -5.5f32),
        (-10.0f32, -6.5),
        (10.0f32, -4.5),
        (28.0f32, -5.5),
        (43.0f32, -4.0),
    ] {
        let a = deg.to_radians();
        let dir = Vec2::new(a.cos(), a.sin());
        painter.line_segment(
            [
                center + dir * (radius + inner * ui_scale),
                center + dir * (radius - 0.5 * ui_scale),
            ],
            Stroke::new(1.0 * ui_scale, pencil),
        );
    }

    let normalized = value.clamp(0.0, 1.0);
    draw_arc(
        &painter,
        center,
        track_radius,
        0.0,
        1.0,
        Stroke::new(2.5 * ui_scale, APP_THEME.grid_line()),
    );
    // Glow pass first, then the hot phosphor trace.
    draw_arc(
        &painter,
        center,
        track_radius,
        0.0,
        normalized,
        Stroke::new(6.0 * ui_scale, APP_THEME.trace_glow()),
    );
    draw_arc(
        &painter,
        center,
        track_radius,
        0.0,
        normalized,
        Stroke::new(2.5 * ui_scale, style.arc_color),
    );

    // Serum-style modulation ring: the arc from the set value to the
    // currently-modulated value, drawn just outside the main track.
    if let Some(offset) = style.mod_arc {
        let target = (normalized + offset).clamp(0.0, 1.0);
        if (target - normalized).abs() > 0.002 {
            let (from, to) = if target >= normalized {
                (normalized, target)
            } else {
                (target, normalized)
            };
            draw_arc(
                &painter,
                center,
                track_radius + 3.5 * ui_scale,
                from,
                to,
                Stroke::new(2.0 * ui_scale, pitch_env_color()),
            );
        }
    }

    let pointer_angle = (START_DEGREES + normalized * SWEEP_DEGREES).to_radians();
    let pointer_dir = Vec2::new(pointer_angle.cos(), pointer_angle.sin());
    painter.line_segment(
        [
            center + pointer_dir * (radius * 0.35),
            center + pointer_dir * (track_radius - 3.0 * ui_scale),
        ],
        Stroke::new(2.0 * ui_scale, accent_color()),
    );

    painter.text(
        Pos2::new(rect.center().x, rect.bottom()),
        Align2::CENTER_BOTTOM,
        display,
        themed_font(style.font * ui_scale),
        APP_THEME.axis_title(),
    );

    changed
}

/// Knob bound to a plugin `FloatParam`, emitting proper automation gestures.
pub(crate) fn param_knob(
    ui: &mut egui::Ui,
    ui_scale: f32,
    label: &str,
    param: &FloatParam,
    setter: &ParamSetter,
    style: KnobStyle,
) -> bool {
    let mut value = param.modulated_normalized_value();
    let display = param.to_string();
    let before = value;
    let changed = render(ui, ui_scale, label, &mut value, &display, style);
    if changed && value != before {
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, value);
        setter.end_set_parameter(param);
    }
    changed
}

/// Knob over an arbitrary linear range, returning the value in real units.
pub(crate) fn range_knob(
    ui: &mut egui::Ui,
    ui_scale: f32,
    label: &str,
    value: &mut f32,
    min: f32,
    max: f32,
    display: &str,
    style: KnobStyle,
) -> bool {
    let span = (max - min).abs().max(f32::EPSILON);
    let mut normalized = ((*value - min) / span).clamp(0.0, 1.0);
    let changed = render(ui, ui_scale, label, &mut normalized, display, style);
    if changed {
        *value = (min + normalized * span).clamp(min.min(max), min.max(max));
    }
    changed
}

/// Knob over a small integer range (octave offsets, voice counts).
///
/// Rounding to the backing integer every frame would throw away any drag
/// smaller than half a step, which makes coarse ranges feel stuck. The
/// continuous position is therefore kept in egui memory under `id_source`
/// and only quantised on the way out, so fractional drag accumulates.
pub(crate) fn int_range_knob(
    ui: &mut egui::Ui,
    ui_scale: f32,
    id_source: &str,
    label: &str,
    value: &mut i32,
    min: i32,
    max: i32,
    display: &str,
    style: KnobStyle,
) -> bool {
    let span = (max - min).max(1) as f32;
    let memory_id = ui.make_persistent_id(id_source);

    let stored: Option<f32> = ui.data(|data| data.get_temp(memory_id));
    // Drop the cached position if the backing value moved elsewhere
    // (patch load, slot switch) so the knob follows the real value.
    let continuous = match stored {
        Some(cached) if cached.round() as i32 == *value => cached,
        _ => *value as f32,
    };

    let mut normalized = ((continuous - min as f32) / span).clamp(0.0, 1.0);
    let moved = render(ui, ui_scale, label, &mut normalized, display, style);
    if !moved {
        return false;
    }

    let next = min as f32 + normalized * span;
    ui.data_mut(|data| data.insert_temp(memory_id, next));

    let rounded = (next.round() as i32).clamp(min, max);
    if rounded != *value {
        *value = rounded;
        return true;
    }
    false
}

/// Knob over a logarithmic range, used for pitch and cutoff where equal
/// rotation should mean equal musical intervals.
pub(crate) fn log_range_knob(
    ui: &mut egui::Ui,
    ui_scale: f32,
    label: &str,
    value: &mut f32,
    min: f32,
    max: f32,
    display: &str,
    style: KnobStyle,
) -> bool {
    let min = min.max(f32::EPSILON);
    let max = max.max(min * 1.000_01);
    let ratio_log = (max / min).ln();
    let current = (*value).clamp(min, max);
    let mut normalized = ((current / min).ln() / ratio_log).clamp(0.0, 1.0);
    let changed = render(ui, ui_scale, label, &mut normalized, display, style);
    if changed {
        *value = (min * (normalized * ratio_log).exp()).clamp(min, max);
    }
    changed
}

fn draw_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    from: f32,
    to: f32,
    stroke: Stroke,
) {
    const SEGMENTS: usize = 48;
    if (to - from).abs() <= f32::EPSILON {
        return;
    }
    let mut points = Vec::with_capacity(SEGMENTS + 1);
    for i in 0..=SEGMENTS {
        let t = from + (to - from) * (i as f32 / SEGMENTS as f32);
        let angle = (START_DEGREES + t * SWEEP_DEGREES).to_radians();
        points.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
    }
    painter.add(egui::Shape::line(points, stroke));
}

/// Arc between absolute angles in degrees (0 = right, 90 = down), used for
/// decorative strokes that are placed by position rather than knob value.
fn draw_arc_degrees(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    from_deg: f32,
    to_deg: f32,
    stroke: Stroke,
) {
    const SEGMENTS: usize = 24;
    let mut points = Vec::with_capacity(SEGMENTS + 1);
    for i in 0..=SEGMENTS {
        let t = i as f32 / SEGMENTS as f32;
        let angle = (from_deg + (to_deg - from_deg) * t).to_radians();
        points.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
    }
    painter.add(egui::Shape::line(points, stroke));
}
