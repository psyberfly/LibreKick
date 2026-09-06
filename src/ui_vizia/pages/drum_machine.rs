use nih_plug_vizia::vizia::prelude::*;

use crate::ui_vizia::{AppScaffoldEvent, AppScaffoldModel};

pub(crate) fn build_drum_machine_page(cx: &mut Context) {
    VStack::new(cx, |cx| {
        Label::new(cx, "Drum Machine").class("page-title");
        Label::new(cx, "Bounce and arrange patterns").class("page-subtitle");

        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                Label::new(cx, "Tempo BPM");
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::DrumMachineSetBpm(-5.0)),
                    |cx| Label::new(cx, "-"),
                )
                .class("control-button");
                Binding::new(cx, AppScaffoldModel::drum_machine, |cx, dm| {
                    Label::new(cx, &format!("{:.1}", dm.get(cx).tempo_bpm)).class("control-value");
                });
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::DrumMachineSetBpm(5.0)),
                    |cx| Label::new(cx, "+"),
                )
                .class("control-button");
            })
            .class("control-row");

            HStack::new(cx, |cx| {
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::DrumMachineBounceToTrack),
                    |cx| Label::new(cx, "Bounce to Track"),
                )
                .class("control-button");
                Button::new(
                    cx,
                    |cx| cx.emit(AppScaffoldEvent::DrumMachineBounceToDesktop),
                    |cx| Label::new(cx, "Bounce to Desktop"),
                )
                .class("control-button");
            })
            .class("control-row");

            Binding::new(cx, AppScaffoldModel::drum_machine, |cx, dm| {
                let dm = dm.get(cx);
                Label::new(cx, &dm.status);
            });

            Binding::new(cx, AppScaffoldModel::drum_machine, |cx, dm| {
                let dm = dm.get(cx);
                VStack::new(cx, |cx| {
                    for (track_index, track) in dm.tracks.iter().enumerate() {
                        HStack::new(cx, |cx| {
                            Label::new(cx, &track.name);
                            for (step_index, step) in track.steps.iter().enumerate() {
                                let label = if *step { "●" } else { "○" };
                                Button::new(
                                    cx,
                                    move |cx| {
                                        cx.emit(AppScaffoldEvent::DrumMachineToggleStep {
                                            track: track_index,
                                            step: step_index,
                                        })
                                    },
                                    move |cx| Label::new(cx, label),
                                )
                                .class("control-button");
                            }
                            Button::new(
                                cx,
                                move |cx| {
                                    cx.emit(AppScaffoldEvent::DrumMachineRemoveTrack(track_index))
                                },
                                |cx| Label::new(cx, "X"),
                            )
                            .class("control-button");
                        })
                        .class("control-row");
                    }
                })
                .height(Auto);
            });
        })
        .class("control-group");
    })
    .class("page-root");
}
