use nih_plug::prelude::{NoteEvent, Plugin, ProcessContext};

#[derive(Clone, Copy, Debug)]
pub struct MidiFrameInput {
    pub trigger: bool,
    pub velocity: f32,
    pub note_hz: Option<f32>,
}

impl Default for MidiFrameInput {
    fn default() -> Self {
        Self {
            trigger: false,
            velocity: 1.0,
            note_hz: None,
        }
    }
}

fn midi_note_to_hz(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

pub fn collect_midi_input<P: Plugin>(context: &mut impl ProcessContext<P>) -> MidiFrameInput {
    let mut midi_input = MidiFrameInput::default();

    while let Some(event) = context.next_event() {
        if let NoteEvent::NoteOn { note, velocity, .. } = event {
            if velocity > 0.0 {
                midi_input.trigger = true;
                midi_input.velocity = velocity.clamp(0.0, 1.0);
                midi_input.note_hz = Some(midi_note_to_hz(note));
            }
        }
    }

    midi_input
}
