use std::collections::HashMap;

use chrono::Local;

use super::db::ActivityDb;
use super::probes::{local_day_bounds, today_local};
use super::types::*;

pub fn build_day(db: &ActivityDb, date: Option<&str>) -> Result<ActivityDay, String> {
    let date = date
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(today_local);

    // Reject future dates
    let today = today_local();
    if date > today {
        return Err("Cannot select a future date".into());
    }

    let (day_start, day_end) = local_day_bounds(&date)?;
    let now = Local::now().timestamp_millis();
    let effective_end = if date == today {
        now.min(day_end)
    } else {
        day_end
    };

    let events_raw = db.events_between(day_start, day_end);
    let mut measured = db.state_intervals_for_day(&date);
    // Clip to day bounds
    measured = measured
        .into_iter()
        .filter_map(|(s, e, state, source, detail)| {
            let s = s.max(day_start);
            let e = e.min(effective_end);
            if e > s {
                Some((s, e, state, source, detail))
            } else {
                None
            }
        })
        .collect();

    let timeline = build_timeline(day_start, effective_end, &events_raw, &measured);
    let mut overview = overview_from_timeline(&timeline, &events_raw);
    let apps = build_apps(db, &date, day_start, effective_end, &timeline);
    overview.foreground_ms = apps.iter().map(|a| a.foreground_ms).sum();
    overview.background_ms = apps.iter().map(|a| a.background_ms).sum();
    let events: Vec<SystemEventRow> = events_raw
        .iter()
        .map(|(ts, kind, detail, inferred)| SystemEventRow {
            ts_ms: *ts,
            kind: kind.clone(),
            detail: detail.clone(),
            inferred: *inferred,
        })
        .collect();

    let measured_ms: i64 = measured.iter().map(|(s, e, ..)| e - s).sum();
    let available_ms = overview.uptime_ms.max(1);
    let monitored_ratio = (measured_ms as f64 / available_ms as f64).clamp(0.0, 1.0);

    let incomplete = monitored_ratio < 0.85
        || overview.unknown_ms > 60_000
        || (date == today && measured.is_empty() && events.is_empty());

    let status = if measured.is_empty() && events.is_empty() {
        "No tracking data for this day yet".into()
    } else if incomplete {
        "Partial coverage — some intervals are unknown or estimated".into()
    } else {
        "Tracking data available".into()
    };

    let mut notes = vec![
        "Active interaction uses keyboard/mouse idle signals only (no keystroke content).".into(),
        "Foreground/background app times are recorded while DevMonitor is running.".into(),
        "Boot, suspend, and resume come from the systemd journal when available.".into(),
        "Suspended and powered-off time is excluded from uptime and app runtime.".into(),
        "Browser domain tracking is opt-in and requires a browser extension (not installed by default).".into(),
    ];
    if overview.active_is_estimate {
        notes.push(
            "Idle probe unavailable for part of the day — active time may be incomplete.".into(),
        );
    }
    if overview.background_is_estimate {
        notes.push(
            "Background runtime is estimated from apps observed while monitoring.".into(),
        );
    }

    Ok(ActivityDay {
        date,
        timezone: Local::now().format("%Z").to_string(),
        tracking: TrackingCoverage {
            monitored_ratio,
            has_journal_events: events.iter().any(|e| {
                matches!(
                    e.kind.as_str(),
                    "boot" | "shutdown" | "suspend" | "resume"
                )
            }),
            has_samples: !measured.is_empty(),
            incomplete,
            status,
        },
        overview,
        timeline,
        apps,
        events,
        legend: legend(),
        notes,
    })
}

fn legend() -> Vec<LegendItem> {
    vec![
        LegendItem {
            state: STATE_ACTIVE.into(),
            label: "Active interaction".into(),
            color_token: "active".into(),
        },
        LegendItem {
            state: STATE_AVAILABLE.into(),
            label: "System available".into(),
            color_token: "available".into(),
        },
        LegendItem {
            state: STATE_IDLE.into(),
            label: "User idle".into(),
            color_token: "idle".into(),
        },
        LegendItem {
            state: STATE_LOCKED.into(),
            label: "Session locked".into(),
            color_token: "locked".into(),
        },
        LegendItem {
            state: STATE_SUSPENDED.into(),
            label: "Suspended".into(),
            color_token: "suspended".into(),
        },
        LegendItem {
            state: STATE_POWERED_OFF.into(),
            label: "Powered off".into(),
            color_token: "powered".into(),
        },
        LegendItem {
            state: STATE_UNKNOWN.into(),
            label: "Unknown / unrecorded".into(),
            color_token: "unknown".into(),
        },
    ]
}

fn build_timeline(
    day_start: i64,
    day_end: i64,
    events: &[(i64, String, Option<String>, bool)],
    measured: &[(i64, i64, String, String, Option<String>)],
) -> Vec<TimelineSegment> {
    // Start with full-day unknown, then overlay journal power states, then measured samples.
    let mut layers: Vec<(i64, i64, String, String, Option<String>)> = vec![(
        day_start,
        day_end,
        STATE_UNKNOWN.into(),
        "estimated".into(),
        Some("No sample for this interval".into()),
    )];

    // Powered off / suspended from event pairs
    layers.extend(power_intervals(day_start, day_end, events));

    // Measured overlays win
    for (s, e, state, source, detail) in measured {
        layers.push((
            *s,
            *e,
            state.clone(),
            source.clone(),
            detail.clone(),
        ));
    }

    let merged = flatten_intervals(day_start, day_end, &layers);
    merged
        .into_iter()
        .map(|(s, e, state, source, detail)| TimelineSegment {
            start_ms: s,
            end_ms: e,
            duration_ms: e - s,
            label: state_label(&state),
            state,
            source,
            detail,
        })
        .collect()
}

fn power_intervals(
    day_start: i64,
    day_end: i64,
    events: &[(i64, String, Option<String>, bool)],
) -> Vec<(i64, i64, String, String, Option<String>)> {
    let mut out = Vec::new();
    let mut last_suspend: Option<i64> = None;
    let mut last_shutdown: Option<(i64, bool)> = None;

    // Before first boot of the day → powered off (if we have a boot)
    if let Some((boot_ts, _, _, _)) = events.iter().find(|e| e.1 == "boot") {
        if *boot_ts > day_start {
            out.push((
                day_start,
                *boot_ts,
                STATE_POWERED_OFF.into(),
                "inferred".into(),
                Some("Before first recorded boot".into()),
            ));
        }
    }

    for (ts, kind, detail, inferred) in events {
        match kind.as_str() {
            "suspend" => last_suspend = Some(*ts),
            "resume" => {
                if let Some(s) = last_suspend.take() {
                    let a = s.max(day_start);
                    let b = (*ts).min(day_end);
                    if b > a {
                        out.push((
                            a,
                            b,
                            STATE_SUSPENDED.into(),
                            if *inferred { "inferred" } else { "measured" }.into(),
                            detail.clone(),
                        ));
                    }
                }
            }
            "shutdown" => last_shutdown = Some((*ts, *inferred)),
            "boot" => {
                if let Some((s, inferred)) = last_shutdown.take() {
                    let a = s.max(day_start);
                    let b = (*ts).min(day_end);
                    if b > a {
                        out.push((
                            a,
                            b,
                            STATE_POWERED_OFF.into(),
                            if inferred { "inferred" } else { "measured" }.into(),
                            Some("Between shutdown and next boot".into()),
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    // Suspend without resume until end of day / now
    if let Some(s) = last_suspend {
        let a = s.max(day_start);
        if day_end > a {
            out.push((
                a,
                day_end,
                STATE_SUSPENDED.into(),
                "inferred".into(),
                Some("Suspended; resume not yet recorded".into()),
            ));
        }
    }

    // Shutdown without later boot
    if let Some((s, inferred)) = last_shutdown {
        let a = s.max(day_start);
        if day_end > a {
            out.push((
                a,
                day_end,
                STATE_POWERED_OFF.into(),
                if inferred { "inferred" } else { "measured" }.into(),
                Some("After shutdown".into()),
            ));
        }
    }

    out
}

/// Later layers overwrite earlier ones (paint order).
fn flatten_intervals(
    day_start: i64,
    day_end: i64,
    layers: &[(i64, i64, String, String, Option<String>)],
) -> Vec<(i64, i64, String, String, Option<String>)> {
    if day_end <= day_start {
        return vec![];
    }
    // Build change points
    let mut points = vec![day_start, day_end];
    for (s, e, ..) in layers {
        if *s > day_start && *s < day_end {
            points.push(*s);
        }
        if *e > day_start && *e < day_end {
            points.push(*e);
        }
    }
    points.sort_unstable();
    points.dedup();

    let mut out = Vec::new();
    for w in points.windows(2) {
        let a = w[0];
        let b = w[1];
        if b <= a {
            continue;
        }
        let mid = a + (b - a) / 2;
        // Top-most layer covering mid
        let mut chosen: Option<(String, String, Option<String>)> = None;
        for (s, e, state, source, detail) in layers {
            if *s <= mid && mid < *e {
                chosen = Some((state.clone(), source.clone(), detail.clone()));
            }
        }
        let (state, source, detail) = chosen.unwrap_or_else(|| {
            (
                STATE_UNKNOWN.into(),
                "estimated".into(),
                Some("Unrecorded".into()),
            )
        });
        if let Some(last) = out.last_mut() {
            let (ls, _le, lstate, lsource, _ld): &mut (i64, i64, String, String, Option<String>) =
                last;
            if *lstate == state && *lsource == source {
                last.1 = b;
                continue;
            }
            let _ = ls;
        }
        out.push((a, b, state, source, detail));
    }
    out
}

fn overview_from_timeline(
    timeline: &[TimelineSegment],
    events: &[(i64, String, Option<String>, bool)],
) -> DayOverview {
    let mut uptime = 0i64;
    let mut active = 0i64;
    let mut idle = 0i64;
    let mut locked = 0i64;
    let mut suspended = 0i64;
    let mut powered = 0i64;
    let mut unknown = 0i64;
    let mut had_available_without_idle = false;

    for seg in timeline {
        match seg.state.as_str() {
            STATE_ACTIVE => {
                active += seg.duration_ms;
                uptime += seg.duration_ms;
            }
            STATE_IDLE => {
                idle += seg.duration_ms;
                uptime += seg.duration_ms;
            }
            STATE_LOCKED => {
                locked += seg.duration_ms;
                uptime += seg.duration_ms;
            }
            STATE_AVAILABLE => {
                uptime += seg.duration_ms;
                had_available_without_idle = true;
            }
            STATE_SUSPENDED => suspended += seg.duration_ms,
            STATE_POWERED_OFF => powered += seg.duration_ms,
            _ => unknown += seg.duration_ms,
        }
    }

    let first_boot = events
        .iter()
        .find(|e| e.1 == "boot")
        .map(|e| e.0);
    let last_end = events
        .iter()
        .rev()
        .find(|e| matches!(e.1.as_str(), "shutdown" | "suspend" | "lock" | "monitor_stop"))
        .map(|e| (e.0, e.1.clone()));

    DayOverview {
        uptime_ms: uptime,
        first_boot_ms: first_boot,
        last_end_ms: last_end.as_ref().map(|e| e.0),
        last_end_kind: last_end.map(|e| e.1),
        active_ms: active,
        idle_ms: idle,
        locked_ms: locked,
        suspended_ms: suspended,
        powered_off_ms: powered,
        unknown_ms: unknown,
        foreground_ms: 0, // filled by caller via apps
        background_ms: 0,
        active_is_estimate: had_available_without_idle,
        background_is_estimate: true,
    }
}

fn build_apps(
    db: &ActivityDb,
    date: &str,
    day_start: i64,
    day_end: i64,
    timeline: &[TimelineSegment],
) -> Vec<AppUsageRow> {
    let rows = db.app_intervals_for_day(date);
    let suspend_off: Vec<(i64, i64)> = timeline
        .iter()
        .filter(|s| s.state == STATE_SUSPENDED || s.state == STATE_POWERED_OFF)
        .map(|s| (s.start_ms, s.end_ms))
        .collect();

    let mut map: HashMap<String, AppAcc> = HashMap::new();
    for (s, e, app_id, app_name, kind, active_ms) in rows {
        let s = s.max(day_start);
        let e = e.min(day_end);
        if e <= s {
            continue;
        }
        let dur = subtract_ranges(s, e, &suspend_off);
        if dur <= 0 {
            continue;
        }
        let acc = map.entry(app_id.clone()).or_insert_with(|| AppAcc {
            app_id: app_id.clone(),
            app_name: app_name.clone(),
            first: s,
            last: e,
            sessions: 0,
            foreground: 0,
            background: 0,
            active: 0,
        });
        acc.app_name = app_name;
        acc.first = acc.first.min(s);
        acc.last = acc.last.max(e);
        if kind == "foreground" {
            acc.foreground += dur;
            acc.sessions += 1;
            acc.active += active_ms.min(dur);
        } else {
            acc.background += dur;
        }
    }

    let mut list: Vec<AppUsageRow> = map
        .into_values()
        .map(|a| AppUsageRow {
            app_id: a.app_id,
            app_name: a.app_name,
            first_ms: a.first,
            last_ms: a.last,
            sessions: a.sessions,
            foreground_ms: a.foreground,
            background_ms: a.background,
            active_ms: a.active,
            background_is_estimate: true,
        })
        .collect();
    list.sort_by(|a, b| b.foreground_ms.cmp(&a.foreground_ms));
    list
}

struct AppAcc {
    app_id: String,
    app_name: String,
    first: i64,
    last: i64,
    sessions: u32,
    foreground: i64,
    background: i64,
    active: i64,
}

fn subtract_ranges(start: i64, end: i64, cut: &[(i64, i64)]) -> i64 {
    let mut total = end - start;
    for (a, b) in cut {
        let lo = (*a).max(start);
        let hi = (*b).min(end);
        if hi > lo {
            total -= hi - lo;
        }
    }
    total.max(0)
}

fn state_label(state: &str) -> String {
    match state {
        STATE_ACTIVE => "Active interaction".into(),
        STATE_AVAILABLE => "System available".into(),
        STATE_IDLE => "User idle".into(),
        STATE_LOCKED => "Session locked".into(),
        STATE_SUSPENDED => "Suspended".into(),
        STATE_POWERED_OFF => "Powered off".into(),
        _ => "Unknown / unrecorded".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtract_suspend_from_app_runtime() {
        let cut = vec![(100, 200)];
        assert_eq!(subtract_ranges(0, 300, &cut), 200);
        assert_eq!(subtract_ranges(100, 200, &cut), 0);
    }

    #[test]
    fn flatten_prefers_later_layers() {
        let layers = vec![
            (0, 100, STATE_UNKNOWN.into(), "estimated".into(), None),
            (20, 50, STATE_ACTIVE.into(), "measured".into(), None),
        ];
        let merged = flatten_intervals(0, 100, &layers);
        let active: i64 = merged
            .iter()
            .filter(|s| s.2 == STATE_ACTIVE)
            .map(|s| s.1 - s.0)
            .sum();
        assert_eq!(active, 30);
    }

    #[test]
    fn power_off_before_boot() {
        let events = vec![(50_000i64, "boot".into(), None, false)];
        let intervals = power_intervals(0, 100_000, &events);
        assert!(intervals.iter().any(|i| i.2 == STATE_POWERED_OFF && i.0 == 0));
    }
}
