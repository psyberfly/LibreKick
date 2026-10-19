use nih_plug_egui::egui::{self, RichText};

use crate::ui::components::{knob, section};
use crate::ui::state::{BezierUiState, NoteSize};
use crate::ui::theme::{text_color, themed_font};

/// Renders the transport controls row as module panels: tempo, bars, note
/// size, and playback — same section/knob spec as the instrument pages.
pub(super) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    daw_tempo: Option<f64>,
) {
    let arr_knob = knob::KnobStyle::panel();

    section::weighted_columns(ui, ui_scale, [0.30, 0.20, 0.28, 0.22], |columns| {
        section::render(
            &mut columns[0],
            ui_scale,
            section::SectionHeader {
                title: "TEMPO",
                enabled: None,
                min_height: 0.0,
            },
            |ui| {
                // When DAW tempo sync is on and a tempo is reported, it wins —
                // the dial mirrors it and stays disabled.
                let synced = state.use_daw_tempo && daw_tempo.is_some();
                if synced {
                    state.manual_tempo = daw_tempo.unwrap() as f32;
                }
                let bpm_display = format!("{:.1} BPM", state.manual_tempo);
                ui.add_enabled_ui(!synced, |ui| {
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "BPM",
                        &mut state.manual_tempo,
                        20.0,
                        300.0,
                        &bpm_display,
                        arr_knob,
                    );
                });
                ui.add_space(4.0 * ui_scale);
                ui.horizontal(|ui| {
                    if daw_tempo.is_some() {
                        ui.checkbox(&mut state.use_daw_tempo, "DAW");
                    }
                    // Fixed 4/4 signature for now.
                    ui.label(
                        RichText::new("4/4")
                            .font(themed_font(11.0 * ui_scale))
                            .color(text_color()),
                    );
                });
            },
        );

        section::render(
            &mut columns[1],
            ui_scale,
            section::SectionHeader {
                title: "LOOP",
                enabled: None,
                min_height: 0.0,
            },
            |ui| {
                // Quarter-bar steps: the backing value is bars, the dial
                // counts quarter bars so each detent is one step.
                let mut quarter_bars = (state.num_bars * 4.0).round() as i32;
                let bars_display = format!("{:.2} bars", quarter_bars as f32 / 4.0);
                if knob::int_range_knob(
                    ui,
                    ui_scale,
                    "arrange-num-bars",
                    "BARS",
                    &mut quarter_bars,
                    1,
                    32,
                    &bars_display,
                    arr_knob,
                ) {
                    state.num_bars = (quarter_bars as f32 / 4.0).clamp(0.25, 8.0);
                }
            },
        );

        section::render(
            &mut columns[2],
            ui_scale,
            section::SectionHeader {
                title: "GRID",
                enabled: None,
                min_height: 0.0,
            },
            |ui| {
                let options: [(NoteSize, &str); NoteSize::ALL.len()] =
                    std::array::from_fn(|i| (NoteSize::ALL[i], NoteSize::ALL[i].label()));
                section::segmented(ui, ui_scale, &mut state.note_size, &options);
            },
        );

        section::render(
            &mut columns[3],
            ui_scale,
            section::SectionHeader {
                title: "PLAYBACK",
                enabled: None,
                min_height: 0.0,
            },
            |ui| {
                ui.checkbox(&mut state.arrange_override, "Override DAW MIDI");
                ui.label(
                    RichText::new("Hold any note to play this pattern")
                        .small()
                        .italics(),
                );
            },
        );
    });
}
