use nih_plug_egui::egui::{self, Align2, Color32, RichText, Sense, Stroke, Vec2};

use crate::ui::{
    components::{envelope_editor, waveform_preview},
    helpers::{curve_lut, note_name_from_hz, waveform_preview_points},
    state::BezierUiState,
};
use crate::shared;

pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    shared_for_ui: &shared::SharedStateHandle,
) {
    ui.add_space(8.0 * ui_scale);
    ui.heading("Bass");
    ui.label(
        RichText::new("Dedicated bass voice page")
            .italics()
            .small(),
    );
    ui.separator();

    ui.columns(2, |columns| {
        columns[0].group(|ui| {
            ui.set_min_height((220.0 * ui_scale).max(180.0));
            ui.label(RichText::new("Oscillator").strong());
            ui.separator();
            ui.label("Waveform");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut state.bass_waveform, shared::BassWaveform::Saw, "Saw");
                ui.add_enabled_ui(false, |ui| {
                    ui.selectable_value(
                        &mut state.bass_waveform,
                        shared::BassWaveform::Square,
                        "Square",
                    );
                    ui.selectable_value(
                        &mut state.bass_waveform,
                        shared::BassWaveform::Sine,
                        "Sine",
                    );
                });
            });
            ui.add_space(6.0 * ui_scale);
            ui.label("Sub Level");
            let mut sub_level = 0.5_f32;
            ui.add_enabled(false, egui::Slider::new(&mut sub_level, 0.0..=1.0));

            ui.add_space(6.0 * ui_scale);
            ui.label("Pitch");
            let bass_note_label = note_name_from_hz(state.bass_pitch_hz, state.tuning_standard.a4_hz());
            let pitch_changed = ui
                .add(
                    egui::Slider::new(&mut state.bass_pitch_hz, 20.0..=2000.0)
                        .text("Hz")
                        .logarithmic(true),
                )
                .changed();
            if pitch_changed {
                state.bass_pitch_hz = state.bass_pitch_hz.clamp(20.0, 2_000.0);
            }
            ui.label(format!("{} {:.2}Hz", bass_note_label, state.bass_pitch_hz));

            ui.add_space(6.0 * ui_scale);
            ui.label("Note Length");
            let bass_note_length_changed = ui
                .add(egui::Slider::new(&mut state.bass_note_length_ms, 1.0..=1000.0).text("ms"))
                .changed();
            if bass_note_length_changed {
                state.bass_note_length_ms = state.bass_note_length_ms.clamp(1.0, 1000.0);
            }

            ui.add_space(6.0 * ui_scale);
            ui.checkbox(&mut state.bass_retrigger, "Retrigger");
            ui.checkbox(
                &mut state.bass_legato_voice_steal,
                "Legato (voice steal)",
            );

            ui.add_space(8.0 * ui_scale);
            envelope_editor::render(
                ui,
                ui_scale,
                "bass-amp-envelope",
                "Amp Envelope",
                &mut state.bass_amp_curve,
                &mut state.bass_amp_selected_point,
            );
        });

        columns[1].group(|ui| {
            ui.set_min_height((220.0 * ui_scale).max(180.0));
            ui.label(RichText::new("Filter").strong());
            ui.separator();
            let filter_mode_label = match state.bass_filter_mode {
                shared::BassFilterMode::LowPass => "Low-pass",
                shared::BassFilterMode::HighPass => "High-pass",
                shared::BassFilterMode::BandPass => "Band-pass",
            };
            ui.label(format!("Mode: {filter_mode_label}"));
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut state.bass_filter_mode,
                    shared::BassFilterMode::LowPass,
                    "Low",
                );
                ui.selectable_value(
                    &mut state.bass_filter_mode,
                    shared::BassFilterMode::HighPass,
                    "High",
                );
                ui.selectable_value(
                    &mut state.bass_filter_mode,
                    shared::BassFilterMode::BandPass,
                    "BP",
                );
            });
            ui.label("Cutoff");
            let cutoff_changed = ui
                .add(egui::Slider::new(&mut state.bass_cutoff_hz, 20.0..=8000.0).text("Hz"))
                .changed();
            if cutoff_changed {
                state.bass_cutoff_hz = state.bass_cutoff_hz.clamp(20.0, 8_000.0);
            }

            ui.add_space(8.0 * ui_scale);
            envelope_editor::render(
                ui,
                ui_scale,
                "bass-filter-envelope",
                "Filter Cutoff Envelope",
                &mut state.bass_filter_curve,
                &mut state.bass_filter_selected_point,
            );
        });
    });

    ui.add_space(10.0 * ui_scale);
    let remaining_height = ui.available_height().max((160.0 * ui_scale).max(120.0));
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
    let waveform_points = waveform_preview_points(
        graph_rect,
        &state.bass_amp_curve.points,
        &state.bass_amp_curve.bends,
        &state.bass_filter_curve.points,
        &state.bass_filter_curve.bends,
        state.tuning_standard.a4_hz(),
        state.bass_note_length_ms.clamp(1.0, 1000.0),
        max_note_length_ms,
        state.waveform_zoom_percent,
        adaptive_zoom_factor,
    );

    let bass_amp_lut = curve_lut(&state.bass_amp_curve.points, &state.bass_amp_curve.bends);
    let bass_filter_lut = curve_lut(&state.bass_filter_curve.points, &state.bass_filter_curve.bends);
    shared::set_bass_amp_lut(shared_for_ui, bass_amp_lut);
    shared::set_bass_filter_lut(shared_for_ui, bass_filter_lut);
    shared::set_bass_note_length_ms(shared_for_ui, state.bass_note_length_ms);
    shared::set_bass_cutoff_hz(shared_for_ui, state.bass_cutoff_hz);
    shared::set_bass_filter_mode(shared_for_ui, state.bass_filter_mode);
    shared::set_bass_pitch_hz(shared_for_ui, state.bass_pitch_hz);
    shared::set_bass_retrigger(shared_for_ui, state.bass_retrigger);
    shared::set_bass_legato_voice_steal(shared_for_ui, state.bass_legato_voice_steal);
    shared::set_bass_waveform(shared_for_ui, state.bass_waveform);
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
}
