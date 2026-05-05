use nih_plug_egui::egui::{self, Align2, Color32, RichText, Sense, Stroke, Vec2};

use crate::ui::{
    components::{envelope_editor, waveform_preview},
    helpers::waveform_preview_points,
    state::BezierUiState,
};

pub(crate) fn render(ui: &mut egui::Ui, ui_scale: f32, state: &mut BezierUiState) {
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
            ui.add_enabled(false, egui::Button::new("Sine"));
            ui.add_enabled(false, egui::Button::new("Saw"));
            ui.add_enabled(false, egui::Button::new("Square"));
            ui.add_space(6.0 * ui_scale);
            ui.label("Sub Level");
            let mut sub_level = 0.5_f32;
            ui.add_enabled(false, egui::Slider::new(&mut sub_level, 0.0..=1.0));

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
            ui.label(RichText::new("Filter + Drive").strong());
            ui.separator();
            ui.label("Cutoff");
            let mut cutoff = 120.0_f32;
            ui.add_enabled(false, egui::Slider::new(&mut cutoff, 20.0..=1200.0).text("Hz"));
            ui.label("Resonance");
            let mut resonance = 0.2_f32;
            ui.add_enabled(false, egui::Slider::new(&mut resonance, 0.0..=1.0));
            ui.label("Drive");
            let mut drive = 0.0_f32;
            ui.add_enabled(false, egui::Slider::new(&mut drive, 0.0..=1.0));

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
        state.note_length_ms.clamp(0.0, max_note_length_ms),
        max_note_length_ms,
        state.waveform_zoom_percent,
        adaptive_zoom_factor,
    );
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
