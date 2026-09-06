use std::sync::{Arc, Mutex};

use crate::config;
use crate::interface::{
    BassFilterMode, Waveform, CURVE_LUT_SIZE, OSCILLOSCOPE_BUFFER_SIZE,
};

pub(crate) struct SharedState {
    pub(crate) amp_lut: [f32; CURVE_LUT_SIZE],
    pub(crate) pitch_lut: [f32; CURVE_LUT_SIZE],
    pub(crate) bass_amp_lut: [f32; CURVE_LUT_SIZE],
    pub(crate) bass_filter_lut: [f32; CURVE_LUT_SIZE],
    pub(crate) keytrack_enabled: bool,
    pub(crate) note_length_ms: f32,
    pub(crate) kick_oscillator_waveform: Waveform,
    pub(crate) kick_retrigger: bool,
    pub(crate) kick_legato_voice_steal: bool,
    pub(crate) bass_note_length_ms: f32,
    pub(crate) bass_cutoff_hz: f32,
    pub(crate) bass_pitch_hz: f32,
    pub(crate) bass_retrigger: bool,
    pub(crate) bass_legato_voice_steal: bool,
    pub(crate) bass_filter_mode: BassFilterMode,
    pub(crate) bass_oscillator_waveform: Waveform,
    pub(crate) osc_kick: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    pub(crate) osc_bass: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    pub(crate) osc_sum: [f32; OSCILLOSCOPE_BUFFER_SIZE],
    pub(crate) osc_len: usize,
    pub(crate) osc_sequence: u64,
    pub(crate) trigger_counter: u64,
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

pub(crate) fn default_curve_luts(
) -> (
    [f32; CURVE_LUT_SIZE],
    [f32; CURVE_LUT_SIZE],
    [f32; CURVE_LUT_SIZE],
    [f32; CURVE_LUT_SIZE],
) {
    let mut amp_lut = [0.0; CURVE_LUT_SIZE];
    let mut pitch_lut = [0.0; CURVE_LUT_SIZE];
    let mut bass_amp_lut = [0.0; CURVE_LUT_SIZE];
    let mut bass_filter_lut = [0.0; CURVE_LUT_SIZE];

    for i in 0..CURVE_LUT_SIZE {
        let t = i as f32 / (CURVE_LUT_SIZE as f32 - 1.0);
        amp_lut[i] = (1.0 - t).clamp(0.0, 1.0);
        pitch_lut[i] = (1.0 - t).clamp(0.0, 1.0);
        bass_amp_lut[i] = (1.0 - t).clamp(0.0, 1.0);
        bass_filter_lut[i] = t.clamp(0.0, 1.0);
    }

    (amp_lut, pitch_lut, bass_amp_lut, bass_filter_lut)
}

pub fn new_shared_state() -> SharedStateHandle {
    let mut state = SharedState::default();
    let (amp_lut, pitch_lut, bass_amp_lut, bass_filter_lut) = default_curve_luts();

    state.amp_lut = amp_lut;
    state.pitch_lut = pitch_lut;
    state.bass_amp_lut = bass_amp_lut;
    state.bass_filter_lut = bass_filter_lut;

    Arc::new(Mutex::new(state))
}
