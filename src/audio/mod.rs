mod core;
pub mod devices;

use nih_plug::prelude::*;

use crate::{
    interface::{AudioEnginePort, OscilloscopeSignal, OSCILLOSCOPE_BUFFER_SIZE},
    midi::MidiFrameInput,
};

use self::{
    core::device::{ControlledDevice, Device},
    devices::{
        bass::BassDevice,
        kick::KickDevice,
    },
};

pub const DRUM_MACHINE_BOUNCE_SAMPLE_RATE: u32 = 48_000;

pub struct AudioEngine {
    kick: KickDevice,
    bass: BassDevice,
}

pub fn bounce_current_patch(engine_port: &dyn AudioEnginePort, duration_seconds: f32) -> Vec<f32> {
    let mut engine = AudioEngine::default();
    engine.set_sample_rate(DRUM_MACHINE_BOUNCE_SAMPLE_RATE as f32);
    let kick_snapshot = engine_port.kick_snapshot();
    let bass_snapshot = engine_port.bass_snapshot();
    engine.sync_devices_from_snapshot(&kick_snapshot, &bass_snapshot, 1.0);
    engine.kick.trigger_with_velocity(1.0);
    engine.bass.note_on_with_velocity(bass_snapshot.bass_pitch_hz, 1.0);

    let total = (duration_seconds.clamp(0.25, 10.0) * DRUM_MACHINE_BOUNCE_SAMPLE_RATE as f32) as usize;
    let note_off_at = ((bass_snapshot.bass_note_length_ms * 0.001) * DRUM_MACHINE_BOUNCE_SAMPLE_RATE as f32) as usize;
    let mut samples = Vec::with_capacity(total);
    for index in 0..total {
        if index == note_off_at {
            engine.bass.note_off_public();
        }
        let sample = (engine.kick.next_sample() + engine.bass.next_sample()).clamp(-1.0, 1.0);
        samples.push(sample);
    }
    samples
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self {
            kick: KickDevice::default(),
            bass: BassDevice::default(),
        }
    }
}

impl AudioEngine {
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.kick.set_sample_rate(sample_rate);
        self.bass.set_sample_rate(sample_rate);
    }

    fn sync_devices_from_snapshot(
        &mut self,
        kick_snapshot: &crate::interface::KickSnapshot,
        bass_snapshot: &crate::interface::BassSnapshot,
        level: f32,
    ) {
        self.kick.apply_snapshot(kick_snapshot, level);
        self.bass.apply_snapshot(bass_snapshot, level);
    }

    pub fn process(
        &mut self,
        buffer: &mut Buffer,
        level: f32,
        trigger_active: bool,
        midi_input: MidiFrameInput,
        engine_port: &impl AudioEnginePort,
    ) -> ProcessStatus {
        let kick_snapshot = engine_port.kick_snapshot();
        let bass_snapshot = engine_port.bass_snapshot();
        self.sync_devices_from_snapshot(&kick_snapshot, &bass_snapshot, level);
        self.kick
            .process_input_frame(trigger_active, &midi_input, kick_snapshot.trigger_counter);
        self.bass.begin_input_frame();

        let mut osc_kick = [0.0_f32; OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_bass = [0.0_f32; OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_sum = [0.0_f32; OSCILLOSCOPE_BUFFER_SIZE];
        let mut osc_len = 0usize;

        for (sample_index, mut channel_samples) in buffer.iter_samples().enumerate() {
            self.bass.process_input_for_sample(sample_index, &midi_input);

            let kick_sample = self.kick.next_sample();
            let bass_sample = self.bass.next_sample();
            let limited_sample = (kick_sample + bass_sample).clamp(-1.0, 1.0);

            if sample_index < OSCILLOSCOPE_BUFFER_SIZE {
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
            engine_port.publish_oscilloscope_signal_block(
                OscilloscopeSignal::Kick,
                &osc_kick[..osc_len],
            );
            engine_port.publish_oscilloscope_signal_block(
                OscilloscopeSignal::Bass,
                &osc_bass[..osc_len],
            );
            engine_port.publish_oscilloscope_signal_block(OscilloscopeSignal::Sum, &osc_sum[..osc_len]);
            engine_port.commit_oscilloscope_frame();
        }

        ProcessStatus::Normal
    }
}
