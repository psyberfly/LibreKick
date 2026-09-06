use nih_plug_vizia::vizia::prelude::*;

use crate::ui_vizia::{AppScaffoldEvent, AppScaffoldModel};

pub(crate) fn build_logs_page(cx: &mut Context) {
    VStack::new(cx, |cx| {
        Label::new(cx, "Logs").class("page-title");
        Label::new(cx, "Application log output").class("page-subtitle");

        HStack::new(cx, |cx| {
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::LogsRefresh),
                |cx| Label::new(cx, "Refresh"),
            )
            .class("control-button");
            Button::new(
                cx,
                |cx| cx.emit(AppScaffoldEvent::LogsClear),
                |cx| Label::new(cx, "Clear"),
            )
            .class("control-button");
        })
        .class("control-row");

        Binding::new(cx, AppScaffoldModel::logs, |cx, log_model| {
            let log_model = log_model.get(cx);
            VStack::new(cx, |cx| {
                if log_model.snapshot.lines.is_empty() {
                    Label::new(cx, "No log messages.");
                } else {
                    for line in log_model.snapshot.lines.iter() {
                        Label::new(cx, line);
                    }
                }
            })
            .height(Auto)
            .class("control-group");
        });
    })
    .class("page-root");
}
