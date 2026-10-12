use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::config;
use crate::shared::{
    LfoParams, LfoShape, ModDest, ModRouting, ModSource, BASS_LFO_COUNT,
    BASS_MOD_SLOT_COUNT,
};

pub const PATCH_FILE_EXTENSION: &str = ".librekick_patch";
pub const BUILTIN_DEFAULT_PATCH_NAME: &str = "default_psy_kick";
const DEFAULT_PATCH_NAME_FILE: &str = ".default_patch_name";

include!(concat!(env!("OUT_DIR"), "/embedded_seed_patches.rs"));

/// Maximum length of a patch description, in characters.
pub const PATCH_DESCRIPTION_MAX_CHARS: usize = 300;

/// Strips line breaks and caps the description at `PATCH_DESCRIPTION_MAX_CHARS`.
pub(crate) fn sanitize_patch_description(raw: &str) -> String {
    raw.replace(['\n', '\r'], " ")
        .chars()
        .take(PATCH_DESCRIPTION_MAX_CHARS)
        .collect()
}

#[derive(Clone, Debug)]
pub struct PatchData {
    pub name: String,
    /// Free-form patch description shown in the menu bar. Empty when unset.
    pub description: String,
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
    /// Bass instrument settings (Note 1). `None` for patches saved before bass support.
    pub bass: Option<BassPatchData>,
    /// Second bass note slot (Note 2). `None` for patches saved before
    /// multi-slot support; the UI then duplicates Note 1.
    pub bass2: Option<BassPatchData>,
    /// Kick oscillator settings. `None` for patches saved before kick support.
    pub kick: Option<KickPatchData>,
    /// Arrange page settings. `None` for patches saved before arrange support.
    pub arrange: Option<ArrangePatchData>,
}

/// Arrange page settings stored inside a patch file.
#[derive(Clone, Debug)]
pub struct ArrangePatchData {
    pub num_bars: f32,
    /// Note size label: "1/16", "1/8", "1/4", or "1".
    pub note_size: String,
    pub use_daw_tempo: bool,
    pub manual_tempo: f32,
    /// When true, held DAW notes drive the internal arrange pattern.
    /// `None` for patches saved before this setting existed.
    pub override_daw_midi: Option<bool>,
    /// MIDI notes as (row, bar_pos, slot) triples. `slot` selects the bass
    /// note slot (0 = Note 1, 1 = Note 2); ignored for the kick lane.
    pub notes: Vec<(usize, f32, u8)>,
}

/// Kick oscillator settings stored inside a patch file.
#[derive(Clone, Debug)]
pub struct KickPatchData {
    pub oscillator_waveform: String,
    pub retrigger: bool,
    pub legato_voice_steal: bool,
    pub pitch_hz: f32,
    /// Kick level (K Level macro). `None` for patches saved before level support.
    pub level: Option<f32>,
    /// Oscillator start phase in degrees. `None` for older patches.
    pub phase_deg: Option<f32>,
}

/// Bass instrument settings stored inside a patch file.
#[derive(Clone, Debug)]
pub struct BassPatchData {
    pub oscillator_waveform: String,
    pub retrigger: bool,
    pub legato_voice_steal: bool,
    pub note_length_ms: f32,
    pub pitch_hz: f32,
    pub cutoff_hz: f32,
    pub filter_mode: String,
    /// 6db / 12db / 24db. Defaults to "6db" for older patches.
    pub filter_slope: String,
    /// 0..1, amount of pre-filter saturation. Defaults to 0.0.
    pub filter_drive: f32,
    pub filter_enabled: bool,
    pub filter_2_enabled: bool,
    pub amp_points: Vec<(f32, f32)>,
    pub amp_bends: Vec<f32>,
    pub filter_points: Vec<(f32, f32)>,
    pub filter_bends: Vec<f32>,
    pub filter_2_points: Vec<(f32, f32)>,
    pub filter_2_bends: Vec<f32>,
    /// Bass level (B Level macro). `None` for patches saved before level support.
    pub level: Option<f32>,
    /// Oscillator start phase in degrees. `None` for older patches.
    pub phase_deg: Option<f32>,
    /// OSC A level in the source mixer. Defaults to 1.0 for older patches,
    /// which reproduces the original single-oscillator behaviour.
    pub osc_a_level: f32,
    /// Unison voice count. Defaults to 1 (no unison) for older patches.
    pub osc_a_unison: i32,
    pub osc_a_unison_detune: f32,
    pub osc_a_unison_blend: f32,
    pub osc_b_enabled: bool,
    pub osc_b_waveform: String,
    pub osc_b_level: f32,
    pub osc_b_octave: i32,
    pub osc_b_detune_cents: f32,
    pub osc_b_unison: i32,
    pub osc_b_unison_detune: f32,
    pub osc_b_unison_blend: f32,
    pub sub_enabled: bool,
    pub sub_waveform: String,
    pub sub_level: f32,
    pub sub_octave: i32,
    pub noise_enabled: bool,
    pub noise_level: f32,
    /// Per-slot LFOs. Defaults to all-disabled for older patches.
    pub lfos: [LfoParams; BASS_LFO_COUNT],
    /// Modulation matrix rows. Defaults to unassigned for older patches.
    pub mod_matrix: [ModRouting; BASS_MOD_SLOT_COUNT],
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
    let mut description: Option<String> = None;
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

    let mut bass_seen = false;
    let mut bass_oscillator_waveform: Option<String> = None;
    let mut bass_retrigger: Option<bool> = None;
    let mut bass_legato_voice_steal: Option<bool> = None;
    let mut bass_note_length_ms: Option<f32> = None;
    let mut bass_pitch_hz: Option<f32> = None;
    let mut bass_cutoff_hz: Option<f32> = None;
    let mut bass_filter_mode: Option<String> = None;
    let mut bass_filter_slope: Option<String> = None;
    let mut bass_filter_drive: Option<f32> = None;
    let mut bass_amp_points: Option<Vec<(f32, f32)>> = None;
    let mut bass_amp_bends: Option<Vec<f32>> = None;
    let mut bass_filter_points: Option<Vec<(f32, f32)>> = None;
    let mut bass_filter_bends: Option<Vec<f32>> = None;
    let mut bass_filter_enabled: Option<bool> = None;
    let mut bass_filter_2_enabled: Option<bool> = None;
    let mut bass_filter_2_points: Option<Vec<(f32, f32)>> = None;
    let mut bass_filter_2_bends: Option<Vec<f32>> = None;
    let mut bass_level: Option<f32> = None;
    let mut bass_phase_deg: Option<f32> = None;
    let mut bass_osc_a_level: Option<f32> = None;
    let mut bass_osc_a_unison: Option<i32> = None;
    let mut bass_osc_a_unison_detune: Option<f32> = None;
    let mut bass_osc_a_unison_blend: Option<f32> = None;
    let mut bass_osc_b_unison: Option<i32> = None;
    let mut bass_osc_b_unison_detune: Option<f32> = None;
    let mut bass_osc_b_unison_blend: Option<f32> = None;
    let mut bass_osc_b_enabled: Option<bool> = None;
    let mut bass_osc_b_waveform: Option<String> = None;
    let mut bass_osc_b_level: Option<f32> = None;
    let mut bass_osc_b_octave: Option<i32> = None;
    let mut bass_osc_b_detune_cents: Option<f32> = None;
    let mut bass_sub_enabled: Option<bool> = None;
    let mut bass_sub_waveform: Option<String> = None;
    let mut bass_sub_level: Option<f32> = None;
    let mut bass_sub_octave: Option<i32> = None;
    let mut bass_noise_enabled: Option<bool> = None;
    let mut bass_noise_level: Option<f32> = None;
    let mut bass_lfos: Vec<(usize, LfoParams)> = Vec::new();
    let mut bass_mod_matrix: Vec<(usize, ModRouting)> = Vec::new();

    // Second bass note slot, stored under `bass2_*` keys.
    let mut bass2_seen = false;
    let mut bass2_raw = BassRaw::default();

    let mut kick_seen = false;
    let mut kick_oscillator_waveform: Option<String> = None;
    let mut kick_retrigger: Option<bool> = None;
    let mut kick_legato_voice_steal: Option<bool> = None;
    let mut kick_pitch_hz: Option<f32> = None;
    let mut kick_level: Option<f32> = None;
    let mut kick_phase_deg: Option<f32> = None;

    let mut arrange_seen = false;
    let mut arrange_num_bars: Option<f32> = None;
    let mut arrange_note_size: Option<String> = None;
    let mut arrange_use_daw_tempo: Option<bool> = None;
    let mut arrange_manual_tempo: Option<f32> = None;
    let mut arrange_override_daw_midi: Option<bool> = None;
    let mut arrange_notes: Option<Vec<(usize, f32, u8)>> = None;

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
            "description" => description = Some(sanitize_patch_description(value)),
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
            "bass_oscillator_waveform" => {
                bass_seen = true;
                bass_oscillator_waveform = Some(value.to_owned());
            }
            "bass_retrigger" => {
                bass_seen = true;
                bass_retrigger = value.parse::<bool>().ok();
            }
            "bass_legato_voice_steal" => {
                bass_seen = true;
                bass_legato_voice_steal = value.parse::<bool>().ok();
            }
            "bass_note_length_ms" => {
                bass_seen = true;
                bass_note_length_ms = value.parse::<f32>().ok();
            }
            "bass_pitch_hz" => {
                bass_seen = true;
                bass_pitch_hz = value.parse::<f32>().ok();
            }
            "bass_cutoff_hz" => {
                bass_seen = true;
                bass_cutoff_hz = value.parse::<f32>().ok();
            }
            "bass_filter_mode" => {
                bass_seen = true;
                bass_filter_mode = Some(value.to_owned());
            }
            "bass_filter_slope" => {
                bass_seen = true;
                bass_filter_slope = Some(value.to_owned());
            }
            "bass_filter_drive" => {
                bass_seen = true;
                bass_filter_drive = value.parse::<f32>().ok();
            }
            "bass_amp_points" => {
                bass_seen = true;
                if !value.is_empty() {
                    bass_amp_points = Some(parse_points(value, "bass_amp_points")?);
                }
            }
            "bass_amp_bends" => {
                bass_seen = true;
                bass_amp_bends = Some(parse_bends(value, "bass_amp_bends")?);
            }
            "bass_filter_points" => {
                bass_seen = true;
                if !value.is_empty() {
                    bass_filter_points = Some(parse_points(value, "bass_filter_points")?);
                }
            }
            "bass_filter_bends" => {
                bass_seen = true;
                bass_filter_bends = Some(parse_bends(value, "bass_filter_bends")?);
            }
            "bass_filter_enabled" => {
                bass_seen = true;
                bass_filter_enabled = value.parse::<bool>().ok();
            }
            "bass_filter_2_enabled" => {
                bass_seen = true;
                bass_filter_2_enabled = value.parse::<bool>().ok();
            }
            "bass_filter_2_points" => {
                bass_seen = true;
                if !value.is_empty() {
                    bass_filter_2_points = Some(parse_points(value, "bass_filter_2_points")?);
                }
            }
            "bass_filter_2_bends" => {
                bass_seen = true;
                bass_filter_2_bends = Some(parse_bends(value, "bass_filter_2_bends")?);
            }
            "bass_level" => {
                bass_seen = true;
                bass_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0));
            }
            "bass_phase_deg" => {
                bass_seen = true;
                bass_phase_deg = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 360.0));
            }
            "bass_osc_a_level" => {
                bass_seen = true;
                bass_osc_a_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0));
            }
            "bass_osc_a_unison" => {
                bass_seen = true;
                bass_osc_a_unison = value.parse::<i32>().ok().map(|v| v.clamp(1, 8));
            }
            "bass_osc_a_unison_detune" => {
                bass_seen = true;
                bass_osc_a_unison_detune =
                    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 100.0));
            }
            "bass_osc_a_unison_blend" => {
                bass_seen = true;
                bass_osc_a_unison_blend = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0));
            }
            "bass_osc_b_unison" => {
                bass_seen = true;
                bass_osc_b_unison = value.parse::<i32>().ok().map(|v| v.clamp(1, 8));
            }
            "bass_osc_b_unison_detune" => {
                bass_seen = true;
                bass_osc_b_unison_detune =
                    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 100.0));
            }
            "bass_osc_b_unison_blend" => {
                bass_seen = true;
                bass_osc_b_unison_blend = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0));
            }
            "bass_osc_b_enabled" => {
                bass_seen = true;
                bass_osc_b_enabled = value.parse::<bool>().ok();
            }
            "bass_osc_b_waveform" => {
                bass_seen = true;
                bass_osc_b_waveform = Some(value.to_owned());
            }
            "bass_osc_b_level" => {
                bass_seen = true;
                bass_osc_b_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0));
            }
            "bass_osc_b_octave" => {
                bass_seen = true;
                bass_osc_b_octave = value.parse::<i32>().ok().map(|v| v.clamp(-4, 4));
            }
            "bass_osc_b_detune_cents" => {
                bass_seen = true;
                bass_osc_b_detune_cents =
                    value.parse::<f32>().ok().map(|v| v.clamp(-1200.0, 1200.0));
            }
            "bass_sub_enabled" => {
                bass_seen = true;
                bass_sub_enabled = value.parse::<bool>().ok();
            }
            "bass_sub_waveform" => {
                bass_seen = true;
                bass_sub_waveform = Some(value.to_owned());
            }
            "bass_sub_level" => {
                bass_seen = true;
                bass_sub_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0));
            }
            "bass_sub_octave" => {
                bass_seen = true;
                bass_sub_octave = value.parse::<i32>().ok().map(|v| v.clamp(-4, 4));
            }
            "bass_noise_enabled" => {
                bass_seen = true;
                bass_noise_enabled = value.parse::<bool>().ok();
            }
            "bass_noise_level" => {
                bass_seen = true;
                bass_noise_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0));
            }
            "kick_oscillator_waveform" => {
                kick_seen = true;
                kick_oscillator_waveform = Some(value.to_owned());
            }
            "kick_retrigger" => {
                kick_seen = true;
                kick_retrigger = value.parse::<bool>().ok();
            }
            "kick_legato_voice_steal" => {
                kick_seen = true;
                kick_legato_voice_steal = value.parse::<bool>().ok();
            }
            "kick_pitch_hz" => {
                kick_seen = true;
                kick_pitch_hz = value.parse::<f32>().ok();
            }
            "kick_level" => {
                kick_seen = true;
                kick_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0));
            }
            "kick_phase_deg" => {
                kick_seen = true;
                kick_phase_deg = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 360.0));
            }
            "arrange_num_bars" => {
                arrange_seen = true;
                arrange_num_bars = value.parse::<f32>().ok();
            }
            "arrange_note_size" => {
                arrange_seen = true;
                arrange_note_size = Some(value.to_owned());
            }
            "arrange_use_daw_tempo" => {
                arrange_seen = true;
                arrange_use_daw_tempo = value.parse::<bool>().ok();
            }
            "arrange_manual_tempo" => {
                arrange_seen = true;
                arrange_manual_tempo = value.parse::<f32>().ok();
            }
            "arrange_override_daw_midi" => {
                arrange_seen = true;
                arrange_override_daw_midi = value.parse::<bool>().ok();
            }
            "arrange_notes" => {
                arrange_seen = true;
                arrange_notes = Some(parse_arrange_notes(value));
            }
            _ => {
                if let Some(index) = parse_indexed_key(key, "bass_lfo_", BASS_LFO_COUNT) {
                    bass_seen = true;
                    if let Some(lfo) = parse_lfo_params(value) {
                        bass_lfos.push((index, lfo));
                    }
                } else if let Some(index) =
                    parse_indexed_key(key, "bass_mod_", BASS_MOD_SLOT_COUNT)
                {
                    bass_seen = true;
                    if let Some(routing) = parse_mod_routing(value) {
                        bass_mod_matrix.push((index, routing));
                    }
                } else if let Some(rest) = key.strip_prefix("bass2_") {
                    bass2_seen = true;
                    bass2_raw.parse_key(rest, value)?;
                }
            }
        }
    }

    Ok(PatchData {
        name: patch_name
            .map(|name| sanitize_patch_name(&name))
            .filter(|name| !name.is_empty())
            .or_else(|| fallback_name.map(sanitize_patch_name).filter(|name| !name.is_empty()))
            .ok_or_else(|| "Missing patch name".to_owned())?,
        description: description.unwrap_or_default(),
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
        bass: if bass_seen {
            Some(BassPatchData {
                oscillator_waveform: bass_oscillator_waveform
                    .unwrap_or_else(|| "saw".to_owned()),
                retrigger: bass_retrigger.unwrap_or(true),
                legato_voice_steal: bass_legato_voice_steal.unwrap_or(false),
                note_length_ms: bass_note_length_ms.unwrap_or(220.0),
                pitch_hz: bass_pitch_hz.unwrap_or(55.0),
                cutoff_hz: bass_cutoff_hz.unwrap_or(120.0),
                filter_mode: bass_filter_mode.unwrap_or_else(|| "lowpass".to_owned()),
                filter_slope: bass_filter_slope.unwrap_or_else(|| "6db".to_owned()),
                filter_drive: bass_filter_drive.unwrap_or(0.0).clamp(0.0, 1.0),
                filter_enabled: bass_filter_enabled.unwrap_or(true),
                filter_2_enabled: bass_filter_2_enabled.unwrap_or(false),
                amp_points: bass_amp_points.unwrap_or_default(),
                amp_bends: bass_amp_bends.unwrap_or_default(),
                filter_points: bass_filter_points.unwrap_or_default(),
                filter_bends: bass_filter_bends.unwrap_or_default(),
                filter_2_points: bass_filter_2_points.unwrap_or_default(),
                filter_2_bends: bass_filter_2_bends.unwrap_or_default(),
                level: bass_level,
                phase_deg: bass_phase_deg,
                osc_a_level: bass_osc_a_level.unwrap_or(1.0),
                osc_a_unison: bass_osc_a_unison.unwrap_or(1),
                osc_a_unison_detune: bass_osc_a_unison_detune.unwrap_or(15.0),
                osc_a_unison_blend: bass_osc_a_unison_blend.unwrap_or(0.75),
                osc_b_enabled: bass_osc_b_enabled.unwrap_or(false),
                osc_b_waveform: bass_osc_b_waveform.unwrap_or_else(|| "saw".to_owned()),
                osc_b_level: bass_osc_b_level.unwrap_or(0.5),
                osc_b_octave: bass_osc_b_octave.unwrap_or(0),
                osc_b_detune_cents: bass_osc_b_detune_cents.unwrap_or(0.0),
                osc_b_unison: bass_osc_b_unison.unwrap_or(1),
                osc_b_unison_detune: bass_osc_b_unison_detune.unwrap_or(15.0),
                osc_b_unison_blend: bass_osc_b_unison_blend.unwrap_or(0.75),
                sub_enabled: bass_sub_enabled.unwrap_or(false),
                sub_waveform: bass_sub_waveform.unwrap_or_else(|| "sine".to_owned()),
                sub_level: bass_sub_level.unwrap_or(0.5),
                sub_octave: bass_sub_octave.unwrap_or(-1),
                noise_enabled: bass_noise_enabled.unwrap_or(false),
                noise_level: bass_noise_level.unwrap_or(0.25),
                lfos: {
                    let mut lfos = [LfoParams::default(); BASS_LFO_COUNT];
                    for (index, lfo) in bass_lfos {
                        if let Some(slot) = lfos.get_mut(index) {
                            *slot = lfo;
                        }
                    }
                    lfos
                },
                mod_matrix: {
                    let mut matrix =
                        [ModRouting::default(); BASS_MOD_SLOT_COUNT];
                    for (index, routing) in bass_mod_matrix {
                        if let Some(row) = matrix.get_mut(index) {
                            *row = routing;
                        }
                    }
                    matrix
                },
            })
        } else {
            None
        },
        bass2: if bass2_seen {
            Some(bass2_raw.into_data())
        } else {
            None
        },
        kick: if kick_seen {
            Some(KickPatchData {
                oscillator_waveform: kick_oscillator_waveform
                    .unwrap_or_else(|| "sine".to_owned()),
                retrigger: kick_retrigger.unwrap_or(true),
                legato_voice_steal: kick_legato_voice_steal.unwrap_or(true),
                pitch_hz: kick_pitch_hz.unwrap_or(55.0),
                level: kick_level,
                phase_deg: kick_phase_deg,
            })
        } else {
            None
        },
        arrange: if arrange_seen {
            Some(ArrangePatchData {
                num_bars: arrange_num_bars.unwrap_or(1.0),
                note_size: arrange_note_size.unwrap_or_else(|| "1/4".to_owned()),
                use_daw_tempo: arrange_use_daw_tempo.unwrap_or(true),
                manual_tempo: arrange_manual_tempo.unwrap_or(120.0),
                override_daw_midi: arrange_override_daw_midi,
                notes: arrange_notes.unwrap_or_default(),
            })
        } else {
            None
        },
    })
}

/// Raw accumulator for one bass slot's `bassN_*` patch keys.
#[derive(Default)]
struct BassRaw {
    oscillator_waveform: Option<String>,
    retrigger: Option<bool>,
    legato_voice_steal: Option<bool>,
    note_length_ms: Option<f32>,
    pitch_hz: Option<f32>,
    cutoff_hz: Option<f32>,
    filter_mode: Option<String>,
    filter_slope: Option<String>,
    filter_drive: Option<f32>,
    filter_enabled: Option<bool>,
    filter_2_enabled: Option<bool>,
    amp_points: Option<Vec<(f32, f32)>>,
    amp_bends: Option<Vec<f32>>,
    filter_points: Option<Vec<(f32, f32)>>,
    filter_bends: Option<Vec<f32>>,
    filter_2_points: Option<Vec<(f32, f32)>>,
    filter_2_bends: Option<Vec<f32>>,
    level: Option<f32>,
    phase_deg: Option<f32>,
    osc_a_level: Option<f32>,
    osc_a_unison: Option<i32>,
    osc_a_unison_detune: Option<f32>,
    osc_a_unison_blend: Option<f32>,
    osc_b_enabled: Option<bool>,
    osc_b_waveform: Option<String>,
    osc_b_level: Option<f32>,
    osc_b_octave: Option<i32>,
    osc_b_detune_cents: Option<f32>,
    osc_b_unison: Option<i32>,
    osc_b_unison_detune: Option<f32>,
    osc_b_unison_blend: Option<f32>,
    sub_enabled: Option<bool>,
    sub_waveform: Option<String>,
    sub_level: Option<f32>,
    sub_octave: Option<i32>,
    noise_enabled: Option<bool>,
    noise_level: Option<f32>,
    /// (index, LFO) pairs from `bass2_lfo_<n>` keys.
    lfos: Vec<(usize, LfoParams)>,
    /// (index, routing) pairs from `bass2_mod_<n>` keys.
    mod_matrix: Vec<(usize, ModRouting)>,
}

impl BassRaw {
    /// Parses a single `bass2_<key>=value` suffix (e.g. `pitch_hz`).
    fn parse_key(&mut self, key: &str, value: &str) -> Result<(), String> {
        match key {
            "oscillator_waveform" => self.oscillator_waveform = Some(value.to_owned()),
            "retrigger" => self.retrigger = value.parse::<bool>().ok(),
            "legato_voice_steal" => self.legato_voice_steal = value.parse::<bool>().ok(),
            "note_length_ms" => self.note_length_ms = value.parse::<f32>().ok(),
            "pitch_hz" => self.pitch_hz = value.parse::<f32>().ok(),
            "cutoff_hz" => self.cutoff_hz = value.parse::<f32>().ok(),
            "filter_mode" => self.filter_mode = Some(value.to_owned()),
            "filter_slope" => self.filter_slope = Some(value.to_owned()),
            "filter_drive" => self.filter_drive = value.parse::<f32>().ok(),
            "filter_enabled" => self.filter_enabled = value.parse::<bool>().ok(),
            "filter_2_enabled" => self.filter_2_enabled = value.parse::<bool>().ok(),
            "amp_points" => {
                if !value.is_empty() {
                    self.amp_points = Some(parse_points(value, "bass2_amp_points")?);
                }
            }
            "amp_bends" => self.amp_bends = Some(parse_bends(value, "bass2_amp_bends")?),
            "filter_points" => {
                if !value.is_empty() {
                    self.filter_points = Some(parse_points(value, "bass2_filter_points")?);
                }
            }
            "filter_bends" => self.filter_bends = Some(parse_bends(value, "bass2_filter_bends")?),
            "filter_2_points" => {
                if !value.is_empty() {
                    self.filter_2_points = Some(parse_points(value, "bass2_filter_2_points")?);
                }
            }
            "filter_2_bends" => self.filter_2_bends = Some(parse_bends(value, "bass2_filter_2_bends")?),
            "level" => self.level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0)),
            "phase_deg" => {
                self.phase_deg = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 360.0))
            }
            "osc_a_level" => {
                self.osc_a_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
            }
            "osc_a_unison" => {
                self.osc_a_unison = value.parse::<i32>().ok().map(|v| v.clamp(1, 8))
            }
            "osc_a_unison_detune" => {
                self.osc_a_unison_detune =
                    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 100.0))
            }
            "osc_a_unison_blend" => {
                self.osc_a_unison_blend = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
            }
            "osc_b_unison" => {
                self.osc_b_unison = value.parse::<i32>().ok().map(|v| v.clamp(1, 8))
            }
            "osc_b_unison_detune" => {
                self.osc_b_unison_detune =
                    value.parse::<f32>().ok().map(|v| v.clamp(0.0, 100.0))
            }
            "osc_b_unison_blend" => {
                self.osc_b_unison_blend = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
            }
            "osc_b_enabled" => self.osc_b_enabled = value.parse::<bool>().ok(),
            "osc_b_waveform" => self.osc_b_waveform = Some(value.to_owned()),
            "osc_b_level" => {
                self.osc_b_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
            }
            "osc_b_octave" => {
                self.osc_b_octave = value.parse::<i32>().ok().map(|v| v.clamp(-4, 4))
            }
            "osc_b_detune_cents" => {
                self.osc_b_detune_cents =
                    value.parse::<f32>().ok().map(|v| v.clamp(-1200.0, 1200.0))
            }
            "sub_enabled" => self.sub_enabled = value.parse::<bool>().ok(),
            "sub_waveform" => self.sub_waveform = Some(value.to_owned()),
            "sub_level" => {
                self.sub_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
            }
            "sub_octave" => self.sub_octave = value.parse::<i32>().ok().map(|v| v.clamp(-4, 4)),
            "noise_enabled" => self.noise_enabled = value.parse::<bool>().ok(),
            "noise_level" => {
                self.noise_level = value.parse::<f32>().ok().map(|v| v.clamp(0.0, 1.0))
            }
            _ => {
                if let Some(index) = parse_indexed_key(key, "lfo_", BASS_LFO_COUNT) {
                    if let Some(lfo) = parse_lfo_params(value) {
                        self.lfos.push((index, lfo));
                    }
                } else if let Some(index) =
                    parse_indexed_key(key, "mod_", BASS_MOD_SLOT_COUNT)
                {
                    if let Some(routing) = parse_mod_routing(value) {
                        self.mod_matrix.push((index, routing));
                    }
                }
            }
        }
        Ok(())
    }

    fn into_data(self) -> BassPatchData {
        BassPatchData {
            oscillator_waveform: self
                .oscillator_waveform
                .unwrap_or_else(|| "saw".to_owned()),
            retrigger: self.retrigger.unwrap_or(true),
            legato_voice_steal: self.legato_voice_steal.unwrap_or(false),
            note_length_ms: self.note_length_ms.unwrap_or(220.0),
            pitch_hz: self.pitch_hz.unwrap_or(55.0),
            cutoff_hz: self.cutoff_hz.unwrap_or(120.0),
            filter_mode: self.filter_mode.unwrap_or_else(|| "lowpass".to_owned()),
            filter_slope: self.filter_slope.unwrap_or_else(|| "6db".to_owned()),
            filter_drive: self.filter_drive.unwrap_or(0.0).clamp(0.0, 1.0),
            filter_enabled: self.filter_enabled.unwrap_or(true),
            filter_2_enabled: self.filter_2_enabled.unwrap_or(false),
            amp_points: self.amp_points.unwrap_or_default(),
            amp_bends: self.amp_bends.unwrap_or_default(),
            filter_points: self.filter_points.unwrap_or_default(),
            filter_bends: self.filter_bends.unwrap_or_default(),
            filter_2_points: self.filter_2_points.unwrap_or_default(),
            filter_2_bends: self.filter_2_bends.unwrap_or_default(),
            level: self.level,
            phase_deg: self.phase_deg,
            osc_a_level: self.osc_a_level.unwrap_or(1.0),
            osc_a_unison: self.osc_a_unison.unwrap_or(1),
            osc_a_unison_detune: self.osc_a_unison_detune.unwrap_or(15.0),
            osc_a_unison_blend: self.osc_a_unison_blend.unwrap_or(0.75),
            osc_b_enabled: self.osc_b_enabled.unwrap_or(false),
            osc_b_waveform: self.osc_b_waveform.unwrap_or_else(|| "saw".to_owned()),
            osc_b_level: self.osc_b_level.unwrap_or(0.5),
            osc_b_octave: self.osc_b_octave.unwrap_or(0),
            osc_b_detune_cents: self.osc_b_detune_cents.unwrap_or(0.0),
            osc_b_unison: self.osc_b_unison.unwrap_or(1),
            osc_b_unison_detune: self.osc_b_unison_detune.unwrap_or(15.0),
            osc_b_unison_blend: self.osc_b_unison_blend.unwrap_or(0.75),
            sub_enabled: self.sub_enabled.unwrap_or(false),
            sub_waveform: self.sub_waveform.unwrap_or_else(|| "sine".to_owned()),
            sub_level: self.sub_level.unwrap_or(0.5),
            sub_octave: self.sub_octave.unwrap_or(-1),
            noise_enabled: self.noise_enabled.unwrap_or(false),
            noise_level: self.noise_level.unwrap_or(0.25),
            lfos: {
                let mut lfos = [LfoParams::default(); BASS_LFO_COUNT];
                for (index, lfo) in self.lfos {
                    if let Some(slot) = lfos.get_mut(index) {
                        *slot = lfo;
                    }
                }
                lfos
            },
            mod_matrix: {
                let mut matrix = [ModRouting::default(); BASS_MOD_SLOT_COUNT];
                for (index, routing) in self.mod_matrix {
                    if let Some(row) = matrix.get_mut(index) {
                        *row = routing;
                    }
                }
                matrix
            },
        }
    }
}

/// Extracts the zero-based index from `<prefix><1-based n>` patch keys like
/// `bass_lfo_3` / `mod_2`. Returns None when the key does not match or the
/// index is out of range.
fn parse_indexed_key(key: &str, prefix: &str, count: usize) -> Option<usize> {
    let index = key.strip_prefix(prefix)?.parse::<usize>().ok()?;
    if (1..=count).contains(&index) {
        Some(index - 1)
    } else {
        None
    }
}

/// `enabled,shape,rate_hz,rate_beats,tempo_sync,depth,phase_deg,retrigger`.
fn parse_lfo_params(value: &str) -> Option<LfoParams> {
    let mut parts = value.split(',');
    let enabled = parts.next()?.parse::<bool>().ok()?;
    let shape = LfoShape::from_key(parts.next()?.trim())?;
    let rate_hz = parts.next()?.parse::<f32>().ok()?.clamp(0.01, 100.0);
    let rate_beats = parts.next()?.parse::<f32>().ok()?.clamp(0.0625, 64.0);
    let tempo_sync = parts.next()?.parse::<bool>().ok()?;
    let depth = parts.next()?.parse::<f32>().ok()?.clamp(0.0, 1.0);
    let phase_deg = parts.next()?.parse::<f32>().ok()?.clamp(0.0, 360.0);
    let retrigger = parts
        .next()
        .and_then(|raw| raw.parse::<bool>().ok())
        .unwrap_or(true);
    Some(LfoParams {
        enabled,
        shape,
        rate_hz,
        rate_beats,
        tempo_sync,
        depth,
        phase_deg,
        retrigger,
    })
}

fn lfo_to_string(lfo: &LfoParams) -> String {
    format!(
        "{},{},{},{},{},{},{},{}",
        lfo.enabled,
        lfo.shape.as_key(),
        lfo.rate_hz,
        lfo.rate_beats,
        lfo.tempo_sync,
        lfo.depth,
        lfo.phase_deg,
        lfo.retrigger
    )
}

/// `source,dest,amount`.
fn parse_mod_routing(value: &str) -> Option<ModRouting> {
    let mut parts = value.split(',');
    let source = ModSource::from_key(parts.next()?.trim())?;
    let dest = ModDest::from_key(parts.next()?.trim())?;
    let amount = parts.next()?.parse::<f32>().ok()?.clamp(-1.0, 1.0);
    Some(ModRouting {
        source,
        dest,
        amount,
    })
}

fn mod_routing_to_string(routing: &ModRouting) -> String {
    format!(
        "{},{},{}",
        routing.source.as_key(),
        routing.dest.as_key(),
        routing.amount
    )
}

/// Parses arrange notes stored as `row,bar_pos[,slot]|...`.
/// The optional third field is the bass note slot (default 0).
/// Empty or malformed segments are skipped.
fn parse_arrange_notes(raw: &str) -> Vec<(usize, f32, u8)> {
    raw.split('|')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .filter_map(|segment| {
            let mut parts = segment.split(',');
            let row = parts.next()?.trim().parse::<usize>().ok()?;
            let pos = parts.next()?.trim().parse::<f32>().ok()?;
            let slot = parts
                .next()
                .and_then(|raw| raw.trim().parse::<u8>().ok())
                .unwrap_or(0);
            Some((row, pos.max(0.0), slot.min(1)))
        })
        .collect()
}

fn arrange_notes_to_string(notes: &[(usize, f32, u8)]) -> String {
    notes
        .iter()
        .map(|(row, pos, slot)| {
            if *slot == 0 {
                format!("{row},{pos:.6}")
            } else {
                format!("{row},{pos:.6},{slot}")
            }
        })
        .collect::<Vec<_>>()
        .join("|")
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
    let serialized = serialize_patch(patch);

    fs::write(&path, serialized)
        .map_err(|error| format!("Failed to save patch '{}': {error}", path.display()))
}

/// Renders a patch to its on-disk key=value representation.
fn serialize_patch(patch: &PatchData) -> String {
    let mut lines = vec![
        format!("name={}", patch.name),
        format!("description={}", sanitize_patch_description(&patch.description)),
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
    ];

    if let Some(bass) = &patch.bass {
        lines.push(format!("bass_oscillator_waveform={}", bass.oscillator_waveform));
        lines.push(format!("bass_retrigger={}", bass.retrigger));
        lines.push(format!("bass_legato_voice_steal={}", bass.legato_voice_steal));
        lines.push(format!("bass_note_length_ms={}", bass.note_length_ms));
        lines.push(format!("bass_pitch_hz={}", bass.pitch_hz));
        lines.push(format!("bass_cutoff_hz={}", bass.cutoff_hz));
        lines.push(format!("bass_filter_mode={}", bass.filter_mode));
        lines.push(format!("bass_filter_slope={}", bass.filter_slope));
        lines.push(format!("bass_filter_drive={}", bass.filter_drive));
        lines.push(format!("bass_filter_enabled={}", bass.filter_enabled));
        lines.push(format!("bass_filter_2_enabled={}", bass.filter_2_enabled));
        lines.push(format!("bass_amp_points={}", points_to_string(&bass.amp_points)));
        lines.push(format!("bass_amp_bends={}", bends_to_string(&bass.amp_bends)));
        lines.push(format!(
            "bass_filter_points={}",
            points_to_string(&bass.filter_points)
        ));
        lines.push(format!(
            "bass_filter_bends={}",
            bends_to_string(&bass.filter_bends)
        ));
        lines.push(format!(
            "bass_filter_2_points={}",
            points_to_string(&bass.filter_2_points)
        ));
        lines.push(format!(
            "bass_filter_2_bends={}",
            bends_to_string(&bass.filter_2_bends)
        ));
        if let Some(level) = bass.level {
            lines.push(format!("bass_level={level}"));
        }
        if let Some(phase_deg) = bass.phase_deg {
            lines.push(format!("bass_phase_deg={phase_deg}"));
        }
        lines.push(format!("bass_osc_a_level={}", bass.osc_a_level));
        lines.push(format!("bass_osc_a_unison={}", bass.osc_a_unison));
        lines.push(format!(
            "bass_osc_a_unison_detune={}",
            bass.osc_a_unison_detune
        ));
        lines.push(format!(
            "bass_osc_a_unison_blend={}",
            bass.osc_a_unison_blend
        ));
        lines.push(format!("bass_osc_b_unison={}", bass.osc_b_unison));
        lines.push(format!(
            "bass_osc_b_unison_detune={}",
            bass.osc_b_unison_detune
        ));
        lines.push(format!(
            "bass_osc_b_unison_blend={}",
            bass.osc_b_unison_blend
        ));
        lines.push(format!("bass_osc_b_enabled={}", bass.osc_b_enabled));
        lines.push(format!("bass_osc_b_waveform={}", bass.osc_b_waveform));
        lines.push(format!("bass_osc_b_level={}", bass.osc_b_level));
        lines.push(format!("bass_osc_b_octave={}", bass.osc_b_octave));
        lines.push(format!(
            "bass_osc_b_detune_cents={}",
            bass.osc_b_detune_cents
        ));
        lines.push(format!("bass_sub_enabled={}", bass.sub_enabled));
        lines.push(format!("bass_sub_waveform={}", bass.sub_waveform));
        lines.push(format!("bass_sub_level={}", bass.sub_level));
        lines.push(format!("bass_sub_octave={}", bass.sub_octave));
        lines.push(format!("bass_noise_enabled={}", bass.noise_enabled));
        lines.push(format!("bass_noise_level={}", bass.noise_level));
        for (index, lfo) in bass.lfos.iter().enumerate() {
            if *lfo != LfoParams::default() {
                lines.push(format!("bass_lfo_{}={}", index + 1, lfo_to_string(lfo)));
            }
        }
        for (index, routing) in bass.mod_matrix.iter().enumerate() {
            if *routing != ModRouting::default() {
                lines.push(format!(
                    "bass_mod_{}={}",
                    index + 1,
                    mod_routing_to_string(routing)
                ));
            }
        }
    }

    if let Some(bass2) = &patch.bass2 {
        lines.push(format!(
            "bass2_oscillator_waveform={}",
            bass2.oscillator_waveform
        ));
        lines.push(format!("bass2_retrigger={}", bass2.retrigger));
        lines.push(format!(
            "bass2_legato_voice_steal={}",
            bass2.legato_voice_steal
        ));
        lines.push(format!("bass2_note_length_ms={}", bass2.note_length_ms));
        lines.push(format!("bass2_pitch_hz={}", bass2.pitch_hz));
        lines.push(format!("bass2_cutoff_hz={}", bass2.cutoff_hz));
        lines.push(format!("bass2_filter_mode={}", bass2.filter_mode));
        lines.push(format!("bass2_filter_slope={}", bass2.filter_slope));
        lines.push(format!("bass2_filter_drive={}", bass2.filter_drive));
        lines.push(format!("bass2_filter_enabled={}", bass2.filter_enabled));
        lines.push(format!("bass2_filter_2_enabled={}", bass2.filter_2_enabled));
        lines.push(format!(
            "bass2_amp_points={}",
            points_to_string(&bass2.amp_points)
        ));
        lines.push(format!(
            "bass2_amp_bends={}",
            bends_to_string(&bass2.amp_bends)
        ));
        lines.push(format!(
            "bass2_filter_points={}",
            points_to_string(&bass2.filter_points)
        ));
        lines.push(format!(
            "bass2_filter_bends={}",
            bends_to_string(&bass2.filter_bends)
        ));
        lines.push(format!(
            "bass2_filter_2_points={}",
            points_to_string(&bass2.filter_2_points)
        ));
        lines.push(format!(
            "bass2_filter_2_bends={}",
            bends_to_string(&bass2.filter_2_bends)
        ));
        if let Some(level) = bass2.level {
            lines.push(format!("bass2_level={level}"));
        }
        if let Some(phase_deg) = bass2.phase_deg {
            lines.push(format!("bass2_phase_deg={phase_deg}"));
        }
        lines.push(format!("bass2_osc_a_level={}", bass2.osc_a_level));
        lines.push(format!("bass2_osc_a_unison={}", bass2.osc_a_unison));
        lines.push(format!(
            "bass2_osc_a_unison_detune={}",
            bass2.osc_a_unison_detune
        ));
        lines.push(format!(
            "bass2_osc_a_unison_blend={}",
            bass2.osc_a_unison_blend
        ));
        lines.push(format!("bass2_osc_b_unison={}", bass2.osc_b_unison));
        lines.push(format!(
            "bass2_osc_b_unison_detune={}",
            bass2.osc_b_unison_detune
        ));
        lines.push(format!(
            "bass2_osc_b_unison_blend={}",
            bass2.osc_b_unison_blend
        ));
        lines.push(format!("bass2_osc_b_enabled={}", bass2.osc_b_enabled));
        lines.push(format!("bass2_osc_b_waveform={}", bass2.osc_b_waveform));
        lines.push(format!("bass2_osc_b_level={}", bass2.osc_b_level));
        lines.push(format!("bass2_osc_b_octave={}", bass2.osc_b_octave));
        lines.push(format!(
            "bass2_osc_b_detune_cents={}",
            bass2.osc_b_detune_cents
        ));
        lines.push(format!("bass2_sub_enabled={}", bass2.sub_enabled));
        lines.push(format!("bass2_sub_waveform={}", bass2.sub_waveform));
        lines.push(format!("bass2_sub_level={}", bass2.sub_level));
        lines.push(format!("bass2_sub_octave={}", bass2.sub_octave));
        lines.push(format!("bass2_noise_enabled={}", bass2.noise_enabled));
        lines.push(format!("bass2_noise_level={}", bass2.noise_level));
        for (index, lfo) in bass2.lfos.iter().enumerate() {
            if *lfo != LfoParams::default() {
                lines.push(format!("bass2_lfo_{}={}", index + 1, lfo_to_string(lfo)));
            }
        }
        for (index, routing) in bass2.mod_matrix.iter().enumerate() {
            if *routing != ModRouting::default() {
                lines.push(format!(
                    "bass2_mod_{}={}",
                    index + 1,
                    mod_routing_to_string(routing)
                ));
            }
        }
    }

    if let Some(kick) = &patch.kick {
        lines.push(format!("kick_oscillator_waveform={}", kick.oscillator_waveform));
        lines.push(format!("kick_retrigger={}", kick.retrigger));
        lines.push(format!("kick_legato_voice_steal={}", kick.legato_voice_steal));
        lines.push(format!("kick_pitch_hz={}", kick.pitch_hz));
        if let Some(level) = kick.level {
            lines.push(format!("kick_level={level}"));
        }
        if let Some(phase_deg) = kick.phase_deg {
            lines.push(format!("kick_phase_deg={phase_deg}"));
        }
    }

    if let Some(arrange) = &patch.arrange {
        lines.push(format!("arrange_num_bars={}", arrange.num_bars));
        lines.push(format!("arrange_note_size={}", arrange.note_size));
        lines.push(format!("arrange_use_daw_tempo={}", arrange.use_daw_tempo));
        lines.push(format!("arrange_manual_tempo={}", arrange.manual_tempo));
        if let Some(override_daw_midi) = arrange.override_daw_midi {
            lines.push(format!("arrange_override_daw_midi={override_daw_midi}"));
        }
        lines.push(format!("arrange_notes={}", arrange_notes_to_string(&arrange.notes)));
    }

    lines.join("\n")
}

pub fn load_patch(name: &str) -> Result<PatchData, String> {
    let path = patch_file_path(name)?;
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to load patch '{}': {error}", path.display()))?;

    parse_patch(&raw, Some(name))
}

pub const SETTINGS_FILE_NAME: &str = "user.librekick_settings";

/// UI-level settings persisted between sessions (not part of patches).
#[derive(Clone, Debug)]
pub struct UiSettingsData {
    pub tuning_a4_hz: f32,
    pub display_scale: f32,
}

fn settings_file_path() -> PathBuf {
    patches_dir_path().join(SETTINGS_FILE_NAME)
}

pub fn save_settings(settings: &UiSettingsData) -> Result<(), String> {
    ensure_patches_dir()?;

    let path = settings_file_path();
    let serialized = [
        format!("tuning_a4_hz={}", settings.tuning_a4_hz),
        format!("display_scale={}", settings.display_scale),
    ]
    .join("\n");

    fs::write(&path, serialized)
        .map_err(|error| format!("Failed to save settings '{}': {error}", path.display()))
}

pub fn load_settings() -> Result<Option<UiSettingsData>, String> {
    ensure_patches_dir()?;

    let path = settings_file_path();
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to load settings '{}': {error}", path.display()))?;

    let mut tuning_a4_hz = None;
    let mut display_scale = None;

    for raw_line in raw.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        match key.trim() {
            "tuning_a4_hz" => {
                tuning_a4_hz = value
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| "Invalid tuning_a4_hz in settings".to_owned())
                    .ok()
            }
            "display_scale" => {
                display_scale = value
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| "Invalid display_scale in settings".to_owned())
                    .ok()
            }
            _ => {}
        }
    }

    Ok(Some(UiSettingsData {
        tuning_a4_hz: tuning_a4_hz.unwrap_or(432.0),
        display_scale: display_scale.unwrap_or(1.0).clamp(0.5, 2.0),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal patch using only the pre-OSC-B key set.
    const LEGACY_PATCH: &str = "\
name=legacy
tuning_a4_hz=432
note_end_ms=145
max_note_length_ms=1000
waveform_zoom_percent=100
amplitude_points=0.000000,1.000000|1.000000,0.000000
pitch_points=0.000000,0.500000|1.000000,0.000000
bass_oscillator_waveform=saw
bass_pitch_hz=45
bass_cutoff_hz=182
bass_filter_mode=lowpass
";

    /// A patch saved before the source mixer existed must load with OSC A at
    /// unity and every new source disabled, which reproduces the original
    /// single-oscillator signal path exactly.
    #[test]
    fn legacy_patch_defaults_to_single_oscillator() {
        let patch = parse_patch(LEGACY_PATCH, None).expect("legacy patch should parse");
        let bass = patch.bass.expect("bass section should be present");

        assert_eq!(bass.osc_a_level, 1.0);
        assert!(!bass.osc_b_enabled);
        assert!(!bass.sub_enabled);
        assert!(!bass.noise_enabled);

        // Pre-existing fields must keep their original values.
        assert_eq!(bass.pitch_hz, 45.0);
        assert_eq!(bass.cutoff_hz, 182.0);
        assert_eq!(bass.oscillator_waveform, "saw");
    }

    /// The second slot shares the same backward-compatible defaults.
    #[test]
    fn legacy_second_slot_defaults_to_single_oscillator() {
        let raw = format!("{LEGACY_PATCH}bass2_oscillator_waveform=saw\n");
        let patch = parse_patch(&raw, None).expect("legacy patch should parse");
        let bass2 = patch.bass2.expect("bass2 section should be present");

        assert_eq!(bass2.osc_a_level, 1.0);
        assert!(!bass2.osc_b_enabled);
        assert!(!bass2.sub_enabled);
        assert!(!bass2.noise_enabled);
        assert_eq!(bass2.sub_octave, -1);
    }

    /// Source settings must survive a save/parse round trip for both slots.
    #[test]
    fn source_settings_round_trip() {
        let mut patch = parse_patch(LEGACY_PATCH, None).expect("legacy patch should parse");
        {
            let bass = patch.bass.as_mut().expect("bass section");
            bass.osc_a_level = 0.75;
            bass.osc_b_enabled = true;
            bass.osc_b_waveform = "square".to_owned();
            bass.osc_b_level = 0.4;
            bass.osc_b_octave = -1;
            bass.osc_b_detune_cents = 12.0;
            bass.sub_enabled = true;
            bass.sub_waveform = "sine".to_owned();
            bass.sub_level = 0.6;
            bass.sub_octave = -2;
            bass.noise_enabled = true;
            bass.noise_level = 0.3;
        }

        let serialized = serialize_patch(&patch);
        let reparsed = parse_patch(&serialized, None).expect("round trip should parse");
        let bass = reparsed.bass.expect("bass section");

        assert_eq!(bass.osc_a_level, 0.75);
        assert!(bass.osc_b_enabled);
        assert_eq!(bass.osc_b_waveform, "square");
        assert_eq!(bass.osc_b_level, 0.4);
        assert_eq!(bass.osc_b_octave, -1);
        assert_eq!(bass.osc_b_detune_cents, 12.0);
        assert!(bass.sub_enabled);
        assert_eq!(bass.sub_level, 0.6);
        assert_eq!(bass.sub_octave, -2);
        assert!(bass.noise_enabled);
        assert_eq!(bass.noise_level, 0.3);
    }
}
