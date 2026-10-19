// The FX rack is implemented but not yet wired into the signal path; keeping
// it compiled means its tests run and it cannot silently rot.
#[allow(dead_code)]
mod fx;
mod lfo;
pub(crate) use self::lfo::shape_at;
mod oscillator;
mod voice;

use nih_plug::prelude::*;

use crate::{common::logger::LOGGER, midi::RoutedMidiEvent, shared};

use self::voice::{BassVoice, KickVoice};
pub use self::voice::{BassVoiceParams, VoiceParams};

/// Preview sample rate for all offline-rendered waveforms (instrument pages
/// and arrange clip). The voices are rate-dependent, so every preview must
/// use this same rate to produce identical output.
pub const PREVIEW_SAMPLE_RATE: f32 = 4_000.0;

#[derive(Clone, Copy)]
pub struct KickDspParams {
    pub kick_level: f32,
    /// Per-slot bass levels (index 0 = Note 1, 1 = Note 2).
    pub bass_levels: [f32; 2],
    pub trigger_active: bool,
    /// Timed kick note-ons dispatched inside the sample loop so a hit lands
    /// on its exact sample instead of the next block start.
    pub kick_events: [Option<RoutedMidiEvent>; 4],
    pub kick_event_count: usize,
    /// One event per played note, carrying the original note number. The
    /// host-bound fan-out copies must never reach the voice: three events on
    /// the same sample would retrigger the mono voice three times, and their
    /// transposed note numbers would re-pitch it.
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
    /// Bass note slot (0 = Note 1, 1 = Note 2); ignored for the kick lane.
    slot: u8,
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
    /// Bass slot of the currently sounding note (0 = Note 1, 1 = Note 2).
    bass_slot: usize,
    /// MIDI note number that currently owns the bass voice. Note-offs for
    /// other notes (overlapping keys) must not release it.
    bass_active_note: Option<u8>,
    /// Internal oversampling factor (1 = host rate, 2 = render at 2x and
    /// average each pair — a decimation null at the host Nyquist).
    oversample_factor: usize,
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
            bass_slot: 0,
            bass_active_note: None,
            oversample_factor: 1,
        }
    }
}

impl KickEngine {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate.max(1.0);
        self.apply_sample_rate();
    }

    /// Pushes host rate x oversample factor into both voices. Safe to call
    /// from `process` when the flag changes — it only touches voices when
    /// the factor actually moved.
    fn apply_sample_rate(&mut self) {
        let rate = self.sample_rate * self.oversample_factor as f32;
        self.voice.set_sample_rate(rate);
        self.bass_voice.set_sample_rate(rate);
    }

    pub fn process(
        &mut self,
        buffer: &mut Buffer,
        params: KickDspParams,
        shared_handle: &shared::SharedStateHandle,
    ) -> ProcessStatus {
        let shared_snapshot = shared::snapshot(shared_handle);
        self.voice.set_anti_aliasing(shared_snapshot.anti_aliasing);
        self.bass_voice
            .set_anti_aliasing(shared_snapshot.anti_aliasing);

        let wanted_os = if shared_snapshot.oversampling { 2 } else { 1 };
        if wanted_os != self.oversample_factor {
            self.oversample_factor = wanted_os;
            self.apply_sample_rate();
        }

        let kick_phase_offset = shared_snapshot.kick_phase_deg / 360.0;

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
            self.arrange_abs_bars = -(offset_samples / self.sample_rate as f64 / bar_seconds);
            self.arrange_bass_off_abs = f64::MAX;
        }
        if !seq_active && self.arrange_active {
            self.bass_voice.note_off();
            self.bass_active_note = None;
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
                    let timing =
                        ((fire - abs_start) * bar_seconds * self.sample_rate as f64) as u32;
                    seq_events[seq_count] = SeqEvent {
                        timing: timing.min(block_samples.saturating_sub(1) as u32),
                        abs_bar: fire,
                        row: note.row,
                        slot: note.slot,
                        is_on: true,
                    };
                    seq_count += 1;
                }
            }
            if self.arrange_bass_off_abs < abs_end && seq_count < MAX_SEQ_EVENTS {
                let off = self.arrange_bass_off_abs.max(abs_start);
                let timing = ((off - abs_start) * bar_seconds * self.sample_rate as f64) as u32;
                seq_events[seq_count] = SeqEvent {
                    timing: timing.min(block_samples.saturating_sub(1) as u32),
                    abs_bar: off,
                    row: 0,
                    slot: 0,
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
            // 1/16 Sync: gate the kick to a 16th at the active tempo (DAW
            // tempo when synced, arrange manual otherwise) instead of the
            // fixed LENGTH value. Applies in both direct-MIDI and arrange
            // override modes.
            note_length_ms: if shared_snapshot.kick_length_override {
                (15_000.0 / tempo_bpm.max(1.0)) as f32
            } else {
                shared_snapshot.note_length_ms
            },
            pitch_hz: shared_snapshot.kick_pitch_hz,
            waveform: shared_snapshot.kick_oscillator_waveform,
            amp_env_enabled: shared_snapshot.amp_env_enabled,
            pitch_env_enabled: shared_snapshot.pitch_env_enabled,
        };

        // Per-slot bass params; the active slot follows the last note played.
        let bass_voice_params: [BassVoiceParams; shared::BASS_SLOT_COUNT] =
            std::array::from_fn(|index| {
                let slot = &shared_snapshot.bass[index];
                BassVoiceParams {
                    level: params.bass_levels[index],
                    tuning_scale,
                    // A slot's self-gate follows one 16th at the active tempo
                    // (DAW tempo when synced, manual otherwise) when the
                    // arrange sequencer runs or the slot's 1/16 sync is on.
                    note_length_ms: if seq_active || slot.length_sync {
                        (15_000.0 / tempo_bpm.max(1.0)) as f32
                    } else {
                        slot.note_length_ms
                    },
                    base_cutoff_hz: slot.cutoff_hz,
                    filter_mode: slot.filter_mode,
                    filter_slope: slot.filter_slope,
                    filter_drive: slot.filter_drive,
                    resonance: slot.resonance,
                    filter_enabled: slot.filter_enabled,
                    filter_2_enabled: slot.filter_2_enabled,
                    waveform: slot.oscillator_waveform,
                    osc_a_level: slot.osc_a_level,
                    osc_a_unison: slot.osc_a_unison,
                    osc_a_unison_detune: slot.osc_a_unison_detune,
                    osc_a_unison_blend: slot.osc_a_unison_blend,
                    osc_b_enabled: slot.osc_b_enabled,
                    osc_b_waveform: slot.osc_b_waveform,
                    osc_b_level: slot.osc_b_level,
                    osc_b_octave: slot.osc_b_octave,
                    osc_b_detune_cents: slot.osc_b_detune_cents,
                    osc_b_unison: slot.osc_b_unison,
                    osc_b_unison_detune: slot.osc_b_unison_detune,
                    osc_b_unison_blend: slot.osc_b_unison_blend,
                    sub_enabled: slot.sub_enabled,
                    sub_waveform: slot.sub_waveform,
                    sub_level: slot.sub_level,
                    sub_octave: slot.sub_octave,
                    noise_enabled: slot.noise_enabled,
                    noise_level: slot.noise_level,
                    glide_ms: slot.glide_ms,
                    lfos: slot.lfos,
                    mod_matrix: slot.mod_matrix,
                    beats_per_second: (tempo_bpm / 60.0) as f32,
                }
            });

        let mut bass_event_index = 0usize;
        let bass_event_count = if override_on {
            0
        } else {
            params.bass_event_count.min(params.bass_events.len())
        };
        let mut seq_index = 0usize;
        let mut kick_event_index = 0usize;
        // Arrange override swallows DAW MIDI — kick events are gated out
        // just like the old block-level trigger flag was.
        let kick_event_count = if override_on {
            0
        } else {
            params.kick_event_count.min(params.kick_events.len())
        };
        let gate_velocity = params.gate_velocity.clamp(0.0, 1.0);
        let note_len_bars = shared_snapshot.arrange_note_len_bars as f64;
        let mut osc_kick = [0.0_f32; shared::OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_bass = [0.0_f32; shared::OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_sum = [0.0_f32; shared::OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_len = 0usize;

        for (sample_index, mut channel_samples) in buffer.iter_samples().enumerate() {
            while seq_index < seq_count && seq_events[seq_index].timing <= sample_index as u32 {
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
                    let slot = (event.slot as usize).min(shared::BASS_SLOT_COUNT - 1);
                    let bass = &shared_snapshot.bass[slot];
                    let semitone = 11 - event.row.min(11) as i32;
                    let note_hz = if bass.keytrack_enabled {
                        bass.pitch_hz * 2.0_f32.powf(semitone as f32 / 12.0)
                    } else {
                        bass.pitch_hz
                    };
                    self.bass_slot = slot;
                    self.bass_voice.note_on(
                        note_hz,
                        gate_velocity,
                        bass.phase_deg / 360.0,
                        bass.retrigger,
                        true,
                        &bass.lfos,
                        bass.glide_ms,
                        bass.glide_auto,
                    );
                    self.arrange_bass_off_abs = event.abs_bar + note_len_bars;
                    // The pattern owns the voice now; it isn't tied to a MIDI
                    // note number, so clear the tracker.
                    self.bass_active_note = None;
                } else {
                    self.bass_voice.note_off();
                    self.bass_active_note = None;
                }
            }

            while kick_event_index < kick_event_count {
                let Some(event) = params.kick_events[kick_event_index] else {
                    kick_event_index += 1;
                    continue;
                };
                if event.timing > sample_index as u32 {
                    break;
                }
                kick_event_index += 1;
                // Keytrack on: the played note sets the kick's pitch.
                if shared_snapshot.keytrack_enabled {
                    self.voice.trigger_with_note_velocity(
                        crate::midi::midi_note_to_hz(event.note),
                        event.velocity.clamp(0.0, 1.0),
                        kick_phase_offset,
                        shared_snapshot.kick_retrigger,
                        shared_snapshot.kick_legato_voice_steal,
                    );
                } else {
                    self.voice.trigger_with_velocity(
                        event.velocity.clamp(0.0, 1.0),
                        kick_phase_offset,
                        shared_snapshot.kick_retrigger,
                        shared_snapshot.kick_legato_voice_steal,
                    );
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
                    // DAW MIDI always plays Note 1 (slot 0); Note 2 is only
                    // reachable via arrange notes in override mode.
                    let bass = &shared_snapshot.bass[0];
                    let note_hz = if bass.keytrack_enabled {
                        // Keytrack: base pitch is C; each semitone above shifts
                        // up. note % 12 gives the pitch class (C=0 .. B=11),
                        // covering both control ranges (24-35 and 48-59).
                        let semitone = (event.note % 12) as f32;
                        bass.pitch_hz * 2.0_f32.powf(semitone / 12.0)
                    } else {
                        // Keytrack off: always the instrument's base pitch.
                        bass.pitch_hz
                    };
                    self.bass_slot = 0;
                    self.bass_voice.note_on(
                        note_hz,
                        event.velocity.clamp(0.0, 1.0),
                        bass.phase_deg / 360.0,
                        bass.retrigger,
                        bass.legato_voice_steal,
                        &bass.lfos,
                        bass.glide_ms,
                        bass.glide_auto,
                    );
                    self.bass_active_note = Some(event.note);
                } else {
                    // Only release if this off matches the sounding note. With
                    // overlapping keys the host sends offs for notes that
                    // already lost the voice; honouring those would chop the
                    // current note mid-flight — the classic legato gap.
                    if self
                        .bass_active_note
                        .map_or(true, |active| active == event.note)
                    {
                        self.bass_voice.note_off();
                        self.bass_active_note = None;
                    }
                }

                bass_event_index += 1;
            }

            // Oversampling: voices run at factor x the host rate; each output
            // sample averages that many sub-samples. The moving average puts
            // a decimation null exactly at the host Nyquist, where fold-back
            // from the nonlinear stages (drive, output ceiling) would
            // otherwise land.
            let mut kick_sample = 0.0;
            let mut bass_sample = 0.0;
            for _ in 0..self.oversample_factor {
                kick_sample += self.voice.next_sample(
                    voice_params,
                    &shared_snapshot.amp_lut,
                    &shared_snapshot.pitch_lut,
                );
                bass_sample += self.bass_voice.next_sample(
                    bass_voice_params[self.bass_slot],
                    &shared_snapshot.bass[self.bass_slot].amp_lut,
                    &shared_snapshot.bass[self.bass_slot].filter_lut,
                    &shared_snapshot.bass[self.bass_slot].filter_2_lut,
                );
            }
            kick_sample /= self.oversample_factor as f32;
            bass_sample /= self.oversample_factor as f32;
            // 0 dB ceiling only: the summed signal passes through raw and a
            // hard ±1.0 clamp is the sole protection against over-full-scale
            // output — no knee, no compression, so everything below the
            // ceiling reaches the host untouched.
            let output_sample = (kick_sample + bass_sample).clamp(-1.0, 1.0);

            if sample_index < shared::OSCILLOSCOPE_BUFFER_SIZE {
                osc_kick[sample_index] = kick_sample;
                osc_bass[sample_index] = bass_sample;
                osc_sum[sample_index] = output_sample;
                osc_len = sample_index + 1;
            }

            for output in channel_samples.iter_mut() {
                *output = output_sample;
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

        // One telemetry frame per block is plenty for UI animation.
        let live = self.bass_voice.mod_snapshot();
        shared::publish_bass_mod_telemetry(
            shared_handle,
            shared::BassModTelemetry {
                active: self.bass_voice.is_active(),
                slot: self.bass_slot,
                env_phase: live.env_phase,
                env_amp: live.env_amp,
                env_filter_1: live.env_filter_1,
                env_filter_2: live.env_filter_2,
                lfo_values: live.lfo_values,
                lfo_phases: live.lfo_phases,
                velocity: live.velocity,
                cutoff_hz: live.cutoff_hz,
                sequence: 0,
            },
        );

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
    /// Which bass note slot plays it (0 = Note 1, 1 = Note 2; bass only).
    pub slot: u8,
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
    filter_2_lut: &[f32; shared::CURVE_LUT_SIZE],
    phase_offset: f32,
    retrigger: bool,
    legato_voice_steal: bool,
) -> Vec<f32> {
    let total_samples = (total_seconds * preview_rate).ceil().max(1.0) as usize;
    let mut buffer = vec![0.0_f32; total_samples];
    let mut voice = BassVoice::default();
    voice.set_sample_rate(preview_rate);
    voice.note_on(
        note_hz,
        1.0,
        phase_offset,
        retrigger,
        legato_voice_steal,
        &params.lfos,
        params.glide_ms,
        false,
    );
    for slot in buffer.iter_mut() {
        if !voice.is_active() {
            break;
        }
        *slot = voice.next_sample(params, amp_lut, filter_lut, filter_2_lut);
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
    bass_levels: [f32; 2],
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
        amp_env_enabled: shared.amp_env_enabled,
        pitch_env_enabled: shared.pitch_env_enabled,
    };
    let bass_params: [BassVoiceParams; shared::BASS_SLOT_COUNT] = std::array::from_fn(|index| {
        let slot = &shared.bass[index];
        BassVoiceParams {
            level: bass_levels[index],
            tuning_scale,
            note_length_ms: slot.note_length_ms,
            base_cutoff_hz: slot.cutoff_hz,
            filter_mode: slot.filter_mode,
            filter_slope: slot.filter_slope,
            filter_drive: slot.filter_drive,
            resonance: slot.resonance,
            filter_enabled: slot.filter_enabled,
            filter_2_enabled: slot.filter_2_enabled,
            waveform: slot.oscillator_waveform,
            osc_a_level: slot.osc_a_level,
            osc_a_unison: slot.osc_a_unison,
            osc_a_unison_detune: slot.osc_a_unison_detune,
            osc_a_unison_blend: slot.osc_a_unison_blend,
            osc_b_enabled: slot.osc_b_enabled,
            osc_b_waveform: slot.osc_b_waveform,
            osc_b_level: slot.osc_b_level,
            osc_b_octave: slot.osc_b_octave,
            osc_b_detune_cents: slot.osc_b_detune_cents,
            osc_b_unison: slot.osc_b_unison,
            osc_b_unison_detune: slot.osc_b_unison_detune,
            osc_b_unison_blend: slot.osc_b_unison_blend,
            sub_enabled: slot.sub_enabled,
            sub_waveform: slot.sub_waveform,
            sub_level: slot.sub_level,
            sub_octave: slot.sub_octave,
            noise_enabled: slot.noise_enabled,
            noise_level: slot.noise_level,
            glide_ms: slot.glide_ms,
            lfos: slot.lfos,
            mod_matrix: slot.mod_matrix,
            beats_per_second: (shared.arrange_manual_tempo / 60.0).max(0.0),
        }
    });

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
            let slot_index = (note.slot as usize).min(shared::BASS_SLOT_COUNT - 1);
            let bass = &shared.bass[slot_index];
            let note_hz = if bass.keytrack_enabled {
                bass.pitch_hz * 2.0_f32.powf(note.semitone as f32 / 12.0)
            } else {
                bass.pitch_hz
            };
            let mut voice = BassVoice::default();
            voice.set_sample_rate(preview_rate);
            voice.note_on(
                note_hz,
                1.0,
                bass.phase_deg / 360.0,
                bass.retrigger,
                bass.legato_voice_steal,
                &bass.lfos,
                bass.glide_ms,
                bass.glide_auto,
            );
            for buf_slot in bass_buffer.iter_mut().skip(start) {
                if !voice.is_active() {
                    break;
                }
                *buf_slot += voice.next_sample(
                    bass_params[slot_index],
                    &bass.amp_lut,
                    &bass.filter_lut,
                    &bass.filter_2_lut,
                );
            }
        }
    }

    (kick_buffer, bass_buffer)
}

/// Engine-level tests: drive `KickEngine::process` through a real nih-plug
/// `Buffer` with `KickDspParams` events, exactly as `Plugin::process` does —
/// event dispatch, voices, filter, oversampling, and limiter end to end.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::RoutedMidiEvent;
    use nih_plug::prelude::Buffer;

    const SAMPLE_RATE: f32 = 44_100.0;
    const BLOCK: usize = 512;

    /// Neutral per-block parameters: nothing held, no events.
    fn dsp_params() -> KickDspParams {
        KickDspParams {
            kick_level: 0.8,
            bass_levels: [0.8, 0.8],
            trigger_active: false,
            kick_events: [None; 4],
            kick_event_count: 0,
            bass_events: [None; 6],
            bass_event_count: 0,
            gate_note_ons: 0,
            gate_note_offs: 0,
            gate_first_on_timing: None,
            gate_velocity: 1.0,
        }
    }

    fn note_on(timing: u32, note: u8) -> RoutedMidiEvent {
        RoutedMidiEvent {
            timing,
            voice_id: None,
            channel: 0,
            note,
            velocity: 1.0,
            is_note_on: true,
        }
    }

    fn note_off(timing: u32, note: u8) -> RoutedMidiEvent {
        RoutedMidiEvent {
            is_note_on: false,
            ..note_on(timing, note)
        }
    }

    /// Renders one stereo block and returns the left channel.
    fn run_block(
        engine: &mut KickEngine,
        shared: &shared::SharedStateHandle,
        params: KickDspParams,
    ) -> Vec<f32> {
        let mut left = vec![0.0f32; BLOCK];
        let mut right = vec![0.0f32; BLOCK];
        let mut buffer = Buffer::default();
        // Safe: both slices outlive `buffer` and have equal length.
        unsafe {
            buffer.set_slices(BLOCK, |slices| {
                *slices = vec![left.as_mut_slice(), right.as_mut_slice()];
            });
        }
        engine.process(&mut buffer, params, shared);
        drop(buffer);
        left
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
    }

    /// A bass slot that holds level until released: flat amp envelope, long
    /// note, fully open filter. Sustained enough to observe gate behavior.
    fn sustaining_slot() -> shared::BassSlotParams {
        let mut slot = shared::BassSlotParams::default();
        for v in slot.amp_lut.iter_mut() {
            *v = 0.8;
        }
        slot.note_length_ms = 1_000.0;
        slot.cutoff_hz = 4_000.0;
        slot.filter_enabled = false;
        slot.filter_2_enabled = false;
        slot
    }

    /// Sends `events` in a single block, then renders `extra_blocks` more
    /// with no events, returning every rendered sample in order.
    fn render_with_events(
        engine: &mut KickEngine,
        shared: &shared::SharedStateHandle,
        events: &[RoutedMidiEvent],
        extra_blocks: usize,
    ) -> Vec<f32> {
        let mut params = dsp_params();
        for (index, event) in events.iter().enumerate() {
            params.bass_events[index] = Some(*event);
        }
        params.bass_event_count = events.len();

        let mut rendered = run_block(engine, shared, params);
        params.bass_event_count = 0;
        for _ in 0..extra_blocks {
            rendered.extend(run_block(engine, shared, params));
        }
        rendered
    }

    /// The full DSP path end to end: a triggered kick and a bass note-on
    /// must produce finite, limiter-bounded, non-silent audio.
    #[test]
    fn engine_renders_bounded_audio() {
        let shared = shared::new_shared_state();
        let mut engine = KickEngine::default();
        engine.set_sample_rate(SAMPLE_RATE);

        let mut params = dsp_params();
        params.trigger_active = true;
        let mut rendered = run_block(&mut engine, &shared, params);
        rendered.extend(render_with_events(
            &mut engine,
            &shared,
            &[note_on(0, 48)],
            3,
        ));

        assert!(
            rendered.iter().all(|s| s.is_finite()),
            "engine produced non-finite samples"
        );
        let peak = rendered.iter().fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(peak <= 1.001, "output escaped the limiter: peak={peak}");
        assert!(rms(&rendered) > 0.05, "engine rendered silence");
    }

    /// With no triggers and no notes the engine must output exact silence —
    /// free-running LFOs must not bleed into the mix.
    #[test]
    fn idle_engine_is_silent() {
        let shared = shared::new_shared_state();
        let mut engine = KickEngine::default();
        engine.set_sample_rate(SAMPLE_RATE);

        for _ in 0..3 {
            let block = run_block(&mut engine, &shared, dsp_params());
            assert!(
                block.iter().all(|s| s.abs() < 1e-7),
                "idle engine leaked signal"
            );
        }
    }

    /// A note-off for a note that does not own the voice must not release
    /// the sounding note — the legato-gap fix. Only the matching off
    /// releases.
    #[test]
    fn overlapping_note_off_does_not_chop_active_note() {
        let shared = shared::new_shared_state();
        shared::set_bass_slot(&shared, 0, sustaining_slot());
        let mut engine = KickEngine::default();
        engine.set_sample_rate(SAMPLE_RATE);

        // Note 48 down, then note 50 down (takes the voice), then the off
        // for note 48 — which no longer owns the voice and is ignored.
        let rendered = render_with_events(
            &mut engine,
            &shared,
            &[note_on(0, 48), note_on(64, 50), note_off(128, 48)],
            1,
        );
        assert!(
            rms(&rendered[BLOCK..]) > 0.05,
            "unrelated note-off chopped the active note"
        );

        // The matching off releases for real (≈8 ms ramp).
        let released = render_with_events(&mut engine, &shared, &[note_off(0, 50)], 1);
        assert!(
            rms(&released[BLOCK..]) < 0.005,
            "matching note-off did not release"
        );
    }

    /// With `length_sync` on, the bass note self-gates to one 16th of the
    /// live tempo: at 100 BPM that is 150 ms, so the note sounds through
    /// the gate and is silent just past it plus the release ramp.
    #[test]
    fn bass_sync_gates_to_sixteenth_of_tempo() {
        const BPM: f64 = 100.0;
        let shared = shared::new_shared_state();
        shared::set_tempo(&shared, Some(BPM));
        let mut slot = sustaining_slot();
        slot.length_sync = true;
        shared::set_bass_slot(&shared, 0, slot);

        let mut engine = KickEngine::default();
        engine.set_sample_rate(SAMPLE_RATE);

        let rendered = render_with_events(&mut engine, &shared, &[note_on(0, 48)], 15);

        let gate_samples = (15_000.0 / BPM as f32 / 1_000.0 * SAMPLE_RATE) as usize;
        let during = rms(&rendered[..gate_samples * 8 / 10]);
        assert!(during > 0.05, "synced note was silent inside the gate");
        let tail_start = gate_samples + (0.020 * SAMPLE_RATE) as usize;
        let after = rms(&rendered[tail_start..]);
        assert!(after < 0.005, "synced note overran the 16th gate");
    }

    /// Toggling 2x oversampling must keep the rendered level in the same
    /// ballpark — it changes harmonic cleanliness, not loudness.
    #[test]
    fn oversampling_toggle_preserves_level() {
        let mut levels = [0.0f32; 2];
        for (i, enabled) in [true, false].iter().enumerate() {
            let shared = shared::new_shared_state();
            shared::set_oversampling(&shared, *enabled);
            shared::set_bass_slot(&shared, 0, sustaining_slot());
            let mut engine = KickEngine::default();
            engine.set_sample_rate(SAMPLE_RATE);

            let rendered = render_with_events(&mut engine, &shared, &[note_on(0, 48)], 4);
            levels[i] = rms(&rendered);
        }

        assert!(
            levels.iter().all(|l| *l > 0.05),
            "render silent: {levels:?}"
        );
        let ratio = levels[0] / levels[1];
        assert!(
            (0.7..=1.4).contains(&ratio),
            "oversampling changed level too much: on={} off={}",
            levels[0],
            levels[1]
        );
    }

    /// A host sample-rate change mid-run must not produce garbage: the
    /// engine re-rates the voices and keeps output bounded and finite.
    #[test]
    fn sample_rate_change_stays_bounded() {
        let shared = shared::new_shared_state();
        shared::set_bass_slot(&shared, 0, sustaining_slot());
        let mut engine = KickEngine::default();
        engine.set_sample_rate(SAMPLE_RATE);

        let mut rendered = render_with_events(&mut engine, &shared, &[note_on(0, 48)], 2);
        engine.set_sample_rate(48_000.0);
        rendered.extend(run_block(&mut engine, &shared, dsp_params()));
        engine.set_sample_rate(SAMPLE_RATE);
        rendered.extend(run_block(&mut engine, &shared, dsp_params()));

        assert!(
            rendered.iter().all(|s| s.is_finite() && s.abs() <= 1.001),
            "sample-rate change broke the output"
        );
    }
}
