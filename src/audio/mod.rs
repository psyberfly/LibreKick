mod voice;

use nih_plug::prelude::*;

use crate::shared;

use self::voice::{KickVoice, VoiceParams};

#[derive(Clone, Copy)]
pub struct KickDspParams {
    pub decay_ms: f32,
    pub base_freq_hz: f32,
    pub pitch_drop_hz: f32,
    pub level: f32,
    pub trigger_active: bool,
    pub midi_trigger: bool,
}

pub struct KickEngine {
    voice: KickVoice,
    last_trigger_param: bool,
    last_shared_trigger_counter: u64,
}

impl Default for KickEngine {
    fn default() -> Self {
        Self {
            voice: KickVoice::default(),
            last_trigger_param: false,
            last_shared_trigger_counter: 0,
        }
    }
}

impl KickEngine {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.voice.set_sample_rate(sample_rate);
    }

    pub fn process(
        &mut self,
        buffer: &mut Buffer,
        params: KickDspParams,
        shared_handle: &shared::SharedStateHandle,
    ) -> ProcessStatus {
        let shared_snapshot = shared::snapshot(shared_handle);

        if params.trigger_active && !self.last_trigger_param {
            self.voice.trigger();
        }
        self.last_trigger_param = params.trigger_active;

        if params.midi_trigger {
            self.voice.trigger();
        }

        if shared_snapshot.trigger_counter != self.last_shared_trigger_counter {
            self.last_shared_trigger_counter = shared_snapshot.trigger_counter;
            self.voice.trigger();
        }

        let voice_params = VoiceParams {
            decay_ms: params.decay_ms,
            base_freq_hz: params.base_freq_hz,
            pitch_drop_hz: params.pitch_drop_hz,
            level: params.level,
        };

        for mut channel_samples in buffer.iter_samples() {
            let sample = self.voice.next_sample(
                voice_params,
                &shared_snapshot.amp_lut,
                &shared_snapshot.pitch_lut,
            );

            for output in channel_samples.iter_mut() {
                *output = sample;
            }
        }

        ProcessStatus::Normal
    }
}
