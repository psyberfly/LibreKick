use std::sync::Arc;

mod helpers;
mod components;
mod pages;
mod state;
mod theme;

use nih_plug::prelude::Editor;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Vec2},
    resizable_window::ResizableWindow,
    EguiState,
};

use crate::{config, shared};

use self::components::brand;
use self::helpers::poll_editor_shortcuts;
use self::state::{BezierUiState, UiPage};
use self::theme::{
    apply_ui_text_scale, apply_widget_style, ui_scale_from_size, APP_THEME,
};

const MIN_POINT_GAP_X: f32 = 0.01;
const WAVEFORM_PREVIEW_DURATION_SECONDS: f32 = 1.0;
const WAVEFORM_PREVIEW_MAX_CYCLES_PER_PIXEL: f32 = 0.3;
const AMP_DB_FLOOR: f32 = -30.0;

pub fn create_testing_editor(
    editor_state: Arc<EguiState>,
    shared_state: shared::SharedStateHandle,
) -> Option<Box<dyn Editor>> {
    let ui_cfg = config::ui_config();
    let resizable_state = editor_state.clone();
    let shared_for_ui = shared_state.clone();

    create_egui_editor(
        editor_state,
        BezierUiState::default(),
        |_ctx, state| {
            let mut fonts = egui::FontDefinitions::default();
            let app_family = APP_THEME.app_font_family();
            if let Some(fallbacks) = fonts.families.get(&egui::FontFamily::Proportional).cloned() {
                fonts.families.insert(app_family, fallbacks);
            }
            _ctx.set_fonts(fonts);
            state.brand_logo = brand::brand_logo_texture(_ctx);
        },
        move |_ctx, _setter, state| {
            ResizableWindow::new("kick-plugin-resize")
                .min_size(Vec2::new(ui_cfg.min_editor_width, ui_cfg.min_editor_height))
                .show(_ctx, &resizable_state, |ui| {
                let snapshot_before = state.snapshot();
                let mut history_action_applied = false;

                let (undo_shortcut, redo_shortcut, cut_shortcut, delete_shortcut) =
                    poll_editor_shortcuts(ui);
                if undo_shortcut {
                    history_action_applied |= state.undo();
                }
                if redo_shortcut {
                    history_action_applied |= state.redo();
                }

                let ui_scale = ui_scale_from_size(ui.available_size_before_wrap())
                    * state.display_scale;
                let app_cfg = config::app_config();
                apply_widget_style(ui, ui_scale);
                apply_ui_text_scale(ui, ui_scale);
                components::scaffold::render(ui, ui_scale, state, |ui, state| {
                if state.active_page == UiPage::Kick {
                pages::kick::render(ui, |ui| {
                pages::kick::render_controls(
                    ui,
                    ui_scale,
                    state,
                    app_cfg,
                    &shared_for_ui,
                    &mut history_action_applied,
                );
                pages::kick::render_editor(
                    ui,
                    ui_scale,
                    state,
                    app_cfg,
                    &shared_for_ui,
                    &snapshot_before,
                    &mut history_action_applied,
                    cut_shortcut,
                    delete_shortcut,
                );
                });
                } else if state.active_page == UiPage::Bass {
                    pages::bass::render(ui, ui_scale, state, &shared_for_ui);
                } else if state.active_page == UiPage::Settings {
                    pages::settings::render(ui, ui_scale, state);
                } else if state.active_page == UiPage::Oscilloscope {
                    pages::oscilloscope::render(ui, ui_scale, state, &shared_for_ui);
                } else {
                    pages::logs::render(ui, ui_scale, state);
                }
                });
            });
        },
    )
}
