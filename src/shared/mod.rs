use std::sync::{Arc, Mutex};

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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Waveform {
    Saw,
    Square,
    Sine,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BassFilterMode {
    LowPass,
    HighPass,
    BandPass,
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

pub fn set_bass_amp_lut(shared: &SharedStateHandle, lut: [f32; CURVE_LUT_SIZE]) {
    if let Ok(mut state) = shared.lock() {
        state.bass_amp_lut = lut;
    }
}

pub fn set_bass_filter_lut(shared: &SharedStateHandle, lut: [f32; CURVE_LUT_SIZE]) {
    if let Ok(mut state) = shared.lock() {
        state.bass_filter_lut = lut;
    }
}

#[derive(Clone)]
pub struct SharedSnapshot {
    pub amp_lut: [f32; CURVE_LUT_SIZE],
    pub pitch_lut: [f32; CURVE_LUT_SIZE],
    pub bass_amp_lut: [f32; CURVE_LUT_SIZE],
    pub bass_filter_lut: [f32; CURVE_LUT_SIZE],
    pub keytrack_enabled: bool,
    pub note_length_ms: f32,
    pub kick_oscillator_waveform: Waveform,
    pub kick_retrigger: bool,
    pub kick_legato_voice_steal: bool,
    pub bass_note_length_ms: f32,
    pub bass_cutoff_hz: f32,
    pub bass_pitch_hz: f32,
    pub bass_retrigger: bool,
    pub bass_legato_voice_steal: bool,
    pub bass_filter_mode: BassFilterMode,
    pub bass_oscillator_waveform: Waveform,
    pub trigger_counter: u64,
}

pub(crate) struct SharedState {
    amp_lut: [f32; CURVE_LUT_SIZE],
    pitch_lut: [f32; CURVE_LUT_SIZE],
    bass_amp_lut: [f32; CURVE_LUT_SIZE],
    bass_filter_lut: [f32; CURVE_LUT_SIZE],
    keytrack_enabled: bool,
    note_length_ms: f32,
    kick_oscillator_waveform: Waveform,
    kick_retrigger: bool,
    kick_legato_voice_steal: bool,
    bass_note_length_ms: f32,
    bass_cutoff_hz: f32,
    bass_pitch_hz: f32,
    bass_retrigger: bool,
    bass_legato_voice_steal: bool,
    bass_filter_mode: BassFilterMode,
    bass_oscillator_waveform: Waveform,
    osc_kick: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    osc_bass: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    osc_sum: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    osc_len: usize,
    osc_sequence: u64,
    trigger_counter: u64,
}

impl Default for SharedState {
    fn default() -> Self {
        let app_cfg = config::app_config();
        Self {
            amp_lut: [0.0; CURVE_LUT_SIZE],
            pitch_lut: [0.0; CURVE_LUT_SIZE],
            bass_amp_lut: [0.0; CURVE_LUT_SIZE],
            bass_filter_lut: [0.0; CURVE_LUT_SIZE],
            keytrack_enabled: false,
            note_length_ms: app_cfg.note_length_max_ms,
            kick_oscillator_waveform: Waveform::Sine,
            kick_retrigger: true,
            kick_legato_voice_steal: true,
            bass_note_length_ms: app_cfg.note_length_max_ms,
            bass_cutoff_hz: 120.0,
            bass_pitch_hz: 55.0,
            bass_retrigger: true,
            bass_legato_voice_steal: false,
            bass_filter_mode: BassFilterMode::LowPass,
            bass_oscillator_waveform: Waveform::Saw,
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
        state.bass_amp_lut[i] = (1.0 - t).clamp(0.0, 1.0);
        state.bass_filter_lut[i] = t.clamp(0.0, 1.0);
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
    if let Ok(mut state) = shared.lock() {
        state.trigger_counter = state.trigger_counter.wrapping_add(1);
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

pub fn set_bass_note_length_ms(shared: &SharedStateHandle, note_length_ms: f32) {
    if let Ok(mut state) = shared.lock() {
        state.bass_note_length_ms = note_length_ms.clamp(1.0, 1000.0);
    }
}

pub fn set_bass_cutoff_hz(shared: &SharedStateHandle, cutoff_hz: f32) {
    if let Ok(mut state) = shared.lock() {
        state.bass_cutoff_hz = cutoff_hz.clamp(20.0, 8_000.0);
    }
}

pub fn set_bass_filter_mode(shared: &SharedStateHandle, mode: BassFilterMode) {
    if let Ok(mut state) = shared.lock() {
        state.bass_filter_mode = mode;
    }
}

pub fn set_bass_pitch_hz(shared: &SharedStateHandle, pitch_hz: f32) {
    if let Ok(mut state) = shared.lock() {
        state.bass_pitch_hz = pitch_hz.clamp(20.0, 2_000.0);
    }
}

pub fn set_bass_retrigger(shared: &SharedStateHandle, bass_retrigger: bool) {
    if let Ok(mut state) = shared.lock() {
        state.bass_retrigger = bass_retrigger;
    }
}

pub fn set_bass_legato_voice_steal(
    shared: &SharedStateHandle,
    bass_legato_voice_steal: bool,
) {
    if let Ok(mut state) = shared.lock() {
        state.bass_legato_voice_steal = bass_legato_voice_steal;
    }
}

pub fn set_bass_oscillator_waveform(shared: &SharedStateHandle, waveform: Waveform) {
    if let Ok(mut state) = shared.lock() {
        state.bass_oscillator_waveform = waveform;
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
            bass_amp_lut: state.bass_amp_lut,
            bass_filter_lut: state.bass_filter_lut,
            keytrack_enabled: state.keytrack_enabled,
            note_length_ms: state.note_length_ms,
            kick_oscillator_waveform: state.kick_oscillator_waveform,
            kick_retrigger: state.kick_retrigger,
            kick_legato_voice_steal: state.kick_legato_voice_steal,
            bass_note_length_ms: state.bass_note_length_ms,
            bass_cutoff_hz: state.bass_cutoff_hz,
            bass_pitch_hz: state.bass_pitch_hz,
            bass_retrigger: state.bass_retrigger,
            bass_legato_voice_steal: state.bass_legato_voice_steal,
            bass_filter_mode: state.bass_filter_mode,
            bass_oscillator_waveform: state.bass_oscillator_waveform,
            trigger_counter: state.trigger_counter,
        };
    }

    let mut amp_lut = [0.0; CURVE_LUT_SIZE];
    let mut pitch_lut = [0.0; CURVE_LUT_SIZE];
    let mut bass_amp_lut = [0.0; CURVE_LUT_SIZE];
    let mut bass_filter_lut = [0.0; CURVE_LUT_SIZE];
    for i in 0..CURVE_LUT_SIZE {
        let t = i as f32 / (CURVE_LUT_SIZE as f32 - 1.0);
        amp_lut[i] = 1.0 - t;
        pitch_lut[i] = 1.0 - t;
        bass_amp_lut[i] = 1.0 - t;
        bass_filter_lut[i] = t;
    }

    SharedSnapshot {
        amp_lut,
        pitch_lut,
        bass_amp_lut,
        bass_filter_lut,
        keytrack_enabled: false,
        note_length_ms: app_cfg.note_length_max_ms,
        kick_oscillator_waveform: Waveform::Sine,
        kick_retrigger: true,
        kick_legato_voice_steal: true,
        bass_note_length_ms: app_cfg.note_length_max_ms,
        bass_cutoff_hz: 120.0,
        bass_pitch_hz: 55.0,
        bass_retrigger: true,
        bass_legato_voice_steal: false,
        bass_filter_mode: BassFilterMode::LowPass,
        bass_oscillator_waveform: Waveform::Saw,
        trigger_counter: 0,
    }
}
