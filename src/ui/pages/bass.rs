use nih_plug_egui::egui::{self, Align2, Color32, Pos2, Sense, Stroke, Vec2};

use nih_plug::prelude::ParamSetter;

use crate::ui::{
    components::{envelope_editor, oscillator_panel, panel, waveform_preview},
    helpers::{axis_x_label, curve_lut, effective_waveform_zoom, waveform_preview_points},
    state::{BezierUiState, EditorSnapshot},
    theme::{accent_color, themed_font, APP_THEME},
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

    let mut envelope_drag_active = false;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
    ui.columns(2, |columns| {
        panel::render(&mut columns[0], "Oscillator", ui_scale, 220.0 * ui_scale, |ui| {
            oscillator_panel::render(
                ui,
                ui_scale,
                oscillator_panel::OscillatorPanelModel {
                    waveform: &mut state.bass[sel].oscillator_waveform,
                    retrigger: &mut state.bass[sel].retrigger,
                    legato_voice_steal: &mut state.bass[sel].legato_voice_steal,
                    pitch_hz: Some(&mut state.bass[sel].pitch_hz),
                    tuning_a4_hz: state.tuning_standard.a4_hz(),
                    phase_deg: Some(&mut state.bass[sel].phase_deg),
                    note_length_ms: Some(&mut state.bass[sel].note_length_ms),
                    level: Some((
                        if sel == 0 {
                            &params.bass1_level
                        } else {
                            &params.bass2_level
                        },
                        setter,
                    )),
                },
            );

            ui.add_space(8.0 * ui_scale);
            ui.checkbox(&mut state.bass[sel].keytrack_enabled, "Keytrack");

            ui.add_space(8.0 * ui_scale);
            envelope_drag_active |= envelope_editor::render(
                ui,
                ui_scale,
                "bass-amp-envelope",
                "Amp Envelope",
                &mut state.bass[sel].amp_curve,
                &mut state.bass_amp_selected_point,
            );
        });

        panel::render(&mut columns[1], "Filter", ui_scale, 220.0 * ui_scale, |ui| {
            let filter_mode_label = match state.bass[sel].filter_mode {
                shared::BassFilterMode::LowPass => "Low-pass",
                shared::BassFilterMode::HighPass => "High-pass",
                shared::BassFilterMode::BandPass => "Band-pass",
            };
            ui.label(format!("Mode: {filter_mode_label}"));
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut state.bass[sel].filter_mode,
                    shared::BassFilterMode::LowPass,
                    "Low",
                );
                ui.selectable_value(
                    &mut state.bass[sel].filter_mode,
                    shared::BassFilterMode::HighPass,
                    "High",
                );
                ui.selectable_value(
                    &mut state.bass[sel].filter_mode,
                    shared::BassFilterMode::BandPass,
                    "BP",
                );
            });
            ui.add_space(4.0 * ui_scale);
            ui.horizontal(|ui| {
                ui.label("Slope:");
                ui.selectable_value(
                    &mut state.bass[sel].filter_slope,
                    shared::BassFilterSlope::S6dB,
                    "6 dB",
                );
                ui.selectable_value(
                    &mut state.bass[sel].filter_slope,
                    shared::BassFilterSlope::S12dB,
                    "12 dB",
                );
                ui.selectable_value(
                    &mut state.bass[sel].filter_slope,
                    shared::BassFilterSlope::S18dB,
                    "18 dB",
                );
                ui.selectable_value(
                    &mut state.bass[sel].filter_slope,
                    shared::BassFilterSlope::S24dB,
                    "24 dB",
                );
            });
            ui.label("Cutoff");
            let cutoff_changed = ui
                .add(crate::ui::helpers::slider_fine_step(
                    ui,
                    egui::Slider::new(&mut state.bass[sel].cutoff_hz, 20.0..=8000.0).text("Hz"),
                    1.0,
                ))
                .changed();
            if cutoff_changed {
                state.bass[sel].cutoff_hz = state.bass[sel].cutoff_hz.clamp(20.0, 8_000.0);
            }

            ui.label("Drive");
            let drive_changed = ui
                .add(crate::ui::helpers::slider_fine_step(
                    ui,
                    egui::Slider::new(&mut state.bass[sel].filter_drive, 0.0..=1.0)
                        .show_value(false),
                    0.01,
                ))
                .changed();
            if drive_changed {
                state.bass[sel].filter_drive = state.bass[sel].filter_drive.clamp(0.0, 1.0);
            }

            ui.add_space(8.0 * ui_scale);
            envelope_drag_active |= envelope_editor::render(
                ui,
                ui_scale,
                "bass-filter-envelope",
                "Filter Cutoff Envelope",
                &mut state.bass[sel].filter_curve,
                &mut state.bass_filter_selected_point,
            );
        });
    });

    ui.add_space(10.0 * ui_scale);
    let remaining_height = ui
        .available_height()
        .clamp((160.0 * ui_scale).max(120.0), 400.0 * ui_scale);
    let (outer_rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(260.0 * ui_scale), remaining_height),
        Sense::hover(),
    );
    let graph_rect = outer_rect.shrink2(Vec2::new(14.0 * ui_scale, 18.0 * ui_scale));
    let painter = ui.painter_at(outer_rect);

    painter.rect_filled(outer_rect, 4.0, Color32::from_rgb(16, 19, 22));
    painter.rect_filled(graph_rect, 4.0, Color32::from_rgb(20, 24, 28));
    painter.rect_stroke(
        graph_rect,
        4.0,
        Stroke::new(1.0, Color32::from_rgb(90, 95, 102)),
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
            waveform: state.bass[sel].oscillator_waveform,
        },
        state.bass[sel].pitch_hz,
        &amp_lut,
        &filter_lut,
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
        Color32::from_rgba_unmultiplied(120, 128, 136, 45),
        Color32::from_rgba_unmultiplied(245, 170, 112, 125),
    );

    painter.text(
        graph_rect.left_top() + Vec2::new(8.0, 8.0),
        Align2::LEFT_TOP,
        "Bass Waveform Preview",
        egui::FontId::proportional(11.0 * ui_scale),
        Color32::from_rgb(185, 191, 198),
    );
        });

    // Stash the pre-drag state so an envelope drag becomes a single undo entry.
    if envelope_drag_active && state.point_drag_snapshot.is_none() {
        state.point_drag_snapshot = Some(snapshot_before.clone());
    }
}
