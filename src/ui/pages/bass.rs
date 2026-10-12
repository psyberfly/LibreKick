use nih_plug_egui::egui::{self, Align2, Color32, Pos2, Sense, Stroke, Vec2};

use nih_plug::prelude::ParamSetter;

use crate::ui::{
    components::{envelope_editor, knob, section, waveform_preview},
    helpers::{
        axis_x_label, curve_lut, effective_waveform_zoom, note_name_from_hz,
        waveform_preview_points,
    },
    state::{BezierUiState, EditorSnapshot},
    theme::{accent_color, crt_overlay, phosphor_color, themed_font, APP_THEME},
};
use crate::{audio, shared, LibreKickParams};

const AXIS_SUBDIVISIONS: usize = 10;

pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    shared_for_ui: &shared::SharedStateHandle,
    params: &LibreKickParams,
    setter: &ParamSetter,
    snapshot_before: &EditorSnapshot,
) {
    // Latest modulation frame from the audio thread; drives playheads, LFO
    // dots and knob rings. While a voice sounds we keep repainting.
    let telemetry = shared::bass_mod_telemetry(shared_for_ui);
    if telemetry.active {
        ui.ctx().request_repaint();
    }

    ui.add_space(8.0 * ui_scale);
    ui.horizontal(|ui| {
        ui.heading("Bass");
        ui.add_space(8.0 * ui_scale);
        // Note slot selectors: click/right-click selects, double-click on
        // Note 2 initializes it as a duplicate of Note 1.
        for index in 0..state.bass.len() {
            let selected = state.bass_selected == index;
            let fill = if selected {
                accent_color()
            } else {
                APP_THEME.graph_bg()
            };
            let size = Vec2::splat(24.0 * ui_scale);
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            ui.painter().rect_filled(
                rect,
                2.0,
                fill,
            );
            ui.painter().rect_stroke(
                rect,
                2.0,
                Stroke::new(1.0, APP_THEME.grid_line()),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                format!("{}", index + 1),
                themed_font(11.0 * ui_scale),
                if selected {
                    Color32::WHITE
                } else {
                    APP_THEME.axis_tick()
                },
            );
            let response = response.on_hover_text(format!("Note {}", index + 1));
            if response.double_clicked() && index == 1 {
                state.bass[1] = state.bass[0].clone();
                state.bass_selected = 1;
            } else if response.clicked() || response.secondary_clicked() {
                state.bass_selected = index;
            }
        }
    });
    ui.separator();

    let sel = state.bass_selected.min(state.bass.len() - 1);
    let live = telemetry.active && telemetry.slot == sel;
    let live_phase = live.then_some(telemetry.env_phase);

    // Knob mod-ring offsets, evaluated from the telemetry frame. Gated on
    // `live` so idle knobs do not show phantom movement. The matrix and pitch
    // are copied out so this closure does not hold a borrow on `state`.
    let ring_matrix = state.bass[sel].mod_matrix;
    let ring_pitch = state.bass[sel].pitch_hz;
    let ring = move |dest: shared::ModDest| -> Option<f32> {
        if !live {
            return None;
        }
        mod_ring_offset(dest, &ring_matrix, &telemetry, ring_pitch)
    };
    let ring_style = |dest: shared::ModDest, diameter: f32| knob::KnobStyle {
        diameter,
        mod_arc: ring(dest),
        ..knob::KnobStyle::default()
    };

    let mut envelope_drag_active = false;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
    ui.columns(2, |columns| {
        section::render(
            &mut columns[0],
            ui_scale,
            section::SectionHeader {
                title: "OSC A",
                enabled: None,
                min_height: 220.0 * ui_scale,
            },
            |ui| {
                section::segmented(
                    ui,
                    ui_scale,
                    &mut state.bass[sel].oscillator_waveform,
                    &[
                        (shared::Waveform::Sine, "SIN"),
                        (shared::Waveform::Saw, "SAW"),
                        (shared::Waveform::Square, "SQR"),
                    ],
                );

                ui.add_space(6.0 * ui_scale);
                let dial = fit_knob_diameter(ui, 5, ui_scale);
                ui.horizontal(|ui| {
                    let level_param = if sel == 0 {
                        &params.bass1_level
                    } else {
                        &params.bass2_level
                    };
                    knob::param_knob(
                        ui,
                        ui_scale,
                        "LEVEL",
                        level_param,
                        setter,
                        knob::KnobStyle {
                            diameter: dial,
                            ..knob::KnobStyle::default()
                        },
                    );

                    let note = note_name_from_hz(
                        state.bass[sel].pitch_hz,
                        state.tuning_standard.a4_hz(),
                    );
                    let pitch_display = format!("{:.0}Hz {note}", state.bass[sel].pitch_hz);
                    knob::log_range_knob(
                        ui,
                        ui_scale,
                        "PITCH",
                        &mut state.bass[sel].pitch_hz,
                        20.0,
                        2_000.0,
                        &pitch_display,
                        ring_style(shared::ModDest::Pitch, dial),
                    );

                    let phase_display = format!("{:.0}\u{00b0}", state.bass[sel].phase_deg);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "PHASE",
                        &mut state.bass[sel].phase_deg,
                        0.0,
                        360.0,
                        &phase_display,
                        knob::KnobStyle {
                            diameter: dial,
                            ..knob::KnobStyle::default()
                        },
                    );

                    let length_display = format!("{:.0} ms", state.bass[sel].note_length_ms);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "LENGTH",
                        &mut state.bass[sel].note_length_ms,
                        1.0,
                        1_000.0,
                        &length_display,
                        knob::KnobStyle {
                            diameter: dial,
                            ..knob::KnobStyle::default()
                        },
                    );

                    let osc_a_display = format!("{:.0}%", state.bass[sel].osc_a_level * 100.0);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "MIX",
                        &mut state.bass[sel].osc_a_level,
                        0.0,
                        1.0,
                        &osc_a_display,
                        ring_style(shared::ModDest::OscAMix, dial),
                    );
                });

                ui.add_space(6.0 * ui_scale);
                let dial = fit_knob_diameter(ui, 3, ui_scale);
                ui.horizontal(|ui| {
                    let voices_display = format!("{}", state.bass[sel].osc_a_unison);
                    let voices_id = format!("bass-osc-a-unison-{sel}");
                    knob::int_range_knob(
                        ui,
                        ui_scale,
                        &voices_id,
                        "VOICES",
                        &mut state.bass[sel].osc_a_unison,
                        1,
                        8,
                        &voices_display,
                        knob::KnobStyle {
                            diameter: dial,
                            ..knob::KnobStyle::default()
                        },
                    );

                    let detune_display =
                        format!("{:.0}c", state.bass[sel].osc_a_unison_detune);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "DETUNE",
                        &mut state.bass[sel].osc_a_unison_detune,
                        0.0,
                        100.0,
                        &detune_display,
                        ring_style(shared::ModDest::UnisonDetune, dial),
                    );

                    let blend_display =
                        format!("{:.0}%", state.bass[sel].osc_a_unison_blend * 100.0);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "BLEND",
                        &mut state.bass[sel].osc_a_unison_blend,
                        0.0,
                        1.0,
                        &blend_display,
                        knob::KnobStyle {
                            diameter: dial,
                            ..knob::KnobStyle::default()
                        },
                    );
                });

                ui.add_space(6.0 * ui_scale);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.bass[sel].keytrack_enabled, "Keytrack");
                    ui.checkbox(&mut state.bass[sel].retrigger, "Retrig");
                    ui.checkbox(&mut state.bass[sel].legato_voice_steal, "Legato");
                });
            },
        );

        section::render(
            &mut columns[1],
            ui_scale,
            section::SectionHeader {
                title: "FILTER",
                enabled: None,
                min_height: 220.0 * ui_scale,
            },
            |ui| {
            section::segmented(
                ui,
                ui_scale,
                &mut state.bass[sel].filter_mode,
                &[
                    (shared::BassFilterMode::LowPass, "LP"),
                    (shared::BassFilterMode::HighPass, "HP"),
                    (shared::BassFilterMode::BandPass, "BP"),
                ],
            );
            ui.add_space(4.0 * ui_scale);
            section::segmented(
                ui,
                ui_scale,
                &mut state.bass[sel].filter_slope,
                &[
                    (shared::BassFilterSlope::S6dB, "6dB"),
                    (shared::BassFilterSlope::S12dB, "12dB"),
                    (shared::BassFilterSlope::S18dB, "18dB"),
                    (shared::BassFilterSlope::S24dB, "24dB"),
                ],
            );
            ui.add_space(6.0 * ui_scale);
            let dial = fit_knob_diameter(ui, 2, ui_scale);
            ui.horizontal(|ui| {
                // While a note plays, the readout shows the modulated value
                // (telemetry) rather than the static knob setting.
                let cutoff_display = if live && telemetry.cutoff_hz > 0.0 {
                    format!("{:.0} Hz", telemetry.cutoff_hz)
                } else {
                    format!("{:.0} Hz", state.bass[sel].cutoff_hz)
                };
                knob::log_range_knob(
                    ui,
                    ui_scale,
                    "CUTOFF",
                    &mut state.bass[sel].cutoff_hz,
                    20.0,
                    8_000.0,
                    &cutoff_display,
                    ring_style(shared::ModDest::Cutoff, dial),
                );

                let drive_display = format!("{:.0}%", state.bass[sel].filter_drive * 100.0);
                knob::range_knob(
                    ui,
                    ui_scale,
                    "DRIVE",
                    &mut state.bass[sel].filter_drive,
                    0.0,
                    1.0,
                    &drive_display,
                    ring_style(shared::ModDest::FilterDrive, dial),
                );
            });

            },
        );
    });

    ui.add_space(8.0 * ui_scale);
    ui.columns(3, |columns| {
        // The enable flags are staged through locals so the header's mutable
        // borrow does not overlap with the body closure's capture of `state`.
        let mut osc_b_enabled = state.bass[sel].osc_b_enabled;
        let mut sub_enabled = state.bass[sel].sub_enabled;
        let mut noise_enabled = state.bass[sel].noise_enabled;

        section::render(
            &mut columns[0],
            ui_scale,
            section::SectionHeader {
                title: "OSC B",
                enabled: Some(&mut osc_b_enabled),
                min_height: 150.0 * ui_scale,
            },
            |ui| {
                section::segmented(
                    ui,
                    ui_scale,
                    &mut state.bass[sel].osc_b_waveform,
                    &[
                        (shared::Waveform::Sine, "SIN"),
                        (shared::Waveform::Saw, "SAW"),
                        (shared::Waveform::Square, "SQR"),
                    ],
                );
                ui.add_space(6.0 * ui_scale);
                let dial = fit_knob_diameter(ui, 6, ui_scale);
                ui.horizontal(|ui| {
                    let level_display = format!("{:.0}%", state.bass[sel].osc_b_level * 100.0);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "MIX",
                        &mut state.bass[sel].osc_b_level,
                        0.0,
                        1.0,
                        &level_display,
                        ring_style(shared::ModDest::OscBMix, dial),
                    );

                    let octave_display = format!("{:+}", state.bass[sel].osc_b_octave);
                    let octave_id = format!("bass-osc-b-octave-{sel}");
                    knob::int_range_knob(
                        ui,
                        ui_scale,
                        &octave_id,
                        "OCT",
                        &mut state.bass[sel].osc_b_octave,
                        -2,
                        2,
                        &octave_display,
                        knob::KnobStyle {
                            diameter: dial,
                            ..knob::KnobStyle::default()
                        },
                    );

                    let detune_display =
                        format!("{:+.0}c", state.bass[sel].osc_b_detune_cents);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "DETUNE",
                        &mut state.bass[sel].osc_b_detune_cents,
                        -50.0,
                        50.0,
                        &detune_display,
                        ring_style(shared::ModDest::OscBDetune, dial),
                    );

                    let voices_display = format!("{}", state.bass[sel].osc_b_unison);
                    let voices_id = format!("bass-osc-b-unison-{sel}");
                    knob::int_range_knob(
                        ui,
                        ui_scale,
                        &voices_id,
                        "VOICES",
                        &mut state.bass[sel].osc_b_unison,
                        1,
                        8,
                        &voices_display,
                        knob::KnobStyle {
                            diameter: dial,
                            ..knob::KnobStyle::default()
                        },
                    );

                    let spread_display =
                        format!("{:.0}c", state.bass[sel].osc_b_unison_detune);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "SPREAD",
                        &mut state.bass[sel].osc_b_unison_detune,
                        0.0,
                        100.0,
                        &spread_display,
                        ring_style(shared::ModDest::UnisonDetune, dial),
                    );

                    let blend_display =
                        format!("{:.0}%", state.bass[sel].osc_b_unison_blend * 100.0);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "BLEND",
                        &mut state.bass[sel].osc_b_unison_blend,
                        0.0,
                        1.0,
                        &blend_display,
                        knob::KnobStyle {
                            diameter: dial,
                            ..knob::KnobStyle::default()
                        },
                    );
                });
            },
        );

        section::render(
            &mut columns[1],
            ui_scale,
            section::SectionHeader {
                title: "SUB",
                enabled: Some(&mut sub_enabled),
                min_height: 150.0 * ui_scale,
            },
            |ui| {
                section::segmented(
                    ui,
                    ui_scale,
                    &mut state.bass[sel].sub_waveform,
                    &[
                        (shared::Waveform::Sine, "SIN"),
                        (shared::Waveform::Saw, "SAW"),
                        (shared::Waveform::Square, "SQR"),
                    ],
                );
                ui.add_space(6.0 * ui_scale);
                let dial = fit_knob_diameter(ui, 2, ui_scale);
                ui.horizontal(|ui| {
                    let level_display = format!("{:.0}%", state.bass[sel].sub_level * 100.0);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "MIX",
                        &mut state.bass[sel].sub_level,
                        0.0,
                        1.0,
                        &level_display,
                        ring_style(shared::ModDest::SubMix, dial),
                    );

                    let octave_display = format!("{:+}", state.bass[sel].sub_octave);
                    let octave_id = format!("bass-sub-octave-{sel}");
                    knob::int_range_knob(
                        ui,
                        ui_scale,
                        &octave_id,
                        "OCT",
                        &mut state.bass[sel].sub_octave,
                        -3,
                        0,
                        &octave_display,
                        knob::KnobStyle {
                            diameter: dial,
                            ..knob::KnobStyle::default()
                        },
                    );
                });
            },
        );

        section::render(
            &mut columns[2],
            ui_scale,
            section::SectionHeader {
                title: "NOISE",
                enabled: Some(&mut noise_enabled),
                min_height: 150.0 * ui_scale,
            },
            |ui| {
                ui.add_space(6.0 * ui_scale);
                let level_display = format!("{:.0}%", state.bass[sel].noise_level * 100.0);
                knob::range_knob(
                    ui,
                    ui_scale,
                    "MIX",
                    &mut state.bass[sel].noise_level,
                    0.0,
                    1.0,
                    &level_display,
                    ring_style(
                        shared::ModDest::NoiseMix,
                        fit_knob_diameter(ui, 1, ui_scale),
                    ),
                );
            },
        );

        state.bass[sel].osc_b_enabled = osc_b_enabled;
        state.bass[sel].sub_enabled = sub_enabled;
        state.bass[sel].noise_enabled = noise_enabled;
    });

    ui.add_space(8.0 * ui_scale);
    ui.columns(2, |columns| {
        // LFO module: a 4-way tab switches which LFO is being edited; the
        // power pill toggles that LFO on and off.
        let lfo_index = state.bass[sel].lfo_selected.min(shared::BASS_LFO_COUNT - 1);
        let mut lfo_enabled = state.bass[sel].lfos[lfo_index].enabled;
        section::render(
            &mut columns[0],
            ui_scale,
            section::SectionHeader {
                title: "LFO",
                enabled: Some(&mut lfo_enabled),
                min_height: 200.0 * ui_scale,
            },
            |ui| {
                section::segmented(
                    ui,
                    ui_scale,
                    &mut state.bass[sel].lfo_selected,
                    &[(0usize, "1"), (1usize, "2"), (2usize, "3"), (3usize, "4")],
                );
                ui.add_space(4.0 * ui_scale);
                section::segmented(
                    ui,
                    ui_scale,
                    &mut state.bass[sel].lfos[lfo_index].shape,
                    &shared::LfoShape::ALL
                        .iter()
                        .map(|shape| (*shape, shape.label()))
                        .collect::<Vec<_>>(),
                );

                ui.add_space(6.0 * ui_scale);
                draw_lfo_display(
                    ui,
                    ui_scale,
                    &state.bass[sel].lfos[lfo_index],
                    &telemetry,
                    live.then_some(lfo_index),
                );

                ui.add_space(6.0 * ui_scale);
                let dial = fit_knob_diameter(ui, 3, ui_scale);
                ui.horizontal(|ui| {
                    let lfo = &mut state.bass[sel].lfos[lfo_index];
                    let dial_style = knob::KnobStyle {
                        diameter: dial,
                        ..knob::KnobStyle::default()
                    };
                    if lfo.tempo_sync {
                        let rate_display = format!("{:.2} bt", lfo.rate_beats);
                        knob::log_range_knob(
                            ui,
                            ui_scale,
                            "RATE",
                            &mut lfo.rate_beats,
                            0.0625,
                            64.0,
                            &rate_display,
                            dial_style,
                        );
                    } else {
                        let rate_display = format!("{:.2} Hz", lfo.rate_hz);
                        knob::log_range_knob(
                            ui,
                            ui_scale,
                            "RATE",
                            &mut lfo.rate_hz,
                            0.01,
                            100.0,
                            &rate_display,
                            dial_style,
                        );
                    }

                    let depth_display = format!("{:.0}%", lfo.depth * 100.0);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "DEPTH",
                        &mut lfo.depth,
                        0.0,
                        1.0,
                        &depth_display,
                        dial_style,
                    );

                    let phase_display = format!("{:.0}\u{00b0}", lfo.phase_deg);
                    knob::range_knob(
                        ui,
                        ui_scale,
                        "PHASE",
                        &mut lfo.phase_deg,
                        0.0,
                        360.0,
                        &phase_display,
                        dial_style,
                    );
                });

                ui.add_space(4.0 * ui_scale);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.bass[sel].lfos[lfo_index].tempo_sync, "Sync");
                    ui.checkbox(&mut state.bass[sel].lfos[lfo_index].retrigger, "Retrig");
                });
            },
        );
        state.bass[sel].lfos[lfo_index].enabled = lfo_enabled;

        // Mod matrix: Serum's routing table — each row picks a source, a
        // destination and a signed amount.
        section::render(
            &mut columns[1],
            ui_scale,
            section::SectionHeader {
                title: "MOD MATRIX",
                enabled: None,
                min_height: 200.0 * ui_scale,
            },
            |ui| {
                // One compact row per routing, like Serum's matrix list.
                for row_index in 0..shared::BASS_MOD_SLOT_COUNT {
                    let routing = &mut state.bass[sel].mod_matrix[row_index];
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}", row_index + 1))
                                .font(themed_font(10.0 * ui_scale))
                                .color(APP_THEME.axis_tick()),
                        );
                        egui::ComboBox::from_id_salt(("bass-mod-src", sel, row_index))
                            .selected_text(
                                egui::RichText::new(routing.source.label())
                                    .font(themed_font(10.0 * ui_scale)),
                            )
                            .width(64.0 * ui_scale)
                            .show_ui(ui, |ui| {
                                for source in shared::ModSource::ALL {
                                    ui.selectable_value(
                                        &mut routing.source,
                                        source,
                                        source.label(),
                                    );
                                }
                            });
                        ui.label(
                            egui::RichText::new(">")
                                .font(themed_font(10.0 * ui_scale))
                                .color(phosphor_color()),
                        );
                        egui::ComboBox::from_id_salt(("bass-mod-dst", sel, row_index))
                            .selected_text(
                                egui::RichText::new(routing.dest.label())
                                    .font(themed_font(10.0 * ui_scale)),
                            )
                            .width(88.0 * ui_scale)
                            .show_ui(ui, |ui| {
                                for dest in shared::ModDest::ALL {
                                    ui.selectable_value(
                                        &mut routing.dest,
                                        dest,
                                        dest.label(),
                                    );
                                }
                            });
                        // Bipolar amount as a draggable numeric field.
                        ui.add(
                            egui::DragValue::new(&mut routing.amount)
                                .range(-1.0..=1.0)
                                .speed(0.005)
                                .custom_formatter(|v, _| format!("{:+.0}%", v * 100.0))
                                .custom_parser(|s| {
                                    s.trim_end_matches('%')
                                        .trim()
                                        .parse::<f64>()
                                        .ok()
                                        .map(|v| v / 100.0)
                                }),
                        );
                    });
                }
            },
        );
    });

    ui.add_space(8.0 * ui_scale);
    ui.columns(2, |columns| {
        section::render(
            &mut columns[0],
            ui_scale,
            section::SectionHeader {
                title: "ENV - AMP",
                enabled: None,
                min_height: 0.0,
            },
            |ui| {
                envelope_drag_active |= envelope_editor::render(
                    ui,
                    ui_scale,
                    "bass-amp-envelope",
                    "",
                    &mut state.bass[sel].amp_curve,
                    &mut state.bass_amp_selected_point,
                    live_phase,
                );
            },
        );

        section::render(
            &mut columns[1],
            ui_scale,
            section::SectionHeader {
                title: "ENV - CUTOFF",
                enabled: None,
                min_height: 0.0,
            },
            |ui| {
                // Serum-style source selector: choose which cutoff envelope is
                // being edited, each with its own independent power pill.
                let selected_filter_index = {
                    let slot = &mut state.bass[sel];
                    ui.horizontal(|ui| {
                        section::segmented(
                            ui,
                            ui_scale,
                            &mut slot.filter_selected,
                            &[(0usize, "ENV 1"), (1usize, "ENV 2")],
                        );
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if section::power_pill(ui, ui_scale, slot.filter_2_enabled)
                                    .clicked()
                                {
                                    slot.filter_2_enabled = !slot.filter_2_enabled;
                                }
                                ui.label("2");
                                if section::power_pill(ui, ui_scale, slot.filter_enabled)
                                    .clicked()
                                {
                                    slot.filter_enabled = !slot.filter_enabled;
                                }
                                ui.label("1");
                            },
                        );
                    });
                    slot.filter_selected
                };

                ui.add_space(4.0 * ui_scale);

                let (curve, selected_point) = if selected_filter_index == 0 {
                    (
                        &mut state.bass[sel].filter_curve,
                        &mut state.bass_filter_selected_point,
                    )
                } else {
                    (
                        &mut state.bass[sel].filter_curve_2,
                        &mut state.bass_filter_2_selected_point,
                    )
                };
                let envelope_id =
                    format!("bass-filter-envelope-{}-{}", sel, selected_filter_index);
                envelope_drag_active |= envelope_editor::render(
                    ui,
                    ui_scale,
                    envelope_id.as_str(),
                    "",
                    curve,
                    selected_point,
                    live_phase,
                );
            },
        );
    });

    ui.add_space(10.0 * ui_scale);
    // Fixed height: available_height() inside a ScrollArea is bounded by the
    // viewport, so the preview would collapse to a sliver at the bottom of
    // the page and break the scroll range.
    let scope_height = 280.0 * ui_scale;
    let (outer_rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(260.0 * ui_scale), scope_height),
        Sense::hover(),
    );
    let graph_rect = outer_rect.shrink2(Vec2::new(14.0 * ui_scale, 18.0 * ui_scale));
    let painter = ui.painter_at(outer_rect);

    painter.rect_filled(outer_rect, 4.0, APP_THEME.graph_bg());
    painter.rect_filled(graph_rect, 4.0, APP_THEME.screen_bg());
    painter.rect_stroke(
        graph_rect,
        4.0,
        Stroke::new(1.0, APP_THEME.graph_border()),
        egui::StrokeKind::Inside,
    );

    let max_note_length_ms = state.note_length_max_ms.max(f32::EPSILON);
    let adaptive_zoom_factor = state.base_note_length_max_ms.max(f32::EPSILON) / max_note_length_ms;
    let note_end_ms = state.bass[sel].note_length_ms.clamp(1.0, 1000.0);
    let note_end_t = (note_end_ms / max_note_length_ms).clamp(0.0, 1.0);
    let zoom = effective_waveform_zoom(state.waveform_zoom_percent, adaptive_zoom_factor);
    let display_length_t = (note_end_t * zoom).clamp(0.0, 1.0);
    let active_right = egui::lerp(graph_rect.left()..=graph_rect.right(), display_length_t);

    // Time axis: subdivisions and labels span the active waveform region,
    // which represents 0..note_end_ms.
    for i in 0..=AXIS_SUBDIVISIONS {
        let f = i as f32 / AXIS_SUBDIVISIONS as f32;
        let x = egui::lerp(graph_rect.left()..=active_right, f);
        painter.line_segment(
            [Pos2::new(x, graph_rect.top()), Pos2::new(x, graph_rect.bottom())],
            Stroke::new(1.0, APP_THEME.grid_line()),
        );
        painter.text(
            Pos2::new(x, graph_rect.bottom() + 2.0 * ui_scale),
            Align2::CENTER_TOP,
            axis_x_label(f * note_end_ms),
            themed_font(10.0 * ui_scale),
            APP_THEME.axis_tick(),
        );
    }

    // Render the real bass voice (post amp envelope + filter) so the
    // preview matches the actual audio output exactly.
    let preview_rate = audio::PREVIEW_SAMPLE_RATE;
    let amp_lut = curve_lut(&state.bass[sel].amp_curve.points, &state.bass[sel].amp_curve.bends);
    let filter_lut = curve_lut(&state.bass[sel].filter_curve.points, &state.bass[sel].filter_curve.bends);
    let filter_2_lut = curve_lut(&state.bass[sel].filter_curve_2.points, &state.bass[sel].filter_curve_2.bends);
    let preview_samples = audio::render_bass_preview(
        preview_rate,
        note_end_ms * 0.001,
        audio::BassVoiceParams {
            level: state.bass_levels[sel],
            tuning_scale: 1.0,
            note_length_ms: state.bass[sel].note_length_ms,
            base_cutoff_hz: state.bass[sel].cutoff_hz,
            filter_mode: state.bass[sel].filter_mode,
            filter_slope: state.bass[sel].filter_slope,
            filter_drive: state.bass[sel].filter_drive,
            filter_enabled: state.bass[sel].filter_enabled,
            filter_2_enabled: state.bass[sel].filter_2_enabled,
            waveform: state.bass[sel].oscillator_waveform,
            osc_a_level: state.bass[sel].osc_a_level,
            osc_a_unison: state.bass[sel].osc_a_unison,
            osc_a_unison_detune: state.bass[sel].osc_a_unison_detune,
            osc_a_unison_blend: state.bass[sel].osc_a_unison_blend,
            osc_b_enabled: state.bass[sel].osc_b_enabled,
            osc_b_waveform: state.bass[sel].osc_b_waveform,
            osc_b_level: state.bass[sel].osc_b_level,
            osc_b_octave: state.bass[sel].osc_b_octave,
            osc_b_detune_cents: state.bass[sel].osc_b_detune_cents,
            osc_b_unison: state.bass[sel].osc_b_unison,
            osc_b_unison_detune: state.bass[sel].osc_b_unison_detune,
            osc_b_unison_blend: state.bass[sel].osc_b_unison_blend,
            sub_enabled: state.bass[sel].sub_enabled,
            sub_waveform: state.bass[sel].sub_waveform,
            sub_level: state.bass[sel].sub_level,
            sub_octave: state.bass[sel].sub_octave,
            noise_enabled: state.bass[sel].noise_enabled,
            noise_level: state.bass[sel].noise_level,
            lfos: state.bass[sel].lfos,
            mod_matrix: state.bass[sel].mod_matrix,
            beats_per_second: (state.manual_tempo / 60.0).max(0.0),
        },
        state.bass[sel].pitch_hz,
        &amp_lut,
        &filter_lut,
        &filter_2_lut,
        state.bass[sel].phase_deg / 360.0,
        state.bass[sel].retrigger,
        state.bass[sel].legato_voice_steal,
    );
    let waveform_points = waveform_preview_points(
        graph_rect,
        &preview_samples,
        preview_rate,
        note_end_ms,
        max_note_length_ms,
        state.waveform_zoom_percent,
        adaptive_zoom_factor,
    );

    // Sync UI state to shared state for DSP
    state.sync_to_shared(shared_for_ui);

    waveform_preview::draw(
        &painter,
        graph_rect,
        &waveform_points,
        APP_THEME.waveform_midline(),
        APP_THEME.waveform_trace(),
    );

    painter.text(
        graph_rect.left_top() + Vec2::new(8.0, 8.0),
        Align2::LEFT_TOP,
        "SCOPE",
        themed_font(11.0 * ui_scale),
        phosphor_color(),
    );

    crt_overlay(&painter, outer_rect, 4.0);
        });

    // Stash the pre-drag state so an envelope drag becomes a single undo entry.
    if envelope_drag_active && state.point_drag_snapshot.is_none() {
        state.point_drag_snapshot = Some(snapshot_before.clone());
    }
}

/// Live value of a modulation source, evaluated the same way the DSP does so
/// knob rings track what the voice is actually receiving.
fn mod_source_value(
    source: shared::ModSource,
    telemetry: &shared::BassModTelemetry,
    pitch_hz: f32,
) -> f32 {
    if let Some(index) = source.lfo_index() {
        return telemetry.lfo_values[index];
    }
    match source {
        shared::ModSource::EnvAmp => telemetry.env_amp,
        shared::ModSource::EnvFilter1 => telemetry.env_filter_1,
        shared::ModSource::EnvFilter2 => telemetry.env_filter_2,
        shared::ModSource::Velocity => telemetry.velocity,
        shared::ModSource::KeyTrack => {
            const MIN_HZ: f32 = 20.0;
            const MAX_HZ: f32 = 2_000.0;
            ((pitch_hz.clamp(MIN_HZ, MAX_HZ) / MIN_HZ).ln() / (MAX_HZ / MIN_HZ).ln())
                .clamp(0.0, 1.0)
        }
        _ => 0.0,
    }
}

/// Total modulation reaching `dest`, converted to the signed knob-normalized
/// offset consumed by `KnobStyle::mod_arc`. Each destination maps its DSP
/// range onto the knob's 0..1 travel: e.g. cutoff moves ±4 octaves across
/// the log knob's 20..8000 Hz span (~8.64 octaves).
fn mod_ring_offset(
    dest: shared::ModDest,
    matrix: &[shared::ModRouting],
    telemetry: &shared::BassModTelemetry,
    pitch_hz: f32,
) -> Option<f32> {
    let total: f32 = matrix
        .iter()
        .filter(|routing| routing.is_active() && routing.dest == dest)
        .map(|routing| {
            mod_source_value(routing.source, telemetry, pitch_hz) * routing.amount
        })
        .sum();

    let offset = match dest {
        shared::ModDest::Cutoff => total * 4.0 / (8_000.0_f32 / 20.0).log2(),
        shared::ModDest::Pitch => total * 2.0 / (2_000.0_f32 / 20.0).log2(),
        shared::ModDest::Level
        | shared::ModDest::OscAMix
        | shared::ModDest::OscBMix
        | shared::ModDest::SubMix
        | shared::ModDest::NoiseMix
        | shared::ModDest::FilterDrive => total,
        // ±100 cents over the -50..50 knob span → total * 100/100.
        shared::ModDest::OscBDetune => total,
        // ±50 cents over the 0..100 knob span.
        shared::ModDest::UnisonDetune => total * 0.5,
        shared::ModDest::None => 0.0,
    };
    (offset.abs() > 0.002).then_some(offset)
}

/// Serum-style LFO display: the shape drawn at its current depth, a centre
/// line, and a glowing dot tracking the live output while a note plays.
fn draw_lfo_display(
    ui: &mut egui::Ui,
    ui_scale: f32,
    lfo: &shared::LfoParams,
    telemetry: &shared::BassModTelemetry,
    live_index: Option<usize>,
) {
    let height = 64.0 * ui_scale;
    let width = ui.available_width().max(120.0 * ui_scale);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 3.0, APP_THEME.graph_bg());
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, APP_THEME.graph_border()),
        egui::StrokeKind::Inside,
    );

    let mid_y = rect.center().y;
    painter.hline(
        rect.x_range(),
        mid_y,
        Stroke::new(1.0, APP_THEME.grid_line()),
    );

    let to_screen = |t: f32, value: f32| {
        Pos2::new(
            egui::lerp(rect.left() + 2.0..=rect.right() - 2.0, t),
            mid_y - value * (rect.height() * 0.5 - 4.0 * ui_scale),
        )
    };

    let amplitude = if lfo.enabled { lfo.depth } else { lfo.depth * 0.25 };
    let trace = |painter: &egui::Painter, stroke: Stroke| {
        match lfo.shape {
            // Sample-and-hold renders as a staircase: 8 deterministic steps.
            shared::LfoShape::Random => {
                for step in 0..8 {
                    let t0 = step as f32 / 8.0;
                    let t1 = (step + 1) as f32 / 8.0;
                    let v0 = random_step(step) * amplitude;
                    let v1 = random_step(step + 1) * amplitude;
                    painter.line_segment([to_screen(t0, v0), to_screen(t1, v0)], stroke);
                    painter.line_segment([to_screen(t1, v0), to_screen(t1, v1)], stroke);
                }
            }
            _ => {
                let mut prev = to_screen(0.0, audio::shape_at(lfo.shape, 0.0) * amplitude);
                for step in 1..=96 {
                    let t = step as f32 / 96.0;
                    let next = to_screen(t, audio::shape_at(lfo.shape, t) * amplitude);
                    painter.line_segment([prev, next], stroke);
                    prev = next;
                }
            }
        }
    };
    trace(&painter, Stroke::new(4.0 * ui_scale, APP_THEME.trace_glow()));
    trace(&painter, Stroke::new(1.5 * ui_scale, accent_color()));

    // Live dot at the true phase and output from the engine.
    if let Some(index) = live_index {
        if lfo.enabled {
            let dot = to_screen(
                telemetry.lfo_phases[index],
                telemetry.lfo_values[index],
            );
            painter.circle_filled(dot, 5.0 * ui_scale, APP_THEME.trace_glow());
            painter.circle_filled(dot, 2.5 * ui_scale, phosphor_color());
        }
    }
}

/// Knob diameter (unscaled points) that spreads `count` knobs evenly across
/// the row's available width so module panels do not sit half-empty. The
/// knob widget cell is `diameter` wide; clamped to a sane visual range.
fn fit_knob_diameter(ui: &egui::Ui, count: usize, ui_scale: f32) -> f32 {
    let count = count.max(1) as f32;
    let gap = ui.spacing().item_spacing.x;
    let cell = (ui.available_width() - gap * (count - 1.0)).max(0.0) / count;
    (cell / ui_scale).clamp(38.0, 118.0)
}

/// Deterministic 0..1-ish value per step for drawing the Random shape.
fn random_step(step: usize) -> f32 {
    let hash = (step as u32)
        .wrapping_mul(2_654_435_761)
        .wrapping_add(0x9E37_79B9);
    (hash % 2000) as f32 / 1000.0 - 1.0
}
