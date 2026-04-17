Open Source Kick Generator Plugin VST!

# USAGE
To run the plugin on Linux, go to `/scripts`:
1. ./compile_linux.sh 
2. ./bundle_linux_vst3.sh
3. ./start_linux./sh (requires Carla to be installed on your machine) or, use install.sh to install the VST into your system VST location and use it via your DAW


# TODO
1. Add note lengt: For UI: use a red triangle on x-axis of graph to indicate max length; apply slight black tint to area after max length for better visibility. 
2. Fix rendered waveform: reduce the frequency of the waveform to make transients more readable; keep everything else the same. 
3. Fix audio: sounds coming out of the plugin don't sound like kicks yet.
4. Add undo, redo buttons 
5. Add waveform zoom

# Fix for TODO #2:

The waveform display can be implemented as a time-domain rendering of a sine oscillator driven by pitch and amplitude envelopes. The x-axis represents time (e.g., 0–1000 ms), scaled by the zoom level, while the y-axis reflects amplitude. The instantaneous frequency from the pitch curve is integrated to compute phase:

phase(t)=∫2πf(t)dt
y(t)=A(t)⋅sin(phase(t))

The zoom parameter (0–100%) controls how much of the total duration is visible:

At 100% zoom, the full duration (e.g., 1000 ms) is displayed
At lower zoom levels, a shorter time window is shown, increasing visible waveform detail (fewer cycles on screen)

For example, at 1 Hz:

100% zoom (1000 ms) → 1 full cycle
50% zoom (500 ms visible) → 0.5 cycle

At higher frequencies, rendering every sample leads to dense, unreadable visuals. Downsampling is required for display purposes. The most reliable method is:

Min/max envelope decimation per pixel column: compute the minimum and maximum sample values within each pixel column and render that range

This approach:

Preserves peaks and transients
Avoids misleading alias-like artifacts
Produces a clear, accurate visual representation

Naive point skipping should be avoided, as it loses important waveform detail.


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