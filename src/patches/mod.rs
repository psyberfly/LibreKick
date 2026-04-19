use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::config;

pub const PATCH_FILE_EXTENSION: &str = ".librekick_patch";
pub const BUILTIN_DEFAULT_PATCH_NAME: &str = "default_psy_kick";
const DEFAULT_PATCH_NAME_FILE: &str = ".default_patch_name";

#[derive(Clone, Debug)]
pub struct PatchData {
    pub name: String,
    pub tuning_a4_hz: f32,
    pub note_end_ms: f32,
    pub max_note_length_ms: f32,
    pub waveform_zoom_percent: f32,
    pub active_curve: String,
    pub amplitude_points: Vec<(f32, f32)>,
    pub pitch_points: Vec<(f32, f32)>,
}

pub fn builtin_default_patch_data() -> PatchData {
    PatchData {
        name: BUILTIN_DEFAULT_PATCH_NAME.to_owned(),
        tuning_a4_hz: 432.0,
        note_end_ms: 206.93333,
        max_note_length_ms: 1000.0,
        waveform_zoom_percent: 200.0,
        active_curve: "pitch".to_owned(),
        amplitude_points: vec![
            (0.000000, 0.815547),
            (0.029639, 0.284431),
            (0.212629, 0.788254),
            (0.570876, 0.466934),
            (1.000000, 0.000000),
        ],
        pitch_points: vec![
            (0.000000, 0.974639),
            (0.216495, 0.112608),
            (0.617268, 0.210654),
            (0.881443, 0.311613),
            (0.990000, 0.829996),
            (1.000000, 0.123286),
        ],
    }
}

pub fn set_default_patch_name(name: &str) -> Result<(), String> {
    ensure_patches_dir()?;

    let cleaned_name = sanitize_patch_name(name);
    if cleaned_name.is_empty() {
        return Err("Patch name is empty after sanitization.".to_owned());
    }

    let patch_path = patch_file_path(&cleaned_name)?;
    if !patch_path.exists() {
        return Err(format!("Patch '{cleaned_name}' does not exist. Save it first."));
    }

    let default_path = default_patch_name_path();
    fs::write(&default_path, cleaned_name).map_err(|error| {
        format!(
            "Failed to save default patch setting '{}': {error}",
            default_path.display()
        )
    })
}

pub fn get_default_patch_name() -> Result<Option<String>, String> {
    ensure_patches_dir()?;

    let default_path = default_patch_name_path();
    if !default_path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&default_path).map_err(|error| {
        format!(
            "Failed to read default patch setting '{}': {error}",
            default_path.display()
        )
    })?;

    let cleaned = sanitize_patch_name(raw.trim());
    if cleaned.is_empty() {
        return Ok(None);
    }

    Ok(Some(cleaned))
}

pub fn ensure_default_patch_setup() -> Result<(), String> {
    ensure_patches_dir()?;

    let builtin_path = patch_file_path(BUILTIN_DEFAULT_PATCH_NAME)?;
    if !builtin_path.exists() {
        let builtin_patch = builtin_default_patch_data();
        save_patch(&builtin_patch)?;
    }

    if get_default_patch_name()?.is_none() {
        set_default_patch_name(BUILTIN_DEFAULT_PATCH_NAME)?;
    }

    Ok(())
}

fn patches_dir_path() -> PathBuf {
    PathBuf::from(config::patches_dir())
}

fn default_patch_name_path() -> PathBuf {
    patches_dir_path().join(DEFAULT_PATCH_NAME_FILE)
}

fn sanitize_patch_name(raw_name: &str) -> String {
    raw_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' '))
        .collect::<String>()
        .trim()
        .to_owned()
}

fn patch_file_path(name: &str) -> Result<PathBuf, String> {
    let cleaned_name = sanitize_patch_name(name);
    if cleaned_name.is_empty() {
        return Err("Patch name is empty after sanitization.".to_owned());
    }

    Ok(patches_dir_path().join(format!("{cleaned_name}{PATCH_FILE_EXTENSION}")))
}

fn points_to_string(points: &[(f32, f32)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{x:.6},{y:.6}"))
        .collect::<Vec<_>>()
        .join("|")
}

fn parse_points(raw: &str, label: &str) -> Result<Vec<(f32, f32)>, String> {
    let mut points = Vec::new();

    for part in raw.split('|').map(str::trim).filter(|segment| !segment.is_empty()) {
        let Some((x_raw, y_raw)) = part.split_once(',') else {
            return Err(format!("Invalid point pair in {label}: '{part}'"));
        };

        let x = x_raw
            .trim()
            .parse::<f32>()
            .map_err(|_| format!("Invalid X value in {label}: '{x_raw}'"))?;
        let y = y_raw
            .trim()
            .parse::<f32>()
            .map_err(|_| format!("Invalid Y value in {label}: '{y_raw}'"))?;

        points.push((x, y));
    }

    if points.len() < 2 {
        return Err(format!("{label} must contain at least two points."));
    }

    Ok(points)
}

fn ensure_patches_dir() -> Result<(), String> {
    let dir = patches_dir_path();
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to create patches directory '{}': {error}", dir.display()))
}

fn patch_name_from_path(path: &Path) -> Option<String> {
    let filename = path.file_name()?.to_string_lossy();
    filename
        .strip_suffix(PATCH_FILE_EXTENSION)
        .map(|name| name.to_owned())
}

pub fn list_patch_names() -> Result<Vec<String>, String> {
    ensure_patches_dir()?;

    let dir = patches_dir_path();
    let mut names = Vec::new();

    let entries =
        fs::read_dir(&dir).map_err(|error| format!("Failed to read '{}': {error}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to read patch entry: {error}"))?;
        let path = entry.path();
        if let Some(name) = patch_name_from_path(&path) {
            names.push(name);
        }
    }

    names.sort();
    Ok(names)
}

pub fn save_patch(patch: &PatchData) -> Result<(), String> {
    ensure_patches_dir()?;

    let path = patch_file_path(&patch.name)?;
    let serialized = [
        format!("name={}", patch.name),
        format!("tuning_a4_hz={}", patch.tuning_a4_hz),
        format!("note_end_ms={}", patch.note_end_ms),
        format!("max_note_length_ms={}", patch.max_note_length_ms),
        format!("waveform_zoom_percent={}", patch.waveform_zoom_percent),
        format!("active_curve={}", patch.active_curve),
        format!("amplitude_points={}", points_to_string(&patch.amplitude_points)),
        format!("pitch_points={}", points_to_string(&patch.pitch_points)),
    ]
    .join("\n");

    fs::write(&path, serialized)
        .map_err(|error| format!("Failed to save patch '{}': {error}", path.display()))
}

pub fn load_patch(name: &str) -> Result<PatchData, String> {
    let path = patch_file_path(name)?;
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to load patch '{}': {error}", path.display()))?;

    let mut patch_name: Option<String> = None;
    let mut tuning_a4_hz: Option<f32> = None;
    let mut note_end_ms: Option<f32> = None;
    let mut max_note_length_ms: Option<f32> = None;
    let mut waveform_zoom_percent: Option<f32> = None;
    let mut active_curve: Option<String> = None;
    let mut amplitude_points: Option<Vec<(f32, f32)>> = None;
    let mut pitch_points: Option<Vec<(f32, f32)>> = None;

    for raw_line in raw.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim();

        match key {
            "name" => patch_name = Some(value.to_owned()),
            "tuning_a4_hz" => {
                tuning_a4_hz =
                    Some(value.parse::<f32>().map_err(|_| "Invalid tuning_a4_hz".to_owned())?)
            }
            "note_end_ms" => {
                note_end_ms = Some(value.parse::<f32>().map_err(|_| "Invalid note_end_ms".to_owned())?)
            }
            "max_note_length_ms" => {
                max_note_length_ms = Some(
                    value
                        .parse::<f32>()
                        .map_err(|_| "Invalid max_note_length_ms".to_owned())?,
                )
            }
            "waveform_zoom_percent" => {
                waveform_zoom_percent = Some(
                    value
                        .parse::<f32>()
                        .map_err(|_| "Invalid waveform_zoom_percent".to_owned())?,
                )
            }
            "active_curve" => active_curve = Some(value.to_owned()),
            "amplitude_points" => amplitude_points = Some(parse_points(value, "amplitude_points")?),
            "pitch_points" => pitch_points = Some(parse_points(value, "pitch_points")?),
            _ => {}
        }
    }

    Ok(PatchData {
        name: patch_name.unwrap_or_else(|| sanitize_patch_name(name)),
        tuning_a4_hz: tuning_a4_hz.ok_or_else(|| "Missing tuning_a4_hz".to_owned())?,
        note_end_ms: note_end_ms.ok_or_else(|| "Missing note_end_ms".to_owned())?,
        max_note_length_ms: max_note_length_ms
            .ok_or_else(|| "Missing max_note_length_ms".to_owned())?,
        waveform_zoom_percent: waveform_zoom_percent
            .ok_or_else(|| "Missing waveform_zoom_percent".to_owned())?,
        active_curve: active_curve.unwrap_or_else(|| "amplitude".to_owned()),
        amplitude_points: amplitude_points.ok_or_else(|| "Missing amplitude_points".to_owned())?,
        pitch_points: pitch_points.ok_or_else(|| "Missing pitch_points".to_owned())?,
    })
}
