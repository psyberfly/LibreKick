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
Download the Linux CLAP3 build from: https://github.com/psyberfly/LibreKick/releases/download/v0.1.0/LibreKick_linux_x86_64.clap

Or build it yourself for a different target and format:

1. `cd /scripts`:
2. `cp sample.env config.env`. Update your `config.env` file to your local needs. Refer `sample.env` for info about building for other operating systems and formats.
3. `./build.sh <target> <format>`
   - Standalone Linux executable: `./build.sh linux desktop`
   - Linux CLAP plugin: `./build.sh linux clap3`
4. `./install.sh <target> <format>` installs the VST into your configured system VST location; ready to use via your DAW.

## MIDI NOTE ROUTING (SINGLE INSTRUMENT CHANNEL)

LibreKick listens on one instrument MIDI channel and splits incoming notes by pitch range:

- Kick zone: MIDI notes `12..23` (C0..B0)
- Bass zone: MIDI notes `24..35` (C1..B1)
- Bass alternate zone: MIDI notes `48..59` (C3..B3)

Notes in the kick zone trigger the kick voice. Notes in the bass zones trigger the bass voice.

Important behavior:

- Both voices are driven from the same MIDI input channel; no separate channel setup is required.
- Bass note length is constrained per note by MIDI note duration and the Bass page `Note Length` control.
- Bass `Retrigger` is oscillator phase retrigger only.
- Bass `Legato (voice steal)` controls overlapping-note handling for the bass voice.

## KEY BINDINGS

- `Ctrl/Cmd + Z`: Undo
- `Ctrl/Cmd + Shift + Z` or `Ctrl/Cmd + Y`: Redo
- `Delete` / `Backspace` / `Ctrl/Cmd + X`: Remove selected point(s)
- `Shift` (in graph): Enter Shift-lock mode for precise point control
  - Move mouse (no button): adjust locked point on X axis
  - Hold left mouse: adjust locked point on Y axis (vertical-only)
- `Ctrl/Cmd + Mouse Wheel` (over graph): Adjust waveform zoom

# KNOWN ISSUES
1. If your window size starts off small you can enlarge the window using the re-sizing handle on the window bottom-right. See: https://github.com/psyberfly/LibreKick/issues/2 

2. FIXME: When CTRL/CMD is held on an edge, the edge bend handle comes vissible at the displacement (stragight line or 0% bend region) near the curve; the handle should up when cursor is on the bent curve because curve shape has effectively changed. the 0% indicator can still be shown. 

# DEV

## Testing
1. Use `./restart.sh` to hot-reload (compile changes and re-launch the VST using Carla; requires Carla locally installed on your machine).

## CONTRIBUTING 
**NOTICE**: In order to stay free and compliant with its GPL licence, this software requires all of its contributors to write original source code and not use AI generated code; AI can be used personally, for research. 

## TODO
1. Make VoiceParams a single type: remove KickVoiceParams and BassVoiceParams and use VoiceParams instead. Make settings common across voice params.

## Architecture

1. `src/lib.rs` (plugin entry)
- Defines plugin metadata, I/O layout, and MIDI capability (`MidiConfig::Basic`).
- Owns shared state handle and `KickEngine` instance.
- Collects/routes incoming MIDI into kick/bass control zones and forwards routed events to DSP.

2. `src/ui/mod.rs` (editor + curve design)
- Egui-based multi-page editor (`Kick`, `Bass`, `Settings`) inside `ResizableWindow`.
- Kick page edits kick amp/pitch curves; Bass page edits bass amp/filter curves and oscillator/filter controls.
- Settings page contains global tuning reference (`A=440` / `A=432`) for note-name display.
- Converts envelope curves to LUTs and publishes all current UI parameters to shared state.

3. `src/shared/mod.rs` (UI ↔ DSP contract)
- Thread-safe shared snapshot containing:
  - kick amp/pitch LUTs
  - bass amp/filter LUTs
  - bass oscillator/filter params (`pitch_hz`, `cutoff_hz`, waveform, filter mode)
  - bass note behavior flags (`retrigger`, `legato voice steal`)
  - trigger counter
- UI writes updates; audio thread reads atomic snapshots.

4. `src/audio/mod.rs` (engine wrapper)
- Owns kick and bass voice state.
- Combines plugin params + shared snapshot + routed MIDI events.
- Applies bass note events sample-accurately (using event timing) for note-on/off handling.
- Applies final output limiter (`clamp(-1.0, 1.0)`).

5. `src/audio/voice.rs` (synthesis voices)
- Kick voice: envelope-driven one-shot synth with velocity and optional key tracking.
- Bass voice: oscillator + selectable filter (`LowPass`, `HighPass`, `BandPass`) with amp/filter envelopes.
- Bass voice supports monophonic note handling with `Retrigger` (phase) and `Legato (voice steal)` behavior.

6. `scripts/` (dev/build workflow)
- Target-aware script dispatch (`TARGET` in `config.env`).
- `build.sh`, `start.sh`, `restart.sh`, `install.sh` wrappers.
- Stale artifact checks ensure bundle/install/start use latest build outputs.

## Audio

Audio processing flow per block:

1. Event intake
- Host MIDI events are read in `process()`.
- MIDI notes are split into kick and bass zones on one instrument channel.
- Bass note-on/off events are buffered with timing offsets for sample-accurate application.

2. Kick trigger resolution
- A kick hit can be triggered from:
  - MIDI note-on
  - Trigger parameter edge
  - UI trigger button (shared trigger counter)

3. Shared snapshot read
- Audio thread reads latest shared snapshot once per block:
  - kick/bass LUTs
  - bass oscillator/filter parameters
  - bass note behavior flags

4. Voice synthesis per sample
- Kick and bass samples are generated every frame.
- Kick uses envelope LUTs and velocity-sensitive one-shot synthesis.
- Bass uses absolute `Hz` pitch control, selected waveform, selected filter mode, and amp/filter envelope LUTs.
- Bass note-off events stop the bass voice, confining notes to MIDI duration plus configured bass note-length cap.
- Global tuning selection is used for note-name reference in UI only (not DSP pitch scaling).

5. Output stage
- Mono sample is copied to all output channels.
- Final sample is hard-limited to `[-1.0, 1.0]`.
