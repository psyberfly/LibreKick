use nih_plug_vizia::vizia::prelude::*;

use crate::ui_vizia::{AppScaffoldEvent, AppScaffoldModel, TuningStandard};

pub(crate) fn build_settings_page(cx: &mut Context) {
    VStack::new(cx, |cx| {
        Label::new(cx, "Settings").class("page-title");
        Label::new(cx, "Tuning and global options").class("page-subtitle");

        VStack::new(cx, |cx| {
            Label::new(cx, "Tuning Standard");

            HStack::new(cx, |cx| {
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::SetTuningStandard(TuningStandard::A440)),
                    |cx| Label::new(cx, "A440"),
                )
                .class("control-button");
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::SetTuningStandard(TuningStandard::A432)),
                    |cx| Label::new(cx, "A432"),
                )
                .class("control-button");
            })
            .class("control-row");

            Binding::new(cx, AppScaffoldModel::tuning_standard, |cx, tuning| {
                let text = match tuning.get(cx) {
                    TuningStandard::A440 => "Selected: A440 (440 Hz)",
                    TuningStandard::A432 => "Selected: A432 (432 Hz)",
                };
                Label::new(cx, text);
            });

            Label::new(cx, "Kick Max Note Length (ms)");
            HStack::new(cx, |cx| {
                Binding::new(cx, AppScaffoldModel::kick_max_note_length_ms, |cx, value| {
                    Label::new(cx, &format!("{:.0}", value.get(cx))).class("control-value");
                });
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::KickSetMaxNoteLengthMs(-100.0)),
                    |cx| Label::new(cx, "-"),
                )
                .class("control-button");
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::KickSetMaxNoteLengthMs(100.0)),
                    |cx| Label::new(cx, "+"),
                )
                .class("control-button");
            })
            .class("control-row");
        })
        .class("control-group");
    })
    .class("page-root");
}
