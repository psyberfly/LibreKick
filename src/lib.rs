use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;

mod audio;
mod shared;
mod ui;

struct KickPlugin {
    params: Arc<KickPluginParams>,
    engine: audio::KickEngine,
    shared: shared::SharedStateHandle,
}

#[derive(Params)]
struct KickPluginParams {
    #[id = "trigger"]
    trigger: BoolParam,

    #[id = "decay_ms"]
    decay_ms: FloatParam,

    #[id = "base_freq_hz"]
    base_freq_hz: FloatParam,

    #[id = "pitch_drop_hz"]
    pitch_drop_hz: FloatParam,

    #[id = "level"]
    level: FloatParam,

    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
}

impl Default for KickPluginParams {
    fn default() -> Self {
        Self {
            trigger: BoolParam::new("Trigger", false),
            decay_ms: FloatParam::new(
                "Decay (ms)",
                220.0,
                FloatRange::Linear {
                    min: 20.0,
                    max: 1000.0,
                },
            ),
            base_freq_hz: FloatParam::new(
                "Base Freq",
                52.0,
                FloatRange::Linear {
                    min: 30.0,
                    max: 120.0,
                },
            ),
            pitch_drop_hz: FloatParam::new(
                "Pitch Drop",
                170.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 400.0,
                },
            ),
            level: FloatParam::new(
                "Level",
                0.8,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            editor_state: EguiState::from_size(760, 420),
        }
    }
}

impl Default for KickPlugin {
    fn default() -> Self {
        let shared = shared::new_shared_state();

        Self {
            params: Arc::new(KickPluginParams::default()),
            engine: audio::KickEngine::default(),
            shared,
        }
    }
}

impl Plugin for KickPlugin {
    const NAME: &'static str = "Kick Plugin";
    const VENDOR: &'static str = "Anorak";
    const URL: &'static str = "https://example.com";
    const EMAIL: &'static str = "anorak@example.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: Some(new_nonzero_u32(2)),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        ui::create_testing_editor(self.params.editor_state.clone(), self.shared.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.engine.set_sample_rate(buffer_config.sample_rate);
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut midi_trigger = false;
        let mut midi_velocity = 1.0;
        while let Some(event) = context.next_event() {
            if let NoteEvent::NoteOn { velocity, .. } = event {
                midi_trigger = true;
                midi_velocity = velocity;
            }
        }

        let dsp_params = audio::KickDspParams {
            decay_ms: self.params.decay_ms.value(),
            base_freq_hz: self.params.base_freq_hz.value(),
            pitch_drop_hz: self.params.pitch_drop_hz.value(),
            level: self.params.level.value(),
            trigger_active: self.params.trigger.value(),
            midi_trigger,
            midi_velocity,
        };

        self.engine.process(buffer, dsp_params, &self.shared)
    }
}

impl Vst3Plugin for KickPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"KickPlgTestLin01";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_vst3!(KickPlugin);
