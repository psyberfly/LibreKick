use nih_plug::prelude::{Plugin, ProcessContext};

use crate::midi::{self, MidiFrameInput};

pub trait EngineInputAdapter<P: Plugin> {
    fn collect_frame_input(
        &mut self,
        context: &mut impl ProcessContext<P>,
    ) -> MidiFrameInput;
}

#[derive(Default)]
pub struct MidiInputAdapter;

impl<P: Plugin> EngineInputAdapter<P> for MidiInputAdapter {
    fn collect_frame_input(
        &mut self,
        context: &mut impl ProcessContext<P>,
    ) -> MidiFrameInput {
        midi::collect_midi_input(context)
    }
}
