use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg::{LineCap, Paint, Path};

use crate::ui_vizia::{AppScaffoldEvent, AppScaffoldModel};

pub(crate) fn build_oscilloscope_page(cx: &mut Context) {
    VStack::new(cx, |cx| {
        Label::new(cx, "Oscilloscope").class("page-title");
        Label::new(cx, "Audio output visualization").class("page-subtitle");

        HStack::new(cx, |cx| {
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::OscilloscopeUpdate),
                |cx| Label::new(cx, "Refresh"),
            )
            .class("control-button");
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::ToggleOscHold),
                |cx| Label::new(cx, "Hold"),
            )
            .class("control-button");
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::AdjustOscZoomX(-0.5)),
                |cx| Label::new(cx, "X-"),
            )
            .class("control-button");
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::AdjustOscZoomX(0.5)),
                |cx| Label::new(cx, "X+"),
            )
            .class("control-button");
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::AdjustOscZoomY(-0.25)),
                |cx| Label::new(cx, "Y-"),
            )
            .class("control-button");
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::AdjustOscZoomY(0.25)),
                |cx| Label::new(cx, "Y+"),
            )
            .class("control-button");
        })
        .class("control-row");

        HStack::new(cx, |cx| {
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::ToggleOscShowKick),
                |cx| Label::new(cx, "Kick"),
            )
            .class("control-button");
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::ToggleOscShowBass),
                |cx| Label::new(cx, "Bass"),
            )
            .class("control-button");
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::ToggleOscShowSum),
                |cx| Label::new(cx, "Sum"),
            )
            .class("control-button");
        })
        .class("control-row");

        Binding::new(cx, AppScaffoldModel::oscilloscope_snapshot, |cx, _| {
            OscilloscopeGraph::new(cx)
                .height(Pixels(240.0))
                .width(Stretch(1.0))
                .class("control-group");
        });
    })
    .class("page-root");
}

struct OscilloscopeGraph;

impl OscilloscopeGraph {
    fn new(cx: &mut Context) -> Handle<Self> {
        Self.build(cx, |_| {})
    }
}

impl View for OscilloscopeGraph {
    fn element(&self) -> Option<&'static str> {
        Some("oscilloscope-graph")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();

        let mut bg_path = Path::new();
        bg_path.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.fill_path(&bg_path, &Paint::color(Color::rgb(19, 23, 27).into()));

        let model = if let Some(model) = cx.data::<AppScaffoldModel>() {
            model
        } else {
            return;
        };

        let snapshot = &model.oscilloscope_snapshot;
        let cfg = &model.oscilloscope;
        if snapshot.len == 0 {
            return;
        }

        let signals: [(&[f32], Color, bool); 3] = [
            (
                &snapshot.kick[..snapshot.len],
                Color::rgb(245, 160, 88),
                cfg.show_kick,
            ),
            (
                &snapshot.bass[..snapshot.len],
                Color::rgb(96, 172, 255),
                cfg.show_bass,
            ),
            (
                &snapshot.sum[..snapshot.len],
                Color::rgb(220, 220, 220),
                cfg.show_sum,
            ),
        ];

        for (signal, color, visible) in signals.iter() {
            if !*visible || signal.len() < 2 {
                continue;
            }
            let mut path = Path::new();
            let points_to_draw = (signal.len() as f32 / cfg.zoom_x).min(signal.len() as f32) as usize;
            let step_width = bounds.w / points_to_draw.max(1) as f32;
            for (i, &sample) in signal.iter().take(points_to_draw).enumerate() {
                let x = bounds.x + i as f32 * step_width;
                let y = bounds.y
                    + bounds.h * 0.5
                    + (sample * cfg.zoom_y).clamp(-1.0, 1.0) * bounds.h * 0.45;
                if i == 0 {
                    path.move_to(x, y);
                } else {
                    path.line_to(x, y);
                }
            }
            let mut paint = Paint::color((*color).into());
            paint.set_line_width(1.5);
            paint.set_line_cap(LineCap::Round);
            canvas.stroke_path(&path, &paint);
        }
    }
}
