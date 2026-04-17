<p align="center">
  <img width="564" height="388" alt="LibreKick Logo 2_compressed" src="https://github.com/user-attachments/assets/1bdde8c0-dc79-4671-beb0-936cd3c82313" />
</p>

# Open Source Waveform Shaper Kick Synth

<img width="282" height="194" alt="image" src="https://github.com/user-attachments/assets/ecae543b-23e5-4d81-aaea-b0ac58061c17" />

# USAGE
Currently, only Linux VST 2.0 build is configured.

1. `cd /scripts`:
2. `cp sample.env config.env` and update your `config.env` file to your local sys
3. `./build.sh`
4. `./install.sh` installs the VST into your configured system VST location; ready to use via your DAW.

## Testing
1. Use `./restart.sh` to hot-reload (compile changes and re-launch the VST using Carla; requires Carla locally installed on your machine).

# TODOs
1. Improve rendered waveform: In pitch envp, frequency change is clamped to halfway point (632Hz).

# ARCHITECTURE

Current app architecture is split into clear modules with a simple UI-to-audio data contract:

1. `src/lib.rs` (plugin entry)
- Defines plugin metadata, I/O layout, and MIDI capability (`MidiConfig::Basic`).
- Owns shared state handle and `KickEngine` instance.
- Parses host `NoteOn` events (note + velocity) and forwards trigger data to DSP.

2. `src/ui/mod.rs` (editor + curve design)
- Egui-based curve editor inside `ResizableWindow`.
- Two curves: amplitude envelope and pitch envelope.
- Converts curve points to LUTs and publishes them to shared state.
- Sends manual trigger events and tuning selection (`A=440` / `A=432`).
- Renders waveform preview behind envelope overlays.

3. `src/shared/mod.rs` (UI ↔ DSP contract)
- Thread-safe shared snapshot containing:
  - amplitude LUT
  - pitch LUT
  - trigger counter
  - tuning reference (`tuning_a4_hz`)
- UI writes updates; audio thread reads atomic snapshots.

4. `src/audio/mod.rs` (engine wrapper)
- Owns voice state and trigger edge tracking.
- Combines parameter values + shared snapshot + MIDI trigger input.
- Passes resolved per-block parameters into voice render loop.
- Applies final output limiter (`clamp(-1.0, 1.0)`).

5. `src/audio/voice.rs` (one-shot synth voice)
- One active kick voice with phase/time state.
- Supports velocity-sensitive triggering.
- Supports MIDI note-based base pitch per hit.
- Applies pitch envelope, amplitude envelope, tuning scale, and decay.

6. `scripts/` (dev/build workflow)
- Target-aware script dispatch (`TARGET` in `config.env`).
- `build.sh`, `start.sh`, `restart.sh`, `install.sh` wrappers.
- Stale artifact checks ensure bundle/install/start use latest build outputs.

# AUDIO

Audio processing flow per block:

1. Event intake
- Host MIDI events are read in `process()`.
- `NoteOn` sets trigger, velocity, and note frequency (Hz).

2. Trigger resolution
- A hit can be triggered from:
  - MIDI note-on
  - Trigger parameter edge
  - UI trigger button (shared trigger counter)

3. Shared snapshot read
- Audio thread reads latest shared snapshot once per block:
  - amp LUT
  - pitch LUT
  - tuning reference frequency

4. Voice synthesis per sample
- Envelope position comes from elapsed hit time and decay.
- Amplitude comes from level × velocity × amp LUT.
- Frequency comes from base frequency (MIDI note if present, else parameter) + pitch drop × pitch LUT.
- Global tuning scale (`tuning_a4_hz / 440`) is applied.
- Sample is generated as sine oscillator output.

5. Output stage
- Mono sample is copied to all output channels.
- Final sample is hard-limited to `[-1.0, 1.0]`.
