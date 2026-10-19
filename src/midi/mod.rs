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
    /// Timed kick note-ons for sample-accurate triggering. `trigger` stays
    /// as the block-level flag; the engine dispatches these at their sample.
    pub kick_events: [Option<RoutedMidiEvent>; 4],
    pub kick_event_count: usize,
    pub bass_trigger: bool,
    pub bass_note_off: bool,
    pub bass_velocity: f32,
    pub bass_note_hz: Option<f32>,
    /// Fan-out events routed to the host (three per played note). These exist
    /// for downstream plugins and must not drive the internal voice.
    pub bass_events: [Option<RoutedMidiEvent>; 6],
    pub bass_event_count: usize,
    /// Events for the internal bass voice: exactly one per played note,
    /// carrying the original note number. Keeping these separate from the
    /// host fan-out stops a single key from retriggering the mono voice three
    /// times and from being re-pitched by the fan-out's transposed copies.
    pub bass_voice_events: [Option<RoutedMidiEvent>; 6],
    pub bass_voice_event_count: usize,
    /// Note-on count this block across ALL notes (arrange-override gate).
    pub gate_note_ons: u32,
    /// Note-off count this block across ALL notes (incl. velocity-0 note-ons).
    pub gate_note_offs: u32,
    /// Sample timing of the first note-on this block, if any.
    pub gate_first_on_timing: Option<u32>,
    /// Velocity of the first note-on this block.
    pub gate_velocity: f32,
}

impl Default for MidiFrameInput {
    fn default() -> Self {
        Self {
            trigger: false,
            velocity: 1.0,
            note_hz: None,
            kick_events: [None; 4],
            kick_event_count: 0,
            bass_trigger: false,
            bass_note_off: false,
            bass_velocity: 1.0,
            bass_note_hz: None,
            bass_events: [None; 6],
            bass_event_count: 0,
            bass_voice_events: [None; 6],
            bass_voice_event_count: 0,
            gate_note_ons: 0,
            gate_note_offs: 0,
            gate_first_on_timing: None,
            gate_velocity: 1.0,
        }
    }
}

pub(crate) fn midi_note_to_hz(note: u8) -> f32 {
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

fn push_bass_voice_event(midi_input: &mut MidiFrameInput, event: RoutedMidiEvent) {
    if midi_input.bass_voice_event_count < midi_input.bass_voice_events.len() {
        midi_input.bass_voice_events[midi_input.bass_voice_event_count] = Some(event);
        midi_input.bass_voice_event_count += 1;
    }
}

/// Routes one bass note event to both destinations: a single event for the
/// internal voice carrying the note as played, and the transposed fan-out
/// copies for the host. `velocity` is 0.0 and `is_note_on` false for releases.
fn route_bass_event(
    midi_input: &mut MidiFrameInput,
    timing: u32,
    voice_id: Option<i32>,
    channel: u8,
    note: u8,
    velocity: f32,
    is_note_on: bool,
) {
    push_bass_voice_event(
        midi_input,
        RoutedMidiEvent {
            timing,
            voice_id,
            channel,
            note,
            velocity,
            is_note_on,
        },
    );
    for mapped in mapped_bass_notes(note) {
        push_bass_event(
            midi_input,
            RoutedMidiEvent {
                timing,
                voice_id,
                channel,
                note: mapped,
                velocity,
                is_note_on,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice_events(input: &MidiFrameInput) -> Vec<RoutedMidiEvent> {
        input.bass_voice_events[..input.bass_voice_event_count]
            .iter()
            .filter_map(|event| *event)
            .collect()
    }

    fn host_events(input: &MidiFrameInput) -> Vec<RoutedMidiEvent> {
        input.bass_events[..input.bass_event_count]
            .iter()
            .filter_map(|event| *event)
            .collect()
    }

    /// One played note must reach the mono voice exactly once. Three events on
    /// the same sample would restart the amp envelope three times (stutter).
    #[test]
    fn one_played_note_triggers_the_voice_once() {
        let mut input = MidiFrameInput::default();
        route_bass_event(&mut input, 0, None, 0, 24, 1.0, true);

        let events = voice_events(&input);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].note, 24);
        assert!(events[0].is_note_on);
    }

    /// The voice must see the note as played. The engine derives pitch from
    /// `note % 12`, so a transposed copy would re-pitch the voice.
    #[test]
    fn voice_event_keeps_the_original_pitch_class() {
        for note in [24_u8, 27, 31, 35, 48, 59] {
            let mut input = MidiFrameInput::default();
            route_bass_event(&mut input, 0, None, 0, note, 1.0, true);

            let events = voice_events(&input);
            assert_eq!(events.len(), 1, "note {note} should yield one voice event");
            assert_eq!(
                events[0].note % 12,
                note % 12,
                "note {note} must keep its pitch class"
            );
        }
    }

    /// Guards the reason the two streams are separate: the host fan-out
    /// deliberately contains a transposed copy whose pitch class differs
    /// (note - 6 is a tritone away), which must never drive the voice.
    #[test]
    fn host_fanout_contains_a_differing_pitch_class() {
        let mut input = MidiFrameInput::default();
        route_bass_event(&mut input, 0, None, 0, 24, 1.0, true);

        let notes: Vec<u8> = host_events(&input).iter().map(|event| event.note).collect();
        assert_eq!(notes, vec![24, 36, 18]);
        assert!(
            notes.iter().any(|note| note % 12 != 24 % 12),
            "fan-out is expected to transpose, hence the separate voice stream"
        );
    }

    #[test]
    fn release_routes_a_single_note_off_to_the_voice() {
        let mut input = MidiFrameInput::default();
        route_bass_event(&mut input, 0, None, 0, 24, 0.0, false);

        let events = voice_events(&input);
        assert_eq!(events.len(), 1);
        assert!(!events[0].is_note_on);
        assert_eq!(events[0].note, 24);
    }

    /// Both streams are bounded; a chord must not overflow either array.
    #[test]
    fn routing_respects_array_capacity() {
        let mut input = MidiFrameInput::default();
        for note in 24..36 {
            route_bass_event(&mut input, 0, None, 0, note, 1.0, true);
        }

        assert!(input.bass_voice_event_count <= input.bass_voice_events.len());
        assert!(input.bass_event_count <= input.bass_events.len());
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
                    midi_input.gate_note_ons += 1;
                    if midi_input.gate_first_on_timing.is_none() {
                        midi_input.gate_first_on_timing = Some(timing);
                        midi_input.gate_velocity = velocity.clamp(0.0, 1.0);
                    }
                    if midi_input.kick_event_count < midi_input.kick_events.len() {
                        midi_input.kick_events[midi_input.kick_event_count] =
                            Some(RoutedMidiEvent {
                                timing,
                                voice_id,
                                channel,
                                note,
                                velocity: velocity.clamp(0.0, 1.0),
                                is_note_on: true,
                            });
                        midi_input.kick_event_count += 1;
                    }
                    if is_kick_control_note(note) {
                        midi_input.trigger = true;
                        midi_input.velocity = velocity.clamp(0.0, 1.0);
                        midi_input.note_hz = None;
                    } else if is_bass_control_note(note) {
                        let velocity = velocity.clamp(0.0, 1.0);
                        midi_input.bass_trigger = true;
                        midi_input.bass_velocity = velocity;
                        midi_input.bass_note_hz = Some(midi_note_to_hz(note));
                        route_bass_event(
                            &mut midi_input,
                            timing,
                            voice_id,
                            channel,
                            note,
                            velocity,
                            true,
                        );
                    }
                } else {
                    // A note-on with zero velocity is a release.
                    midi_input.gate_note_offs += 1;
                    if is_bass_control_note(note) {
                        midi_input.bass_note_off = true;
                        route_bass_event(
                            &mut midi_input,
                            timing,
                            voice_id,
                            channel,
                            note,
                            0.0,
                            false,
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
                midi_input.gate_note_offs += 1;
                if is_bass_control_note(note) {
                    midi_input.bass_note_off = true;
                    route_bass_event(&mut midi_input, timing, voice_id, channel, note, 0.0, false);
                }
            }
            _ => {}
        }
    }

    midi_input
}
