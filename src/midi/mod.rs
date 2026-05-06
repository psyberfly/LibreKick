use nih_plug::prelude::{NoteEvent, Plugin, ProcessContext};

const KICK_NOTE_MIN: u8 = 12;
const KICK_NOTE_MAX: u8 = 23;
const BASS_CONTROL_NOTE_MIN: u8 = 24;
const BASS_CONTROL_NOTE_MAX: u8 = 35;
const BASS_CONTROL_ALT_NOTE_MIN: u8 = 48;
const BASS_CONTROL_ALT_NOTE_MAX: u8 = 59;

#[derive(Clone, Copy, Debug)]
pub struct RoutedMidiEvent {
    pub timing: u32,
    pub voice_id: Option<i32>,
    pub channel: u8,
    pub note: u8,
    pub velocity: f32,
    pub is_note_on: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct MidiFrameInput {
    pub trigger: bool,
    pub velocity: f32,
    pub note_hz: Option<f32>,
    pub bass_trigger: bool,
    pub bass_note_off: bool,
    pub bass_velocity: f32,
    pub bass_note_hz: Option<f32>,
    pub bass_events: [Option<RoutedMidiEvent>; 6],
    pub bass_event_count: usize,
}

impl Default for MidiFrameInput {
    fn default() -> Self {
        Self {
            trigger: false,
            velocity: 1.0,
            note_hz: None,
            bass_trigger: false,
            bass_note_off: false,
            bass_velocity: 1.0,
            bass_note_hz: None,
            bass_events: [None; 6],
            bass_event_count: 0,
        }
    }
}

fn midi_note_to_hz(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

fn is_kick_control_note(note: u8) -> bool {
    (KICK_NOTE_MIN..=KICK_NOTE_MAX).contains(&note)
}

fn is_bass_control_note(note: u8) -> bool {
    (BASS_CONTROL_NOTE_MIN..=BASS_CONTROL_NOTE_MAX).contains(&note)
        || (BASS_CONTROL_ALT_NOTE_MIN..=BASS_CONTROL_ALT_NOTE_MAX).contains(&note)
}

fn mapped_bass_notes(note: u8) -> [u8; 3] {
    [note, note.saturating_add(12), note.saturating_sub(6)]
}

fn push_bass_event(midi_input: &mut MidiFrameInput, event: RoutedMidiEvent) {
    if midi_input.bass_event_count < midi_input.bass_events.len() {
        midi_input.bass_events[midi_input.bass_event_count] = Some(event);
        midi_input.bass_event_count += 1;
    }
}

pub fn collect_midi_input<P: Plugin>(context: &mut impl ProcessContext<P>) -> MidiFrameInput {
    let mut midi_input = MidiFrameInput::default();

    while let Some(event) = context.next_event() {
        match event {
            NoteEvent::NoteOn {
                timing,
                voice_id,
                channel,
                note,
                velocity,
            } => {
                if velocity > 0.0 {
                    if is_kick_control_note(note) {
                        midi_input.trigger = true;
                        midi_input.velocity = velocity.clamp(0.0, 1.0);
                        midi_input.note_hz = None;
                    } else if is_bass_control_note(note) {
                        let velocity = velocity.clamp(0.0, 1.0);
                        midi_input.bass_trigger = true;
                        midi_input.bass_velocity = velocity;
                        midi_input.bass_note_hz = Some(midi_note_to_hz(note));
                        for mapped in mapped_bass_notes(note) {
                            push_bass_event(
                                &mut midi_input,
                                RoutedMidiEvent {
                                    timing,
                                    voice_id,
                                    channel,
                                    note: mapped,
                                    velocity,
                                    is_note_on: true,
                                },
                            );
                        }
                    }
                } else if is_bass_control_note(note) {
                    midi_input.bass_note_off = true;
                    for mapped in mapped_bass_notes(note) {
                        push_bass_event(
                            &mut midi_input,
                            RoutedMidiEvent {
                                timing,
                                voice_id,
                                channel,
                                note: mapped,
                                velocity: 0.0,
                                is_note_on: false,
                            },
                        );
                    }
                }
            }
            NoteEvent::NoteOff {
                timing,
                voice_id,
                channel,
                note,
                ..
            } => {
                if is_bass_control_note(note) {
                    midi_input.bass_note_off = true;
                    for mapped in mapped_bass_notes(note) {
                        push_bass_event(
                            &mut midi_input,
                            RoutedMidiEvent {
                                timing,
                                voice_id,
                                channel,
                                note: mapped,
                                velocity: 0.0,
                                is_note_on: false,
                            },
                        );
                    }
                }
            }
            _ => {}
        }
    }

    midi_input
}
