#![allow(dead_code)]

use std::sync::Arc;

use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};

use crate::config;
use crate::interface::{
    AudioEnginePort, BassCommand, BassFilterMode, CurveKind, KickCommand, UiEnginePort, Waveform,
    CURVE_LUT_SIZE,
};

mod components;
mod pages;
mod theme;

use components::nav_menu::build_nav_menu;
use components::signal_shaper::{
    constrain_curve_points, envelope_value_linear, normalize_segment_bends, SignalCurve, SignalPoint,
    SignalShaperEvent, SignalShaperState,
};
use pages::{
    bass::build_bass_page, drum_machine::build_drum_machine_page, kick::build_kick_page,
    logs::build_logs_page, oscilloscope::build_oscilloscope_page, settings::build_settings_page,
};

const APP_SCAFFOLD_STYLE: &str = r#"
.app-root {
    width: 100%;
    height: 100%;
    background-color: #111315;
}

.app-scaffold {
    width: 100%;
    height: 100%;
    col-between: 12px;
    child-left: 12px;
    child-right: 12px;
    child-top: 12px;
    child-bottom: 12px;
}

.app-content {
    size: 1s;
    background-color: #171a1d;
    border-color: #2a2f35;
    border-width: 1px;
    child-left: 16px;
    child-right: 16px;
    child-top: 16px;
    child-bottom: 16px;
    row-between: 10px;
}

.placeholder-panel {
    size: 1s;
    background-color: #121518;
    border-color: #2f3740;
    border-width: 1px;
}

.app-nav {
    width: 250px;
    background-color: #0f1113;
    border-color: #2a2f35;
    border-width: 1px;
    row-between: 8px;
    child-left: 12px;
    child-right: 12px;
    child-top: 12px;
    child-bottom: 12px;
}

.page-button {
    width: 1s;
    height: 34px;
}

.page-root {
    width: 1s;
    height: 1s;
    row-between: 10px;
}

.page-title {
    font-size: 22;
}

.page-subtitle {
    font-size: 13;
    color: #acb4bf;
}

.control-row {
    width: 1s;
    col-between: 8px;
    child-top: 2px;
    child-bottom: 2px;
}

.control-group {
    width: 1s;
    row-between: 8px;
    child-left: 8px;
    child-right: 8px;
    child-top: 8px;
    child-bottom: 8px;
    background-color: #12161a;
    border-width: 1px;
    border-color: #2a3139;
}

.control-button {
    min-width: 74px;
    height: 30px;
}

.control-value {
    color: #d8dee9;
}
"#;

#[derive(Clone, Copy, PartialEq, Eq, Data)]
pub(crate) enum ScaffoldPage {
    Kick,
    Bass,
    DrumMachine,
    Settings,
    Oscilloscope,
    Logs,
}

impl ScaffoldPage {
    fn title(self) -> &'static str {
        match self {
            Self::Kick => "Kick",
            Self::Bass => "Bass",
            Self::DrumMachine => "Drum Machine",
            Self::Settings => "Settings",
            Self::Oscilloscope => "Oscilloscope",
            Self::Logs => "Logs",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TuningStandard {
    A440,
    A432,
}

impl TuningStandard {
    fn a4_hz(self) -> f32 {
        match self {
            Self::A440 => 440.0,
            Self::A432 => 432.0,
        }
    }
}

fn tuning_standard_from_a4_hz(hz: f32) -> TuningStandard {
    if (hz - 432.0).abs() <= (hz - 440.0).abs() {
        TuningStandard::A432
    } else {
        TuningStandard::A440
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct OscilloscopeModel {
    pub(super) hold: bool,
    pub(super) zoom_x: f32,
    pub(super) zoom_y: f32,
    pub(super) show_kick: bool,
    pub(super) show_bass: bool,
    pub(super) show_sum: bool,
}

impl Default for OscilloscopeModel {
    fn default() -> Self {
        Self {
            hold: false,
            zoom_x: 1.0,
            zoom_y: 1.0,
            show_kick: true,
            show_bass: true,
            show_sum: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct LogModel {
    pub(super) snapshot: crate::common::logger::LogSnapshot,
}

const DRUM_MACHINE_STEPS: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct DrumMachineTrack {
    pub(super) name: String,
    pub(super) source_path: String,
    pub(super) steps: Vec<bool>,
}

impl Default for DrumMachineTrack {
    fn default() -> Self {
        Self {
            name: String::new(),
            source_path: String::new(),
            steps: vec![false; DRUM_MACHINE_STEPS],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct DrumMachineModel {
    pub(super) tempo_bpm: f32,
    pub(super) input_path: String,
    pub(super) status: String,
    pub(super) tracks: Vec<DrumMachineTrack>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct PatchModel {
    pub(super) available: Vec<String>,
    pub(super) selected_name: Option<String>,
    pub(super) default_name: Option<String>,
    pub(super) new_name: String,
    pub(super) status: String,
}

#[derive(Lens)]
pub(super) struct AppScaffoldModel {
    pub(super) active_page: ScaffoldPage,
    pub(super) ui_engine_port: Arc<dyn UiEnginePort + Send + Sync>,
    pub(super) audio_engine_port: Arc<dyn AudioEnginePort + Send + Sync>,
    pub(super) tuning_standard: TuningStandard,
    pub(super) kick_max_note_length_ms: f32,
    pub(super) kick_waveform: Waveform,
    pub(super) kick_retrigger: bool,
    pub(super) kick_legato_voice_steal: bool,
    pub(super) kick_keytrack_enabled: bool,
    pub(super) kick_note_length_ms: f32,
    pub(super) kick_active_curve: CurveKind,
    pub(super) bass_waveform: Waveform,
    pub(super) bass_filter_mode: BassFilterMode,
    pub(super) bass_retrigger: bool,
    pub(super) bass_legato_voice_steal: bool,
    pub(super) bass_note_length_ms: f32,
    pub(super) bass_cutoff_hz: f32,
    pub(super) bass_pitch_hz: f32,
    pub(super) kick_amp_shaper: SignalShaperState,
    pub(super) kick_pitch_shaper: SignalShaperState,
    pub(super) bass_amp_shaper: SignalShaperState,
    pub(super) bass_filter_shaper: SignalShaperState,
    pub(super) oscilloscope: OscilloscopeModel,
    pub(super) oscilloscope_snapshot: crate::interface::OscilloscopeSnapshot,
    pub(super) logs: LogModel,
    pub(super) drum_machine: DrumMachineModel,
    pub(super) patches: PatchModel,
}

#[derive(Clone)]
pub(super) enum AppScaffoldEvent {
    SelectPage(ScaffoldPage),
    KickSetWaveform(Waveform),
    KickToggleRetrigger,
    KickToggleLegatoVoiceSteal,
    KickToggleKeytrack,
    KickAdjustNoteLengthMs(f32),
    KickAdjustMaxNoteLengthMs(f32),
    KickShaperUndo,
    KickShaperRedo,
    KickTrigger,
    BassSetWaveform(Waveform),
    BassSetFilterMode(BassFilterMode),
    BassToggleRetrigger,
    BassToggleLegatoVoiceSteal,
    BassAdjustNoteLengthMs(f32),
    BassAdjustCutoffHz(f32),
    BassAdjustPitchHz(f32),
    KickAmpShaper(SignalShaperEvent),
    KickPitchShaper(SignalShaperEvent),
    KickSetActiveCurve(CurveKind),
    BassAmpShaper(SignalShaperEvent),
    BassFilterShaper(SignalShaperEvent),
    SetTuningStandard(TuningStandard),
    KickSetMaxNoteLengthMs(f32),
    OscilloscopeUpdate,
    ToggleOscHold,
    AdjustOscZoomX(f32),
    AdjustOscZoomY(f32),
    ToggleOscShowKick,
    ToggleOscShowBass,
    ToggleOscShowSum,
    LogsRefresh,
    LogsClear,
    DrumMachineSetBpm(f32),
    DrumMachineSetInput(String),
    DrumMachineAddTrack,
    DrumMachineRemoveTrack(usize),
    DrumMachineToggleStep { track: usize, step: usize },
    DrumMachineBounceToTrack,
    DrumMachineBounceToDesktop,
    PatchLoad(String),
    PatchSave,
    PatchSetDefault,
    PatchRefreshList,
    PatchSetNewName(String),
}

impl AppScaffoldModel {
    fn sync_kick_commands(&self) {
        self.ui_engine_port
            .apply_kick_command(KickCommand::SetAmpLut(curve_to_lut(&self.kick_amp_shaper.curve)));
        self.ui_engine_port
            .apply_kick_command(KickCommand::SetPitchLut(curve_to_lut(&self.kick_pitch_shaper.curve)));
        self.ui_engine_port
            .apply_kick_command(KickCommand::SetWaveform(self.kick_waveform));
        self.ui_engine_port
            .apply_kick_command(KickCommand::SetRetrigger(self.kick_retrigger));
        self.ui_engine_port.apply_kick_command(KickCommand::SetLegatoVoiceSteal(
            self.kick_legato_voice_steal,
        ));
        self.ui_engine_port.apply_kick_command(KickCommand::SetKeytrackEnabled(
            self.kick_keytrack_enabled,
        ));
        self.ui_engine_port
            .apply_kick_command(KickCommand::SetNoteLengthMs(self.kick_note_length_ms));
    }

    fn sync_bass_commands(&self) {
        self.ui_engine_port
            .apply_bass_command(BassCommand::SetAmpLut(curve_to_lut(&self.bass_amp_shaper.curve)));
        self.ui_engine_port
            .apply_bass_command(BassCommand::SetFilterLut(curve_to_lut(&self.bass_filter_shaper.curve)));
        self.ui_engine_port
            .apply_bass_command(BassCommand::SetWaveform(self.bass_waveform));
        self.ui_engine_port
            .apply_bass_command(BassCommand::SetFilterMode(self.bass_filter_mode));
        self.ui_engine_port
            .apply_bass_command(BassCommand::SetRetrigger(self.bass_retrigger));
        self.ui_engine_port.apply_bass_command(BassCommand::SetLegatoVoiceSteal(
            self.bass_legato_voice_steal,
        ));
        self.ui_engine_port
            .apply_bass_command(BassCommand::SetNoteLengthMs(self.bass_note_length_ms));
        self.ui_engine_port
            .apply_bass_command(BassCommand::SetCutoffHz(self.bass_cutoff_hz));
        self.ui_engine_port
            .apply_bass_command(BassCommand::SetPitchHz(self.bass_pitch_hz));
    }
}

impl Model for AppScaffoldModel {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|event, _meta| match event {
            AppScaffoldEvent::SelectPage(page) => {
                self.active_page = *page;
            }
            AppScaffoldEvent::KickSetWaveform(waveform) => {
                self.kick_waveform = *waveform;
                self.ui_engine_port
                    .apply_kick_command(KickCommand::SetWaveform(*waveform));
            }
            AppScaffoldEvent::KickToggleRetrigger => {
                self.kick_retrigger = !self.kick_retrigger;
                self.ui_engine_port
                    .apply_kick_command(KickCommand::SetRetrigger(self.kick_retrigger));
            }
            AppScaffoldEvent::KickToggleLegatoVoiceSteal => {
                self.kick_legato_voice_steal = !self.kick_legato_voice_steal;
                self.ui_engine_port.apply_kick_command(KickCommand::SetLegatoVoiceSteal(
                    self.kick_legato_voice_steal,
                ));
            }
            AppScaffoldEvent::KickToggleKeytrack => {
                self.kick_keytrack_enabled = !self.kick_keytrack_enabled;
                self.ui_engine_port.apply_kick_command(KickCommand::SetKeytrackEnabled(
                    self.kick_keytrack_enabled,
                ));
            }
            AppScaffoldEvent::KickAdjustNoteLengthMs(delta) => {
                self.kick_note_length_ms = (self.kick_note_length_ms + *delta)
                    .clamp(0.0, self.kick_max_note_length_ms);
                self.ui_engine_port
                    .apply_kick_command(KickCommand::SetNoteLengthMs(self.kick_note_length_ms));
            }
            AppScaffoldEvent::KickAdjustMaxNoteLengthMs(delta) => {
                self.kick_max_note_length_ms =
                    (self.kick_max_note_length_ms + *delta).clamp(100.0, 5000.0);
                self.kick_note_length_ms =
                    self.kick_note_length_ms.clamp(0.0, self.kick_max_note_length_ms);
            }
            AppScaffoldEvent::KickSetMaxNoteLengthMs(value) => {
                self.kick_max_note_length_ms = value.clamp(100.0, 5000.0);
                self.kick_note_length_ms = self.kick_note_length_ms.clamp(0.0, self.kick_max_note_length_ms);
            }
            AppScaffoldEvent::KickTrigger => {
                self.ui_engine_port
                    .apply_kick_command(KickCommand::RequestTrigger);
            }
            AppScaffoldEvent::BassSetWaveform(waveform) => {
                self.bass_waveform = *waveform;
                self.ui_engine_port
                    .apply_bass_command(BassCommand::SetWaveform(*waveform));
            }
            AppScaffoldEvent::BassSetFilterMode(mode) => {
                self.bass_filter_mode = *mode;
                self.ui_engine_port
                    .apply_bass_command(BassCommand::SetFilterMode(*mode));
            }
            AppScaffoldEvent::BassToggleRetrigger => {
                self.bass_retrigger = !self.bass_retrigger;
                self.ui_engine_port
                    .apply_bass_command(BassCommand::SetRetrigger(self.bass_retrigger));
            }
            AppScaffoldEvent::BassToggleLegatoVoiceSteal => {
                self.bass_legato_voice_steal = !self.bass_legato_voice_steal;
                self.ui_engine_port.apply_bass_command(BassCommand::SetLegatoVoiceSteal(
                    self.bass_legato_voice_steal,
                ));
            }
            AppScaffoldEvent::BassAdjustNoteLengthMs(delta) => {
                self.bass_note_length_ms = (self.bass_note_length_ms + *delta).clamp(1.0, 1000.0);
                self.ui_engine_port
                    .apply_bass_command(BassCommand::SetNoteLengthMs(self.bass_note_length_ms));
            }
            AppScaffoldEvent::BassAdjustCutoffHz(delta) => {
                self.bass_cutoff_hz = (self.bass_cutoff_hz + *delta).clamp(20.0, 8000.0);
                self.ui_engine_port
                    .apply_bass_command(BassCommand::SetCutoffHz(self.bass_cutoff_hz));
            }
            AppScaffoldEvent::BassAdjustPitchHz(delta) => {
                self.bass_pitch_hz = (self.bass_pitch_hz + *delta).clamp(20.0, 2000.0);
                self.ui_engine_port
                    .apply_bass_command(BassCommand::SetPitchHz(self.bass_pitch_hz));
            }
            AppScaffoldEvent::KickAmpShaper(event) => {
                self.kick_amp_shaper.apply(*event);
                self.ui_engine_port.apply_kick_command(KickCommand::SetAmpLut(
                    curve_to_lut(&self.kick_amp_shaper.curve),
                ));
            }
            AppScaffoldEvent::KickPitchShaper(event) => {
                self.kick_pitch_shaper.apply(*event);
                self.ui_engine_port.apply_kick_command(KickCommand::SetPitchLut(
                    curve_to_lut(&self.kick_pitch_shaper.curve),
                ));
            }
            AppScaffoldEvent::KickSetActiveCurve(kind) => {
                self.kick_active_curve = *kind;
            }
            AppScaffoldEvent::BassAmpShaper(event) => {
                self.bass_amp_shaper.apply(*event);
                self.ui_engine_port.apply_bass_command(BassCommand::SetAmpLut(
                    curve_to_lut(&self.bass_amp_shaper.curve),
                ));
            }
            AppScaffoldEvent::BassFilterShaper(event) => {
                self.bass_filter_shaper.apply(*event);
                self.ui_engine_port.apply_bass_command(BassCommand::SetFilterLut(
                    curve_to_lut(&self.bass_filter_shaper.curve),
                ));
            }
            AppScaffoldEvent::SetTuningStandard(tuning) => {
                self.tuning_standard = *tuning;
            }
            AppScaffoldEvent::OscilloscopeUpdate => {
                if !self.oscilloscope.hold {
                    self.oscilloscope_snapshot = self.ui_engine_port.oscilloscope_snapshot();
                }
            }
            AppScaffoldEvent::ToggleOscHold => {
                self.oscilloscope.hold = !self.oscilloscope.hold;
            }
            AppScaffoldEvent::AdjustOscZoomX(delta) => {
                self.oscilloscope.zoom_x = (self.oscilloscope.zoom_x + *delta).clamp(1.0, 16.0);
            }
            AppScaffoldEvent::AdjustOscZoomY(delta) => {
                self.oscilloscope.zoom_y = (self.oscilloscope.zoom_y + *delta).clamp(0.25, 4.0);
            }
            AppScaffoldEvent::ToggleOscShowKick => {
                self.oscilloscope.show_kick = !self.oscilloscope.show_kick;
            }
            AppScaffoldEvent::ToggleOscShowBass => {
                self.oscilloscope.show_bass = !self.oscilloscope.show_bass;
            }
            AppScaffoldEvent::ToggleOscShowSum => {
                self.oscilloscope.show_sum = !self.oscilloscope.show_sum;
            }
            AppScaffoldEvent::LogsRefresh => {
                self.logs.snapshot = crate::common::logger::LOGGER.snapshot();
            }
            AppScaffoldEvent::LogsClear => {
                crate::common::logger::LOGGER.clear();
                self.logs.snapshot = crate::common::logger::LOGGER.snapshot();
            }
            AppScaffoldEvent::DrumMachineSetBpm(bpm) => {
                self.drum_machine.tempo_bpm = (*bpm).clamp(60.0, 220.0);
            }
            AppScaffoldEvent::DrumMachineSetInput(value) => {
                self.drum_machine.input_path = value.clone();
            }
            AppScaffoldEvent::DrumMachineAddTrack => {
                let input = self.drum_machine.input_path.trim().to_owned();
                if input.is_empty() {
                    self.drum_machine.status = "Enter a bounced audio file path first.".to_owned();
                } else if std::path::Path::new(&input).exists() {
                    let name = drum_track_name_from_path(&input, self.drum_machine.tracks.len());
                    self.drum_machine.tracks.push(DrumMachineTrack {
                        name,
                        source_path: input,
                        steps: vec![false; DRUM_MACHINE_STEPS],
                    });
                    self.drum_machine.input_path.clear();
                    self.drum_machine.status = "Track added to drum machine grid.".to_owned();
                } else {
                    self.drum_machine.status = "Path does not exist.".to_owned();
                }
            }
            AppScaffoldEvent::DrumMachineRemoveTrack(index) => {
                if *index < self.drum_machine.tracks.len() {
                    self.drum_machine.tracks.remove(*index);
                    self.drum_machine.status = "Track removed.".to_owned();
                }
            }
            AppScaffoldEvent::DrumMachineToggleStep { track, step } => {
                if let Some(track) = self.drum_machine.tracks.get_mut(*track) {
                    if let Some(v) = track.steps.get_mut(*step) {
                        *v = !*v;
                    }
                }
            }
            AppScaffoldEvent::DrumMachineBounceToTrack => {
                self.drum_machine.status = "Bouncing...".to_owned();
                match bounce_current_synth_to_track(&*self.audio_engine_port, &self.drum_machine) {
                    Ok(name) => self.drum_machine.status = format!("Bounced and added: {name}"),
                    Err(err) => self.drum_machine.status = format!("Bounce failed: {err}"),
                }
            }
            AppScaffoldEvent::DrumMachineBounceToDesktop => {
                self.drum_machine.status = "Bouncing to desktop...".to_owned();
                match bounce_current_synth_to_desktop(&*self.audio_engine_port) {
                    Ok(path) => {
                        self.drum_machine.status = format!("Bounced to: {}", path.display())
                    }
                    Err(err) => self.drum_machine.status = format!("Desktop bounce failed: {err}"),
                }
            }
            AppScaffoldEvent::KickShaperUndo => {
                match self.kick_active_curve {
                    CurveKind::Amplitude => self.kick_amp_shaper.apply(SignalShaperEvent::Undo),
                    CurveKind::Pitch => self.kick_pitch_shaper.apply(SignalShaperEvent::Undo),
                }
                self.sync_kick_commands();
            }
            AppScaffoldEvent::KickShaperRedo => {
                match self.kick_active_curve {
                    CurveKind::Amplitude => self.kick_amp_shaper.apply(SignalShaperEvent::Redo),
                    CurveKind::Pitch => self.kick_pitch_shaper.apply(SignalShaperEvent::Redo),
                }
                self.sync_kick_commands();
            }
            AppScaffoldEvent::PatchLoad(name) => {
                match crate::patches::load_patch(name) {
                    Ok(patch) => {
                        self.tuning_standard =
                            tuning_standard_from_a4_hz(patch.tuning_a4_hz);
                        self.kick_keytrack_enabled = patch.keytrack_enabled;
                        self.kick_max_note_length_ms = patch.max_note_length_ms;
                        self.kick_note_length_ms =
                            patch.note_end_ms.clamp(0.0, patch.max_note_length_ms);

                        let mut amp_points: Vec<_> = patch
                            .amplitude_points
                            .iter()
                            .map(|(x, y)| SignalPoint::new(*x, *y))
                            .collect();
                        let mut amp_bends = patch.amplitude_bends;
                        constrain_curve_points(&mut amp_points);
                        normalize_segment_bends(&amp_points, &mut amp_bends);
                        self.kick_amp_shaper.curve.points = amp_points;
                        self.kick_amp_shaper.curve.bends = amp_bends;
                        self.kick_amp_shaper.selected_point = Some(1)
                            .filter(|i| *i < self.kick_amp_shaper.curve.points.len());

                        let mut pitch_points: Vec<_> = patch
                            .pitch_points
                            .iter()
                            .map(|(x, y)| SignalPoint::new(*x, *y))
                            .collect();
                        let mut pitch_bends = patch.pitch_bends;
                        constrain_curve_points(&mut pitch_points);
                        normalize_segment_bends(&pitch_points, &mut pitch_bends);
                        self.kick_pitch_shaper.curve.points = pitch_points;
                        self.kick_pitch_shaper.curve.bends = pitch_bends;
                        self.kick_pitch_shaper.selected_point = Some(1)
                            .filter(|i| *i < self.kick_pitch_shaper.curve.points.len());

                        self.sync_kick_commands();
                        self.patches.selected_name = Some(name.clone());
                        self.patches.default_name =
                            crate::patches::get_default_patch_name().unwrap_or_default();
                        self.patches.status = format!("Loaded patch: {name}");
                    }
                    Err(err) => self.patches.status = format!("Failed to load: {err}"),
                }
            }
            AppScaffoldEvent::PatchSave => {
                let name = if self.patches.new_name.trim().is_empty() {
                    let base = "patch".to_owned();
                    let index = (1..)
                        .find(|n| {
                            !self.patches.available.contains(&format!("{base}_{n}"))
                        })
                        .unwrap_or(1);
                    format!("{base}_{index}")
                } else {
                    self.patches.new_name.trim().to_owned()
                };

                let patch = crate::patches::PatchData {
                    name: name.clone(),
                    tuning_a4_hz: self.tuning_standard.a4_hz(),
                    keytrack_enabled: self.kick_keytrack_enabled,
                    note_end_ms: self.kick_note_length_ms,
                    max_note_length_ms: self.kick_max_note_length_ms,
                    waveform_zoom_percent: 100.0,
                    active_curve: "amplitude".to_owned(),
                    amplitude_points: self
                        .kick_amp_shaper
                        .curve
                        .points
                        .iter()
                        .map(|p| (p.x, p.y))
                        .collect(),
                    amplitude_bends: self.kick_amp_shaper.curve.bends.clone(),
                    pitch_points: self
                        .kick_pitch_shaper
                        .curve
                        .points
                        .iter()
                        .map(|p| (p.x, p.y))
                        .collect(),
                    pitch_bends: self.kick_pitch_shaper.curve.bends.clone(),
                };

                match crate::patches::save_patch(&patch) {
                    Ok(()) => {
                        self.patches.new_name.clear();
                        self.patches.selected_name = Some(name.clone());
                        self.patches.available =
                            crate::patches::list_patch_names().unwrap_or_default();
                        self.patches.default_name =
                            crate::patches::get_default_patch_name().unwrap_or_default();
                        self.patches.status = format!("Saved patch: {name}");
                    }
                    Err(err) => self.patches.status = format!("Failed to save: {err}"),
                }
            }
            AppScaffoldEvent::PatchSetDefault => {
                if let Some(name) = self.patches.selected_name.clone() {
                    match crate::patches::set_default_patch_name(&name) {
                        Ok(()) => {
                            self.patches.default_name = Some(name.clone());
                            self.patches.status = format!("Set default: {name}");
                        }
                        Err(err) => self.patches.status = format!("Failed to set default: {err}"),
                    }
                }
            }
            AppScaffoldEvent::PatchRefreshList => {
                self.patches.available = crate::patches::list_patch_names().unwrap_or_default();
                self.patches.default_name =
                    crate::patches::get_default_patch_name().unwrap_or_default();
            }
            AppScaffoldEvent::PatchSetNewName(value) => {
                self.patches.new_name = value.clone();
            }
        });
    }
}

pub(crate) fn default_state() -> Arc<ViziaState> {
    let ui_cfg = config::ui_config();
    let width = ui_cfg.base_editor_width as u32;
    let height = ui_cfg.base_editor_height as u32;
    ViziaState::new(move || (width, height))
}

pub(crate) fn create_app_scaffold_editor<P>(
    editor_state: Arc<ViziaState>,
    ui_engine_port: P,
) -> Option<Box<dyn Editor>>
where
    P: UiEnginePort + AudioEnginePort + Clone + Send + Sync + 'static,
{
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_thin(cx);
        let _ = cx.add_stylesheet(APP_SCAFFOLD_STYLE);

        let app_cfg = config::app_config();
        let engine_port = ui_engine_port.clone();
        let ui_engine_port: Arc<dyn UiEnginePort + Send + Sync> = Arc::new(engine_port.clone());
        let audio_engine_port: Arc<dyn AudioEnginePort + Send + Sync> = Arc::new(engine_port);
        let model = AppScaffoldModel {
            active_page: ScaffoldPage::Kick,
            ui_engine_port,
            audio_engine_port,
            tuning_standard: tuning_standard_from_a4_hz(app_cfg.default_tuning_a4_hz),
            kick_max_note_length_ms: app_cfg.note_length_max_ms,
            kick_waveform: Waveform::Sine,
            kick_retrigger: true,
            kick_legato_voice_steal: true,
            kick_keytrack_enabled: false,
            kick_note_length_ms: app_cfg.note_length_max_ms,
            kick_active_curve: CurveKind::Amplitude,
            bass_waveform: Waveform::Saw,
            bass_filter_mode: BassFilterMode::LowPass,
            bass_retrigger: true,
            bass_legato_voice_steal: false,
            bass_note_length_ms: app_cfg.note_length_max_ms,
            bass_cutoff_hz: 120.0,
            bass_pitch_hz: 55.0,
            kick_amp_shaper: SignalShaperState::new(kick_amp_curve()),
            kick_pitch_shaper: SignalShaperState::new(kick_pitch_curve()),
            bass_amp_shaper: SignalShaperState::new(bass_amp_curve()),
            bass_filter_shaper: SignalShaperState::new(bass_filter_curve()),
            oscilloscope: OscilloscopeModel::default(),
            oscilloscope_snapshot: crate::interface::OscilloscopeSnapshot {
                kick: [0.0; crate::interface::OSCILLOSCOPE_BUFFER_SIZE],
                bass: [0.0; crate::interface::OSCILLOSCOPE_BUFFER_SIZE],
                sum: [0.0; crate::interface::OSCILLOSCOPE_BUFFER_SIZE],
                len: 0,
                sequence: 0,
            },
            logs: LogModel::default(),
            drum_machine: DrumMachineModel::default(),
            patches: PatchModel::default(),
        };

        model.sync_kick_commands();
        model.sync_bass_commands();
        model.build(cx);

        HStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                Binding::new(cx, AppScaffoldModel::active_page, |cx, active_page| {
                    let active_page = active_page.get(cx);
                    match active_page {
                        ScaffoldPage::Kick => {
                            build_kick_page(cx);
                        }
                        ScaffoldPage::Bass => {
                            build_bass_page(cx);
                        }
                        ScaffoldPage::DrumMachine => {
                            build_drum_machine_page(cx);
                        }
                        ScaffoldPage::Settings => {
                            build_settings_page(cx);
                        }
                        ScaffoldPage::Oscilloscope => {
                            build_oscilloscope_page(cx);
                        }
                        ScaffoldPage::Logs => {
                            build_logs_page(cx);
                        }
                    }
                });
            })
            .class("app-content")
            .width(Stretch(1.0))
            .height(Stretch(1.0));

            build_nav_menu(cx);
        })
        .class("app-scaffold")
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .class("app-root");
    })
}

fn curve_to_lut(curve: &SignalCurve) -> [f32; CURVE_LUT_SIZE] {
    let mut lut = [0.0; CURVE_LUT_SIZE];
    let den = (CURVE_LUT_SIZE - 1) as f32;
    for (i, value) in lut.iter_mut().enumerate() {
        let t = i as f32 / den;
        *value = envelope_value_linear(&curve.points, &curve.bends, t).clamp(0.0, 1.0);
    }
    lut
}

fn kick_amp_curve() -> SignalCurve {
    SignalCurve::normalized_default()
}

fn kick_pitch_curve() -> SignalCurve {
    SignalCurve {
        points: vec![
            SignalPoint::new(0.0, 1.0),
            SignalPoint::new(0.08, 0.98),
            SignalPoint::new(0.30, 0.30),
            SignalPoint::new(1.0, 0.08),
        ],
        bends: vec![0.0; 3],
    }
}

fn bass_amp_curve() -> SignalCurve {
    SignalCurve::normalized_default()
}

fn bass_filter_curve() -> SignalCurve {
    SignalCurve {
        points: vec![
            SignalPoint::new(0.0, 0.2),
            SignalPoint::new(0.15, 0.55),
            SignalPoint::new(0.55, 0.8),
            SignalPoint::new(1.0, 1.0),
        ],
        bends: vec![0.0; 3],
    }
}

fn kick_amp_lut() -> [f32; CURVE_LUT_SIZE] {
    curve_to_lut(&kick_amp_curve())
}

fn kick_pitch_lut() -> [f32; CURVE_LUT_SIZE] {
    curve_to_lut(&kick_pitch_curve())
}

fn bass_amp_lut() -> [f32; CURVE_LUT_SIZE] {
    curve_to_lut(&bass_amp_curve())
}

fn bass_filter_lut() -> [f32; CURVE_LUT_SIZE] {
    curve_to_lut(&bass_filter_curve())
}

pub(crate) fn format_time_ms(normalized: f32) -> String {
    format!("{:.0} ms", normalized.clamp(0.0, 1.0) * 1000.0)
}

pub(crate) fn format_amp_db(normalized: f32) -> String {
    let db_floor = -60.0;
    let db = db_floor + normalized.clamp(0.0, 1.0) * (0.0 - db_floor);
    format!("{db:.1} dB")
}

pub(crate) fn format_pitch_hz(normalized: f32) -> String {
    let min_hz = 20.0;
    let max_hz = 2000.0;
    let hz = min_hz + normalized.clamp(0.0, 1.0) * (max_hz - min_hz);
    if hz >= 1000.0 {
        format!("{:.2} kHz", hz / 1000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

impl Data for TuningStandard {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Data for crate::interface::OscilloscopeSnapshot {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Data for crate::common::logger::LogSnapshot {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Data for OscilloscopeModel {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Data for LogModel {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Data for DrumMachineTrack {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Data for DrumMachineModel {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Data for PatchModel {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Data for CurveKind {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

fn drum_track_name_from_path(path: &str, fallback_index: usize) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| format!("Track {}", fallback_index + 1))
}

fn bounce_current_synth_to_track(
    port: &dyn AudioEnginePort,
    drum: &DrumMachineModel,
) -> Result<String, String> {
    let max_ms = 0.0_f32; // replaced by actual lengths below
    let max_ms = max_ms.max(drum.tempo_bpm); // placeholder; we use note lengths instead
    let _ = max_ms;
    // Actually compute a sensible duration from the synth envelopes.
    // For now bounce 0.5s plus a small tail to avoid truncation.
    let duration = 0.75_f32.clamp(0.25, 10.0);
    let samples = crate::audio::bounce_current_patch(port, duration);
    let output_dir = std::env::temp_dir().join("librekick-bounces");
    std::fs::create_dir_all(&output_dir).map_err(|e| format!("{e}"))?;
    let filename = format!(
        "librekick_bounce_{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    let path = output_dir.join(&filename);
    write_wav32f(&path, &samples, crate::audio::DRUM_MACHINE_BOUNCE_SAMPLE_RATE)?;
    let name = drum_track_name_from_path(&path.to_string_lossy(), drum.tracks.len());
    Ok(name)
}

fn bounce_current_synth_to_desktop(port: &dyn AudioEnginePort) -> Result<std::path::PathBuf, String> {
    let desktop = std::env::var("HOME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|home| std::path::PathBuf::from(home).join("Desktop"))
        .ok_or_else(|| "Desktop path not available.".to_owned())?;
    let duration = 0.75_f32.clamp(0.25, 10.0);
    let samples = crate::audio::bounce_current_patch(port, duration);
    let filename = format!(
        "librekick_bounce_{}.wav",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    let path = desktop.join(filename);
    write_wav32f(&path, &samples, crate::audio::DRUM_MACHINE_BOUNCE_SAMPLE_RATE)?;
    Ok(path)
}

fn write_wav32f(
    path: &std::path::Path,
    samples: &[f32],
    sample_rate: u32,
) -> Result<(), String> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
    }
    let mut file = std::fs::File::create(path).map_err(|e| format!("{e}"))?;

    let channels: u16 = 1;
    let bit_depth: u16 = 32;
    let data_size = (samples.len() * 4) as u32;
    let chunk_size = data_size + 36;

    file.write_all(b"RIFF").map_err(|e| format!("{e}"))?;
    file.write_all(&chunk_size.to_le_bytes()).map_err(|e| format!("{e}"))?;
    file.write_all(b"WAVE").map_err(|e| format!("{e}"))?;
    file.write_all(b"fmt ").map_err(|e| format!("{e}"))?;
    file.write_all(&16_u32.to_le_bytes()).map_err(|e| format!("{e}"))?;
    file.write_all(&3_u16.to_le_bytes()).map_err(|e| format!("{e}"))?; // IEEE float
    file.write_all(&channels.to_le_bytes()).map_err(|e| format!("{e}"))?;
    file.write_all(&sample_rate.to_le_bytes()).map_err(|e| format!("{e}"))?;
    file.write_all(&(sample_rate * channels as u32 * (bit_depth as u32 / 8)).to_le_bytes())
        .map_err(|e| format!("{e}"))?;
    file.write_all(&(channels * (bit_depth / 8)).to_le_bytes())
        .map_err(|e| format!("{e}"))?;
    file.write_all(&bit_depth.to_le_bytes()).map_err(|e| format!("{e}"))?;
    file.write_all(b"data").map_err(|e| format!("{e}"))?;
    file.write_all(&data_size.to_le_bytes()).map_err(|e| format!("{e}"))?;
    for sample in samples {
        file.write_all(&sample.to_le_bytes()).map_err(|e| format!("{e}"))?;
    }
    Ok(())
}
