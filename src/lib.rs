use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;

mod audio;
mod config;
mod midi;
mod patches;
mod shared;
mod ui;

struct LibreKick {
    params: Arc<LibreKickParams>,
    engine: audio::KickEngine,
    shared: shared::SharedStateHandle,
}

#[derive(Params)]
struct LibreKickParams {
    #[id = "trigger"]
    trigger: BoolParam,

    #[id = "level"]
    level: FloatParam,

    #[persist = "editor-state-v3"]
    editor_state: Arc<EguiState>,
}

impl Default for LibreKickParams {
    fn default() -> Self {
        let ui_cfg = config::ui_config();
        Self {
            trigger: BoolParam::new("Trigger", false),
            level: FloatParam::new(
                "Level",
                0.8,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            editor_state: EguiState::from_size(
                ui_cfg.base_editor_width as u32,
                ui_cfg.base_editor_height as u32,
            ),
        }
    }
}

impl Default for LibreKick {
    fn default() -> Self {
        let shared = shared::new_shared_state();

        Self {
            params: Arc::new(LibreKickParams::default()),
            engine: audio::KickEngine::default(),
            shared,
        }
    }
}

impl Plugin for LibreKick {
    const NAME: &'static str = "LibreKick";
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
        let midi_input = midi::collect_midi_input(context);

        let dsp_params = audio::KickDspParams {
            level: self.params.level.value(),
            trigger_active: self.params.trigger.value(),
            midi_trigger: midi_input.trigger,
            midi_velocity: midi_input.velocity,
            midi_note_hz: midi_input.note_hz,
        };

        self.engine.process(buffer, dsp_params, &self.shared)
    }
}

impl Vst3Plugin for LibreKick {
    const VST3_CLASS_ID: [u8; 16] = *b"LibreKickLin0001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

impl ClapPlugin for LibreKick {
    const CLAP_ID: &'static str = "com.psyberfly.librekick";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Kick drum synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Drum];
}

nih_export_clap!(LibreKick);
nih_export_vst3!(LibreKick);
