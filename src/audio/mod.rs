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
    pub midi_velocity: f32,
    pub midi_note_hz: Option<f32>,
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
            if let Some(note_hz) = params.midi_note_hz {
                self.voice.trigger_with_note_velocity(
                    note_hz,
                    params.midi_velocity.clamp(0.0, 1.0),
                );
            } else {
                self.voice
                    .trigger_with_velocity(params.midi_velocity.clamp(0.0, 1.0));
            }
        }

        if shared_snapshot.trigger_counter != self.last_shared_trigger_counter {
            self.last_shared_trigger_counter = shared_snapshot.trigger_counter;
            self.voice.trigger();
        }

        let tuning_scale = (shared_snapshot.tuning_a4_hz / 440.0).clamp(0.9, 1.1);

        let voice_params = VoiceParams {
            decay_ms: params.decay_ms,
            base_freq_hz: params.base_freq_hz,
            pitch_drop_hz: params.pitch_drop_hz,
            level: params.level,
            tuning_scale,
        };

        for mut channel_samples in buffer.iter_samples() {
            let sample = self.voice.next_sample(
                voice_params,
                &shared_snapshot.amp_lut,
                &shared_snapshot.pitch_lut,
            );
            let limited_sample = sample.clamp(-1.0, 1.0);

            for output in channel_samples.iter_mut() {
                *output = limited_sample;
            }
        }

        ProcessStatus::Normal
    }
}
