use std::sync::{Arc, Mutex};

use crate::common::logger::LOGGER;
use crate::config;

pub const CURVE_LUT_SIZE: usize = 256;
pub const OSCILLOSCOPE_BUFFER_SIZE: usize = 2048;

#[derive(Clone, Copy)]
pub enum CurveKind {
    Amplitude,
    Pitch,
}

pub fn set_kick_oscillator_waveform(shared: &SharedStateHandle, waveform: Waveform) {
    if let Ok(mut state) = shared.lock() {
        state.kick_oscillator_waveform = waveform;
    }
}

pub fn set_kick_retrigger(shared: &SharedStateHandle, kick_retrigger: bool) {
    if let Ok(mut state) = shared.lock() {
        state.kick_retrigger = kick_retrigger;
    }
}

pub fn set_kick_legato_voice_steal(
    shared: &SharedStateHandle,
    kick_legato_voice_steal: bool,
) {
    if let Ok(mut state) = shared.lock() {
        state.kick_legato_voice_steal = kick_legato_voice_steal;
    }
}

pub fn set_kick_pitch_hz(shared: &SharedStateHandle, pitch_hz: f32) {
    if let Ok(mut state) = shared.lock() {
        state.kick_pitch_hz = pitch_hz.clamp(20.0, 2_000.0);
    }
}

pub fn set_kick_phase_deg(shared: &SharedStateHandle, phase_deg: f32) {
    if let Ok(mut state) = shared.lock() {
        state.kick_phase_deg = phase_deg.clamp(0.0, 360.0);
    }
}


/// Maximum number of arrange notes synced to the audio thread.
pub const ARRANGE_MAX_NOTES: usize = 256;

/// An arrange-page note synced to the audio thread.
/// `row`: 0-11 = bass semitones (B at top .. C at bottom), 12 = kick lane.
/// `bar_pos`: position in bars (fractional).
/// `slot`: which bass note slot plays it (0 = Note 1, 1 = Note 2).
#[derive(Clone, Copy, Debug, Default)]
pub struct ArrangeNoteData {
    pub row: u8,
    pub bar_pos: f32,
    pub slot: u8,
}

pub fn set_arrange_override(shared: &SharedStateHandle, enabled: bool) {
    if let Ok(mut state) = shared.lock() {
        state.arrange_override = enabled;
    }
}

/// Syncs the arrange pattern and its timing settings to the audio thread.
/// `notes` are (row, bar_pos, slot) triples; at most `ARRANGE_MAX_NOTES` are kept.
pub fn set_arrange_pattern(
    shared: &SharedStateHandle,
    notes: &[(usize, f32, u8)],
    num_bars: f32,
    note_len_bars: f32,
    manual_tempo: f32,
    use_daw_tempo: bool,
) {
    if let Ok(mut state) = shared.lock() {
        let count = notes.len().min(ARRANGE_MAX_NOTES);
        for (entry, &(row, bar_pos, slot)) in
            state.arrange_notes.iter_mut().zip(notes.iter()).take(count)
        {
            *entry = ArrangeNoteData {
                row: row.min(12) as u8,
                bar_pos,
                slot: slot.min(BASS_SLOT_COUNT as u8 - 1),
            };
        }
        state.arrange_note_count = count;
        state.arrange_num_bars = num_bars.clamp(0.25, 8.0);
        state.arrange_note_len_bars = note_len_bars.clamp(0.0625, 1.0);
        state.arrange_manual_tempo = manual_tempo.clamp(20.0, 300.0);
        state.arrange_use_daw_tempo = use_daw_tempo;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Waveform {
    Saw,
    Square,
    Sine,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BassFilterMode {
    LowPass,
    HighPass,
    BandPass,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BassFilterSlope {
    /// 6 dB/octave (first-order, 1 pole).
    S6dB,
    /// 12 dB/octave (second-order, 2 poles).
    S12dB,
    /// 18 dB/octave (third-order, 3 poles).
    S18dB,
    /// 24 dB/octave (fourth-order, 4 poles).
    S24dB,
}

impl BassFilterSlope {
    /// Number of cascaded one-pole stages for this slope.
    pub const fn poles(&self) -> usize {
        match self {
            Self::S6dB => 1,
            Self::S12dB => 2,
            Self::S18dB => 3,
            Self::S24dB => 4,
        }
    }
}

/// Number of LFOs available per bass slot.
pub const BASS_LFO_COUNT: usize = 4;
/// Number of modulation matrix rows per bass slot.
pub const BASS_MOD_SLOT_COUNT: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LfoShape {
    Sine,
    Triangle,
    SawUp,
    SawDown,
    Square,
    /// Sample-and-hold random, stepping once per cycle.
    Random,
}

impl LfoShape {
    pub const ALL: [LfoShape; 6] = [
        Self::Sine,
        Self::Triangle,
        Self::SawUp,
        Self::SawDown,
        Self::Square,
        Self::Random,
    ];

    pub const fn label(&self) -> &'static str {
        match self {
            Self::Sine => "SIN",
            Self::Triangle => "TRI",
            Self::SawUp => "SAW",
            Self::SawDown => "RMP",
            Self::Square => "SQR",
            Self::Random => "RND",
        }
    }

    /// Stable key used in patch files.
    pub const fn as_key(&self) -> &'static str {
        match self {
            Self::Sine => "sine",
            Self::Triangle => "triangle",
            Self::SawUp => "saw_up",
            Self::SawDown => "saw_down",
            Self::Square => "square",
            Self::Random => "random",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "sine" => Some(Self::Sine),
            "triangle" => Some(Self::Triangle),
            "saw_up" => Some(Self::SawUp),
            "saw_down" => Some(Self::SawDown),
            "square" => Some(Self::Square),
            "random" => Some(Self::Random),
            _ => None,
        }
    }
}

/// A modulation source that can drive a destination through the mod matrix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModSource {
    None,
    Lfo1,
    Lfo2,
    Lfo3,
    Lfo4,
    /// The amp envelope, 0..1 over the note.
    EnvAmp,
    /// Cutoff envelope 1, 0..1 over the note.
    EnvFilter1,
    /// Cutoff envelope 2, 0..1 over the note.
    EnvFilter2,
    /// Note velocity, 0..1.
    Velocity,
    /// Note pitch mapped to 0..1 across the usable range.
    KeyTrack,
}

impl ModSource {
    pub const ALL: [ModSource; 10] = [
        Self::None,
        Self::Lfo1,
        Self::Lfo2,
        Self::Lfo3,
        Self::Lfo4,
        Self::EnvAmp,
        Self::EnvFilter1,
        Self::EnvFilter2,
        Self::Velocity,
        Self::KeyTrack,
    ];

    pub const fn label(&self) -> &'static str {
        match self {
            Self::None => "--",
            Self::Lfo1 => "LFO 1",
            Self::Lfo2 => "LFO 2",
            Self::Lfo3 => "LFO 3",
            Self::Lfo4 => "LFO 4",
            Self::EnvAmp => "ENV AMP",
            Self::EnvFilter1 => "ENV FC 1",
            Self::EnvFilter2 => "ENV FC 2",
            Self::Velocity => "VELOCITY",
            Self::KeyTrack => "KEYTRACK",
        }
    }

    pub const fn as_key(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lfo1 => "lfo1",
            Self::Lfo2 => "lfo2",
            Self::Lfo3 => "lfo3",
            Self::Lfo4 => "lfo4",
            Self::EnvAmp => "env_amp",
            Self::EnvFilter1 => "env_filter1",
            Self::EnvFilter2 => "env_filter2",
            Self::Velocity => "velocity",
            Self::KeyTrack => "keytrack",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_key() == key)
    }

    /// LFO index this source refers to, if any.
    pub const fn lfo_index(&self) -> Option<usize> {
        match self {
            Self::Lfo1 => Some(0),
            Self::Lfo2 => Some(1),
            Self::Lfo3 => Some(2),
            Self::Lfo4 => Some(3),
            _ => None,
        }
    }
}

/// A parameter a modulation source can be routed to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModDest {
    None,
    /// Filter cutoff, in octaves.
    Cutoff,
    /// Oscillator pitch, in octaves.
    Pitch,
    /// Output level, multiplicative.
    Level,
    OscAMix,
    OscBMix,
    SubMix,
    NoiseMix,
    /// OSC B fine detune, in cents.
    OscBDetune,
    /// Unison detune spread, in cents.
    UnisonDetune,
    FilterDrive,
}

impl ModDest {
    pub const ALL: [ModDest; 11] = [
        Self::None,
        Self::Cutoff,
        Self::Pitch,
        Self::Level,
        Self::OscAMix,
        Self::OscBMix,
        Self::SubMix,
        Self::NoiseMix,
        Self::OscBDetune,
        Self::UnisonDetune,
        Self::FilterDrive,
    ];

    pub const fn label(&self) -> &'static str {
        match self {
            Self::None => "--",
            Self::Cutoff => "CUTOFF",
            Self::Pitch => "PITCH",
            Self::Level => "LEVEL",
            Self::OscAMix => "OSC A MIX",
            Self::OscBMix => "OSC B MIX",
            Self::SubMix => "SUB MIX",
            Self::NoiseMix => "NOISE MIX",
            Self::OscBDetune => "OSC B DETUNE",
            Self::UnisonDetune => "UNISON DETUNE",
            Self::FilterDrive => "DRIVE",
        }
    }

    pub const fn as_key(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Cutoff => "cutoff",
            Self::Pitch => "pitch",
            Self::Level => "level",
            Self::OscAMix => "osc_a_mix",
            Self::OscBMix => "osc_b_mix",
            Self::SubMix => "sub_mix",
            Self::NoiseMix => "noise_mix",
            Self::OscBDetune => "osc_b_detune",
            Self::UnisonDetune => "unison_detune",
            Self::FilterDrive => "filter_drive",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_key() == key)
    }
}

/// One LFO's settings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LfoParams {
    pub enabled: bool,
    pub shape: LfoShape,
    /// Free-running rate in Hz, used when `tempo_sync` is false.
    pub rate_hz: f32,
    /// Cycle length in beats, used when `tempo_sync` is true.
    pub rate_beats: f32,
    pub tempo_sync: bool,
    /// Output scale, 0..1.
    pub depth: f32,
    /// Start phase in degrees, applied on retrigger.
    pub phase_deg: f32,
    /// Restart the phase on each note-on instead of running free.
    pub retrigger: bool,
}

impl Default for LfoParams {
    fn default() -> Self {
        Self {
            enabled: false,
            shape: LfoShape::Sine,
            rate_hz: 4.0,
            rate_beats: 1.0,
            tempo_sync: false,
            depth: 1.0,
            phase_deg: 0.0,
            retrigger: true,
        }
    }
}

/// One mod matrix row: source -> destination with a signed amount.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ModRouting {
    pub source: ModSource,
    pub dest: ModDest,
    /// Signed depth, -1..1.
    pub amount: f32,
}

impl Default for ModRouting {
    fn default() -> Self {
        Self {
            source: ModSource::None,
            dest: ModDest::None,
            amount: 0.0,
        }
    }
}

impl ModRouting {
    /// A routing only contributes when both ends are assigned.
    pub fn is_active(&self) -> bool {
        self.source != ModSource::None
            && self.dest != ModDest::None
            && self.amount.abs() > f32::EPSILON
    }
}

/// Live modulation readouts published by the audio thread so the UI can
/// animate playheads, LFO dots and knob mod rings.
#[derive(Clone, Copy, Debug)]
pub struct BassModTelemetry {
    /// True while a bass voice is sounding.
    pub active: bool,
    /// Which bass slot is sounding (0 = Note 1, 1 = Note 2).
    pub slot: usize,
    /// Normalised position through the note, 0..1.
    pub env_phase: f32,
    pub env_amp: f32,
    pub env_filter_1: f32,
    pub env_filter_2: f32,
    /// Current LFO outputs, -1..1 (already scaled by depth).
    pub lfo_values: [f32; BASS_LFO_COUNT],
    /// Current LFO phases, 0..1 — lets the UI dot sit on the drawn curve.
    pub lfo_phases: [f32; BASS_LFO_COUNT],
    /// Velocity of the sounding note, for the VELOCITY mod source.
    pub velocity: f32,
    /// Post-modulation cutoff in Hz, for the filter display.
    pub cutoff_hz: f32,
    /// Increments every published frame so the UI can detect staleness.
    pub sequence: u64,
}

impl Default for BassModTelemetry {
    fn default() -> Self {
        Self {
            active: false,
            slot: 0,
            env_phase: 0.0,
            env_amp: 0.0,
            env_filter_1: 0.0,
            env_filter_2: 0.0,
            lfo_values: [0.0; BASS_LFO_COUNT],
            lfo_phases: [0.0; BASS_LFO_COUNT],
            velocity: 0.0,
            cutoff_hz: 0.0,
            sequence: 0,
        }
    }
}

/// Publishes one frame of modulation telemetry from the audio thread.
pub fn publish_bass_mod_telemetry(
    shared: &SharedStateHandle,
    mut telemetry: BassModTelemetry,
) {
    if let Ok(mut state) = shared.lock() {
        telemetry.sequence = state.bass_mod_telemetry.sequence.wrapping_add(1);
        state.bass_mod_telemetry = telemetry;
    }
}

pub fn bass_mod_telemetry(shared: &SharedStateHandle) -> BassModTelemetry {
    shared
        .lock()
        .map(|state| state.bass_mod_telemetry)
        .unwrap_or_default()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OscilloscopeSignal {
    Kick,
    Bass,
    Sum,
}

#[derive(Clone)]
pub struct OscilloscopeSnapshot {
    pub kick: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    pub bass: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    pub sum: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    pub len: usize,
    pub sequence: u64,
}

/// Number of bass note slots (Note 1, Note 2, ...).
pub const BASS_SLOT_COUNT: usize = 2;

/// All per-slot bass settings shared with the DSP thread.
#[derive(Clone)]
pub struct BassSlotParams {
    pub amp_lut: [f32; CURVE_LUT_SIZE],
    pub filter_lut: [f32; CURVE_LUT_SIZE],
    pub filter_2_lut: [f32; CURVE_LUT_SIZE],
    pub filter_enabled: bool,
    pub filter_2_enabled: bool,
    pub note_length_ms: f32,
    pub cutoff_hz: f32,
    pub filter_mode: BassFilterMode,
    pub filter_slope: BassFilterSlope,
    pub filter_drive: f32,
    pub pitch_hz: f32,
    pub retrigger: bool,
    pub legato_voice_steal: bool,
    pub oscillator_waveform: Waveform,
    /// OSC A output level in the source mixer. 1.0 keeps pre-mixer behaviour.
    pub osc_a_level: f32,
    /// Unison voice count for OSC A. 1 disables unison.
    pub osc_a_unison: i32,
    /// Total unison detune spread for OSC A, in cents.
    pub osc_a_unison_detune: f32,
    /// Outer-voice level for OSC A unison, 0..1.
    pub osc_a_unison_blend: f32,
    pub osc_b_enabled: bool,
    pub osc_b_waveform: Waveform,
    pub osc_b_level: f32,
    /// OSC B octave offset relative to OSC A.
    pub osc_b_octave: i32,
    /// OSC B fine detune in cents.
    pub osc_b_detune_cents: f32,
    pub osc_b_unison: i32,
    pub osc_b_unison_detune: f32,
    pub osc_b_unison_blend: f32,
    pub sub_enabled: bool,
    pub sub_waveform: Waveform,
    pub sub_level: f32,
    /// Sub octave offset relative to OSC A (negative = below).
    pub sub_octave: i32,
    pub noise_enabled: bool,
    pub noise_level: f32,
    pub keytrack_enabled: bool,
    /// Oscillator start phase in degrees (0-360), applied on retrigger.
    pub phase_deg: f32,
    /// Per-slot LFOs available as modulation sources.
    pub lfos: [LfoParams; BASS_LFO_COUNT],
    /// Modulation matrix rows routing sources to destinations.
    pub mod_matrix: [ModRouting; BASS_MOD_SLOT_COUNT],
}

impl Default for BassSlotParams {
    fn default() -> Self {
        let app_cfg = config::app_config();
        Self {
            amp_lut: [0.0; CURVE_LUT_SIZE],
            filter_lut: [0.0; CURVE_LUT_SIZE],
            filter_2_lut: [0.0; CURVE_LUT_SIZE],
            filter_enabled: true,
            filter_2_enabled: false,
            note_length_ms: app_cfg.note_length_max_ms,
            cutoff_hz: 120.0,
            filter_mode: BassFilterMode::LowPass,
            filter_slope: BassFilterSlope::S6dB,
            filter_drive: 0.0,
            pitch_hz: 55.0,
            retrigger: true,
            legato_voice_steal: false,
            oscillator_waveform: Waveform::Saw,
            osc_a_level: 1.0,
            osc_a_unison: 1,
            osc_a_unison_detune: 15.0,
            osc_a_unison_blend: 0.75,
            osc_b_enabled: false,
            osc_b_waveform: Waveform::Saw,
            osc_b_level: 0.5,
            osc_b_octave: 0,
            osc_b_detune_cents: 0.0,
            osc_b_unison: 1,
            osc_b_unison_detune: 15.0,
            osc_b_unison_blend: 0.75,
            sub_enabled: false,
            sub_waveform: Waveform::Sine,
            sub_level: 0.5,
            sub_octave: -1,
            noise_enabled: false,
            noise_level: 0.25,
            keytrack_enabled: false,
            phase_deg: 0.0,
            lfos: [LfoParams::default(); BASS_LFO_COUNT],
            mod_matrix: [ModRouting::default(); BASS_MOD_SLOT_COUNT],
        }
    }
}

/// Writes one bass slot's settings (index 0 = Note 1, 1 = Note 2).
pub fn set_bass_slot(shared: &SharedStateHandle, index: usize, params: BassSlotParams) {
    if let Ok(mut state) = shared.lock() {
        if let Some(slot) = state.bass.get_mut(index) {
            *slot = params;
        }
    }
}

#[derive(Clone)]
pub struct SharedSnapshot {
    pub amp_lut: [f32; CURVE_LUT_SIZE],
    pub pitch_lut: [f32; CURVE_LUT_SIZE],
    /// Bass settings per note slot (index 0 = Note 1, 1 = Note 2).
    pub bass: [BassSlotParams; BASS_SLOT_COUNT],
    pub keytrack_enabled: bool,
    pub note_length_ms: f32,
    pub kick_oscillator_waveform: Waveform,
    pub kick_retrigger: bool,
    pub kick_legato_voice_steal: bool,
    pub kick_pitch_hz: f32,
    /// Oscillator start phase in degrees (0-360), applied on retrigger.
    pub kick_phase_deg: f32,
    /// When true, the internal arrange pattern plays while any DAW note is
    /// held, instead of routing DAW MIDI directly to the voices.
    pub arrange_override: bool,
    pub arrange_notes: [ArrangeNoteData; ARRANGE_MAX_NOTES],
    pub arrange_note_count: usize,
    pub arrange_num_bars: f32,
    /// Length of one arrange note in bars (from the note-size setting).
    pub arrange_note_len_bars: f32,
    pub arrange_manual_tempo: f32,
    pub arrange_use_daw_tempo: bool,
    pub tempo: Option<f64>,
    pub trigger_counter: u64,
}

pub(crate) struct SharedState {
    amp_lut: [f32; CURVE_LUT_SIZE],
    pitch_lut: [f32; CURVE_LUT_SIZE],
    bass: [BassSlotParams; BASS_SLOT_COUNT],
    keytrack_enabled: bool,
    note_length_ms: f32,
    kick_oscillator_waveform: Waveform,
    kick_retrigger: bool,
    kick_legato_voice_steal: bool,
    kick_pitch_hz: f32,
    kick_phase_deg: f32,
    arrange_override: bool,
    arrange_notes: [ArrangeNoteData; ARRANGE_MAX_NOTES],
    arrange_note_count: usize,
    arrange_num_bars: f32,
    arrange_note_len_bars: f32,
    arrange_manual_tempo: f32,
    arrange_use_daw_tempo: bool,
    osc_kick: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    osc_bass: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    osc_sum: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    osc_len: usize,
    osc_sequence: u64,
    trigger_counter: u64,
    tempo: Option<f64>,
    bass_mod_telemetry: BassModTelemetry,
}

impl Default for SharedState {
    fn default() -> Self {
        let app_cfg = config::app_config();
        Self {
            amp_lut: [0.0; CURVE_LUT_SIZE],
            pitch_lut: [0.0; CURVE_LUT_SIZE],
            bass: [BassSlotParams::default(), BassSlotParams::default()],
            keytrack_enabled: false,
            note_length_ms: app_cfg.note_length_max_ms,
            kick_oscillator_waveform: Waveform::Sine,
            kick_retrigger: true,
            kick_legato_voice_steal: true,
            kick_pitch_hz: 55.0,
            kick_phase_deg: 0.0,
            arrange_override: false,
            arrange_notes: [ArrangeNoteData::default(); ARRANGE_MAX_NOTES],
            arrange_note_count: 0,
            arrange_num_bars: 1.0,
            arrange_note_len_bars: 0.25,
            arrange_manual_tempo: 120.0,
            arrange_use_daw_tempo: true,
            tempo: None,
            osc_kick: [0.0; OSCILLOSCOPE_BUFFER_SIZE],
            osc_bass: [0.0; OSCILLOSCOPE_BUFFER_SIZE],
            osc_sum: [0.0; OSCILLOSCOPE_BUFFER_SIZE],
            osc_len: 0,
            osc_sequence: 0,
            trigger_counter: 0,
            bass_mod_telemetry: BassModTelemetry::default(),
        }
    }
}

pub type SharedStateHandle = Arc<Mutex<SharedState>>;

pub fn new_shared_state() -> SharedStateHandle {
    let mut state = SharedState::default();

    for i in 0..CURVE_LUT_SIZE {
        let t = i as f32 / (CURVE_LUT_SIZE as f32 - 1.0);
        state.amp_lut[i] = (1.0 - t).clamp(0.0, 1.0);
        state.pitch_lut[i] = (1.0 - t).clamp(0.0, 1.0);
        for slot in state.bass.iter_mut() {
            slot.amp_lut[i] = (1.0 - t).clamp(0.0, 1.0);
            slot.filter_lut[i] = t.clamp(0.0, 1.0);
        }
    }

    Arc::new(Mutex::new(state))
}

pub fn set_curve_lut(shared: &SharedStateHandle, kind: CurveKind, lut: [f32; CURVE_LUT_SIZE]) {
    if let Ok(mut state) = shared.lock() {
        match kind {
            CurveKind::Amplitude => state.amp_lut = lut,
            CurveKind::Pitch => state.pitch_lut = lut,
        }
    }
}

pub fn request_trigger(shared: &SharedStateHandle) {
    LOGGER.entering("shared::request_trigger", "{}".to_owned());
    if let Ok(mut state) = shared.lock() {
        state.trigger_counter = state.trigger_counter.wrapping_add(1);
        LOGGER.debug(format!("trigger_counter={}", state.trigger_counter));
        LOGGER.leaving("shared::request_trigger");
    } else {
        LOGGER.error("shared::request_trigger failed to acquire state lock");
    }
}

pub fn set_keytrack_enabled(shared: &SharedStateHandle, keytrack_enabled: bool) {
    if let Ok(mut state) = shared.lock() {
        state.keytrack_enabled = keytrack_enabled;
    }
}

pub fn set_note_length_ms(shared: &SharedStateHandle, note_length_ms: f32) {
    let app_cfg = config::app_config();
    if let Ok(mut state) = shared.lock() {
        state.note_length_ms = note_length_ms.clamp(0.0, app_cfg.note_length_max_ms);
    }
}

pub fn set_tempo(shared: &SharedStateHandle, tempo: Option<f64>) {
    if let Ok(mut state) = shared.lock() {
        state.tempo = tempo;
    }
}

pub fn publish_oscilloscope_signal_block(
    shared: &SharedStateHandle,
    signal: OscilloscopeSignal,
    samples: &[f32],
) {
    if let Ok(mut state) = shared.lock() {
        let len = samples.len().min(OSCILLOSCOPE_BUFFER_SIZE);
        let target = match signal {
            OscilloscopeSignal::Kick => &mut state.osc_kick,
            OscilloscopeSignal::Bass => &mut state.osc_bass,
            OscilloscopeSignal::Sum => &mut state.osc_sum,
        };
        target[..len].copy_from_slice(&samples[..len]);
        state.osc_len = state.osc_len.max(len);
    }
}

pub fn commit_oscilloscope_frame(shared: &SharedStateHandle) {
    if let Ok(mut state) = shared.lock() {
        state.osc_len = state.osc_len.min(OSCILLOSCOPE_BUFFER_SIZE);
        state.osc_sequence = state.osc_sequence.wrapping_add(1);
    }
}

pub fn oscilloscope_snapshot(shared: &SharedStateHandle) -> OscilloscopeSnapshot {
    if let Ok(state) = shared.lock() {
        return OscilloscopeSnapshot {
            kick: state.osc_kick,
            bass: state.osc_bass,
            sum: state.osc_sum,
            len: state.osc_len,
            sequence: state.osc_sequence,
        };
    }

    OscilloscopeSnapshot {
        kick: [0.0; OSCILLOSCOPE_BUFFER_SIZE],
        bass: [0.0; OSCILLOSCOPE_BUFFER_SIZE],
        sum: [0.0; OSCILLOSCOPE_BUFFER_SIZE],
        len: 0,
        sequence: 0,
    }
}

pub fn snapshot(shared: &SharedStateHandle) -> SharedSnapshot {
    let app_cfg = config::app_config();
    if let Ok(state) = shared.lock() {
        return SharedSnapshot {
            amp_lut: state.amp_lut,
            pitch_lut: state.pitch_lut,
            bass: state.bass.clone(),
            keytrack_enabled: state.keytrack_enabled,
            note_length_ms: state.note_length_ms,
            kick_oscillator_waveform: state.kick_oscillator_waveform,
            kick_retrigger: state.kick_retrigger,
            kick_legato_voice_steal: state.kick_legato_voice_steal,
            kick_pitch_hz: state.kick_pitch_hz,
            kick_phase_deg: state.kick_phase_deg,
            arrange_override: state.arrange_override,
            arrange_notes: state.arrange_notes,
            arrange_note_count: state.arrange_note_count,
            arrange_num_bars: state.arrange_num_bars,
            arrange_note_len_bars: state.arrange_note_len_bars,
            arrange_manual_tempo: state.arrange_manual_tempo,
            arrange_use_daw_tempo: state.arrange_use_daw_tempo,
            tempo: state.tempo,
            trigger_counter: state.trigger_counter,
        };
    }

    let mut amp_lut = [0.0; CURVE_LUT_SIZE];
    let mut pitch_lut = [0.0; CURVE_LUT_SIZE];
    let mut bass = [BassSlotParams::default(), BassSlotParams::default()];
    for i in 0..CURVE_LUT_SIZE {
        let t = i as f32 / (CURVE_LUT_SIZE as f32 - 1.0);
        amp_lut[i] = 1.0 - t;
        pitch_lut[i] = 1.0 - t;
        for slot in bass.iter_mut() {
            slot.amp_lut[i] = 1.0 - t;
            slot.filter_lut[i] = t;
        }
    }

    SharedSnapshot {
        amp_lut,
        pitch_lut,
        bass,
        keytrack_enabled: false,
        note_length_ms: app_cfg.note_length_max_ms,
        kick_oscillator_waveform: Waveform::Sine,
        kick_retrigger: true,
        kick_legato_voice_steal: true,
        kick_pitch_hz: 55.0,
        kick_phase_deg: 0.0,
        arrange_override: false,
        arrange_notes: [ArrangeNoteData::default(); ARRANGE_MAX_NOTES],
        arrange_note_count: 0,
        arrange_num_bars: 1.0,
        arrange_note_len_bars: 0.25,
        arrange_manual_tempo: 120.0,
        arrange_use_daw_tempo: true,
        tempo: None,
        trigger_counter: 0,
    }
}
