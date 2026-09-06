use std::f32::consts::TAU;

use crate::{
    audio::core::{
        device::{
            ControlCommand,
            ControlSnapshot,
            ControlledDevice,
            Device,
            DeviceEvent,
            DeviceParams,
        },
        oscillator::Oscillator,
        voice::Voice,
    },
    interface::{BassFilterMode, Waveform, CURVE_LUT_SIZE},
    midi::MidiFrameInput,
};

#[derive(Clone)]
pub struct BassSnapshot {
    pub amp_lut: [f32; CURVE_LUT_SIZE],
    pub filter_lut: [f32; CURVE_LUT_SIZE],
    pub bass_note_length_ms: f32,
    pub bass_cutoff_hz: f32,
    pub bass_pitch_hz: f32,
    pub retrigger: bool,
    pub legato_voice_steal: bool,
    pub bass_filter_mode: BassFilterMode,
    pub waveform: Waveform,
}

#[derive(Clone)]
pub enum BassCommand {
    SetAmpLut([f32; CURVE_LUT_SIZE]),
    SetFilterLut([f32; CURVE_LUT_SIZE]),
    SetNoteLengthMs(f32),
    SetCutoffHz(f32),
    SetFilterMode(BassFilterMode),
    SetPitchHz(f32),
    SetRetrigger(bool),
    SetLegatoVoiceSteal(bool),
    SetWaveform(Waveform),
}

#[derive(Clone, Copy)]
pub struct BassParams {
    pub level: f32,
    pub tuning_scale: f32,
    pub note_length_ms: f32,
    pub base_cutoff_hz: f32,
    pub pitch_hz: f32,
    pub filter_mode: BassFilterMode,
    pub waveform: Waveform,
}

impl DeviceParams for BassParams {}

#[derive(Clone, Copy)]
pub enum BassEvent {
    NoteOn {
        note_hz: f32,
        velocity: f32,
    },

    NoteOff,
}

impl DeviceEvent for BassEvent {}

pub struct BassDevice {
    params: BassParams,

    voice: Voice,
    oscillator: Oscillator,

    bass_amp_lut: [f32; CURVE_LUT_SIZE],
    bass_filter_lut: [f32; CURVE_LUT_SIZE],

    retrigger: bool,
    legato_voice_steal: bool,

    hp_prev_in: f32,
    hp_prev_out: f32,
    lp_prev_out: f32,

    bass_event_index: usize,
}

impl Default for BassDevice {
    fn default() -> Self {
        Self {
            params: BassParams {
                level: 1.0,
                tuning_scale: 1.0,
                note_length_ms: 250.0,
                base_cutoff_hz: 120.0,
                pitch_hz: 55.0,
                filter_mode: BassFilterMode::LowPass,
                waveform: Waveform::Saw,
            },

            voice: Voice::default(),
            oscillator: Oscillator::default(),

            bass_amp_lut: [1.0; CURVE_LUT_SIZE],
            bass_filter_lut: [1.0; CURVE_LUT_SIZE],

            retrigger: true,
            legato_voice_steal: false,

            hp_prev_in: 0.0,
            hp_prev_out: 0.0,
            lp_prev_out: 0.0,

            bass_event_index: 0,
        }
    }
}

impl BassDevice {
    pub fn set_amp_lut(
        &mut self,
        lut: [f32; CURVE_LUT_SIZE],
    ) {
        self.bass_amp_lut = lut;
    }

    pub fn set_filter_lut(
        &mut self,
        lut: [f32; CURVE_LUT_SIZE],
    ) {
        self.bass_filter_lut = lut;
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.voice.set_sample_rate(sample_rate);
        self.oscillator.set_sample_rate(sample_rate);
    }

    pub fn begin_input_frame(&mut self) {
        self.bass_event_index = 0;
    }

    pub fn process_input_for_sample(
        &mut self,
        sample_index: usize,
        midi_input: &MidiFrameInput,
    ) {
        let bass_event_count = midi_input.bass_event_count.min(midi_input.bass_events.len());

        while self.bass_event_index < bass_event_count {
            let Some(event) = midi_input.bass_events[self.bass_event_index] else {
                self.bass_event_index += 1;
                continue;
            };

            if event.timing > sample_index as u32 {
                break;
            }

            if event.is_note_on {
                let note_hz = 440.0 * 2.0_f32.powf((event.note as f32 - 69.0) / 12.0);
                self.process_event(BassEvent::NoteOn {
                    note_hz,
                    velocity: event.velocity.clamp(0.0, 1.0),
                });
            } else {
                self.process_event(BassEvent::NoteOff);
            }

            self.bass_event_index += 1;
        }
    }

    pub fn note_on_with_velocity(&mut self, note_hz: f32, velocity: f32) {
        self.note_on(
            note_hz,
            velocity,
            self.retrigger,
            self.legato_voice_steal,
        );
    }

    pub fn note_off_public(&mut self) {
        self.note_off();
    }

    fn note_on(
        &mut self,
        note_hz: f32,
        velocity: f32,
        retrigger: bool,
        legato_voice_steal: bool,
    ) {
        if !self.oscillator.note_on(
            retrigger,
            legato_voice_steal,
        ) {
            return;
        }

        self.voice.note_on(
            note_hz,
            velocity,
        );
    }

    fn note_off(&mut self) {
        self.voice.note_off();
        self.oscillator.note_off();

        self.hp_prev_in = 0.0;
        self.hp_prev_out = 0.0;
        self.lp_prev_out = 0.0;
    }
}

impl Device for BassDevice {
    type Params = BassParams;
    type Event = BassEvent;

    fn update_params(
        &mut self,
        params: Self::Params,
    ) {
        self.params = params;
    }

    fn process_event(
        &mut self,
        event: Self::Event,
    ) {
        match event {
            BassEvent::NoteOn {
                note_hz,
                velocity,
            } => {
                self.note_on_with_velocity(note_hz, velocity);
            }

            BassEvent::NoteOff => {
                self.note_off();
            }
        }
    }

    fn next_sample(&mut self) -> f32 {
        if !self.voice.is_active() {
            return 0.0;
        }

        let note_length_seconds =
            (self.params.note_length_ms * 0.001)
                .clamp(0.001, 2.0);

        let normalized_time =
            (self.voice.time_seconds()
                / note_length_seconds)
                .clamp(0.0, 1.0);

        let lut_index =
            ((normalized_time
                * (CURVE_LUT_SIZE as f32 - 1.0))
                .round() as usize)
                .min(CURVE_LUT_SIZE - 1);

        let amp_env =
            self.bass_amp_lut[lut_index]
                .clamp(0.0, 1.0);

        let cutoff_env =
            self.bass_filter_lut[lut_index]
                .clamp(0.0, 1.0);

        let amplitude =
            self.params.level
                .clamp(0.0, 1.0)
                * self.voice.velocity()
                * amp_env;

        let frequency =
            (self.params.pitch_hz
                * self.params.tuning_scale.max(0.5))
                .clamp(20.0, 20_000.0);

        let raw =
            self.oscillator.render_sample(
                self.params.waveform,
                frequency,
                amplitude,
            );

        let base_cutoff =
            self.params.base_cutoff_hz
                .clamp(20.0, 8_000.0);

        let cutoff_hz =
            (base_cutoff
                * (0.25 + cutoff_env * 0.75))
                .clamp(20.0, 8_000.0);

        let dt =
            1.0 / self.voice.sample_rate();

        let rc =
            1.0 / (TAU * cutoff_hz.max(20.0));

        let hp_alpha =
            rc / (rc + dt);

        let lp_alpha =
            dt / (rc + dt);

        let hp =
            hp_alpha
                * (
                    self.hp_prev_out
                        + raw
                        - self.hp_prev_in
                );

        self.hp_prev_in = raw;
        self.hp_prev_out = hp;

        let filtered =
            match self.params.filter_mode {
                BassFilterMode::LowPass => {
                    self.lp_prev_out =
                        self.lp_prev_out
                            + lp_alpha
                                * (
                                    raw
                                        - self.lp_prev_out
                                );

                    self.lp_prev_out
                }

                BassFilterMode::HighPass => hp,

                BassFilterMode::BandPass => {
                    self.lp_prev_out =
                        self.lp_prev_out
                            + lp_alpha
                                * (
                                    hp
                                        - self.lp_prev_out
                                );

                    self.lp_prev_out
                }
            };

        self.voice.advance();

        if self.voice.time_seconds()
            >= note_length_seconds
            || normalized_time >= 1.0
            || amplitude < 0.0005
        {
            self.note_off();
        }

        filtered
    }
}

impl ControlSnapshot for BassSnapshot {}
impl ControlCommand for BassCommand {}

impl ControlledDevice for BassDevice {
    type Snapshot = BassSnapshot;
    type Command = BassCommand;

    fn apply_snapshot(&mut self, snapshot: &Self::Snapshot, level: f32) {
        self.set_amp_lut(snapshot.amp_lut);
        self.set_filter_lut(snapshot.filter_lut);

        self.retrigger = snapshot.retrigger;
        self.legato_voice_steal = snapshot.legato_voice_steal;

        self.update_params(BassParams {
            level,
            tuning_scale: 1.0,
            note_length_ms: snapshot.bass_note_length_ms,
            base_cutoff_hz: snapshot.bass_cutoff_hz,
            pitch_hz: snapshot.bass_pitch_hz,
            filter_mode: snapshot.bass_filter_mode,
            waveform: snapshot.waveform,
        });
    }

    fn apply_command(&mut self, command: &Self::Command) {
        match command {
            BassCommand::SetAmpLut(lut) => self.set_amp_lut(*lut),
            BassCommand::SetFilterLut(lut) => self.set_filter_lut(*lut),
            BassCommand::SetNoteLengthMs(ms) => {
                self.params.note_length_ms = *ms;
            }
            BassCommand::SetCutoffHz(hz) => {
                self.params.base_cutoff_hz = *hz;
            }
            BassCommand::SetFilterMode(mode) => {
                self.params.filter_mode = *mode;
            }
            BassCommand::SetPitchHz(hz) => {
                self.params.pitch_hz = *hz;
            }
            BassCommand::SetRetrigger(value) => {
                self.retrigger = *value;
            }
            BassCommand::SetLegatoVoiceSteal(value) => {
                self.legato_voice_steal = *value;
            }
            BassCommand::SetWaveform(waveform) => {
                self.params.waveform = *waveform;
            }
        }
    }
}