use std::cmp::Ordering;

use nih_plug_egui::egui::{Pos2, TextureHandle};

use crate::{config, patches};

use super::{
    helpers::{constrain_curve_points, normalize_segment_bends},
    tuning_standard_from_a4_hz,
    CurveKind,
    TuningStandard,
    HISTORY_STACK_CAP, NOTE_LENGTH_MAX_SLIDER_MAX_MS, NOTE_LENGTH_MAX_SLIDER_MIN_MS,
};

#[derive(Clone, PartialEq)]
pub(super) struct Curve {
    pub(super) points: Vec<Pos2>,
    pub(super) bends: Vec<f32>,
}

#[derive(Clone, PartialEq)]
pub(super) struct EditorSnapshot {
    pub(super) amplitude_curve: Curve,
    pub(super) pitch_curve: Curve,
    pub(super) active_curve: CurveKind,
    pub(super) tuning_standard: TuningStandard,
    pub(super) keytrack_enabled: bool,
    pub(super) note_length_ms: f32,
    pub(super) note_length_max_ms: f32,
    pub(super) waveform_zoom_percent: f32,
    pub(super) selected_point: Option<usize>,
}

#[derive(Clone, PartialEq)]
pub(super) struct PatchSnapshot {
    pub(super) amplitude_curve: Curve,
    pub(super) pitch_curve: Curve,
    pub(super) active_curve: CurveKind,
    pub(super) tuning_standard: TuningStandard,
    pub(super) keytrack_enabled: bool,
    pub(super) note_length_ms: f32,
    pub(super) note_length_max_ms: f32,
    pub(super) waveform_zoom_percent: f32,
}

impl Curve {
    pub(super) fn default_amplitude() -> Self {
        Self {
            points: vec![
                Pos2::new(0.0, 1.0),
                Pos2::new(0.12, 0.94),
                Pos2::new(0.42, 0.24),
                Pos2::new(1.0, 0.0),
            ],
            bends: vec![0.0; 3],
        }
    }

    pub(super) fn default_pitch() -> Self {
        Self {
            points: vec![
                Pos2::new(0.0, 1.0),
                Pos2::new(0.08, 0.98),
                Pos2::new(0.30, 0.30),
                Pos2::new(1.0, 0.08),
            ],
            bends: vec![0.0; 3],
        }
    }
}

pub(super) struct BezierUiState {
    pub(super) amplitude_curve: Curve,
    pub(super) pitch_curve: Curve,
    pub(super) active_curve: CurveKind,
    pub(super) tuning_standard: TuningStandard,
    pub(super) keytrack_enabled: bool,
    pub(super) note_length_ms: f32,
    pub(super) note_length_max_ms: f32,
    pub(super) base_note_length_max_ms: f32,
    pub(super) waveform_zoom_percent: f32,
    pub(super) selected_point: Option<usize>,
    pub(super) selected_points: Vec<usize>,
    pub(super) selection_drag_start: Option<Pos2>,
    pub(super) selection_drag_current: Option<Pos2>,
    pub(super) shift_locked_point: Option<usize>,
    pub(super) shift_lock_x_freeze_until_seconds: f64,
    pub(super) shift_lock_require_horizontal_reengage: bool,
    pub(super) shift_lock_reengage_anchor_screen_x: Option<f32>,
    pub(super) edge_bend_drag_segment: Option<usize>,
    pub(super) edge_bend_drag_start_pointer_y: Option<f32>,
    pub(super) edge_bend_drag_start_value: f32,
    pub(super) undo_stack: Vec<EditorSnapshot>,
    pub(super) redo_stack: Vec<EditorSnapshot>,
    pub(super) point_drag_snapshot: Option<EditorSnapshot>,
    pub(super) brand_logo: Option<TextureHandle>,
    pub(super) show_help_popup: bool,
    pub(super) available_patches: Vec<String>,
    pub(super) selected_patch_name: Option<String>,
    pub(super) selected_patch_snapshot: Option<PatchSnapshot>,
    pub(super) default_patch_name: Option<String>,
    pub(super) new_patch_name: String,
    pub(super) patch_status: Option<String>,
}

impl Default for BezierUiState {
    fn default() -> Self {
        let note_length_max_ms = config::app_config().note_length_max_ms;
        let mut state = Self {
            amplitude_curve: Curve::default_amplitude(),
            pitch_curve: Curve::default_pitch(),
            active_curve: CurveKind::Amplitude,
            tuning_standard: TuningStandard::A432,
            keytrack_enabled: false,
            note_length_ms: note_length_max_ms,
            note_length_max_ms,
            base_note_length_max_ms: note_length_max_ms,
            waveform_zoom_percent: 100.0,
            selected_point: Some(1),
            selected_points: vec![1],
            selection_drag_start: None,
            selection_drag_current: None,
            shift_locked_point: None,
            shift_lock_x_freeze_until_seconds: 0.0,
            shift_lock_require_horizontal_reengage: false,
            shift_lock_reengage_anchor_screen_x: None,
            edge_bend_drag_segment: None,
            edge_bend_drag_start_pointer_y: None,
            edge_bend_drag_start_value: 0.0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            point_drag_snapshot: None,
            brand_logo: None,
            show_help_popup: false,
            available_patches: Vec::new(),
            selected_patch_name: None,
            selected_patch_snapshot: None,
            default_patch_name: None,
            new_patch_name: String::new(),
            patch_status: None,
        };

        if let Err(error) = patches::ensure_default_patch_setup() {
            state.patch_status = Some(error);
        }
        state.refresh_patch_list();

        match patches::get_default_patch_name() {
            Ok(Some(default_name)) => {
                state.default_patch_name = Some(default_name.clone());
                match patches::load_patch(&default_name) {
                    Ok(patch) => {
                        state.apply_patch_data(patch);
                        state.mark_patch_clean(default_name.clone());
                        state.patch_status = Some(format!("Loaded default patch: {default_name}"));
                    }
                    Err(error) => {
                        state.patch_status = Some(format!("Failed to load default patch: {error}"));
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                state.patch_status = Some(format!("Failed to read default patch: {error}"));
            }
        }

        state
    }
}

impl BezierUiState {
    fn push_bounded_snapshot(stack: &mut Vec<EditorSnapshot>, snapshot: EditorSnapshot) {
        stack.push(snapshot);
        let overflow = stack.len().saturating_sub(HISTORY_STACK_CAP);
        if overflow > 0 {
            stack.drain(0..overflow);
        }
    }

    pub(super) fn push_undo_snapshot(&mut self, snapshot: EditorSnapshot) {
        if self.snapshot() != snapshot {
            Self::push_bounded_snapshot(&mut self.undo_stack, snapshot);
            self.redo_stack.clear();
        }
    }

    pub(super) fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            amplitude_curve: self.amplitude_curve.clone(),
            pitch_curve: self.pitch_curve.clone(),
            active_curve: self.active_curve,
            tuning_standard: self.tuning_standard,
            keytrack_enabled: self.keytrack_enabled,
            note_length_ms: self.note_length_ms,
            note_length_max_ms: self.note_length_max_ms,
            waveform_zoom_percent: self.waveform_zoom_percent,
            selected_point: self.selected_point,
        }
    }

    fn apply_snapshot(&mut self, snapshot: EditorSnapshot) {
        self.amplitude_curve = snapshot.amplitude_curve;
        self.pitch_curve = snapshot.pitch_curve;
        self.active_curve = snapshot.active_curve;
        self.tuning_standard = snapshot.tuning_standard;
        self.keytrack_enabled = snapshot.keytrack_enabled;
        self.note_length_ms = snapshot.note_length_ms;
        self.note_length_max_ms = snapshot.note_length_max_ms;
        self.waveform_zoom_percent = snapshot.waveform_zoom_percent;
        self.selected_point = snapshot.selected_point;
    }

    pub(super) fn commit_history_if_changed(&mut self, before: &EditorSnapshot) {
        self.push_undo_snapshot(before.clone());
    }

    pub(super) fn undo(&mut self) -> bool {
        if let Some(snapshot) = self.undo_stack.pop() {
            let current = self.snapshot();
            Self::push_bounded_snapshot(&mut self.redo_stack, current);
            self.apply_snapshot(snapshot);
            return true;
        }
        false
    }

    pub(super) fn redo(&mut self) -> bool {
        if let Some(snapshot) = self.redo_stack.pop() {
            let current = self.snapshot();
            Self::push_bounded_snapshot(&mut self.undo_stack, current);
            self.apply_snapshot(snapshot);
            return true;
        }
        false
    }

    pub(super) fn active_curve(&self) -> &Curve {
        match self.active_curve {
            CurveKind::Amplitude => &self.amplitude_curve,
            CurveKind::Pitch => &self.pitch_curve,
        }
    }

    pub(super) fn active_curve_mut(&mut self) -> &mut Curve {
        match self.active_curve {
            CurveKind::Amplitude => &mut self.amplitude_curve,
            CurveKind::Pitch => &mut self.pitch_curve,
        }
    }

    pub(super) fn refresh_patch_list(&mut self) {
        match patches::list_patch_names() {
            Ok(patches) => {
                self.available_patches = patches;
            }
            Err(error) => {
                self.patch_status = Some(error);
            }
        }
    }

    fn patch_snapshot(&self) -> PatchSnapshot {
        PatchSnapshot {
            amplitude_curve: self.amplitude_curve.clone(),
            pitch_curve: self.pitch_curve.clone(),
            active_curve: self.active_curve,
            tuning_standard: self.tuning_standard,
            keytrack_enabled: self.keytrack_enabled,
            note_length_ms: self.note_length_ms,
            note_length_max_ms: self.note_length_max_ms,
            waveform_zoom_percent: self.waveform_zoom_percent,
        }
    }

    pub(super) fn is_selected_patch_dirty(&self) -> bool {
        self.selected_patch_snapshot
            .as_ref()
            .is_some_and(|saved| self.patch_snapshot() != *saved)
    }

    pub(super) fn selected_patch_indicator_text(&self) -> String {
        match &self.selected_patch_name {
            Some(name) => {
                let mut label = name.clone();
                if self.is_selected_patch_dirty() {
                    label.push('*');
                }
                if self.default_patch_name.as_deref() == Some(name.as_str()) {
                    label.push_str(" (default)");
                }
                label
            }
            None => "No patch selected".to_owned(),
        }
    }

    pub(super) fn mark_patch_clean(&mut self, patch_name: String) {
        self.selected_patch_name = Some(patch_name.clone());
        self.selected_patch_snapshot = Some(self.patch_snapshot());
        self.new_patch_name = patch_name;
    }

    pub(super) fn to_patch_data(&self, name: String) -> patches::PatchData {
        patches::PatchData {
            name,
            tuning_a4_hz: self.tuning_standard.a4_hz(),
            keytrack_enabled: self.keytrack_enabled,
            note_end_ms: self.note_length_ms,
            max_note_length_ms: self.note_length_max_ms,
            waveform_zoom_percent: self.waveform_zoom_percent,
            active_curve: match self.active_curve {
                CurveKind::Amplitude => "amplitude".to_owned(),
                CurveKind::Pitch => "pitch".to_owned(),
            },
            amplitude_points: self
                .amplitude_curve
                .points
                .iter()
                .map(|point| (point.x, point.y))
                .collect(),
            amplitude_bends: self.amplitude_curve.bends.clone(),
            pitch_points: self
                .pitch_curve
                .points
                .iter()
                .map(|point| (point.x, point.y))
                .collect(),
            pitch_bends: self.pitch_curve.bends.clone(),
        }
    }

    pub(super) fn apply_patch_data(&mut self, patch: patches::PatchData) {
        self.amplitude_curve.points =
            points_from_patch(&patch.amplitude_points, &Curve::default_amplitude().points);
        self.amplitude_curve.bends = bends_from_patch(&patch.amplitude_bends, self.amplitude_curve.points.len());
        self.pitch_curve.points = points_from_patch(&patch.pitch_points, &Curve::default_pitch().points);
        self.pitch_curve.bends = bends_from_patch(&patch.pitch_bends, self.pitch_curve.points.len());
        self.active_curve = if patch.active_curve.eq_ignore_ascii_case("pitch") {
            CurveKind::Pitch
        } else {
            CurveKind::Amplitude
        };
        self.tuning_standard = tuning_standard_from_a4_hz(patch.tuning_a4_hz);
        self.keytrack_enabled = patch.keytrack_enabled;

        self.note_length_max_ms = patch
            .max_note_length_ms
            .clamp(NOTE_LENGTH_MAX_SLIDER_MIN_MS, NOTE_LENGTH_MAX_SLIDER_MAX_MS);
        self.base_note_length_max_ms = self.note_length_max_ms;
        self.note_length_ms = patch.note_end_ms.clamp(0.0, self.note_length_max_ms);

        let app_cfg = config::app_config();
        self.waveform_zoom_percent = patch.waveform_zoom_percent.clamp(
            app_cfg.waveform_zoom_min_percent,
            app_cfg.waveform_zoom_max_percent,
        );

        self.selection_drag_start = None;
        self.selection_drag_current = None;
        let selected_index = 1.min(self.active_curve().points.len().saturating_sub(1));
        self.selected_point = Some(selected_index);
        self.selected_points = vec![selected_index];
    }
}

fn points_from_patch(raw_points: &[(f32, f32)], fallback: &[Pos2]) -> Vec<Pos2> {
    let mut points: Vec<Pos2> = raw_points
        .iter()
        .map(|(x, y)| Pos2::new(x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)))
        .collect();

    if points.len() < 2 {
        return fallback.to_vec();
    }

    points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(Ordering::Equal));
    constrain_curve_points(&mut points);
    points
}

fn bends_from_patch(raw_bends: &[f32], point_count: usize) -> Vec<f32> {
    let mut bends = raw_bends.to_vec();
    let dummy_points = vec![Pos2::ZERO; point_count.max(2)];
    normalize_segment_bends(&dummy_points, &mut bends);
    bends
}
