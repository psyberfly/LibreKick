use nih_plug_egui::egui;

use crate::{config, patches, shared};

use super::super::{
    apply_ui_text_scale, brand_title_logo, CurveKind, TuningStandard, APP_THEME,
    NOTE_LENGTH_MAX_SLIDER_MAX_MS, NOTE_LENGTH_MAX_SLIDER_MIN_MS,
};
use super::super::state::BezierUiState;

pub(crate) fn render(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    add_contents(ui);
}

pub(crate) fn render_controls(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    app_cfg: &config::AppConfig,
    shared_for_ui: &shared::SharedStateHandle,
    history_action_applied: &mut bool,
) {
    ui.add_space(6.0 * ui_scale);
    ui.horizontal(|ui| {
        brand_title_logo(ui, state.brand_logo.as_ref(), ui_scale);
        ui.add_space(6.0 * ui_scale);
        ui.label(
            egui::RichText::new("Prototype")
                .strong()
                .color(APP_THEME.axis_title()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new("?").min_size(egui::Vec2::new(18.0 * ui_scale, 18.0 * ui_scale)))
                .clicked()
            {
                state.show_help_popup = true;
            }

            ui.vertical(|ui| {
                ui.menu_button("Patches", |ui| {
                    apply_ui_text_scale(ui, ui_scale);
                    ui.set_min_width(300.0 * ui_scale);
                    ui.label(format!("Dir: {}", config::patches_dir()));
                    ui.label(format!("Current: {}", state.selected_patch_indicator_text()));
                    ui.separator();
                    ui.label("Load Patch");

                    let patch_names = state.available_patches.clone();
                    if patch_names.is_empty() {
                        ui.label("No patches found.");
                    } else {
                        for patch_name in patch_names {
                            if ui.button(&patch_name).clicked() {
                                let before = state.snapshot();
                                match patches::load_patch(&patch_name) {
                                    Ok(patch) => {
                                        state.apply_patch_data(patch);
                                        state.mark_patch_clean(patch_name.clone());
                                        state.commit_history_if_changed(&before);
                                        state.patch_status = Some(format!("Loaded patch: {patch_name}"));
                                        ui.close_menu();
                                    }
                                    Err(error) => {
                                        state.patch_status =
                                            Some(format!("Failed to load patch: {error}"));
                                        ui.close_menu();
                                    }
                                }
                            }
                        }
                    }

                    ui.separator();
                    ui.label("Save Patch");
                    ui.text_edit_singleline(&mut state.new_patch_name);
                    let can_save_patch = !state.new_patch_name.trim().is_empty();
                    if ui
                        .add_enabled(can_save_patch, egui::Button::new("Save"))
                        .clicked()
                    {
                        let patch_name = state.new_patch_name.trim().to_owned();
                        let patch_data = state.to_patch_data(patch_name.clone());
                        match patches::save_patch(&patch_data) {
                            Ok(()) => {
                                state.mark_patch_clean(patch_name.clone());
                                state.patch_status = Some(format!("Saved patch: {patch_name}"));
                                state.refresh_patch_list();
                            }
                            Err(error) => {
                                state.patch_status = Some(format!("Failed to save patch: {error}"));
                            }
                        }
                    }

                    if ui.button("Set current as default").clicked() {
                        let selected_name = state.selected_patch_name.clone();
                        match selected_name {
                            Some(name) if !state.is_selected_patch_dirty() => {
                                match patches::set_default_patch_name(&name) {
                                    Ok(()) => {
                                        state.default_patch_name = Some(name.clone());
                                        state.patch_status = Some(format!("Default patch set: {name}"));
                                    }
                                    Err(error) => {
                                        state.patch_status = Some(format!(
                                            "Failed to set default patch: {error}"
                                        ));
                                    }
                                }
                            }
                            _ => {
                                state.patch_status =
                                    Some("Save this patch first, then set it as default.".to_owned());
                            }
                        }
                    }

                    if ui.button("Refresh List").clicked() {
                        state.refresh_patch_list();
                    }

                    if let Some(status) = &state.patch_status {
                        ui.separator();
                        ui.label(status);
                    }
                });

                ui.label(
                    egui::RichText::new(state.selected_patch_indicator_text())
                        .small()
                        .color(APP_THEME.axis_tick()),
                );
            });
        });
    });

    if state.show_help_popup {
        egui::Window::new("Help")
            .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-10.0 * ui_scale, 10.0 * ui_scale))
            .collapsible(false)
            .resizable(false)
            .open(&mut state.show_help_popup)
            .show(ui.ctx(), |ui| {
                ui.label("Mouse controls:");
                ui.label("- Drag points to shape the selected envelope.");
                ui.label("- Double-click inside the graph to add a point.");
                ui.label("- Right-click a point to remove it.");
                ui.label("- Ctrl/Cmd + mouse wheel over graph adjusts zoom.");
                ui.add_space(4.0 * ui_scale);
                ui.label("Keyboard shortcuts:");
                ui.label("- Ctrl/Cmd + Z: Undo");
                ui.label("- Ctrl/Cmd + Shift + Z or Ctrl/Cmd + Y: Redo");
                ui.label("- Delete/Backspace/Ctrl/Cmd + X: Remove selected point(s)");
                ui.add_space(4.0 * ui_scale);
                ui.label("Shift-lock mode:");
                ui.label("- Hold Shift and click near a point to lock it.");
                ui.label("- Move mouse (no button) to adjust locked point on X.");
                ui.label("- Hold left mouse to adjust locked point on Y (vertical-only).");
                ui.add_space(4.0 * ui_scale);
                ui.label("Envelope basics:");
                ui.label("- Amplitude envelope controls volume over time.");
                ui.label("- Pitch envelope controls pitch over time.");
            });
    }

    ui.add_space(8.0 * ui_scale);
    ui.horizontal(|ui| {
        ui.label("Curve:");
        ui.selectable_value(&mut state.active_curve, CurveKind::Amplitude, "Amplitude");
        ui.selectable_value(&mut state.active_curve, CurveKind::Pitch, "Pitch");
        ui.separator();
        ui.label("Tuning:");
        ui.selectable_value(&mut state.tuning_standard, TuningStandard::A440, "A=440");
        ui.selectable_value(&mut state.tuning_standard, TuningStandard::A432, "A=432");
        ui.separator();
        ui.checkbox(&mut state.keytrack_enabled, "Keytrack");
        ui.separator();
        if ui.button("Trigger").clicked() {
            shared::request_trigger(shared_for_ui);
        }
        ui.separator();
        ui.label("Max Note Length");
        let max_length_changed = ui
            .add(
                egui::Slider::new(
                    &mut state.note_length_max_ms,
                    NOTE_LENGTH_MAX_SLIDER_MIN_MS..=NOTE_LENGTH_MAX_SLIDER_MAX_MS,
                )
                .text("ms")
                .step_by(1.0),
            )
            .changed();
        if max_length_changed {
            state.note_length_max_ms = state
                .note_length_max_ms
                .clamp(NOTE_LENGTH_MAX_SLIDER_MIN_MS, NOTE_LENGTH_MAX_SLIDER_MAX_MS);
            state.note_length_ms = state.note_length_ms.clamp(0.0, state.note_length_max_ms);
        }
        ui.separator();
        if ui.button("-").clicked() {
            state.waveform_zoom_percent =
                (state.waveform_zoom_percent - app_cfg.waveform_zoom_step_percent).clamp(
                    app_cfg.waveform_zoom_min_percent,
                    app_cfg.waveform_zoom_max_percent,
                );
        }
        ui.label(format!("Zoom {:.0}%", state.waveform_zoom_percent));
        if ui.button("+").clicked() {
            state.waveform_zoom_percent =
                (state.waveform_zoom_percent + app_cfg.waveform_zoom_step_percent).clamp(
                    app_cfg.waveform_zoom_min_percent,
                    app_cfg.waveform_zoom_max_percent,
                );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(10.0 * ui_scale);
            let redo_clicked = ui
                .add_enabled(!state.redo_stack.is_empty(), egui::Button::new(">"))
                .on_hover_ui(|ui| {
                    apply_ui_text_scale(ui, ui_scale);
                    ui.label("Redo (Ctrl/Cmd + Y)");
                })
                .clicked();
            let undo_clicked = ui
                .add_enabled(!state.undo_stack.is_empty(), egui::Button::new("<"))
                .on_hover_ui(|ui| {
                    apply_ui_text_scale(ui, ui_scale);
                    ui.label("Undo (Ctrl/Cmd + Z)");
                })
                .clicked();
            if redo_clicked {
                *history_action_applied |= state.redo();
            }
            if undo_clicked {
                *history_action_applied |= state.undo();
            }
        });
    });
    ui.add_space(8.0);
}
