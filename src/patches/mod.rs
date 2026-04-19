use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::config;

pub const PATCH_FILE_EXTENSION: &str = ".librekick_patch";

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

fn patches_dir_path() -> PathBuf {
    PathBuf::from(config::patches_dir())
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
