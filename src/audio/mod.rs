use nih_plug::prelude::*;

pub fn process_silence(buffer: &mut Buffer) -> ProcessStatus {
    for mut channel_samples in buffer.iter_samples() {
        for sample in channel_samples.iter_mut() {
            *sample = 0.0;
        }
    }

    ProcessStatus::Normal
}
