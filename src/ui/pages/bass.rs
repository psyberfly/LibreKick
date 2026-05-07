use nih_plug_egui::egui::{self, Align2, Color32, RichText, Sense, Stroke, Vec2};

use crate::ui::{
    components::{envelope_editor, oscillator_panel, panel, waveform_preview},
    helpers::{curve_lut, waveform_preview_points},
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
        panel::render(&mut columns[0], "Oscillator", ui_scale, 220.0 * ui_scale, |ui| {
            oscillator_panel::render(
                ui,
                ui_scale,
                oscillator_panel::OscillatorPanelModel {
                    waveform: &mut state.bass_oscillator_waveform,
                    retrigger: &mut state.bass_retrigger,
                    legato_voice_steal: &mut state.bass_legato_voice_steal,
                    pitch_hz: Some(&mut state.bass_pitch_hz),
                    note_length_ms: Some(&mut state.bass_note_length_ms),
                },
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

        panel::render(&mut columns[1], "Filter", ui_scale, 220.0 * ui_scale, |ui| {
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
    shared::set_bass_oscillator_waveform(shared_for_ui, state.bass_oscillator_waveform);
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
