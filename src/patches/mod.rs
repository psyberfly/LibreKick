use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::config;

pub const PATCH_FILE_EXTENSION: &str = ".librekick_patch";
pub const BUILTIN_DEFAULT_PATCH_NAME: &str = "default_psy_kick";
const DEFAULT_PATCH_NAME_FILE: &str = ".default_patch_name";

include!(concat!(env!("OUT_DIR"), "/embedded_seed_patches.rs"));

#[derive(Clone, Debug)]
pub struct PatchData {
    pub name: String,
    pub tuning_a4_hz: f32,
    pub keytrack_enabled: bool,
    pub note_end_ms: f32,
    pub max_note_length_ms: f32,
    pub waveform_zoom_percent: f32,
    pub active_curve: String,
    pub amplitude_points: Vec<(f32, f32)>,
    pub amplitude_bends: Vec<f32>,
    pub pitch_points: Vec<(f32, f32)>,
    pub pitch_bends: Vec<f32>,
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

    let mut seeded_patch_names = Vec::new();
    for (filename, raw_patch) in EMBEDDED_SEED_PATCHES.iter().copied() {
        let fallback_name = filename
            .strip_suffix(PATCH_FILE_EXTENSION)
            .map(sanitize_patch_name)
            .unwrap_or_default();
        let patch = parse_patch(raw_patch, if fallback_name.is_empty() { None } else { Some(&fallback_name) })?;

        if !patch_file_path(&patch.name)?.exists() {
            save_patch(&patch)?;
        }

        seeded_patch_names.push(patch.name);
    }

    seeded_patch_names.sort();
    seeded_patch_names.dedup();

    let default_exists = get_default_patch_name()?
        .as_ref()
        .is_some_and(|name| patch_file_path(name).map(|path| path.exists()).unwrap_or(false));

    if !default_exists {
        let selected_default = if seeded_patch_names
            .iter()
            .any(|name| name == BUILTIN_DEFAULT_PATCH_NAME)
        {
            Some(BUILTIN_DEFAULT_PATCH_NAME.to_owned())
        } else if let Some(first_seeded) = seeded_patch_names.first() {
            Some(first_seeded.clone())
        } else {
            list_patch_names()?.into_iter().next()
        };

        if let Some(default_name) = selected_default {
            set_default_patch_name(&default_name)?;
        }
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

fn bends_to_string(bends: &[f32]) -> String {
    bends
        .iter()
        .map(|bend| format!("{:.6}", bend.clamp(-1.0, 1.0)))
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

fn parse_bends(raw: &str, label: &str) -> Result<Vec<f32>, String> {
    let mut bends = Vec::new();

    for part in raw.split('|').map(str::trim).filter(|segment| !segment.is_empty()) {
        let bend = part
            .parse::<f32>()
            .map_err(|_| format!("Invalid bend value in {label}: '{part}'"))?;
        bends.push(bend.clamp(-1.0, 1.0));
    }

    Ok(bends)
}

fn parse_patch(raw: &str, fallback_name: Option<&str>) -> Result<PatchData, String> {
    let mut patch_name: Option<String> = None;
    let mut tuning_a4_hz: Option<f32> = None;
    let mut keytrack_enabled: Option<bool> = None;
    let mut note_end_ms: Option<f32> = None;
    let mut max_note_length_ms: Option<f32> = None;
    let mut waveform_zoom_percent: Option<f32> = None;
    let mut active_curve: Option<String> = None;
    let mut amplitude_points: Option<Vec<(f32, f32)>> = None;
    let mut amplitude_bends: Option<Vec<f32>> = None;
    let mut pitch_points: Option<Vec<(f32, f32)>> = None;
    let mut pitch_bends: Option<Vec<f32>> = None;

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
            "keytrack_enabled" => {
                keytrack_enabled =
                    Some(value.parse::<bool>().map_err(|_| "Invalid keytrack_enabled".to_owned())?)
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
            "amplitude_bends" => amplitude_bends = Some(parse_bends(value, "amplitude_bends")?),
            "pitch_points" => pitch_points = Some(parse_points(value, "pitch_points")?),
            "pitch_bends" => pitch_bends = Some(parse_bends(value, "pitch_bends")?),
            _ => {}
        }
    }

    Ok(PatchData {
        name: patch_name
            .map(|name| sanitize_patch_name(&name))
            .filter(|name| !name.is_empty())
            .or_else(|| fallback_name.map(sanitize_patch_name).filter(|name| !name.is_empty()))
            .ok_or_else(|| "Missing patch name".to_owned())?,
        tuning_a4_hz: tuning_a4_hz.ok_or_else(|| "Missing tuning_a4_hz".to_owned())?,
        keytrack_enabled: keytrack_enabled.unwrap_or(false),
        note_end_ms: note_end_ms.ok_or_else(|| "Missing note_end_ms".to_owned())?,
        max_note_length_ms: max_note_length_ms
            .ok_or_else(|| "Missing max_note_length_ms".to_owned())?,
        waveform_zoom_percent: waveform_zoom_percent
            .ok_or_else(|| "Missing waveform_zoom_percent".to_owned())?,
        active_curve: active_curve.unwrap_or_else(|| "amplitude".to_owned()),
        amplitude_points: amplitude_points.ok_or_else(|| "Missing amplitude_points".to_owned())?,
        amplitude_bends: amplitude_bends.unwrap_or_default(),
        pitch_points: pitch_points.ok_or_else(|| "Missing pitch_points".to_owned())?,
        pitch_bends: pitch_bends.unwrap_or_default(),
    })
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
        format!("keytrack_enabled={}", patch.keytrack_enabled),
        format!("note_end_ms={}", patch.note_end_ms),
        format!("max_note_length_ms={}", patch.max_note_length_ms),
        format!("waveform_zoom_percent={}", patch.waveform_zoom_percent),
        format!("active_curve={}", patch.active_curve),
        format!("amplitude_points={}", points_to_string(&patch.amplitude_points)),
        format!("amplitude_bends={}", bends_to_string(&patch.amplitude_bends)),
        format!("pitch_points={}", points_to_string(&patch.pitch_points)),
        format!("pitch_bends={}", bends_to_string(&patch.pitch_bends)),
    ]
    .join("\n");

    fs::write(&path, serialized)
        .map_err(|error| format!("Failed to save patch '{}': {error}", path.display()))
}

pub fn load_patch(name: &str) -> Result<PatchData, String> {
    let path = patch_file_path(name)?;
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to load patch '{}': {error}", path.display()))?;

    parse_patch(&raw, Some(name))
}
