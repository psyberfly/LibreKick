use std::sync::{Arc, Mutex};

use crate::config;

pub const CURVE_LUT_SIZE: usize = 256;

#[derive(Clone, Copy)]
pub enum CurveKind {
    Amplitude,
    Pitch,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BassWaveform {
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
    pub bass_note_length_ms: f32,
    pub bass_cutoff_hz: f32,
    pub bass_pitch_hz: f32,
    pub bass_retrigger: bool,
    pub bass_legato_voice_steal: bool,
    pub bass_filter_mode: BassFilterMode,
    pub bass_waveform: BassWaveform,
    pub trigger_counter: u64,
}

pub(crate) struct SharedState {
    amp_lut: [f32; CURVE_LUT_SIZE],
    pitch_lut: [f32; CURVE_LUT_SIZE],
    bass_amp_lut: [f32; CURVE_LUT_SIZE],
    bass_filter_lut: [f32; CURVE_LUT_SIZE],
    keytrack_enabled: bool,
    note_length_ms: f32,
    bass_note_length_ms: f32,
    bass_cutoff_hz: f32,
    bass_pitch_hz: f32,
    bass_retrigger: bool,
    bass_legato_voice_steal: bool,
    bass_filter_mode: BassFilterMode,
    bass_waveform: BassWaveform,
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
            bass_note_length_ms: app_cfg.note_length_max_ms,
            bass_cutoff_hz: 120.0,
            bass_pitch_hz: 55.0,
            bass_retrigger: true,
            bass_legato_voice_steal: false,
            bass_filter_mode: BassFilterMode::LowPass,
            bass_waveform: BassWaveform::Saw,
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

pub fn set_bass_waveform(shared: &SharedStateHandle, waveform: BassWaveform) {
    if let Ok(mut state) = shared.lock() {
        state.bass_waveform = waveform;
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
            bass_note_length_ms: state.bass_note_length_ms,
            bass_cutoff_hz: state.bass_cutoff_hz,
            bass_pitch_hz: state.bass_pitch_hz,
            bass_retrigger: state.bass_retrigger,
            bass_legato_voice_steal: state.bass_legato_voice_steal,
            bass_filter_mode: state.bass_filter_mode,
            bass_waveform: state.bass_waveform,
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
        bass_note_length_ms: app_cfg.note_length_max_ms,
        bass_cutoff_hz: 120.0,
        bass_pitch_hz: 55.0,
        bass_retrigger: true,
        bass_legato_voice_steal: false,
        bass_filter_mode: BassFilterMode::LowPass,
        bass_waveform: BassWaveform::Saw,
        trigger_counter: 0,
    }
}
