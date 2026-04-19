use std::{fs, sync::OnceLock};

#[derive(Clone, Copy)]
pub struct UiConfig {
    pub base_editor_width: f32,
    pub base_editor_height: f32,
    pub min_editor_width: f32,
    pub min_editor_height: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            base_editor_width: 980.0,
            base_editor_height: 560.0,
            min_editor_width: 520.0,
            min_editor_height: 320.0,
        }
    }
}

fn parse_ui_config() -> UiConfig {
    let mut config = UiConfig::default();
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
            _ => {}
        }
    }

    config
}

pub fn ui_config() -> &'static UiConfig {
    static CONFIG: OnceLock<UiConfig> = OnceLock::new();
    CONFIG.get_or_init(parse_ui_config)
}
