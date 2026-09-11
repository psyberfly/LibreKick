use std::cmp::Ordering;

use nih_plug_egui::egui::{Pos2, TextureHandle};

use crate::{config, patches, shared};

use super::helpers::{constrain_curve_points, normalize_segment_bends};

pub(super) const HISTORY_STACK_CAP: usize = 200;
pub(super) const NOTE_LENGTH_MAX_SLIDER_MIN_MS: f32 = 100.0;
pub(super) const NOTE_LENGTH_MAX_SLIDER_MAX_MS: f32 = 5000.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CurveKind {
    Amplitude,
    Pitch,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TuningStandard {
    A440,
    A432,
}

impl TuningStandard {
    pub(super) fn a4_hz(self) -> f32 {
        match self {
            TuningStandard::A440 => 440.0,
            TuningStandard::A432 => 432.0,
        }
    }
}

pub(super) fn tuning_standard_from_a4_hz(hz: f32) -> TuningStandard {
    if (hz - 432.0).abs() <= (hz - 440.0).abs() {
        TuningStandard::A432
    } else {
        TuningStandard::A440
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct Curve {
    pub(super) points: Vec<Pos2>,
    pub(super) bends: Vec<f32>,
}

/// All per-slot bass settings. The bass page has two slots (Note 1 / Note 2);
/// arrange notes pick which slot they play.
#[derive(Clone, PartialEq)]
pub(super) struct BassSlot {
    pub(super) amp_curve: Curve,
    pub(super) filter_curve: Curve,
    pub(super) filter_curve_2: Curve,
    pub(super) oscillator_waveform: shared::Waveform,
    pub(super) retrigger: bool,
    pub(super) legato_voice_steal: bool,
    pub(super) note_length_ms: f32,
    pub(super) pitch_hz: f32,
    pub(super) cutoff_hz: f32,
    pub(super) filter_mode: shared::BassFilterMode,
    pub(super) filter_slope: shared::BassFilterSlope,
    pub(super) filter_drive: f32,
    pub(super) filter_enabled: bool,
    pub(super) filter_2_enabled: bool,
    pub(super) filter_selected: usize,
    pub(super) keytrack_enabled: bool,
    pub(super) phase_deg: f32,
}

impl Default for BassSlot {
    fn default() -> Self {
        Self {
            amp_curve: Curve::default_amplitude(),
            filter_curve: Curve::default_pitch(),
            filter_curve_2: Curve::default_pitch(),
            oscillator_waveform: shared::Waveform::Saw,
            retrigger: true,
            legato_voice_steal: false,
            note_length_ms: 220.0,
            pitch_hz: 55.0,
            cutoff_hz: 120.0,
            filter_mode: shared::BassFilterMode::LowPass,
            filter_slope: shared::BassFilterSlope::S6dB,
            filter_drive: 0.0,
            filter_enabled: true,
            filter_2_enabled: false,
            filter_selected: 0,
            keytrack_enabled: false,
            phase_deg: 0.0,
        }
    }
}

/// Core patch data shared between EditorSnapshot and PatchSnapshot.
/// This eliminates duplication of ~20 fields between the two structs.
#[derive(Clone, PartialEq)]
pub(super) struct CorePatchData {
    pub(super) amplitude_curve: Curve,
    pub(super) pitch_curve: Curve,
    pub(super) active_curve: CurveKind,
    pub(super) tuning_standard: TuningStandard,
    pub(super) keytrack_enabled: bool,
    pub(super) kick_oscillator_waveform: shared::Waveform,
    pub(super) kick_retrigger: bool,
    pub(super) kick_legato_voice_steal: bool,
    pub(super) kick_pitch_hz: f32,
    pub(super) kick_phase_deg: f32,
    pub(super) note_length_ms: f32,
    pub(super) note_length_max_ms: f32,
    pub(super) waveform_zoom_percent: f32,
    /// Bass settings for both note slots (index 0 = Note 1, 1 = Note 2).
    pub(super) bass: [BassSlot; 2],
    pub(super) num_bars: f32,
    pub(super) note_size: NoteSize,
    pub(super) use_daw_tempo: bool,
    pub(super) manual_tempo: f32,
    /// When true, held DAW notes drive the internal arrange pattern instead
    /// of routing MIDI directly to the voices.
    pub(super) arrange_override: bool,
    pub(super) midi_notes: Vec<ArrangeNote>,
}

/// Editor snapshot for undo/redo - includes UI-specific state
#[derive(Clone, PartialEq)]
pub(super) struct EditorSnapshot {
    pub(super) core: CorePatchData,
    // Editor-only fields
    pub(super) selected_point: Option<usize>,
    pub(super) bass_amp_selected_point: Option<usize>,
    pub(super) bass_filter_selected_point: Option<usize>,
    pub(super) bass_filter_2_selected_point: Option<usize>,
}

/// Patch snapshot for save/load - includes patch metadata
#[derive(Clone, PartialEq)]
pub(super) struct PatchSnapshot {
    pub(super) core: CorePatchData,
    // Patch-only fields
    pub(super) kick_level: f32,
    /// Per-slot bass levels (index 0 = Note 1, 1 = Note 2).
    pub(super) bass_levels: [f32; 2],
    pub(super) description: String,
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

/// A note placed on the arrange page MIDI channel.
/// `row`: 0-11 = bass octave (B at top .. C at bottom), 12 = kick lane.
/// `bar_pos`: position in bars (fractional, e.g. 1.5 = middle of bar 2).
/// `slot`: which bass note slot plays it (0 = Note 1, 1 = Note 2; bass rows only).
/// Notes are fixed-length: one beat (0.25 bars in 4/4).
#[derive(Clone, Copy, PartialEq)]
pub(super) struct ArrangeNote {
    pub(super) row: usize,
    pub(super) bar_pos: f32,
    pub(super) slot: u8,
}

/// Note size options for the arrange grid, expressed as a fraction of a 4/4 bar.
/// Ordered finest to coarsest (left to right on the slider).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NoteSize {
    Sixteenth, // 1/16 = 0.0625 bars
    Eighth,    // 1/8  = 0.125 bars
    Quarter,   // 1/4  = 0.25 bars
    Whole,     // 1    = 1.0 bars
}

impl NoteSize {
    pub(super) const ALL: [NoteSize; 4] = [
        NoteSize::Sixteenth,
        NoteSize::Eighth,
        NoteSize::Quarter,
        NoteSize::Whole,
    ];

    /// Length of one note in bars.
    pub(super) fn bars(self) -> f32 {
        match self {
            NoteSize::Sixteenth => 0.0625,
            NoteSize::Eighth => 0.125,
            NoteSize::Quarter => 0.25,
            NoteSize::Whole => 1.0,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            NoteSize::Sixteenth => "1/16",
            NoteSize::Eighth => "1/8",
            NoteSize::Quarter => "1/4",
            NoteSize::Whole => "1",
        }
    }

    pub(super) fn from_label(raw: &str) -> Option<Self> {
        match raw.trim() {
            "1/16" => Some(NoteSize::Sixteenth),
            "1/8" => Some(NoteSize::Eighth),
            "1/4" => Some(NoteSize::Quarter),
            "1" => Some(NoteSize::Whole),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum UiPage {
    Kick,
    Bass,
    Arrange,
    Settings,
    Oscilloscope,
    Logs,
}

pub(super) struct BezierUiState {
    pub(super) active_page: UiPage,
    pub(super) amplitude_curve: Curve,
    pub(super) pitch_curve: Curve,
    pub(super) active_curve: CurveKind,
    pub(super) tuning_standard: TuningStandard,
    pub(super) keytrack_enabled: bool,
    pub(super) kick_oscillator_waveform: shared::Waveform,
    pub(super) kick_retrigger: bool,
    pub(super) kick_legato_voice_steal: bool,
    pub(super) kick_pitch_hz: f32,
    /// Kick oscillator start phase in degrees (0-360), applied on retrigger.
    pub(super) kick_phase_deg: f32,
    /// Mirror of the `kick_level` plugin parameter, synced each frame so
    /// patch dirty-tracking and saving can see it.
    pub(super) kick_level: f32,
    pub(super) note_length_ms: f32,
    pub(super) note_length_max_ms: f32,
    /// Mirror of the `bass1_level`/`bass2_level` plugin parameters, synced
    /// each frame (index 0 = Note 1, 1 = Note 2).
    pub(super) bass_levels: [f32; 2],
    /// Bass settings for both note slots (index 0 = Note 1, 1 = Note 2).
    pub(super) bass: [BassSlot; 2],
    /// Which bass slot the bass page is currently editing (0 or 1).
    pub(super) bass_selected: usize,
    pub(super) manual_tempo: f32,
    pub(super) use_daw_tempo: bool,
    pub(super) num_bars: f32,
    pub(super) note_size: NoteSize,
    /// When true, held DAW notes drive the internal arrange pattern instead
    /// of routing MIDI directly to the voices.
    pub(super) arrange_override: bool,
    pub(super) midi_channel_zoom: f32,
    pub(super) midi_channel_scroll_offset: f32,
    /// Horizontal zoom for the Audio Clip preview (1.0 = whole clip).
    pub(super) clip_zoom: f32,
    /// Scroll position of the Audio Clip preview, in bars.
    pub(super) clip_scroll_offset: f32,
    pub(super) midi_notes: Vec<ArrangeNote>,
    pub(super) dragging_note: Option<usize>,
    /// Cached offline-rendered waveforms for the Audio Clip preview.
    pub(super) clip_preview_kick: Vec<f32>,
    pub(super) clip_preview_bass: Vec<f32>,
    /// Hash of the inputs used to render `clip_preview_samples`.
    pub(super) clip_preview_key: u64,
    pub(super) osc_hold: bool,
    pub(super) osc_zoom_x: f32,
    pub(super) osc_zoom_y: f32,
    pub(super) osc_show_kick: bool,
    pub(super) osc_show_bass: bool,
    pub(super) osc_show_sum: bool,
    pub(super) osc_snapshot: shared::OscilloscopeSnapshot,
    pub(super) base_note_length_max_ms: f32,
    pub(super) waveform_zoom_percent: f32,
    pub(super) selected_point: Option<usize>,
    pub(super) bass_amp_selected_point: Option<usize>,
    pub(super) bass_filter_selected_point: Option<usize>,
    pub(super) bass_filter_2_selected_point: Option<usize>,
    pub(super) selected_points: Vec<usize>,
    pub(super) selection_drag_start: Option<Pos2>,
    pub(super) selection_drag_current: Option<Pos2>,
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
    /// Description of the current patch, shown in the menu bar and
    /// edited in the Patches menu.
    pub(super) patch_description: String,
    /// User-adjustable display scale multiplier (1.0 = automatic from window size).
    pub(super) display_scale: f32,
}

impl Default for BezierUiState {
    fn default() -> Self {
        let note_length_max_ms = config::app_config().note_length_max_ms;
        let mut state = Self {
            active_page: UiPage::Kick,
            amplitude_curve: Curve::default_amplitude(),
            pitch_curve: Curve::default_pitch(),
            active_curve: CurveKind::Amplitude,
            tuning_standard: TuningStandard::A432,
            keytrack_enabled: false,
            kick_oscillator_waveform: shared::Waveform::Sine,
            kick_retrigger: true,
            kick_legato_voice_steal: true,
            kick_pitch_hz: 55.0,
            kick_phase_deg: 0.0,
            kick_level: 0.8,
            note_length_ms: note_length_max_ms,
            note_length_max_ms,
            bass_levels: [0.8; 2],
            bass: [BassSlot::default(), BassSlot::default()],
            bass_selected: 0,
            manual_tempo: 120.0,
            use_daw_tempo: true,
            num_bars: 1.0,
            note_size: NoteSize::Quarter,
            arrange_override: false,
            midi_channel_zoom: 1.0,
            midi_channel_scroll_offset: 0.0,
            clip_zoom: 1.0,
            clip_scroll_offset: 0.0,
            midi_notes: Vec::new(),
            dragging_note: None,
            clip_preview_kick: Vec::new(),
            clip_preview_bass: Vec::new(),
            clip_preview_key: 0,
            osc_hold: false,
            osc_zoom_x: 1.0,
            osc_zoom_y: 1.0,
            osc_show_kick: true,
            osc_show_bass: true,
            osc_show_sum: true,
            osc_snapshot: shared::OscilloscopeSnapshot {
                kick: [0.0; shared::OSCILLOSCOPE_BUFFER_SIZE],
                bass: [0.0; shared::OSCILLOSCOPE_BUFFER_SIZE],
                sum: [0.0; shared::OSCILLOSCOPE_BUFFER_SIZE],
                len: 0,
                sequence: 0,
            },
            base_note_length_max_ms: note_length_max_ms,
            waveform_zoom_percent: 100.0,
            selected_point: Some(1),
            bass_amp_selected_point: Some(1),
            bass_filter_selected_point: Some(1),
            bass_filter_2_selected_point: Some(1),
            selected_points: vec![1],
            selection_drag_start: None,
            selection_drag_current: None,
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
            patch_description: String::new(),
            display_scale: 1.0,
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

        match patches::load_settings() {
            Ok(Some(settings)) => {
                state.tuning_standard = tuning_standard_from_a4_hz(settings.tuning_a4_hz);
                state.display_scale = settings.display_scale;
            }
            Ok(None) => {}
            Err(error) => {
                state.patch_status = Some(format!("Failed to load settings: {error}"));
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
            core: CorePatchData {
                amplitude_curve: self.amplitude_curve.clone(),
                pitch_curve: self.pitch_curve.clone(),
                active_curve: self.active_curve,
                tuning_standard: self.tuning_standard,
                keytrack_enabled: self.keytrack_enabled,
                kick_oscillator_waveform: self.kick_oscillator_waveform,
                kick_retrigger: self.kick_retrigger,
                kick_legato_voice_steal: self.kick_legato_voice_steal,
                kick_pitch_hz: self.kick_pitch_hz,
                kick_phase_deg: self.kick_phase_deg,
                note_length_ms: self.note_length_ms,
                note_length_max_ms: self.note_length_max_ms,
                waveform_zoom_percent: self.waveform_zoom_percent,
                bass: self.bass.clone(),
                num_bars: self.num_bars,
                note_size: self.note_size,
                use_daw_tempo: self.use_daw_tempo,
                manual_tempo: self.manual_tempo,
                arrange_override: self.arrange_override,
                midi_notes: self.midi_notes.clone(),
            },
            selected_point: self.selected_point,
            bass_amp_selected_point: self.bass_amp_selected_point,
            bass_filter_selected_point: self.bass_filter_selected_point,
            bass_filter_2_selected_point: self.bass_filter_2_selected_point,
        }
    }

    fn apply_snapshot(&mut self, snapshot: EditorSnapshot) {
        self.amplitude_curve = snapshot.core.amplitude_curve;
        self.pitch_curve = snapshot.core.pitch_curve;
        self.active_curve = snapshot.core.active_curve;
        self.tuning_standard = snapshot.core.tuning_standard;
        self.keytrack_enabled = snapshot.core.keytrack_enabled;
        self.kick_oscillator_waveform = snapshot.core.kick_oscillator_waveform;
        self.kick_retrigger = snapshot.core.kick_retrigger;
        self.kick_legato_voice_steal = snapshot.core.kick_legato_voice_steal;
        self.kick_pitch_hz = snapshot.core.kick_pitch_hz;
        self.kick_phase_deg = snapshot.core.kick_phase_deg;
        self.note_length_ms = snapshot.core.note_length_ms;
        self.note_length_max_ms = snapshot.core.note_length_max_ms;
        self.waveform_zoom_percent = snapshot.core.waveform_zoom_percent;
        self.selected_point = snapshot.selected_point;
        self.bass = snapshot.core.bass;
        self.num_bars = snapshot.core.num_bars;
        self.note_size = snapshot.core.note_size;
        self.use_daw_tempo = snapshot.core.use_daw_tempo;
        self.manual_tempo = snapshot.core.manual_tempo;
        self.arrange_override = snapshot.core.arrange_override;
        self.midi_notes = snapshot.core.midi_notes;
        self.dragging_note = None;
        self.bass_amp_selected_point = snapshot.bass_amp_selected_point;
        self.bass_filter_selected_point = snapshot.bass_filter_selected_point;
        self.bass_filter_2_selected_point = snapshot.bass_filter_2_selected_point;
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
            core: CorePatchData {
                amplitude_curve: self.amplitude_curve.clone(),
                pitch_curve: self.pitch_curve.clone(),
                active_curve: self.active_curve,
                tuning_standard: self.tuning_standard,
                keytrack_enabled: self.keytrack_enabled,
                kick_oscillator_waveform: self.kick_oscillator_waveform,
                kick_retrigger: self.kick_retrigger,
                kick_legato_voice_steal: self.kick_legato_voice_steal,
                kick_pitch_hz: self.kick_pitch_hz,
                kick_phase_deg: self.kick_phase_deg,
                note_length_ms: self.note_length_ms,
                note_length_max_ms: self.note_length_max_ms,
                waveform_zoom_percent: self.waveform_zoom_percent,
                bass: self.bass.clone(),
                num_bars: self.num_bars,
                note_size: self.note_size,
                use_daw_tempo: self.use_daw_tempo,
                manual_tempo: self.manual_tempo,
                arrange_override: self.arrange_override,
                midi_notes: self.midi_notes.clone(),
            },
            kick_level: self.kick_level,
            bass_levels: self.bass_levels,
            description: self.patch_description.clone(),
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

    /// Syncs the current UI state (curves, oscillator settings, etc.) into the
    /// shared state so the DSP can use them. Called at plugin initialization
    /// and every frame the kick/bass pages render.
    pub fn sync_to_shared(&self, shared: &shared::SharedStateHandle) {
        use crate::ui::helpers::curve_lut;

        // Kick curves and settings
        let amp_lut = curve_lut(&self.amplitude_curve.points, &self.amplitude_curve.bends);
        let pitch_lut = curve_lut(&self.pitch_curve.points, &self.pitch_curve.bends);
        shared::set_curve_lut(shared, shared::CurveKind::Amplitude, amp_lut);
        shared::set_curve_lut(shared, shared::CurveKind::Pitch, pitch_lut);
        shared::set_keytrack_enabled(shared, self.keytrack_enabled);
        shared::set_note_length_ms(shared, self.note_length_ms);
        shared::set_kick_oscillator_waveform(shared, self.kick_oscillator_waveform);
        shared::set_kick_retrigger(shared, self.kick_retrigger);
        shared::set_kick_legato_voice_steal(shared, self.kick_legato_voice_steal);
        shared::set_kick_pitch_hz(shared, self.kick_pitch_hz);
        shared::set_kick_phase_deg(shared, self.kick_phase_deg);

        // Bass curves and settings — both note slots
        for (index, slot) in self.bass.iter().enumerate() {
            let bass_amp_lut = curve_lut(&slot.amp_curve.points, &slot.amp_curve.bends);
            let bass_filter_lut =
                curve_lut(&slot.filter_curve.points, &slot.filter_curve.bends);
            let bass_filter_2_lut =
                curve_lut(&slot.filter_curve_2.points, &slot.filter_curve_2.bends);
            shared::set_bass_slot(
                shared,
                index,
                shared::BassSlotParams {
                    amp_lut: bass_amp_lut,
                    filter_lut: bass_filter_lut,
                    filter_2_lut: bass_filter_2_lut,
                    filter_enabled: slot.filter_enabled,
                    filter_2_enabled: slot.filter_2_enabled,
                    note_length_ms: slot.note_length_ms,
                    cutoff_hz: slot.cutoff_hz,
                    filter_mode: slot.filter_mode,
                    filter_slope: slot.filter_slope,
                    filter_drive: slot.filter_drive,
                    pitch_hz: slot.pitch_hz,
                    retrigger: slot.retrigger,
                    legato_voice_steal: slot.legato_voice_steal,
                    oscillator_waveform: slot.oscillator_waveform,
                    keytrack_enabled: slot.keytrack_enabled,
                    phase_deg: slot.phase_deg,
                },
            );
        }
        shared::set_arrange_override(shared, self.arrange_override);
        let arrange_notes: Vec<(usize, f32, u8)> = self
            .midi_notes
            .iter()
            .map(|note| (note.row, note.bar_pos, note.slot))
            .collect();
        shared::set_arrange_pattern(
            shared,
            &arrange_notes,
            self.num_bars,
            self.note_size.bars(),
            self.manual_tempo,
            self.use_daw_tempo,
        );
    }

    pub(super) fn to_patch_data(&self, name: String) -> patches::PatchData {
        patches::PatchData {
            name,
            description: patches::sanitize_patch_description(&self.patch_description),
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
            bass: Some(bass_slot_to_patch(&self.bass[0], Some(self.bass_levels[0]))),
            bass2: Some(bass_slot_to_patch(&self.bass[1], Some(self.bass_levels[1]))),
            kick: Some(patches::KickPatchData {
                oscillator_waveform: waveform_to_patch(self.kick_oscillator_waveform)
                    .to_owned(),
                retrigger: self.kick_retrigger,
                legato_voice_steal: self.kick_legato_voice_steal,
                pitch_hz: self.kick_pitch_hz,
                level: Some(self.kick_level),
                phase_deg: Some(self.kick_phase_deg),
            }),
            arrange: Some(patches::ArrangePatchData {
                num_bars: self.num_bars,
                note_size: self.note_size.label().to_owned(),
                use_daw_tempo: self.use_daw_tempo,
                manual_tempo: self.manual_tempo,
                override_daw_midi: Some(self.arrange_override),
                notes: self
                    .midi_notes
                    .iter()
                    .map(|note| (note.row, note.bar_pos, note.slot))
                    .collect(),
            }),
        }
    }

    /// Applies patch data to the UI state. Returns the (kick, bass) levels
    /// stored in the patch, if any, so the caller can forward them to the
    /// corresponding plugin parameters.
    pub(super) fn apply_patch_data(
        &mut self,
        patch: patches::PatchData,
    ) -> (Option<f32>, [Option<f32>; 2]) {
        self.patch_description = patch.description;
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

        let bass_levels = [
            patch.bass.as_ref().and_then(|bass| bass.level),
            patch.bass2.as_ref().and_then(|bass| bass.level),
        ];
        if let Some(bass) = patch.bass {
            apply_bass_patch(&mut self.bass[0], &bass);
        }
        // Note 2 defaults to a duplicate of Note 1 when the patch has no
        // second slot saved.
        self.bass[1] = self.bass[0].clone();
        if let Some(bass2) = patch.bass2 {
            apply_bass_patch(&mut self.bass[1], &bass2);
        }
        self.bass_selected = 0;

        let kick_level = patch.kick.as_ref().and_then(|kick| kick.level);
        if let Some(kick) = patch.kick {
            if let Some(waveform) = waveform_from_patch(&kick.oscillator_waveform) {
                self.kick_oscillator_waveform = waveform;
            }
            self.kick_retrigger = kick.retrigger;
            self.kick_legato_voice_steal = kick.legato_voice_steal;
            self.kick_pitch_hz = kick.pitch_hz.clamp(20.0, 2_000.0);
            if let Some(phase_deg) = kick.phase_deg {
                self.kick_phase_deg = phase_deg.clamp(0.0, 360.0);
            }
        }

        if let Some(arrange) = patch.arrange {
            self.num_bars = arrange.num_bars.clamp(0.25, 8.0);
            if let Some(note_size) = NoteSize::from_label(&arrange.note_size) {
                self.note_size = note_size;
            }
            self.use_daw_tempo = arrange.use_daw_tempo;
            self.manual_tempo = arrange.manual_tempo.clamp(20.0, 300.0);
            if let Some(override_daw_midi) = arrange.override_daw_midi {
                self.arrange_override = override_daw_midi;
            }
            self.midi_notes = arrange
                .notes
                .into_iter()
                .filter(|(row, _, _)| *row < 13)
                .map(|(row, bar_pos, slot)| ArrangeNote {
                    row,
                    bar_pos,
                    slot: slot.min(1),
                })
                .collect();
            self.dragging_note = None;
            self.midi_channel_scroll_offset = 0.0;
        }

        self.selection_drag_start = None;
        self.selection_drag_current = None;
        let selected_index = 1.min(self.active_curve().points.len().saturating_sub(1));
        self.selected_point = Some(selected_index);
        self.selected_points = vec![selected_index];

        (kick_level, bass_levels)
    }
}

fn bass_slot_to_patch(slot: &BassSlot, level: Option<f32>) -> patches::BassPatchData {
    patches::BassPatchData {
        oscillator_waveform: waveform_to_patch(slot.oscillator_waveform).to_owned(),
        retrigger: slot.retrigger,
        legato_voice_steal: slot.legato_voice_steal,
        note_length_ms: slot.note_length_ms,
        pitch_hz: slot.pitch_hz,
        cutoff_hz: slot.cutoff_hz,
        filter_mode: bass_filter_mode_to_patch(slot.filter_mode).to_owned(),
        filter_slope: bass_filter_slope_to_patch(slot.filter_slope).to_owned(),
        filter_drive: slot.filter_drive,
        filter_enabled: slot.filter_enabled,
        filter_2_enabled: slot.filter_2_enabled,
        amp_points: slot
            .amp_curve
            .points
            .iter()
            .map(|point| (point.x, point.y))
            .collect(),
        amp_bends: slot.amp_curve.bends.clone(),
        filter_points: slot
            .filter_curve
            .points
            .iter()
            .map(|point| (point.x, point.y))
            .collect(),
        filter_bends: slot.filter_curve.bends.clone(),
        filter_2_points: slot
            .filter_curve_2
            .points
            .iter()
            .map(|point| (point.x, point.y))
            .collect(),
        filter_2_bends: slot.filter_curve_2.bends.clone(),
        level,
        phase_deg: Some(slot.phase_deg),
    }
}

fn apply_bass_patch(slot: &mut BassSlot, bass: &patches::BassPatchData) {
    if let Some(waveform) = waveform_from_patch(&bass.oscillator_waveform) {
        slot.oscillator_waveform = waveform;
    }
    slot.retrigger = bass.retrigger;
    slot.legato_voice_steal = bass.legato_voice_steal;
    slot.note_length_ms = bass.note_length_ms.clamp(1.0, 1000.0);
    slot.pitch_hz = bass.pitch_hz.clamp(20.0, 2_000.0);
    slot.cutoff_hz = bass.cutoff_hz.clamp(20.0, 8_000.0);
    slot.filter_drive = bass.filter_drive.clamp(0.0, 1.0);
    slot.filter_enabled = bass.filter_enabled;
    slot.filter_2_enabled = bass.filter_2_enabled;
    if let Some(mode) = bass_filter_mode_from_patch(&bass.filter_mode) {
        slot.filter_mode = mode;
    }
    if let Some(slope) = bass_filter_slope_from_patch(&bass.filter_slope) {
        slot.filter_slope = slope;
    }
    slot.amp_curve.points =
        points_from_patch(&bass.amp_points, &Curve::default_amplitude().points);
    slot.amp_curve.bends = bends_from_patch(&bass.amp_bends, slot.amp_curve.points.len());
    slot.filter_curve.points =
        points_from_patch(&bass.filter_points, &Curve::default_pitch().points);
    slot.filter_curve.bends =
        bends_from_patch(&bass.filter_bends, slot.filter_curve.points.len());
    slot.filter_curve_2.points =
        points_from_patch(&bass.filter_2_points, &Curve::default_pitch().points);
    slot.filter_curve_2.bends =
        bends_from_patch(&bass.filter_2_bends, slot.filter_curve_2.points.len());
    if let Some(phase_deg) = bass.phase_deg {
        slot.phase_deg = phase_deg.clamp(0.0, 360.0);
    }
}

fn waveform_to_patch(waveform: shared::Waveform) -> &'static str {
    match waveform {
        shared::Waveform::Sine => "sine",
        shared::Waveform::Saw => "saw",
        shared::Waveform::Square => "square",
    }
}

fn waveform_from_patch(raw: &str) -> Option<shared::Waveform> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "sine" => Some(shared::Waveform::Sine),
        "saw" => Some(shared::Waveform::Saw),
        "square" => Some(shared::Waveform::Square),
        _ => None,
    }
}

fn bass_filter_mode_to_patch(mode: shared::BassFilterMode) -> &'static str {
    match mode {
        shared::BassFilterMode::LowPass => "lowpass",
        shared::BassFilterMode::HighPass => "highpass",
        shared::BassFilterMode::BandPass => "bandpass",
    }
}

fn bass_filter_mode_from_patch(raw: &str) -> Option<shared::BassFilterMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "lowpass" | "low" => Some(shared::BassFilterMode::LowPass),
        "highpass" | "high" => Some(shared::BassFilterMode::HighPass),
        "bandpass" | "bp" => Some(shared::BassFilterMode::BandPass),
        _ => None,
    }
}

fn bass_filter_slope_to_patch(slope: shared::BassFilterSlope) -> &'static str {
    match slope {
        shared::BassFilterSlope::S6dB => "6db",
        shared::BassFilterSlope::S12dB => "12db",
        shared::BassFilterSlope::S18dB => "18db",
        shared::BassFilterSlope::S24dB => "24db",
    }
}

fn bass_filter_slope_from_patch(raw: &str) -> Option<shared::BassFilterSlope> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "6db" | "6" => Some(shared::BassFilterSlope::S6dB),
        "12db" | "12" => Some(shared::BassFilterSlope::S12dB),
        "18db" | "18" => Some(shared::BassFilterSlope::S18dB),
        "24db" | "24" => Some(shared::BassFilterSlope::S24dB),
        _ => None,
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
