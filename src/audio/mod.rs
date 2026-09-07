mod oscillator;
mod voice;

use nih_plug::prelude::*;

use crate::{
    common::logger::LOGGER,
    midi::RoutedMidiEvent,
    shared,
};

use self::voice::{BassVoice, KickVoice};
pub use self::voice::{BassVoiceParams, VoiceParams};

/// Preview sample rate for all offline-rendered waveforms (instrument pages
/// and arrange clip). The voices are rate-dependent, so every preview must
/// use this same rate to produce identical output.
pub const PREVIEW_SAMPLE_RATE: f32 = 4_000.0;

#[derive(Clone, Copy)]
pub struct KickDspParams {
    pub kick_level: f32,
    pub bass_level: f32,
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
            LOGGER.debug("kick trigger via trigger_active rising edge");
            self.voice.trigger(
                shared_snapshot.kick_retrigger,
                shared_snapshot.kick_legato_voice_steal,
            );
        }
        self.last_trigger_param = params.trigger_active;

        if params.midi_trigger {
            LOGGER.debug(format!(
                "kick trigger via MIDI velocity={:.3} note_hz={:?}",
                params.midi_velocity, params.midi_note_hz
            ));
            if let Some(note_hz) = params.midi_note_hz {
                self.voice.trigger_with_note_velocity(
                    note_hz,
                    params.midi_velocity.clamp(0.0, 1.0),
                    shared_snapshot.kick_retrigger,
                    shared_snapshot.kick_legato_voice_steal,
                );
            } else {
                self.voice.trigger_with_velocity(
                    params.midi_velocity.clamp(0.0, 1.0),
                    shared_snapshot.kick_retrigger,
                    shared_snapshot.kick_legato_voice_steal,
                );
            }
        }

        if shared_snapshot.trigger_counter != self.last_shared_trigger_counter {
            self.last_shared_trigger_counter = shared_snapshot.trigger_counter;
            LOGGER.debug("kick trigger via shared::request_trigger counter");
            self.voice.trigger(
                shared_snapshot.kick_retrigger,
                shared_snapshot.kick_legato_voice_steal,
            );
        }

        let tuning_scale = 1.0;

        let voice_params = VoiceParams {
            level: params.kick_level,
            keytrack_enabled: shared_snapshot.keytrack_enabled,
            tuning_scale,
            note_length_ms: shared_snapshot.note_length_ms,
            pitch_hz: shared_snapshot.kick_pitch_hz,
            waveform: shared_snapshot.kick_oscillator_waveform,
        };

        let bass_voice_params = BassVoiceParams {
            level: params.bass_level,
            tuning_scale,
            note_length_ms: shared_snapshot.bass_note_length_ms,
            base_cutoff_hz: shared_snapshot.bass_cutoff_hz,
            pitch_hz: shared_snapshot.bass_pitch_hz,
            filter_mode: shared_snapshot.bass_filter_mode,
            waveform: shared_snapshot.bass_oscillator_waveform,
        };

        let mut bass_event_index = 0usize;
        let bass_event_count = params.bass_event_count.min(params.bass_events.len());
        let mut osc_kick = [0.0_f32; shared::OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_bass = [0.0_f32; shared::OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_sum = [0.0_f32; shared::OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_len = 0usize;

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
                    let note_hz = if shared_snapshot.bass_keytrack_enabled && event.note >= 24 && event.note <= 35 {
                        // Bass keytrack mode: C0-C1 (MIDI 24-35) shifts bass note by semitones
                        // C0 (24) = base pitch, C#0 (25) = +1 semitone, ..., C1 (35) = +11 semitones
                        let semitone_shift = (event.note - 24) as f32;
                        shared_snapshot.bass_pitch_hz * 2.0_f32.powf(semitone_shift / 12.0)
                    } else {
                        // Normal mode: use the actual MIDI note frequency
                        440.0 * 2.0_f32.powf((event.note as f32 - 69.0) / 12.0)
                    };
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

            if sample_index < shared::OSCILLOSCOPE_BUFFER_SIZE {
                osc_kick[sample_index] = kick_sample;
                osc_bass[sample_index] = bass_sample;
                osc_sum[sample_index] = limited_sample;
                osc_len = sample_index + 1;
            }

            for output in channel_samples.iter_mut() {
                *output = limited_sample;
            }
        }

        if osc_len > 0 {
            shared::publish_oscilloscope_signal_block(
                shared_handle,
                shared::OscilloscopeSignal::Kick,
                &osc_kick[..osc_len],
            );
            shared::publish_oscilloscope_signal_block(
                shared_handle,
                shared::OscilloscopeSignal::Bass,
                &osc_bass[..osc_len],
            );
            shared::publish_oscilloscope_signal_block(
                shared_handle,
                shared::OscilloscopeSignal::Sum,
                &osc_sum[..osc_len],
            );
            shared::commit_oscilloscope_frame(shared_handle);
        }

        ProcessStatus::Normal
    }
}

/// A single note to render in the arrangement preview.
#[derive(Clone, Copy)]
pub struct ArrangeNoteSpec {
    /// true = kick lane, false = bass lane
    pub is_kick: bool,
    /// Semitone offset above the base bass pitch (bass only).
    pub semitone: i32,
    /// Note start time in seconds within the clip.
    pub start_seconds: f32,
}

/// Renders a single kick hit through the real `KickVoice` at a preview rate.
/// Returns the post-amp/pitch-envelope samples — identical to what the audio
/// thread produces for one trigger.
pub fn render_kick_preview(
    preview_rate: f32,
    total_seconds: f32,
    params: VoiceParams,
    amp_lut: &[f32; shared::CURVE_LUT_SIZE],
    pitch_lut: &[f32; shared::CURVE_LUT_SIZE],
    retrigger: bool,
    legato_voice_steal: bool,
) -> Vec<f32> {
    let total_samples = (total_seconds * preview_rate).ceil().max(1.0) as usize;
    let mut buffer = vec![0.0_f32; total_samples];
    let mut voice = KickVoice::default();
    voice.set_sample_rate(preview_rate);
    voice.trigger_with_velocity(1.0, retrigger, legato_voice_steal);
    for slot in buffer.iter_mut() {
        if !voice.is_active() {
            break;
        }
        *slot = voice.next_sample(params, amp_lut, pitch_lut);
    }
    buffer
}

/// Renders a single bass note through the real `BassVoice` at a preview rate.
/// Returns the post-amp-envelope, post-filter samples — identical to what the
/// audio thread produces for one note-on.
pub fn render_bass_preview(
    preview_rate: f32,
    total_seconds: f32,
    params: BassVoiceParams,
    note_hz: f32,
    amp_lut: &[f32; shared::CURVE_LUT_SIZE],
    filter_lut: &[f32; shared::CURVE_LUT_SIZE],
    retrigger: bool,
    legato_voice_steal: bool,
) -> Vec<f32> {
    let total_samples = (total_seconds * preview_rate).ceil().max(1.0) as usize;
    let mut buffer = vec![0.0_f32; total_samples];
    let mut voice = BassVoice::default();
    voice.set_sample_rate(preview_rate);
    voice.note_on(note_hz, 1.0, retrigger, legato_voice_steal);
    for slot in buffer.iter_mut() {
        if !voice.is_active() {
            break;
        }
        *slot = voice.next_sample(params, amp_lut, filter_lut);
    }
    buffer
}

/// Renders the arrangement's notes through the kick/bass voices at a reduced
/// preview sample rate. Returns `(kick, bass)` mono buffers so the UI can
/// draw each instrument in its own color.
pub fn render_arrangement_preview(
    notes: &[ArrangeNoteSpec],
    total_seconds: f32,
    preview_rate: f32,
    shared: &shared::SharedSnapshot,
    kick_level: f32,
    bass_level: f32,
) -> (Vec<f32>, Vec<f32>) {
    let total_samples = (total_seconds * preview_rate).ceil().max(1.0) as usize;
    let mut kick_buffer = vec![0.0_f32; total_samples];
    let mut bass_buffer = vec![0.0_f32; total_samples];

    let tuning_scale = 1.0_f32;
    let kick_params = VoiceParams {
        level: kick_level,
        keytrack_enabled: shared.keytrack_enabled,
        tuning_scale,
        note_length_ms: shared.note_length_ms,
        pitch_hz: shared.kick_pitch_hz,
        waveform: shared.kick_oscillator_waveform,
    };
    let bass_params = BassVoiceParams {
        level: bass_level,
        tuning_scale,
        note_length_ms: shared.bass_note_length_ms,
        base_cutoff_hz: shared.bass_cutoff_hz,
        pitch_hz: shared.bass_pitch_hz,
        filter_mode: shared.bass_filter_mode,
        waveform: shared.bass_oscillator_waveform,
    };

    for note in notes {
        let start = (note.start_seconds * preview_rate) as usize;
        if start >= total_samples {
            continue;
        }

        if note.is_kick {
            let mut voice = KickVoice::default();
            voice.set_sample_rate(preview_rate);
            voice.trigger_with_velocity(
                1.0,
                shared.kick_retrigger,
                shared.kick_legato_voice_steal,
            );
            for slot in kick_buffer.iter_mut().skip(start) {
                if !voice.is_active() {
                    break;
                }
                *slot += voice.next_sample(kick_params, &shared.amp_lut, &shared.pitch_lut);
            }
        } else {
            let note_hz = shared.bass_pitch_hz
                * 2.0_f32.powf(note.semitone as f32 / 12.0);
            let mut voice = BassVoice::default();
            voice.set_sample_rate(preview_rate);
            voice.note_on(
                note_hz,
                1.0,
                shared.bass_retrigger,
                shared.bass_legato_voice_steal,
            );
            for slot in bass_buffer.iter_mut().skip(start) {
                if !voice.is_active() {
                    break;
                }
                *slot += voice.next_sample(
                    bass_params,
                    &shared.bass_amp_lut,
                    &shared.bass_filter_lut,
                );
            }
        }
    }

    (kick_buffer, bass_buffer)
}
