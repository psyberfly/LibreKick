use nih_plug_vizia::vizia::prelude::*;

use crate::interface::{BassFilterMode, Waveform};

use super::super::components::signal_shaper::{
    build_signal_shaper, SignalShaperConfig, SignalShaperTheme,
};
use super::super::{AppScaffoldEvent, AppScaffoldModel};

pub(crate) fn build_bass_page(cx: &mut Context) {
    let amp_config = SignalShaperConfig {
        title: "Bass Amp Signal Shaper",
        ..SignalShaperConfig::default()
    }
    .with_theme(SignalShaperTheme {
        curve: Color::rgb(245, 160, 88),
        point: Color::rgb(235, 108, 62),
        selected_point: Color::rgb(255, 198, 70),
        ..SignalShaperTheme::default()
    });
    let filter_config = SignalShaperConfig {
        title: "Filter Signal Shaper",
        ..SignalShaperConfig::default()
    }
    .with_theme(SignalShaperTheme {
        curve: Color::rgb(160, 220, 130),
        point: Color::rgb(118, 180, 98),
        selected_point: Color::rgb(190, 242, 172),
        ..SignalShaperTheme::default()
    });

    VStack::new(cx, |cx| {
        Label::new(cx, "Bass").class("page-title");
        Label::new(cx, "Bass oscillator and filter controls").class("page-subtitle");

        VStack::new(cx, |cx| {
            Label::new(cx, "Oscillator");
            waveform_picker_bass(cx);
            toggle_row(
                cx,
                "Retrigger",
                AppScaffoldModel::bass_retrigger,
                AppScaffoldEvent::BassToggleRetrigger,
            );
            toggle_row(
                cx,
                "Legato Voice Steal",
                AppScaffoldModel::bass_legato_voice_steal,
                AppScaffoldEvent::BassToggleLegatoVoiceSteal,
            );
            step_row(
                cx,
                "Note Length",
                AppScaffoldModel::bass_note_length_ms,
                -10.0,
                10.0,
                AppScaffoldEvent::BassAdjustNoteLengthMs,
            );
        })
        .class("control-group");

        VStack::new(cx, |cx| {
            Label::new(cx, "Filter");
            filter_mode_picker(cx);
            step_row(
                cx,
                "Cutoff (Hz)",
                AppScaffoldModel::bass_cutoff_hz,
                -25.0,
                25.0,
                AppScaffoldEvent::BassAdjustCutoffHz,
            );
            step_row(
                cx,
                "Pitch (Hz)",
                AppScaffoldModel::bass_pitch_hz,
                -1.0,
                1.0,
                AppScaffoldEvent::BassAdjustPitchHz,
            );
        })
        .class("control-group");

        Binding::new(cx, AppScaffoldModel::bass_amp_shaper, move |cx, state| {
            let state = state.get(cx);
            build_signal_shaper(cx, amp_config, &state, None, Color::default(), AppScaffoldEvent::BassAmpShaper);
        });
        Binding::new(cx, AppScaffoldModel::bass_filter_shaper, move |cx, state| {
            let state = state.get(cx);
            build_signal_shaper(cx, filter_config, &state, None, Color::default(), AppScaffoldEvent::BassFilterShaper);
        });
    })
    .class("page-root");
}

fn waveform_picker_bass(cx: &mut Context) {
    HStack::new(cx, |cx| {
        Label::new(cx, "Waveform");
        waveform_button(cx, "Sine", Waveform::Sine);
        waveform_button(cx, "Saw", Waveform::Saw);
        waveform_button(cx, "Square", Waveform::Square);
    })
    .class("control-row");
}

fn waveform_button(cx: &mut Context, label: &'static str, waveform: Waveform) {
    Button::new(
        cx,
        move |cx| cx.emit(AppScaffoldEvent::BassSetWaveform(waveform)),
        move |cx| Label::new(cx, label),
    )
    .class("control-button");
}

fn filter_mode_picker(cx: &mut Context) {
    HStack::new(cx, |cx| {
        Label::new(cx, "Mode");
        filter_mode_button(cx, "Low", BassFilterMode::LowPass);
        filter_mode_button(cx, "High", BassFilterMode::HighPass);
        filter_mode_button(cx, "Band", BassFilterMode::BandPass);
    })
    .class("control-row");
}

fn filter_mode_button(cx: &mut Context, label: &'static str, mode: BassFilterMode) {
    Button::new(
        cx,
        move |cx| cx.emit(AppScaffoldEvent::BassSetFilterMode(mode)),
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
