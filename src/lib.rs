use std::sync::Arc;

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;

mod audio;
mod ui;

struct KickPlugin {
    params: Arc<KickPluginParams>,
}

#[derive(Params)]
struct KickPluginParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
}

impl Default for KickPluginParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(760, 420),
        }
    }
}

impl Default for KickPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(KickPluginParams::default()),
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

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        ui::create_testing_editor(self.params.editor_state.clone())
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        audio::process_silence(buffer)
    }
}

impl Vst3Plugin for KickPlugin {
    const VST3_CLASS_ID: [u8; 16] = *b"KickPlgTestLin01";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx];
}

nih_export_vst3!(KickPlugin);
