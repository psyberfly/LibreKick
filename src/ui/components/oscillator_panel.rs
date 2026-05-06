use nih_plug_egui::egui;

use crate::shared;

pub(crate) struct OscillatorPanelModel<'a> {
    pub(crate) waveform: &'a mut shared::Waveform,
    pub(crate) retrigger: &'a mut bool,
    pub(crate) legato_voice_steal: &'a mut bool,
    pub(crate) pitch_hz: Option<&'a mut f32>,
    pub(crate) note_length_ms: Option<&'a mut f32>,
}

pub(crate) fn render(ui: &mut egui::Ui, ui_scale: f32, model: OscillatorPanelModel<'_>) {
    ui.label("Waveform");
    ui.horizontal(|ui| {
        ui.selectable_value(model.waveform, shared::Waveform::Sine, "Sine");
        ui.selectable_value(model.waveform, shared::Waveform::Saw, "Saw");
        ui.selectable_value(model.waveform, shared::Waveform::Square, "Square");
    });

    if let Some(pitch_hz) = model.pitch_hz {
        ui.add_space(6.0 * ui_scale);
        ui.label("Pitch");
        let changed = ui
            .add(
                egui::Slider::new(pitch_hz, 20.0..=2000.0)
                    .text("Hz")
                    .logarithmic(true),
            )
            .changed();
        if changed {
            *pitch_hz = (*pitch_hz).clamp(20.0, 2_000.0);
        }
        ui.label(format!("{:.2}Hz", *pitch_hz));
    }

    if let Some(note_length_ms) = model.note_length_ms {
        ui.add_space(6.0 * ui_scale);
        ui.label("Note Length");
        let changed = ui
            .add(egui::Slider::new(note_length_ms, 1.0..=1000.0).text("ms"))
            .changed();
        if changed {
            *note_length_ms = (*note_length_ms).clamp(1.0, 1000.0);
        }
    }

    ui.add_space(6.0 * ui_scale);
    ui.checkbox(model.retrigger, "Retrigger");
    ui.checkbox(model.legato_voice_steal, "Legato (voice steal)");
}
