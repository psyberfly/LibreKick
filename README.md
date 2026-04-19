<p align="center">
  <img src="https://github.com/user-attachments/assets/d2d13586-41f2-4105-b7a7-fb4a50fd1c8b" width="50%" />
</p>

# Open Source Waveform Shaper Kick Synthesiser

## About
LibreKick is an aduio synthesiser plugin for DAWs to generate Kick samples using visual waveshaping.  

Latest releases: https://github.com/psyberfly/LibreKick/releases  
Origin story: https://librekick.org  
**This project is in early alpha dev stage.**    
  
<p>&nbsp;</p>

<p align="center">
<img alt="image" src="https://github.com/user-attachments/assets/3e0bde10-123a-49e0-b9f5-6d788c2610ec" width="80%" />
</p>


# USAGE
1. `cd /scripts`:
2. `cp sample.env config.env` and update your `config.env` file to your local needs.
3. `./build.sh`
4. `./install.sh` installs the VST into your configured system VST location; ready to use via your DAW.

# DEV

## Testing
1. Use `./restart.sh` to hot-reload (compile changes and re-launch the VST using Carla; requires Carla locally installed on your machine).

## CONTRIBUTING 
**NOTICE**: In order to stay free and compliant with its GPL licence, this software requires all of its contributors to write original source code and not use AI generated code; AI can be used personally, for research. 

## Architecture

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

## Audio

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
