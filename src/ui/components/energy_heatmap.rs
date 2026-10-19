//! Band-split energy heatmap for the rendered waveform previews.
//!
//! Shows where energy sits over time, split into frequency bands, so a
//! producer can see at a glance whether a bassline's sub is ringing on too
//! long, whether the body and growl bands overlap with the kick, or whether
//! noise is leaking into the top band.
//!
//! The analysis runs on the same offline-rendered samples the waveform trace
//! is drawn from, so the heatmap always describes exactly what is on screen
//! and costs nothing on the audio thread.
//!
//! ## Frequency ceiling
//!
//! Previews render at [`crate::audio::PREVIEW_SAMPLE_RATE`] (4 kHz), so the
//! highest frequency they can represent at all is 2 kHz. The bands below are
//! chosen to fit under that ceiling: a band above it would always read empty
//! and would be actively misleading. This display therefore tells you about
//! sub/body/growl/edge balance, not about genuine high-frequency content.

use nih_plug_egui::egui::{Align2, Color32, Painter, Pos2, Rect, Stroke};

use crate::ui::theme::themed_font;

pub(crate) const BAND_COUNT: usize = 4;

/// Band split points in Hz. Chosen for bass/kick work and kept under the
/// 2 kHz preview Nyquist limit: sub fundamentals, body/punch, growl/presence,
/// then the top band where buzz and noise show up.
const CROSSOVERS: [f32; BAND_COUNT - 1] = [60.0, 160.0, 500.0];

/// Row labels, ordered low band first to match the band arrays.
pub(crate) const BAND_LABELS: [&str; BAND_COUNT] = ["SUB", "LOW", "MID", "EDGE"];

/// Quietest level the heatmap shows, in dBFS. Anything below reads as empty.
const FLOOR_DB: f32 = -60.0;

/// One time slice of the analysed signal. Every field is measured from the
/// samples themselves — nothing here is fed by a parameter value, so the
/// visuals built from it are a structural picture of the actual sound.
#[derive(Clone, Copy, Default)]
struct Column {
    /// Per-band energy, normalised to 0..1 on a fixed dBFS scale so colours
    /// mean the same thing on every page.
    bands: [f32; BAND_COUNT],
    /// Overall energy of the slice, same normalisation.
    level: f32,
    /// Energy-weighted centroid across the bands, 0..1. 0 means all the
    /// energy sits in SUB, 1 means it all sits in EDGE. This is what the
    /// filter audibly does to the signal, measured rather than inferred.
    brightness: f32,
}

#[derive(Default)]
pub(crate) struct EnergyMap {
    columns: Vec<Column>,
}

impl EnergyMap {
    pub(crate) fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    fn column_at(&self, t: f32) -> Column {
        if self.columns.is_empty() {
            return Column::default();
        }
        let last = self.columns.len() - 1;
        let index = (t.clamp(0.0, 1.0) * last as f32).round() as usize;
        self.columns[index.min(last)]
    }

    /// Band intensities at normalised position `t` across the analysed span.
    pub(crate) fn sample_at(&self, t: f32) -> [f32; BAND_COUNT] {
        self.column_at(t).bands
    }

    /// Overall energy at normalised position `t`, 0..1.
    pub(crate) fn level_at(&self, t: f32) -> f32 {
        self.column_at(t).level
    }

    /// Spectral centroid at normalised position `t`, 0..1.
    pub(crate) fn brightness_at(&self, t: f32) -> f32 {
        self.column_at(t).brightness
    }

    /// Loudest normalised intensity in each band across the whole span.
    #[cfg(test)]
    fn band_peaks(&self) -> [f32; BAND_COUNT] {
        let mut peaks = [0.0f32; BAND_COUNT];
        for column in &self.columns {
            for (peak, value) in peaks.iter_mut().zip(column.bands) {
                *peak = peak.max(value);
            }
        }
        peaks
    }
}

/// Splits `samples` into bands and measures each band's RMS energy per
/// output column.
///
/// Band splitting uses two cascaded one-pole lowpasses per crossover
/// (12 dB/oct), matching the filter style the voices themselves use, with
/// each band taken as the difference between neighbouring lowpass outputs.
/// That keeps the bands complementary, so no energy is double counted or
/// lost between them.
pub(crate) fn analyze(samples: &[f32], sample_rate: f32, column_count: usize) -> EnergyMap {
    if samples.is_empty() || sample_rate <= 0.0 || column_count == 0 {
        return EnergyMap {
            columns: Vec::new(),
        };
    }

    // More columns than samples would leave gaps with nothing to average.
    let column_count = column_count.min(samples.len());

    let alphas = CROSSOVERS.map(|hz| one_pole_alpha(hz, sample_rate));
    // [crossover][cascade stage]
    let mut lp = [[0.0f32; 2]; BAND_COUNT - 1];

    let mut sums = vec![[0.0f64; BAND_COUNT]; column_count];
    // Overall energy is taken from the raw samples rather than from the band
    // sum, so it stays a true measurement of the signal.
    let mut level_sums = vec![0.0f64; column_count];
    let mut counts = vec![0u32; column_count];
    let last_sample = (samples.len() - 1).max(1) as f32;

    for (index, &sample) in samples.iter().enumerate() {
        let mut lowpassed = [0.0f32; BAND_COUNT - 1];
        for (band, alpha) in alphas.iter().enumerate() {
            lp[band][0] += alpha * (sample - lp[band][0]);
            lp[band][1] += alpha * (lp[band][0] - lp[band][1]);
            lowpassed[band] = lp[band][1];
        }

        let bands = [
            lowpassed[0],
            lowpassed[1] - lowpassed[0],
            lowpassed[2] - lowpassed[1],
            sample - lowpassed[2],
        ];

        let position = index as f32 / last_sample;
        let column =
            ((position * (column_count - 1) as f32).round() as usize).min(column_count - 1);
        for (sum, value) in sums[column].iter_mut().zip(bands) {
            *sum += (value as f64) * (value as f64);
        }
        level_sums[column] += (sample as f64) * (sample as f64);
        counts[column] += 1;
    }

    let columns = sums
        .iter()
        .zip(&level_sums)
        .zip(&counts)
        .map(|((sum, &level_sum), &count)| {
            if count == 0 {
                return Column::default();
            }

            // Linear RMS per band, kept linear for the centroid so loud
            // bands pull the brightness in proportion to real energy.
            let mut linear = [0.0f32; BAND_COUNT];
            for (slot, total) in linear.iter_mut().zip(sum) {
                *slot = (total / count as f64).sqrt() as f32;
            }

            let mut bands = [0.0f32; BAND_COUNT];
            for (slot, rms) in bands.iter_mut().zip(linear) {
                *slot = normalized_db(rms);
            }

            let total: f32 = linear.iter().sum();
            let brightness = if total > 1.0e-9 {
                let weighted: f32 = linear
                    .iter()
                    .enumerate()
                    .map(|(band, energy)| band as f32 * energy)
                    .sum();
                (weighted / total / (BAND_COUNT - 1) as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };

            Column {
                bands,
                level: normalized_db((level_sum / count as f64).sqrt() as f32),
                brightness,
            }
        })
        .collect();

    EnergyMap { columns }
}

/// One-pole lowpass coefficient for `cutoff_hz` at `sample_rate`. Clamped
/// below Nyquist so a crossover above the preview's range cannot blow up.
fn one_pole_alpha(cutoff_hz: f32, sample_rate: f32) -> f32 {
    let nyquist = sample_rate * 0.5;
    let cutoff = cutoff_hz.clamp(1.0, (nyquist * 0.99).max(1.0));
    let alpha = 1.0 - (-std::f32::consts::TAU * cutoff / sample_rate).exp();
    alpha.clamp(0.0, 1.0)
}

/// Maps an RMS amplitude onto 0..1 across `FLOOR_DB..0` dBFS. dB rather than
/// linear, because linear energy would leave everything but the loudest
/// moment looking black.
fn normalized_db(rms: f32) -> f32 {
    if rms <= 1.0e-9 {
        return 0.0;
    }
    let db = 20.0 * rms.log10();
    ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

/// Blackbody-style ramp: deep red, orange, yellow, white. Stays inside the
/// amber CRT palette while still reading as a conventional heatmap.
///
/// This is the single colour source for every signal-driven visual, so the
/// same measurement always produces the same colour wherever it is drawn.
fn ramp_rgb(t: f32) -> [f32; 3] {
    const STOPS: [(f32, [f32; 3]); 5] = [
        (0.00, [40.0, 8.0, 4.0]),
        (0.30, [150.0, 30.0, 8.0]),
        (0.60, [240.0, 110.0, 20.0]),
        (0.85, [255.0, 200.0, 70.0]),
        (1.00, [255.0, 245.0, 210.0]),
    ];

    let t = t.clamp(0.0, 1.0);
    for pair in STOPS.windows(2) {
        let (lo_t, lo) = pair[0];
        let (hi_t, hi) = pair[1];
        if t <= hi_t {
            let span = (hi_t - lo_t).max(f32::EPSILON);
            let local = ((t - lo_t) / span).clamp(0.0, 1.0);
            return [
                lo[0] + (hi[0] - lo[0]) * local,
                lo[1] + (hi[1] - lo[1]) * local,
                lo[2] + (hi[2] - lo[2]) * local,
            ];
        }
    }
    STOPS[STOPS.len() - 1].1
}

/// Heatmap cell colour: position on the ramp and opacity both come from the
/// cell's measured energy, so quiet cells let the panel show through instead
/// of painting a solid block.
fn heat_color(intensity: f32) -> Color32 {
    let t = intensity.clamp(0.0, 1.0);
    if t <= 0.001 {
        return Color32::TRANSPARENT;
    }
    let rgb = ramp_rgb(t);
    let alpha = (60.0 + 195.0 * t).clamp(0.0, 255.0);
    Color32::from_rgba_unmultiplied(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, alpha as u8)
}

/// Colour for a point on the waveform, generated entirely from what the
/// signal is doing there: hue/temperature from the spectral centroid, and
/// opacity from the measured energy. Dull red means the energy has collapsed
/// into the sub band; pale and bright means it is spread up into EDGE.
pub(crate) fn signal_color(brightness: f32, level: f32) -> Color32 {
    let rgb = ramp_rgb(brightness);
    let alpha = (40.0 + 215.0 * level.clamp(0.0, 1.0)).clamp(0.0, 255.0);
    Color32::from_rgba_unmultiplied(rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, alpha as u8)
}

/// Draws the heatmap into `rect`: one row per band, lowest band at the
/// bottom so frequency increases upward like a spectrogram.
///
/// `window` is the normalised span of the map to show, letting a zoomed or
/// scrolled view draw a slice of a map that was analysed once over the whole
/// signal. `playhead` draws a marker at a normalised position within that
/// window, which is what animates the display during playback.
pub(crate) fn draw(
    painter: &Painter,
    rect: Rect,
    map: &EnergyMap,
    window: (f32, f32),
    playhead: Option<f32>,
    ui_scale: f32,
) {
    if map.is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let (window_start, window_end) = (window.0.min(window.1), window.1.max(window.0));
    let row_height = rect.height() / BAND_COUNT as f32;

    // One cell per ~1.5 points keeps the fill smooth without painting a
    // separate rect for every pixel.
    let cell_width = (1.5 * ui_scale).max(1.0);
    let cell_count = (rect.width() / cell_width).ceil().max(1.0) as usize;

    for cell in 0..cell_count {
        let x0 = rect.left() + cell as f32 * cell_width;
        let x1 = (x0 + cell_width).min(rect.right());
        if x0 >= rect.right() {
            break;
        }
        let position = (cell as f32 + 0.5) / cell_count as f32;
        let t = window_start + (window_end - window_start) * position;
        let bands = map.sample_at(t);

        for (band, intensity) in bands.iter().enumerate() {
            let color = heat_color(*intensity);
            if color == Color32::TRANSPARENT {
                continue;
            }
            // Band 0 is the lowest, drawn at the bottom.
            let top = rect.bottom() - row_height * (band + 1) as f32;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x1, top + row_height)),
                0.0,
                color,
            );
        }
    }

    // Row separators and labels, drawn over the cells.
    let label_font = themed_font(8.0 * ui_scale);
    for (band, label) in BAND_LABELS.iter().enumerate() {
        let top = rect.bottom() - row_height * (band + 1) as f32;
        if band > 0 {
            painter.line_segment(
                [Pos2::new(rect.left(), top), Pos2::new(rect.right(), top)],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 120)),
            );
        }
        painter.text(
            Pos2::new(rect.left() + 3.0 * ui_scale, top + row_height * 0.5),
            Align2::LEFT_CENTER,
            *label,
            label_font.clone(),
            Color32::from_rgba_unmultiplied(255, 230, 190, 150),
        );
    }

    if let Some(phase) = playhead {
        let x = rect.left() + phase.clamp(0.0, 1.0) * rect.width();
        painter.vline(
            x,
            rect.y_range(),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 170)),
        );
    }
}

/// Paints the band energies inside the waveform's own silhouette instead
/// of a fixed strip: for each x column the four bands are stretched across
/// the wave's vertical span at that column, so the display is literally the
/// shape of the sound coloured by where its energy lives in frequency.
/// Columns where the wave is silent collapse to nothing — no signal, no
/// heat. `window` maps the shown x span onto the analysed map, same as
/// [`draw`].
pub(crate) fn draw_in_waveform(
    painter: &Painter,
    rect: Rect,
    waveform_points: &[Pos2],
    map: &EnergyMap,
    window: (f32, f32),
    ui_scale: f32,
) {
    if map.is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let (window_start, window_end) = (window.0.min(window.1), window.1.max(window.0));
    let cell_width = (1.5 * ui_scale).max(1.0);
    let cell_count = (rect.width() / cell_width).ceil().max(1.0) as usize;

    // First pass: each column's amplitude — the wave point's furthest
    // distance from the midline within a small window. A cell only
    // ~1.5 points wide catches roughly one wave point, and the polyline is
    // mostly smooth, so a local min/max would hug the trace and read as a
    // border rather than a fill. Mirroring the amplitude about the midline
    // gives the classic filled-waveform silhouette the bands live inside.
    const SPAN_RADIUS: usize = 4;
    let mid = rect.center().y;
    let mut column_amp = vec![f32::NAN; cell_count];
    let mut first_cell = usize::MAX;
    let mut last_cell = 0usize;
    for point in waveform_points {
        if point.x < rect.left() || point.x >= rect.right() {
            continue;
        }
        let cell = ((point.x - rect.left()) / cell_width) as usize;
        let cell = cell.min(cell_count - 1);
        let deviation = (point.y - mid).abs();
        column_amp[cell] = if column_amp[cell].is_nan() {
            deviation
        } else {
            column_amp[cell].max(deviation)
        };
        first_cell = first_cell.min(cell);
        last_cell = last_cell.max(cell);
    }
    if first_cell > last_cell {
        return;
    }

    for cell in first_cell..=last_cell {
        let lo = cell.saturating_sub(SPAN_RADIUS);
        let hi = (cell + SPAN_RADIUS).min(cell_count - 1);
        let mut amplitude = 0.0f32;
        let mut any = false;
        for other in lo..=hi {
            if column_amp[other].is_finite() {
                amplitude = amplitude.max(column_amp[other]);
                any = true;
            }
        }
        let span = amplitude * 2.0;
        if !any || span < 1.0 {
            continue;
        }
        let bottom = mid + amplitude;
        let x0 = rect.left() + cell as f32 * cell_width;
        let x1 = (x0 + cell_width).min(rect.right());
        let position = (cell as f32 + 0.5) / cell_count as f32;
        let t = window_start + (window_end - window_start) * position;
        let bands = map.sample_at(t);

        for (band, intensity) in bands.iter().enumerate() {
            let color = heat_color(*intensity);
            if color == Color32::TRANSPARENT {
                continue;
            }
            // Band 0 (SUB) hugs the bottom of the silhouette.
            let band_top = bottom - span * (band + 1) as f32 / BAND_COUNT as f32;
            let band_bottom = bottom - span * band as f32 / BAND_COUNT as f32;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(x0, band_top), Pos2::new(x1, band_bottom)),
                0.0,
                color,
            );
        }
    }

    // Band labels sit at fixed quarters of the graph so they read as a
    // legend even where the wave is thin at the left edge.
    let label_font = themed_font(8.0 * ui_scale);
    let row_height = rect.height() / BAND_COUNT as f32;
    for (band, label) in BAND_LABELS.iter().enumerate() {
        let label_top = rect.bottom() - row_height * (band + 1) as f32;
        painter.text(
            Pos2::new(rect.left() + 3.0 * ui_scale, label_top + row_height * 0.5),
            Align2::LEFT_CENTER,
            *label,
            label_font.clone(),
            Color32::from_rgba_unmultiplied(255, 230, 190, 150),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = crate::audio::PREVIEW_SAMPLE_RATE;

    /// A sine at `hz`, long enough for the band filters to settle.
    fn tone(hz: f32, amplitude: f32) -> Vec<f32> {
        let len = (RATE * 0.5) as usize;
        (0..len)
            .map(|i| {
                let phase = std::f32::consts::TAU * hz * (i as f32 / RATE);
                phase.sin() * amplitude
            })
            .collect()
    }

    /// Index of the band holding the most energy, measured over the settled
    /// second half of the signal so filter warm-up does not skew it.
    fn dominant_band(samples: &[f32]) -> usize {
        let settled = &samples[samples.len() / 2..];
        let peaks = analyze(settled, RATE, 64).band_peaks();
        peaks
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(index, _)| index)
            .unwrap()
    }

    #[test]
    fn empty_input_produces_empty_map() {
        let map = analyze(&[], RATE, 32);
        assert!(map.is_empty());
        assert_eq!(map.sample_at(0.5), [0.0; BAND_COUNT]);
    }

    #[test]
    fn silence_reads_as_no_energy() {
        let map = analyze(&vec![0.0; 1024], RATE, 32);
        assert_eq!(map.band_peaks(), [0.0; BAND_COUNT]);
    }

    /// Each band must actually capture the range it claims to cover,
    /// otherwise the display would point a producer at the wrong frequencies.
    #[test]
    fn each_band_captures_its_own_range() {
        assert_eq!(dominant_band(&tone(40.0, 0.8)), 0, "40 Hz belongs to SUB");
        assert_eq!(dominant_band(&tone(110.0, 0.8)), 1, "110 Hz belongs to LOW");
        assert_eq!(dominant_band(&tone(300.0, 0.8)), 2, "300 Hz belongs to MID");
        assert_eq!(
            dominant_band(&tone(1200.0, 0.8)),
            3,
            "1200 Hz belongs to EDGE"
        );
    }

    /// A louder signal must read hotter, so the colour ramp tracks level.
    #[test]
    fn intensity_increases_with_level() {
        let quiet = analyze(&tone(40.0, 0.05), RATE, 32).band_peaks()[0];
        let loud = analyze(&tone(40.0, 0.8), RATE, 32).band_peaks()[0];
        assert!(
            loud > quiet,
            "louder sub should read hotter: loud={loud}, quiet={quiet}"
        );
        assert!(loud <= 1.0 && quiet >= 0.0);
    }

    /// Energy below the display floor must not register.
    #[test]
    fn level_below_floor_reads_empty() {
        // -60 dBFS is the floor, so -100 dBFS must clamp to zero.
        let map = analyze(&tone(40.0, 0.00001), RATE, 32);
        assert_eq!(map.band_peaks()[0], 0.0);
    }

    /// Energy must land in the column where it actually occurs, so the
    /// heatmap lines up with the waveform above it.
    #[test]
    fn energy_is_placed_in_the_correct_column() {
        // Silent first half, tone in the second half.
        let mut samples = vec![0.0; (RATE * 0.25) as usize];
        samples.extend(tone(40.0, 0.8));

        let map = analyze(&samples, RATE, 32);
        let early = map.sample_at(0.1);
        let late = map.sample_at(0.9);
        assert_eq!(early[0], 0.0, "silent region should be empty");
        assert!(late[0] > 0.0, "tone region should hold energy");
    }

    #[test]
    fn sample_at_clamps_out_of_range_positions() {
        let map = analyze(&tone(40.0, 0.5), RATE, 16);
        assert_eq!(map.sample_at(-5.0), map.sample_at(0.0));
        assert_eq!(map.sample_at(5.0), map.sample_at(1.0));
    }

    /// Column count must never exceed the sample count, or columns would be
    /// left with nothing averaged into them and punch holes in the display.
    #[test]
    fn column_count_is_capped_to_sample_count() {
        let map = analyze(&[0.5, -0.5, 0.5], RATE, 1024);
        assert!(!map.is_empty());
        assert!(map.columns.len() <= 3);
    }

    /// Brightness is a spectral centroid, so it must rise as the signal's
    /// energy moves up the bands. This is the measurement the trace colour
    /// is generated from, so if it did not track content the colour would be
    /// meaningless.
    #[test]
    fn brightness_tracks_frequency_content() {
        let sub = analyze(&tone(40.0, 0.8), RATE, 32).brightness_at(0.9);
        let mid = analyze(&tone(300.0, 0.8), RATE, 32).brightness_at(0.9);
        let edge = analyze(&tone(1200.0, 0.8), RATE, 32).brightness_at(0.9);

        assert!(
            sub < mid && mid < edge,
            "brightness should rise with frequency: sub={sub}, mid={mid}, edge={edge}"
        );
        assert!((0.0..=1.0).contains(&sub) && (0.0..=1.0).contains(&edge));
    }

    /// Level must follow the signal's actual amplitude, since it drives the
    /// opacity and dot size.
    #[test]
    fn level_tracks_amplitude() {
        let quiet = analyze(&tone(40.0, 0.05), RATE, 32).level_at(0.9);
        let loud = analyze(&tone(40.0, 0.8), RATE, 32).level_at(0.9);
        assert!(loud > quiet, "louder signal should read hotter");
    }

    /// Silence must produce no brightness, rather than a default that would
    /// paint a colour onto nothing.
    #[test]
    fn silence_has_no_brightness_or_level() {
        let map = analyze(&vec![0.0; 1024], RATE, 32);
        assert_eq!(map.brightness_at(0.5), 0.0);
        assert_eq!(map.level_at(0.5), 0.0);
    }

    /// The same measurement must always produce the same colour, so the two
    /// visuals that share the ramp stay consistent with each other.
    #[test]
    fn signal_color_is_a_pure_function_of_the_measurement() {
        assert_eq!(signal_color(0.5, 0.5), signal_color(0.5, 0.5));
        let dull = signal_color(0.0, 1.0);
        let bright = signal_color(1.0, 1.0);
        assert!(
            bright.g() > dull.g(),
            "higher centroid should read brighter"
        );
        let quiet = signal_color(0.5, 0.0);
        let loud = signal_color(0.5, 1.0);
        assert!(loud.a() > quiet.a(), "louder should be more opaque");
    }

    #[test]
    fn heat_ramp_spans_empty_to_hot() {
        assert_eq!(heat_color(0.0), Color32::TRANSPARENT);
        let mid = heat_color(0.5);
        let hot = heat_color(1.0);
        assert!(hot.a() > mid.a(), "hotter cells should be more opaque");
        assert!(hot.r() >= mid.r(), "hotter cells should be brighter");
    }
}
