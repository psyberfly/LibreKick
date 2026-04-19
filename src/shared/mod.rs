use std::sync::{Arc, Mutex};

use crate::config;

pub const CURVE_LUT_SIZE: usize = 256;

#[derive(Clone, Copy)]
pub enum CurveKind {
    Amplitude,
    Pitch,
}

#[derive(Clone)]
pub struct SharedSnapshot {
    pub amp_lut: [f32; CURVE_LUT_SIZE],
    pub pitch_lut: [f32; CURVE_LUT_SIZE],
    pub tuning_a4_hz: f32,
    pub note_length_ms: f32,
    pub trigger_counter: u64,
}

pub(crate) struct SharedState {
    amp_lut: [f32; CURVE_LUT_SIZE],
    pitch_lut: [f32; CURVE_LUT_SIZE],
    tuning_a4_hz: f32,
    note_length_ms: f32,
    trigger_counter: u64,
}

impl Default for SharedState {
    fn default() -> Self {
        let app_cfg = config::app_config();
        Self {
            amp_lut: [0.0; CURVE_LUT_SIZE],
            pitch_lut: [0.0; CURVE_LUT_SIZE],
            tuning_a4_hz: app_cfg.default_tuning_a4_hz,
            note_length_ms: app_cfg.note_length_max_ms,
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

pub fn set_tuning_a4_hz(shared: &SharedStateHandle, tuning_a4_hz: f32) {
    if let Ok(mut state) = shared.lock() {
        state.tuning_a4_hz = tuning_a4_hz.max(f32::EPSILON);
    }
}

pub fn set_note_length_ms(shared: &SharedStateHandle, note_length_ms: f32) {
    let app_cfg = config::app_config();
    if let Ok(mut state) = shared.lock() {
        state.note_length_ms = note_length_ms.clamp(0.0, app_cfg.note_length_max_ms);
    }
}

pub fn snapshot(shared: &SharedStateHandle) -> SharedSnapshot {
    let app_cfg = config::app_config();
    if let Ok(state) = shared.lock() {
        return SharedSnapshot {
            amp_lut: state.amp_lut,
            pitch_lut: state.pitch_lut,
            tuning_a4_hz: state.tuning_a4_hz,
            note_length_ms: state.note_length_ms,
            trigger_counter: state.trigger_counter,
        };
    }

    let mut amp_lut = [0.0; CURVE_LUT_SIZE];
    let mut pitch_lut = [0.0; CURVE_LUT_SIZE];
    for i in 0..CURVE_LUT_SIZE {
        let t = i as f32 / (CURVE_LUT_SIZE as f32 - 1.0);
        amp_lut[i] = 1.0 - t;
        pitch_lut[i] = 1.0 - t;
    }

    SharedSnapshot {
        amp_lut,
        pitch_lut,
        tuning_a4_hz: app_cfg.default_tuning_a4_hz,
        note_length_ms: app_cfg.note_length_max_ms,
        trigger_counter: 0,
    }
}
