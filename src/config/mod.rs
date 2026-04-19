use std::{fs, sync::OnceLock};

#[derive(Clone, Copy)]
pub struct AppConfig {
    pub base_editor_width: f32,
    pub base_editor_height: f32,
    pub min_editor_width: f32,
    pub min_editor_height: f32,
    pub note_length_max_ms: f32,
    pub default_tuning_a4_hz: f32,
    pub waveform_zoom_min_percent: f32,
    pub waveform_zoom_max_percent: f32,
    pub waveform_zoom_step_percent: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_editor_width: 980.0,
            base_editor_height: 560.0,
            min_editor_width: 520.0,
            min_editor_height: 320.0,
            note_length_max_ms: 1000.0,
            default_tuning_a4_hz: 432.0,
            waveform_zoom_min_percent: 1.0,
            waveform_zoom_max_percent: 200.0,
            waveform_zoom_step_percent: 5.0,
        }
    }
}

fn parse_app_config() -> AppConfig {
    let mut config = AppConfig::default();
    let raw_config = fs::read_to_string(format!(
        "{}/src/config/librekick.env",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap_or_else(|_| include_str!("librekick.sample.env").to_owned());

    for raw_line in raw_config.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');

        match key {
            "BASE_EDITOR_WIDTH" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    config.base_editor_width = parsed;
                }
            }
            "BASE_EDITOR_HEIGHT" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    config.base_editor_height = parsed;
                }
            }
            "MIN_EDITOR_WIDTH" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    config.min_editor_width = parsed;
                }
            }
            "MIN_EDITOR_HEIGHT" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    config.min_editor_height = parsed;
                }
            }
            "NOTE_LENGTH_MAX_MS" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    config.note_length_max_ms = parsed;
                }
            }
            "DEFAULT_TUNING_A4_HZ" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    config.default_tuning_a4_hz = parsed;
                }
            }
            "WAVEFORM_ZOOM_MIN_PERCENT" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    config.waveform_zoom_min_percent = parsed;
                }
            }
            "WAVEFORM_ZOOM_MAX_PERCENT" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    config.waveform_zoom_max_percent = parsed;
                }
            }
            "WAVEFORM_ZOOM_STEP_PERCENT" => {
                if let Ok(parsed) = value.parse::<f32>() {
                    config.waveform_zoom_step_percent = parsed;
                }
            }
            _ => {}
        }
    }

    if config.waveform_zoom_max_percent < config.waveform_zoom_min_percent {
        std::mem::swap(
            &mut config.waveform_zoom_min_percent,
            &mut config.waveform_zoom_max_percent,
        );
    }

    config
}

pub fn app_config() -> &'static AppConfig {
    static CONFIG: OnceLock<AppConfig> = OnceLock::new();
    CONFIG.get_or_init(parse_app_config)
}

pub fn ui_config() -> &'static AppConfig {
    app_config()
}
