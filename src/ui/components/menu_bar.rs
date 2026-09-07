use nih_plug::prelude::ParamSetter;
use nih_plug_egui::egui;

use crate::{config, patches, LibreKickParams};
use crate::ui::components::brand;
use crate::ui::state::BezierUiState;
use crate::ui::theme::{apply_ui_text_scale, APP_THEME};

/// Renders the top menu bar shown on every page: brand logo, version,
/// help button, undo/redo, and the patch selector.
///
/// `page_width` is the width of the page-content column below; the undo/redo
/// buttons are right-aligned to that edge so they sit at the end of the page
/// content rather than above the nav menu.
pub(crate) fn render(
    ui: &mut egui::Ui,
    ui_scale: f32,
    state: &mut BezierUiState,
    params: &LibreKickParams,
    setter: &ParamSetter,
    page_width: f32,
    history_action_applied: &mut bool,
) {
    ui.add_space(6.0 * ui_scale);
    ui.horizontal(|ui| {
        let row_left = ui.max_rect().left();
        brand::brand_title_logo(ui, state.brand_logo.as_ref(), ui_scale);
        ui.add_space(6.0 * ui_scale);
        ui.label(
            egui::RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                .strong()
                .color(APP_THEME.axis_title()),
        );

        ui.add_space(12.0 * ui_scale);

        // Patches button + name + description (left-aligned).
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
                                        let (kick_level, bass_level) =
                                            state.apply_patch_data(patch);
                                        if let Some(level) = kick_level {
                                            setter.set_parameter(&params.kick_level, level);
                                            state.kick_level = level;
                                        }
                                        if let Some(level) = bass_level {
                                            setter.set_parameter(&params.bass_level, level);
                                            state.bass_level = level;
                                        }
                                        state.mark_patch_clean(patch_name.clone());
                                        state.commit_history_if_changed(&before);
                                        *history_action_applied = true;
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
                    ui.label("Description");
                    let desc_response = ui.add(
                        egui::TextEdit::singleline(&mut state.patch_description)
                            .desired_width(280.0 * ui_scale)
                            .hint_text("Patch description…"),
                    );
                    if desc_response.changed() {
                        state.patch_description =
                            patches::sanitize_patch_description(&state.patch_description);
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
                                
                                // Set the newly saved patch as the default
                                match patches::set_default_patch_name(&patch_name) {
                                    Ok(()) => {
                                        state.default_patch_name = Some(patch_name.clone());
                                    }
                                    Err(error) => {
                                        state.patch_status = Some(format!(
                                            "Saved patch but failed to set as default: {error}"
                                        ));
                                    }
                                }
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

        if !state.patch_description.is_empty() {
            ui.label(
                egui::RichText::new("-")
                    .small()
                    .color(APP_THEME.axis_tick()),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(state.patch_description.as_str())
                        .small()
                        .italics()
                        .color(APP_THEME.axis_tick()),
                )
                .truncate(),
            )
            .on_hover_text("Edit the description in the Patches menu");
        }

        // Right-align undo/redo/help to the page-content right edge.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let page_right = row_left + page_width;
            let pad = (ui.available_rect_before_wrap().right() - page_right).max(0.0);
            ui.add_space(pad);

            let button_size = egui::Vec2::splat(30.0 * ui_scale);
            if ui
                .add(egui::Button::new("?").min_size(button_size))
                .clicked()
            {
                state.show_help_popup = true;
            }

            ui.add_space(10.0 * ui_scale);

            let redo_clicked = ui
                .add_enabled(!state.redo_stack.is_empty(), egui::Button::new(">").min_size(button_size))
                .on_hover_ui(|ui| {
                    apply_ui_text_scale(ui, ui_scale);
                    ui.label("Redo (Ctrl/Cmd + Y)");
                })
                .clicked();
            let undo_clicked = ui
                .add_enabled(!state.undo_stack.is_empty(), egui::Button::new("<").min_size(button_size))
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
}
