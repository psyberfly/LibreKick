Open Source Kick Generator Plugin VST!

# USAGE
To run the plugin on Linux, go to `/scripts`:
1. ./compile_linux.sh 
2. ./bundle_linux_vst3.sh
3. ./start_linux./sh (requires Carla to be installed on your machine) or, use install.sh to install the VST into your system VST location and use it via your DAW

# PLAN

Minimal plan to build a Kick-style plugin in Rust, focusing only on core functionality.

1. Framework setup

Use nih-plug

Create cdylib crate
Implement plugin struct (e.g., KickSynth)
Enable VST3 (and optionally CLAP)
2. Core data model

Define:

phase: f32
sample_index: usize
amp_table: Vec<f32>
freq_table: Vec<f32>
sample_rate: f32

Optional:

is_active: bool
3. Curve system (offline → tables)

Start simple:

Represent curve as a few control points
Convert to fixed-size arrays (e.g., 2048 samples)

For each table index:

Map index → normalized time (0–1)
Evaluate cubic Bezier
Fill:
amp_table[i]
freq_table[i]

Do this outside audio thread

4. Audio processing loop

Per sample:

freq = freq_table[sample_index]
amp = amp_table[sample_index]
phase += 2π * freq / sample_rate
out = amp * sin(phase)
sample_index += 1

If end reached:

output 0 or stop
5. Triggering

On note-on (or manual trigger):

phase = 0
sample_index = 0
is_active = true
6. Parameters (minimal)

Expose:

start frequency
end frequency
decay time
gain

Skip UI Bezier editor initially.

7. Thread safety
Rebuild tables when parameters change
Swap using Arc or double buffer
No allocation in audio callback
8. Basic output validation
Confirm:
clean sine drop (kick sound)
no clicks (except intentional transient)
9. First improvements

Add in order:

Click/transient (short noise burst)
Distortion (simple waveshaper)
Oversampling (2x or 4x)
Better curve resolution
10. UI (last step)
Add Bezier editor (drag points)
Convert UI → tables
Sync safely to DSP
Summary

You are building:

Bezier → lookup tables
Table-driven oscillator
Trigger/reset system

That is the functional core of a Kick 2–style plugin.