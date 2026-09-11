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
    pub note_length_ms: f32,
    pub cutoff_hz: f32,
    pub filter_mode: BassFilterMode,
    pub filter_slope: BassFilterSlope,
    pub filter_drive: f32,
    pub pitch_hz: f32,
    pub retrigger: bool,
    pub legato_voice_steal: bool,
    pub oscillator_waveform: Waveform,
    pub keytrack_enabled: bool,
    /// Oscillator start phase in degrees (0-360), applied on retrigger.
    pub phase_deg: f32,
}

impl Default for BassSlotParams {
    fn default() -> Self {
        let app_cfg = config::app_config();
        Self {
            amp_lut: [0.0; CURVE_LUT_SIZE],
            filter_lut: [0.0; CURVE_LUT_SIZE],
            note_length_ms: app_cfg.note_length_max_ms,
            cutoff_hz: 120.0,
            filter_mode: BassFilterMode::LowPass,
            filter_slope: BassFilterSlope::S6dB,
            filter_drive: 0.0,
            pitch_hz: 55.0,
            retrigger: true,
            legato_voice_steal: false,
            oscillator_waveform: Waveform::Saw,
            keytrack_enabled: false,
            phase_deg: 0.0,
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
