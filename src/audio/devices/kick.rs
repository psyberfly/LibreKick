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
    interface::{Waveform, CURVE_LUT_SIZE},
    midi::MidiFrameInput,
};

#[derive(Clone)]
pub struct KickSnapshot {
    pub amp_lut: [f32; CURVE_LUT_SIZE],
    pub pitch_lut: [f32; CURVE_LUT_SIZE],
    pub keytrack_enabled: bool,
    pub note_length_ms: f32,
    pub waveform: Waveform,
    pub retrigger: bool,
    pub legato_voice_steal: bool,
    pub trigger_counter: u64,
}

#[derive(Clone)]
pub enum KickCommand {
    SetAmpLut([f32; CURVE_LUT_SIZE]),
    SetPitchLut([f32; CURVE_LUT_SIZE]),
    SetKeytrackEnabled(bool),
    SetNoteLengthMs(f32),
    SetWaveform(Waveform),
    SetRetrigger(bool),
    SetLegatoVoiceSteal(bool),
    RequestTrigger,
}

#[derive(Clone, Copy)]
pub struct KickParams {
    pub level: f32,
    pub keytrack_enabled: bool,
    pub tuning_scale: f32,
    pub note_length_ms: f32,
    pub waveform: Waveform,
}

impl DeviceParams for KickParams {}

#[derive(Clone, Copy)]
pub enum KickEvent {
    Trigger {
        note_hz: f32,
        velocity: f32,
    },

    NoteOff,
}

impl DeviceEvent for KickEvent {}

pub struct KickDevice {
    params: KickParams,

    voice: Voice,
    oscillator: Oscillator,

    amp_lut: [f32; CURVE_LUT_SIZE],
    pitch_lut: [f32; CURVE_LUT_SIZE],

    retrigger: bool,
    legato_voice_steal: bool,

    hit_note_hz: Option<f32>,
    last_trigger_param: bool,
    last_shared_trigger_counter: u64,
}

impl Default for KickDevice {
    fn default() -> Self {
        Self {
            params: KickParams {
                level: 1.0,
                keytrack_enabled: false,
                tuning_scale: 1.0,
                note_length_ms: 250.0,
                waveform: Waveform::Sine,
            },

            voice: Voice::default(),
            oscillator: Oscillator::default(),

            amp_lut: [1.0; CURVE_LUT_SIZE],
            pitch_lut: [0.5; CURVE_LUT_SIZE],

            retrigger: true,
            legato_voice_steal: true,

            hit_note_hz: None,
            last_trigger_param: false,
            last_shared_trigger_counter: 0,
        }
    }
}

impl KickDevice {
    pub fn set_amp_lut(
        &mut self,
        lut: [f32; CURVE_LUT_SIZE],
    ) {
        self.amp_lut = lut;
    }

    pub fn set_pitch_lut(
        &mut self,
        lut: [f32; CURVE_LUT_SIZE],
    ) {
        self.pitch_lut = lut;
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.voice.set_sample_rate(sample_rate);
        self.oscillator.set_sample_rate(sample_rate);
    }

    pub fn process_input_frame(
        &mut self,
        trigger_active: bool,
        midi_input: &MidiFrameInput,
        shared_trigger_counter: u64,
    ) {
        if trigger_active && !self.last_trigger_param {
            self.process_event(KickEvent::Trigger {
                note_hz: 55.0,
                velocity: 1.0,
            });
        }
        self.last_trigger_param = trigger_active;

        if midi_input.trigger {
            if let Some(note_hz) = midi_input.note_hz {
                self.process_event(KickEvent::Trigger {
                    note_hz,
                    velocity: midi_input.velocity.clamp(0.0, 1.0),
                });
            } else {
                self.trigger_with_velocity(midi_input.velocity.clamp(0.0, 1.0));
            }
        }

        if shared_trigger_counter != self.last_shared_trigger_counter {
            self.last_shared_trigger_counter = shared_trigger_counter;
            self.trigger_with_velocity(1.0);
        }
    }

    pub fn trigger_with_note_velocity(&mut self, note_hz: f32, velocity: f32) {
        self.trigger(note_hz, velocity, self.retrigger, self.legato_voice_steal);
    }

    pub fn trigger_with_velocity(&mut self, velocity: f32) {
        let note_hz = self.hit_note_hz.unwrap_or(55.0);
        self.trigger(note_hz, velocity, self.retrigger, self.legato_voice_steal);
    }

    pub fn note_off_public(&mut self) {
        self.note_off();
    }

    fn trigger(
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

        self.hit_note_hz =
            Some(note_hz.max(20.0));
    }

    fn note_off(&mut self) {
        self.voice.note_off();
        self.oscillator.note_off();
    }
}

impl Device for KickDevice {
    type Params = KickParams;
    type Event = KickEvent;

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
            KickEvent::Trigger {
                note_hz,
                velocity,
            } => {
                self.trigger_with_note_velocity(note_hz, velocity);
            }

            KickEvent::NoteOff => {
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
                .clamp(0.0, 1.0);

        if note_length_seconds <= 0.0 {
            self.note_off();
            return 0.0;
        }

        let normalized_time =
            (self.voice.time_seconds()
                / note_length_seconds)
                .clamp(0.0, 1.0);

        let lut_index =
            ((normalized_time
                * (CURVE_LUT_SIZE as f32 - 1.0))
                .round() as usize)
                .min(CURVE_LUT_SIZE - 1);

        let amp_curve =
            self.amp_lut[lut_index]
                .clamp(0.0, 1.0);

        let pitch_curve =
            self.pitch_lut[lut_index]
                .clamp(0.0, 1.0);

        let curve_hz =
            pitch_curve_to_hz(pitch_curve);

        let base_hz =
            if self.params.keytrack_enabled {
                self.hit_note_hz
                    .unwrap_or(curve_hz)
            } else {
                curve_hz
            };

        let amplitude =
            self.params.level
                .clamp(0.0, 1.0)
                * self.voice.velocity()
                * amp_curve;

        let frequency =
            (base_hz
                * self.params.tuning_scale
                    .max(0.5))
                .max(20.0);

        let sample =
            self.oscillator.render_sample(
                self.params.waveform,
                frequency,
                amplitude,
            );

        self.voice.advance();

        if self.voice.time_seconds()
            >= note_length_seconds
            || normalized_time >= 1.0
            || amplitude < 0.0005
        {
            self.note_off();
        }

        sample
    }
}

impl ControlSnapshot for KickSnapshot {}
impl ControlCommand for KickCommand {}

impl ControlledDevice for KickDevice {
    type Snapshot = KickSnapshot;
    type Command = KickCommand;

    fn apply_snapshot(&mut self, snapshot: &Self::Snapshot, level: f32) {
        self.set_amp_lut(snapshot.amp_lut);
        self.set_pitch_lut(snapshot.pitch_lut);

        self.retrigger = snapshot.retrigger;
        self.legato_voice_steal = snapshot.legato_voice_steal;

        self.update_params(KickParams {
            level,
            keytrack_enabled: snapshot.keytrack_enabled,
            tuning_scale: 1.0,
            note_length_ms: snapshot.note_length_ms,
            waveform: snapshot.waveform,
        });
    }

    fn apply_command(&mut self, command: &Self::Command) {
        match command {
            KickCommand::SetAmpLut(lut) => self.set_amp_lut(*lut),
            KickCommand::SetPitchLut(lut) => self.set_pitch_lut(*lut),
            KickCommand::SetKeytrackEnabled(enabled) => {
                self.params.keytrack_enabled = *enabled;
            }
            KickCommand::SetNoteLengthMs(ms) => {
                self.params.note_length_ms = *ms;
            }
            KickCommand::SetWaveform(waveform) => {
                self.params.waveform = *waveform;
            }
            KickCommand::SetRetrigger(value) => {
                self.retrigger = *value;
            }
            KickCommand::SetLegatoVoiceSteal(value) => {
                self.legato_voice_steal = *value;
            }
            KickCommand::RequestTrigger => {
                self.trigger_with_velocity(1.0);
            }
        }
    }
}

fn pitch_curve_to_hz(
    value: f32,
) -> f32 {
    let min_hz = 20.0_f32;
    let max_hz = 20_000.0_f32;

    min_hz
        * (max_hz / min_hz)
            .powf(value.clamp(0.0, 1.0))
}