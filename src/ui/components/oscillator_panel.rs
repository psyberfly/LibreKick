use nih_plug::prelude::{FloatParam, ParamSetter};
use nih_plug_egui::egui;

use crate::shared;
use crate::ui::helpers::float_param_slider;

pub(crate) struct OscillatorPanelModel<'a> {
    pub(crate) waveform: &'a mut shared::Waveform,
    pub(crate) retrigger: &'a mut bool,
    pub(crate) legato_voice_steal: &'a mut bool,
    pub(crate) pitch_hz: Option<&'a mut f32>,
    pub(crate) note_length_ms: Option<&'a mut f32>,
    /// Optional level parameter (param + setter) rendered left of the pitch slider.
    pub(crate) level: Option<(&'a FloatParam, &'a ParamSetter<'a>)>,
}

pub(crate) fn render(ui: &mut egui::Ui, ui_scale: f32, model: OscillatorPanelModel<'_>) {
    ui.label("Waveform");
    ui.horizontal(|ui| {
        ui.selectable_value(model.waveform, shared::Waveform::Sine, "Sine");
        ui.selectable_value(model.waveform, shared::Waveform::Saw, "Saw");
        ui.selectable_value(model.waveform, shared::Waveform::Square, "Square");
    });

    if model.level.is_some() || model.pitch_hz.is_some() {
        ui.add_space(6.0 * ui_scale);
        ui.horizontal(|ui| {
            if let Some((param, setter)) = model.level {
                ui.vertical(|ui| {
                    ui.label("Level");
                    float_param_slider(ui, setter, param, "");
                });
            }
            if let Some(pitch_hz) = model.pitch_hz {
                ui.vertical(|ui| {
                    ui.label("Pitch");
                    ui.horizontal(|ui| {
                        let changed = ui
                            .add(crate::ui::helpers::slider_fine_step(
                                ui,
                                egui::Slider::new(pitch_hz, 20.0..=2000.0)
                                    .text("Hz")
                                    .logarithmic(true),
                                1.0,
                            ))
                            .changed();
                        if changed {
                            *pitch_hz = (*pitch_hz).clamp(20.0, 2_000.0);
                        }
                        ui.label(format!("{:.2}Hz", *pitch_hz));
                    });
                });
            }
        });
    }

    if let Some(note_length_ms) = model.note_length_ms {
        ui.add_space(6.0 * ui_scale);
        ui.label("Note Length");
        let changed = ui
            .add(crate::ui::helpers::slider_fine_step(
                ui,
                egui::Slider::new(note_length_ms, 1.0..=1000.0).text("ms"),
                1.0,
            ))
            .changed();
        if changed {
            *note_length_ms = (*note_length_ms).clamp(1.0, 1000.0);
        }
    }

    ui.add_space(6.0 * ui_scale);
    ui.checkbox(model.retrigger, "Retrigger");
    ui.checkbox(model.legato_voice_steal, "Legato (voice steal)");
}
