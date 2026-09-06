use nih_plug_vizia::vizia::prelude::*;

use crate::interface::CurveKind;

use super::super::components::signal_shaper::{
    build_signal_shaper, SignalShaperAxis, SignalShaperConfig, SignalShaperTheme,
};
use super::super::{
    format_amp_db, format_pitch_hz, format_time_ms, AppScaffoldEvent, AppScaffoldModel,
    TuningStandard,
};

pub(crate) fn build_kick_page(cx: &mut Context) {
    let amp_config = SignalShaperConfig {
        title: "Amplitude",
        axis: SignalShaperAxis {
            x_name: "Time",
            y_name: "Amount (dB)",
            x_formatter: format_time_ms,
            y_formatter: format_amp_db,
        },
        ..SignalShaperConfig::default()
    }
    .with_theme(SignalShaperTheme {
        curve: Color::rgb(245, 160, 88),
        point: Color::rgb(235, 108, 62),
        selected_point: Color::rgb(255, 198, 70),
        ..SignalShaperTheme::default()
    });
    let pitch_config = SignalShaperConfig {
        title: "Pitch",
        axis: SignalShaperAxis {
            x_name: "Time",
            y_name: "Pitch (Hz)",
            x_formatter: format_time_ms,
            y_formatter: format_pitch_hz,
        },
        ..SignalShaperConfig::default()
    }
    .with_theme(SignalShaperTheme {
        curve: Color::rgb(96, 172, 255),
        point: Color::rgb(88, 142, 220),
        selected_point: Color::rgb(176, 214, 255),
        ..SignalShaperTheme::default()
    });
    let amp_color = Color::rgb(245, 160, 88);
    let pitch_color = Color::rgb(96, 172, 255);

    VStack::new(cx, move |cx| {
        Label::new(cx, "Kick").class("page-title");
        Label::new(cx, "Kick controls and envelopes").class("page-subtitle");

        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                Label::new(cx, "Curve:");
                curve_button(cx, "Amplitude", CurveKind::Amplitude);
                curve_button(cx, "Pitch", CurveKind::Pitch);
                Label::new(cx, "Tuning:").left(Pixels(12.0));
                tuning_button(cx, "A=440", TuningStandard::A440);
                tuning_button(cx, "A=432", TuningStandard::A432);
            })
            .class("control-row");

            HStack::new(cx, |cx| {
                toggle_row(
                    cx,
                    "Keytrack",
                    AppScaffoldModel::kick_keytrack_enabled,
                    AppScaffoldEvent::KickToggleKeytrack,
                );
                step_row(
                    cx,
                    "Note Length",
                    AppScaffoldModel::kick_note_length_ms,
                    -10.0,
                    10.0,
                    AppScaffoldEvent::KickAdjustNoteLengthMs,
                );
                step_row(
                    cx,
                    "Max Note Length",
                    AppScaffoldModel::kick_max_note_length_ms,
                    -100.0,
                    100.0,
                    AppScaffoldEvent::KickAdjustMaxNoteLengthMs,
                );
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::KickTrigger),
                    |cx| Label::new(cx, "Trigger"),
                )
                .class("control-button");
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::KickShaperUndo),
                    |cx| Label::new(cx, "<"),
                )
                .class("control-button");
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::KickShaperRedo),
                    |cx| Label::new(cx, ">"),
                )
                .class("control-button");
            })
            .class("control-row");
        })
        .class("control-group");

        VStack::new(cx, |cx| {
            Label::new(cx, "Patches").class("page-subtitle");
            HStack::new(cx, |cx| {
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::PatchSave),
                    |cx| Label::new(cx, "Save Current"),
                )
                .class("control-button");
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::PatchRefreshList),
                    |cx| Label::new(cx, "Refresh"),
                )
                .class("control-button");
            })
            .class("control-row");

            Binding::new(cx, AppScaffoldModel::patches, |cx, patches| {
                let patches = patches.get(cx);
                VStack::new(cx, |cx| {
                    if patches.available.is_empty() {
                        Label::new(cx, "No saved patches.");
                    } else {
                        for name in patches.available.iter() {
                            let is_default = patches.default_name.as_deref() == Some(name);
                            let label = if is_default {
                                format!("{name} (default)")
                            } else {
                                name.clone()
                            };
                            HStack::new(cx, |cx| {
                                Label::new(cx, &label);
                                let load_name = name.to_string();
                                Button::new(
                                    cx,
                                    move |cx| cx.emit(AppScaffoldEvent::PatchLoad(load_name.clone())),
                                    |cx| Label::new(cx, "Load"),
                                )
                                .class("control-button");
                                let default_name = name.to_string();
                                Button::new(
                                    cx,
                                    move |cx| {
                                        cx.emit(AppScaffoldEvent::PatchLoad(default_name.clone()));
                                        cx.emit(AppScaffoldEvent::PatchSetDefault);
                                    },
                                    |cx| Label::new(cx, "Default"),
                                )
                                .class("control-button");
                            })
                            .class("control-row");
                        }
                    }
                    Label::new(cx, &patches.status);
                })
                .height(Auto);
            });
        })
        .class("control-group");

        Binding::new(cx, AppScaffoldModel::kick_active_curve, move |cx, _| {
            let model = cx.data::<AppScaffoldModel>().unwrap();
            let active = model.kick_active_curve;
            match active {
                CurveKind::Amplitude => Binding::new(cx, AppScaffoldModel::kick_amp_shaper, move |cx, state| {
                    let state = state.get(cx);
                    let secondary =
                        cx.data::<AppScaffoldModel>().map(|m| m.kick_pitch_shaper.clone());
                    build_signal_shaper(
                        cx,
                        amp_config,
                        &state,
                        secondary,
                        pitch_color,
                        AppScaffoldEvent::KickAmpShaper,
                    );
                }),
                CurveKind::Pitch => Binding::new(cx, AppScaffoldModel::kick_pitch_shaper, move |cx, state| {
                    let state = state.get(cx);
                    let secondary =
                        cx.data::<AppScaffoldModel>().map(|m| m.kick_amp_shaper.clone());
                    build_signal_shaper(
                        cx,
                        pitch_config,
                        &state,
                        secondary,
                        amp_color,
                        AppScaffoldEvent::KickPitchShaper,
                    );
                }),
            }
        });
    })
    .class("page-root");
}

fn curve_button(cx: &mut Context, label: &'static str, kind: CurveKind) {
    Button::new(
        cx,
        move |cx| cx.emit(AppScaffoldEvent::KickSetActiveCurve(kind)),
        move |cx| Label::new(cx, label),
    )
    .class("control-button");
}

fn tuning_button(cx: &mut Context, label: &'static str, tuning: TuningStandard) {
    Button::new(
        cx,
        move |cx| cx.emit(AppScaffoldEvent::SetTuningStandard(tuning)),
        move |cx| Label::new(cx, label),
    )
    .class("control-button");
}

fn toggle_row<L>(cx: &mut Context, label: &'static str, lens: L, event: AppScaffoldEvent)
where
    L: Lens<Target = bool> + Copy + 'static,
{
    HStack::new(cx, |cx| {
        Label::new(cx, label);
        Binding::new(cx, lens, move |cx, value| {
            let value = value.get(cx);
            let text = if value { "On" } else { "Off" };
            let ev = event.clone();
            Button::new(cx, move |cx| cx.emit(ev.clone()), move |cx| Label::new(cx, text))
                .class("control-button");
        });
    })
    .class("control-row");
}

fn step_row<L>(
    cx: &mut Context,
    label: &'static str,
    lens: L,
    step_down: f32,
    step_up: f32,
    event: fn(f32) -> AppScaffoldEvent,
) where
    L: Lens<Target = f32> + Copy + 'static,
{
    HStack::new(cx, |cx| {
        Label::new(cx, label);
        Binding::new(cx, lens, |cx, value| {
            Label::new(cx, &format!("{:.1}", value.get(cx))).class("control-value");
        });
        Button::new(
            cx,
            move |cx| cx.emit(event(step_down)),
            |cx| Label::new(cx, "-"),
        )
        .class("control-button");
        Button::new(
            cx,
            move |cx| cx.emit(event(step_up)),
            |cx| Label::new(cx, "+"),
        )
        .class("control-button");
    })
    .class("control-row");
}
