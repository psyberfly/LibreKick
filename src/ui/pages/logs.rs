use nih_plug_egui::egui::{self, Color32, RichText};
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

use crate::common::logger::LOGGER;

use super::super::state::BezierUiState;

fn parse_log_line(line: &str) -> (Option<i128>, &str, &str) {
    let mut parts = line.splitn(3, "] ");
    let ts_ms = parts
        .next()
        .and_then(|value| value.trim_start_matches('[').trim().parse::<i128>().ok());
    let kind = parts
        .next()
        .map(|value| value.trim_start_matches('[').trim())
        .unwrap_or("DEBUG");
    let message = parts.next().unwrap_or(line).trim();
    (ts_ms, kind, message)
}

fn format_time_utc_to_local(ts_ms: Option<i128>) -> String {
    let Some(ts_ms) = ts_ms else {
        return "UTC ?".to_owned();
    };

    let nanos = ts_ms.saturating_mul(1_000_000);
    let Ok(utc) = OffsetDateTime::from_unix_timestamp_nanos(nanos) else {
        return "UTC ?".to_owned();
    };

    if let Some(local_offset) = resolve_local_offset(utc) {
        let local = utc.to_offset(local_offset);
        let local_text = local.format(&Rfc3339).unwrap_or_else(|_| "?".to_owned());
        return format!("LOCAL {local_text}");
    }

    let utc_text = utc.format(&Rfc3339).unwrap_or_else(|_| "?".to_owned());
    format!("UTC {utc_text}")
}

fn resolve_local_offset(utc: OffsetDateTime) -> Option<UtcOffset> {
    if let Ok(offset) = UtcOffset::local_offset_at(utc) {
        return Some(offset);
    }

    #[cfg(target_os = "linux")]
    {
        let timestamp: libc::time_t = utc.unix_timestamp() as libc::time_t;
        let mut local_tm = std::mem::MaybeUninit::<libc::tm>::uninit();
        let local_tm_ptr = unsafe { libc::localtime_r(&timestamp, local_tm.as_mut_ptr()) };
        if !local_tm_ptr.is_null() {
            let local_tm = unsafe { local_tm.assume_init() };
            if let Ok(offset) = UtcOffset::from_whole_seconds(local_tm.tm_gmtoff as i32) {
                return Some(offset);
            }
        }
    }

    None
}

fn color_for_log_kind(kind: &str) -> Color32 {
    match kind {
        "ERROR" => Color32::from_rgb(240, 84, 84),
        "DEBUG" => Color32::from_rgb(114, 220, 129),
        "ENTER" | "LEAVE" => Color32::from_rgb(96, 174, 255),
        _ => Color32::from_rgb(165, 171, 178),
    }
}

pub(crate) fn render(ui: &mut egui::Ui, ui_scale: f32, _state: &mut BezierUiState) {
    ui.add_space(8.0 * ui_scale);
    ui.heading("Logs");
    ui.label(
        RichText::new("Session log stream (enter/debug/error/leave)")
            .italics()
            .small(),
    );
    ui.separator();

    let snapshot = LOGGER.snapshot();

    ui.horizontal(|ui| {
        ui.label(format!(
            "Entries: {}  •  Sequence: {}",
            snapshot.lines.len(),
            snapshot.sequence
        ));
        if ui.button("Clear").clicked() {
            LOGGER.clear();
        }
    });

    ui.separator();

    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for line in snapshot.lines.iter() {
                let (ts_ms, kind, message) = parse_log_line(line);
                let time_text = format_time_utc_to_local(ts_ms);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(time_text)
                            .monospace()
                            .color(Color32::from_rgb(245, 239, 214)),
                    );
                    ui.label(RichText::new("|").monospace().color(Color32::from_rgb(165, 171, 178)));
                    ui.label(
                        RichText::new(kind)
                            .monospace()
                            .strong()
                            .color(color_for_log_kind(kind)),
                    );
                    ui.label(RichText::new("|").monospace().color(Color32::from_rgb(165, 171, 178)));
                    ui.label(
                        RichText::new(message)
                            .monospace()
                            .color(Color32::from_rgb(245, 239, 214)),
                    );
                });
            }
        });
}
