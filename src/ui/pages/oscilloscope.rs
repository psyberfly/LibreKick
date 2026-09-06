use nih_plug_egui::egui::{self, Color32, RichText};

use crate::{shared, ui::components::oscilloscope};

use super::super::state::BezierUiState;

pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    shared_for_ui: &shared::SharedStateHandle,
) {
    ui.add_space(8.0 * ui_scale);
    ui.heading("Oscilloscope");
    ui.label(
        RichText::new("Realtime kick, bass, and combined output traces")
            .italics()
            .small(),
    );
    ui.separator();

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.osc_hold, "Hold");
        ui.label("X Zoom");
        ui.add(egui::Slider::new(&mut state.osc_zoom_x, 1.0..=16.0).logarithmic(true));
        ui.label("Y Zoom");
        ui.add(egui::Slider::new(&mut state.osc_zoom_y, 0.25..=4.0).logarithmic(true));
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut state.osc_show_kick, "Kick");
        ui.checkbox(&mut state.osc_show_bass, "Bass");
        ui.checkbox(&mut state.osc_show_sum, "Total");
        if ui.button("Reset Zoom").clicked() {
            state.osc_zoom_x = 1.0;
            state.osc_zoom_y = 1.0;
        }
    });

    if !state.osc_hold {
        state.osc_snapshot = shared::oscilloscope_snapshot(shared_for_ui);
    }

    ui.label(format!(
        "Frame #{} • {} samples",
        state.osc_snapshot.sequence, state.osc_snapshot.len
    ));

    let len = state.osc_snapshot.len.min(shared::OSCILLOSCOPE_BUFFER_SIZE);
    let kick = &state.osc_snapshot.kick[..len];
    let bass = &state.osc_snapshot.bass[..len];
    let sum = &state.osc_snapshot.sum[..len];

    let traces = [
        oscilloscope::OscilloscopeTrace {
            label: "Kick",
            color: Color32::from_rgb(245, 170, 112),
            samples: kick,
            visible: state.osc_show_kick,
        },
        oscilloscope::OscilloscopeTrace {
            label: "Bass",
            color: Color32::from_rgb(96, 174, 255),
            samples: bass,
            visible: state.osc_show_bass,
        },
        oscilloscope::OscilloscopeTrace {
            label: "Total",
            color: Color32::from_rgb(114, 220, 129),
            samples: sum,
            visible: state.osc_show_sum,
        },
    ];

    oscilloscope::render(
        ui,
        ui_scale,
        &oscilloscope::OscilloscopeSettings {
            zoom_x: state.osc_zoom_x,
            zoom_y: state.osc_zoom_y,
        },
        &traces,
    );
}
