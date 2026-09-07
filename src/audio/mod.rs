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
    /// Note-on count this block across ALL notes (arrange-override gate).
    pub gate_note_ons: u32,
    /// Note-off count this block across ALL notes (incl. velocity-0 note-ons).
    pub gate_note_offs: u32,
    /// Sample timing of the first note-on this block, if any.
    pub gate_first_on_timing: Option<u32>,
    /// Velocity of the first note-on this block.
    pub gate_velocity: f32,
}

/// A scheduled arrange-pattern event within one audio block.
#[derive(Clone, Copy, Default)]
struct SeqEvent {
    /// Sample offset within the block.
    timing: u32,
    /// Absolute bar position of the note-on (used to schedule the note-off).
    abs_bar: f64,
    /// Arrange row: 0-11 = bass semitones, 12 = kick lane.
    row: u8,
    is_on: bool,
}

pub struct KickEngine {
    voice: KickVoice,
    bass_voice: BassVoice,
    last_trigger_param: bool,
    last_shared_trigger_counter: u64,
    sample_rate: f32,
    /// Number of DAW notes currently held (arrange-override gate).
    arrange_held: u32,
    /// Whether the arrange sequencer is currently running.
    arrange_active: bool,
    /// Sequencer playhead in bars since the gate opened (loops over num_bars).
    arrange_abs_bars: f64,
    /// Absolute bar position at which the current bass note ends.
    arrange_bass_off_abs: f64,
}

impl Default for KickEngine {
    fn default() -> Self {
        Self {
            voice: KickVoice::default(),
            bass_voice: BassVoice::default(),
            last_trigger_param: false,
            last_shared_trigger_counter: 0,
            sample_rate: 44_100.0,
            arrange_held: 0,
            arrange_active: false,
            arrange_abs_bars: 0.0,
            arrange_bass_off_abs: f64::MAX,
        }
    }
}

impl KickEngine {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
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

        let kick_phase_offset = shared_snapshot.kick_phase_deg / 360.0;
        let bass_phase_offset = shared_snapshot.bass_phase_deg / 360.0;

        if params.trigger_active && !self.last_trigger_param {
            LOGGER.debug("kick trigger via trigger_active rising edge");
            self.voice.trigger(
                kick_phase_offset,
                shared_snapshot.kick_retrigger,
                shared_snapshot.kick_legato_voice_steal,
            );
        }
        self.last_trigger_param = params.trigger_active;

        // --- Arrange override gate: any held DAW note runs the internal ---
        // --- pattern instead of routing MIDI directly to the voices.    ---
        let override_on = shared_snapshot.arrange_override;
        self.arrange_held = self
            .arrange_held
            .saturating_add(params.gate_note_ons)
            .saturating_sub(params.gate_note_offs);
        let seq_active = override_on && self.arrange_held > 0;

        let tempo_bpm = if shared_snapshot.arrange_use_daw_tempo {
            shared_snapshot
                .tempo
                .unwrap_or(shared_snapshot.arrange_manual_tempo as f64)
        } else {
            shared_snapshot.arrange_manual_tempo as f64
        };
        let bar_seconds = 4.0 * 60.0 / tempo_bpm.max(1.0);
        let num_bars = (shared_snapshot.arrange_num_bars as f64).max(0.25);

        if seq_active && !self.arrange_active {
            // Align bar 0 to the gate note-on sample within this block.
            let offset_samples = params.gate_first_on_timing.unwrap_or(0) as f64;
            self.arrange_abs_bars =
                -(offset_samples / self.sample_rate as f64 / bar_seconds);
            self.arrange_bass_off_abs = f64::MAX;
        }
        if !seq_active && self.arrange_active {
            self.bass_voice.note_off();
        }
        self.arrange_active = seq_active;

        // Schedule this block's pattern events (note-ons + pending bass off).
        const MAX_SEQ_EVENTS: usize = shared::ARRANGE_MAX_NOTES + 1;
        let mut seq_events = [SeqEvent::default(); MAX_SEQ_EVENTS];
        let mut seq_count = 0usize;
        let block_samples = buffer.samples();
        let block_bars = block_samples as f64 / self.sample_rate as f64 / bar_seconds;
        let abs_start = self.arrange_abs_bars;
        let abs_end = abs_start + block_bars;

        if seq_active {
            let note_count = shared_snapshot
                .arrange_note_count
                .min(shared::ARRANGE_MAX_NOTES);
            for note in &shared_snapshot.arrange_notes[..note_count] {
                let pos = note.bar_pos as f64;
                if pos < 0.0 || pos >= num_bars {
                    continue;
                }
                // Next occurrence of this note at or after the playhead.
                let mut fire = (abs_start / num_bars).floor() * num_bars + pos;
                while fire < abs_start - 1e-9 {
                    fire += num_bars;
                }
                if fire < abs_end && seq_count < MAX_SEQ_EVENTS {
                    let timing = ((fire - abs_start) * bar_seconds
                        * self.sample_rate as f64) as u32;
                    seq_events[seq_count] = SeqEvent {
                        timing: timing.min(block_samples.saturating_sub(1) as u32),
                        abs_bar: fire,
                        row: note.row,
                        is_on: true,
                    };
                    seq_count += 1;
                }
            }
            if self.arrange_bass_off_abs < abs_end && seq_count < MAX_SEQ_EVENTS {
                let off = self.arrange_bass_off_abs.max(abs_start);
                let timing =
                    ((off - abs_start) * bar_seconds * self.sample_rate as f64) as u32;
                seq_events[seq_count] = SeqEvent {
                    timing: timing.min(block_samples.saturating_sub(1) as u32),
                    abs_bar: off,
                    row: 0,
                    is_on: false,
                };
                seq_count += 1;
                self.arrange_bass_off_abs = f64::MAX;
            }
            // Offs before ons at equal timing so back-to-back notes don't
            // have the new note-on killed by the previous note's off.
            seq_events[..seq_count].sort_by_key(|e| (e.timing, e.is_on));
            self.arrange_abs_bars = abs_end;
        }

        if params.midi_trigger && !override_on {
            LOGGER.debug(format!(
                "kick trigger via MIDI velocity={:.3} note_hz={:?}",
                params.midi_velocity, params.midi_note_hz
            ));
            if let Some(note_hz) = params.midi_note_hz {
                self.voice.trigger_with_note_velocity(
                    note_hz,
                    params.midi_velocity.clamp(0.0, 1.0),
                    kick_phase_offset,
                    shared_snapshot.kick_retrigger,
                    shared_snapshot.kick_legato_voice_steal,
                );
            } else {
                self.voice.trigger_with_velocity(
                    params.midi_velocity.clamp(0.0, 1.0),
                    kick_phase_offset,
                    shared_snapshot.kick_retrigger,
                    shared_snapshot.kick_legato_voice_steal,
                );
            }
        }

        if shared_snapshot.trigger_counter != self.last_shared_trigger_counter {
            self.last_shared_trigger_counter = shared_snapshot.trigger_counter;
            LOGGER.debug("kick trigger via shared::request_trigger counter");
            self.voice.trigger(
                kick_phase_offset,
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
            filter_mode: shared_snapshot.bass_filter_mode,
            waveform: shared_snapshot.bass_oscillator_waveform,
        };

        let mut bass_event_index = 0usize;
        let bass_event_count = if override_on {
            0
        } else {
            params.bass_event_count.min(params.bass_events.len())
        };
        let mut seq_index = 0usize;
        let gate_velocity = params.gate_velocity.clamp(0.0, 1.0);
        let note_len_bars = shared_snapshot.arrange_note_len_bars as f64;
        let mut osc_kick = [0.0_f32; shared::OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_bass = [0.0_f32; shared::OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_sum = [0.0_f32; shared::OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_len = 0usize;

        for (sample_index, mut channel_samples) in buffer.iter_samples().enumerate() {
            while seq_index < seq_count
                && seq_events[seq_index].timing <= sample_index as u32
            {
                let event = seq_events[seq_index];
                seq_index += 1;
                if event.row as usize == 12 {
                    // Kick lane — always steal so every hit sounds
                    self.voice.trigger_with_velocity(
                        gate_velocity,
                        kick_phase_offset,
                        shared_snapshot.kick_retrigger,
                        true,
                    );
                } else if event.is_on {
                    // Bass rows 0-11: row 0 = B (+11 semitones) .. row 11 = C (+0)
                    let semitone = 11 - event.row.min(11) as i32;
                    let note_hz = if shared_snapshot.bass_keytrack_enabled {
                        shared_snapshot.bass_pitch_hz
                            * 2.0_f32.powf(semitone as f32 / 12.0)
                    } else {
                        shared_snapshot.bass_pitch_hz
                    };
                    self.bass_voice.note_on(
                        note_hz,
                        gate_velocity,
                        bass_phase_offset,
                        shared_snapshot.bass_retrigger,
                        true,
                    );
                    self.arrange_bass_off_abs = event.abs_bar + note_len_bars;
                } else {
                    self.bass_voice.note_off();
                }
            }

            while bass_event_index < bass_event_count {
                let Some(event) = params.bass_events[bass_event_index] else {
                    bass_event_index += 1;
                    continue;
                };

                if event.timing > sample_index as u32 {
                    break;
                }

                if event.is_note_on {
                    let note_hz = if shared_snapshot.bass_keytrack_enabled {
                        // Keytrack: base pitch is C; each semitone above shifts
                        // up. note % 12 gives the pitch class (C=0 .. B=11),
                        // covering both control ranges (24-35 and 48-59).
                        let semitone = (event.note % 12) as f32;
                        shared_snapshot.bass_pitch_hz * 2.0_f32.powf(semitone / 12.0)
                    } else {
                        // Keytrack off: always the instrument's base pitch.
                        shared_snapshot.bass_pitch_hz
                    };
                    self.bass_voice.note_on(
                        note_hz,
                        event.velocity.clamp(0.0, 1.0),
                        bass_phase_offset,
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
    phase_offset: f32,
    retrigger: bool,
    legato_voice_steal: bool,
) -> Vec<f32> {
    let total_samples = (total_seconds * preview_rate).ceil().max(1.0) as usize;
    let mut buffer = vec![0.0_f32; total_samples];
    let mut voice = KickVoice::default();
    voice.set_sample_rate(preview_rate);
    voice.trigger_with_velocity(1.0, phase_offset, retrigger, legato_voice_steal);
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
    phase_offset: f32,
    retrigger: bool,
    legato_voice_steal: bool,
) -> Vec<f32> {
    let total_samples = (total_seconds * preview_rate).ceil().max(1.0) as usize;
    let mut buffer = vec![0.0_f32; total_samples];
    let mut voice = BassVoice::default();
    voice.set_sample_rate(preview_rate);
    voice.note_on(note_hz, 1.0, phase_offset, retrigger, legato_voice_steal);
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
                shared.kick_phase_deg / 360.0,
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
            let note_hz = if shared.bass_keytrack_enabled {
                shared.bass_pitch_hz * 2.0_f32.powf(note.semitone as f32 / 12.0)
            } else {
                shared.bass_pitch_hz
            };
            let mut voice = BassVoice::default();
            voice.set_sample_rate(preview_rate);
            voice.note_on(
                note_hz,
                1.0,
                shared.bass_phase_deg / 360.0,
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
