mod voice;

use nih_plug::prelude::*;

use crate::{midi::RoutedMidiEvent, shared};

use self::voice::{BassVoice, BassVoiceParams, KickVoice, VoiceParams};

#[derive(Clone, Copy)]
pub struct KickDspParams {
    pub level: f32,
    pub trigger_active: bool,
    pub midi_trigger: bool,
    pub midi_velocity: f32,
    pub midi_note_hz: Option<f32>,
    pub bass_events: [Option<RoutedMidiEvent>; 6],
    pub bass_event_count: usize,
}

pub struct KickEngine {
    voice: KickVoice,
    bass_voice: BassVoice,
    last_trigger_param: bool,
    last_shared_trigger_counter: u64,
}

impl Default for KickEngine {
    fn default() -> Self {
        Self {
            voice: KickVoice::default(),
            bass_voice: BassVoice::default(),
            last_trigger_param: false,
            last_shared_trigger_counter: 0,
        }
    }
}

impl KickEngine {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.voice.set_sample_rate(sample_rate);
        self.bass_voice.set_sample_rate(sample_rate);
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

        let tuning_scale = 1.0;

        let voice_params = VoiceParams {
            level: params.level,
            keytrack_enabled: shared_snapshot.keytrack_enabled,
            tuning_scale,
            note_length_ms: shared_snapshot.note_length_ms,
        };

        let bass_voice_params = BassVoiceParams {
            level: params.level,
            tuning_scale,
            note_length_ms: shared_snapshot.bass_note_length_ms,
            base_cutoff_hz: shared_snapshot.bass_cutoff_hz,
            pitch_hz: shared_snapshot.bass_pitch_hz,
            filter_mode: shared_snapshot.bass_filter_mode,
            waveform: shared_snapshot.bass_waveform,
        };

        let mut bass_event_index = 0usize;
        let bass_event_count = params.bass_event_count.min(params.bass_events.len());

        for (sample_index, mut channel_samples) in buffer.iter_samples().enumerate() {
            while bass_event_index < bass_event_count {
                let Some(event) = params.bass_events[bass_event_index] else {
                    bass_event_index += 1;
                    continue;
                };

                if event.timing > sample_index as u32 {
                    break;
                }

                if event.is_note_on {
                    let note_hz = 440.0 * 2.0_f32.powf((event.note as f32 - 69.0) / 12.0);
                    self.bass_voice.note_on(
                        note_hz,
                        event.velocity.clamp(0.0, 1.0),
                        shared_snapshot.bass_retrigger,
                        shared_snapshot.bass_legato_voice_steal,
                    );
                } else {
                    self.bass_voice.note_off();
                }

                bass_event_index += 1;
            }

            let kick_sample = self.voice.next_sample(
                voice_params,
                &shared_snapshot.amp_lut,
                &shared_snapshot.pitch_lut,
            );
            let bass_sample = self.bass_voice.next_sample(
                bass_voice_params,
                &shared_snapshot.bass_amp_lut,
                &shared_snapshot.bass_filter_lut,
            );
            let limited_sample = (kick_sample + bass_sample).clamp(-1.0, 1.0);

            for output in channel_samples.iter_mut() {
                *output = limited_sample;
            }
        }

        ProcessStatus::Normal
    }
}
