use std::process::Command;

use chrono::{Local, TimeZone};
use serde_json::Value;

use super::db::ActivityDb;

#[derive(Debug, Clone, Default)]
pub struct Sample {
    pub idle_ms: Option<u64>,
    pub locked: Option<bool>,
    pub app_id: Option<String>,
    pub app_name: Option<String>,
}

pub fn take_sample_once() -> Sample {
    let focus = probe_focus();
    Sample {
        idle_ms: probe_idle_ms(),
        locked: probe_locked(),
        app_id: focus.as_ref().map(|(id, _)| id.clone()),
        app_name: focus.map(|(_, name)| name),
    }
}

fn probe_idle_ms() -> Option<u64> {
    // 1) xprintidle (X11)
    if let Ok(out) = Command::new("xprintidle").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if let Ok(ms) = s.parse::<u64>() {
                return Some(ms);
            }
        }
    }
    // 2) GNOME Mutter IdleMonitor via gdbus
    if let Ok(out) = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Mutter.IdleMonitor",
            "--object-path",
            "/org/gnome/Mutter/IdleMonitor/Core",
            "--method",
            "org.gnome.Mutter.IdleMonitor.GetIdletime",
        ])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            // Typical: (uint64 12345,)
            if let Some(num) = text
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .ok()
            {
                return Some(num);
            }
        }
    }
    None
}

fn probe_locked() -> Option<bool> {
    // GNOME ScreenSaver
    if let Ok(out) = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.ScreenSaver",
            "--object-path",
            "/org/gnome/ScreenSaver",
            "--method",
            "org.gnome.ScreenSaver.GetActive",
        ])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            if text.contains("true") {
                return Some(true);
            }
            if text.contains("false") {
                return Some(false);
            }
        }
    }
    // freedesktop ScreenSaver
    if let Ok(out) = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.ScreenSaver",
            "--object-path",
            "/org/freedesktop/ScreenSaver",
            "--method",
            "org.freedesktop.ScreenSaver.GetActive",
        ])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
            if text.contains("true") {
                return Some(true);
            }
            if text.contains("false") {
                return Some(false);
            }
        }
    }
    // loginctl LockedHint for current session
    if let Ok(out) = Command::new("bash")
        .args([
            "-c",
            r#"SID=$(loginctl list-sessions --no-legend 2>/dev/null | awk '$3==ENVIRON["USER"]{print $1; exit}'); [ -n "$SID" ] && loginctl show-session "$SID" -p LockedHint --value 2>/dev/null"#,
        ])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_ascii_lowercase();
            if text == "yes" {
                return Some(true);
            }
            if text == "no" {
                return Some(false);
            }
        }
    }
    None
}

fn probe_focus() -> Option<(String, String)> {
    // Prefer xdotool (X11 / XWayland)
    if let Ok(out) = Command::new("xdotool")
        .args(["getactivewindow", "getwindowpid"])
        .output()
    {
        if out.status.success() {
            let pid = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if let Ok(pid_n) = pid.parse::<u32>() {
                return app_from_pid(pid_n);
            }
        }
    }
    // xprop fallback
    if let Ok(out) = Command::new("bash")
        .args([
            "-c",
            r#"WID=$(xprop -root _NET_ACTIVE_WINDOW 2>/dev/null | awk -F'# ' '{print $2}'); [ -n "$WID" ] && xprop -id "$WID" _NET_WM_PID 2>/dev/null | awk -F'= ' '{print $2}'"#,
        ])
        .output()
    {
        if out.status.success() {
            let pid = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if let Ok(pid_n) = pid.parse::<u32>() {
                return app_from_pid(pid_n);
            }
        }
    }
    None
}

fn app_from_pid(pid: u32) -> Option<(String, String)> {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()?
        .trim()
        .to_string();
    if comm.is_empty() {
        return None;
    }
    // Try desktop name via /proc/pid/exe basename
    let exe = std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().to_string()));
    let app_id = exe
        .clone()
        .unwrap_or_else(|| comm.clone())
        .to_ascii_lowercase()
        .replace(' ', "-");
    let app_name = pretty_app_name(&comm, exe.as_deref());
    Some((app_id, app_name))
}

fn pretty_app_name(comm: &str, exe: Option<&str>) -> String {
    let raw = exe.unwrap_or(comm);
    let base = raw.rsplit('/').next().unwrap_or(raw);
    // Strip common suffixes
    let base = base.trim_end_matches("-bin").trim_end_matches(".bin");
    // Title-ish
    let mut out = String::new();
    let mut cap = true;
    for ch in base.chars() {
        if ch == '-' || ch == '_' {
            out.push(' ');
            cap = true;
        } else if cap {
            out.extend(ch.to_uppercase());
            cap = false;
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() {
        comm.to_string()
    } else {
        out
    }
}

/// Pull boot/suspend/resume (and best-effort lock) events from journalctl.
pub fn ingest_journal_events(db: &ActivityDb) -> Result<u32, String> {
    let mut n = 0u32;
    n += ingest_boots(db)?;
    n += ingest_message_ids(
        db,
        "6bbd95ee977941e497c48be27c5045e0",
        "suspend",
        "System suspended",
    )?;
    n += ingest_message_ids(
        db,
        "8811e6df2a8e40f58a43cea26bf49da0",
        "resume",
        "System resumed",
    )?;
    n += ingest_grep_journal(
        db,
        &["Screen lock", "Screen locked", "Locked"],
        "lock",
        40,
    )?;
    n += ingest_grep_journal(
        db,
        &["Screen unlock", "Screen unlocked", "Unlocked"],
        "unlock",
        40,
    )?;
    Ok(n)
}

fn ingest_boots(db: &ActivityDb) -> Result<u32, String> {
    let out = Command::new("journalctl")
        .args(["--list-boots", "--no-pager"])
        .output()
        .map_err(|e| format!("journalctl list-boots: {e}"))?;
    if !out.status.success() {
        return Ok(0);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut count = 0u32;
    // Lines like:  0  abc...  Mon 2026-10-09 08:42:15 IST—Fri 2026-10-09 16:00:00 IST
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Parse first timestamp after boot id (ISO-ish in locale). Use journal JSON for current boot instead.
        let _ = line;
    }
    // More reliable: journalctl -b -o json -n 1 for each recent boot via --list-boots ids
    let out2 = Command::new("journalctl")
        .args([
            "-o",
            "json",
            "--since",
            "90 days ago",
            "-n",
            "5",
            "_PID=1",
            "MESSAGE_ID=b07a249cd024464a97da400ab1b75384", // systemd startup finished alternate; may miss
        ])
        .output();
    let _ = out2;

    // Use journalctl --list-boots and for each boot get first entry timestamp
    for idx in -5i32..=0 {
        let out = Command::new("journalctl")
            .args([
                "-b",
                &idx.to_string(),
                "-o",
                "json",
                "-n",
                "1",
                "--no-pager",
            ])
            .output();
        let Ok(out) = out else { continue };
        if !out.status.success() {
            continue;
        }
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(ts) = parse_journal_ts_line(text.lines().next().unwrap_or("")) {
            let _ = db.insert_system_event(ts, "boot", Some(&format!("Boot index {idx}")), false);
            count += 1;
        }
        // Last message of previous boot ≈ shutdown / crash boundary (inferred)
        if idx < 0 {
            let out_end = Command::new("journalctl")
                .args([
                    "-b",
                    &idx.to_string(),
                    "-o",
                    "json",
                    "-n",
                    "1",
                    "-r",
                    "--no-pager",
                ])
                .output();
            if let Ok(out_end) = out_end {
                if out_end.status.success() {
                    let text = String::from_utf8_lossy(&out_end.stdout);
                    if let Some(ts) = parse_journal_ts_line(text.lines().next().unwrap_or("")) {
                        let _ = db.insert_system_event(
                            ts,
                            "shutdown",
                            Some("Inferred from end of previous boot journal"),
                            true,
                        );
                        count += 1;
                    }
                }
            }
        }
    }
    Ok(count)
}

fn ingest_message_ids(
    db: &ActivityDb,
    message_id: &str,
    kind: &str,
    detail: &str,
) -> Result<u32, String> {
    let out = Command::new("journalctl")
        .args([
            "-o",
            "json",
            "--since",
            "90 days ago",
            "--no-pager",
            &format!("MESSAGE_ID={message_id}"),
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Ok(0);
    }
    let mut n = 0u32;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(ts) = parse_journal_ts_line(line) {
            let _ = db.insert_system_event(ts, kind, Some(detail), false);
            n += 1;
        }
    }
    Ok(n)
}

fn ingest_grep_journal(
    db: &ActivityDb,
    patterns: &[&str],
    kind: &str,
    limit: usize,
) -> Result<u32, String> {
    // Best-effort user session messages (may be noisy — we de-dupe by UNIQUE).
    let mut n = 0u32;
    for pat in patterns {
        let out = Command::new("journalctl")
            .args([
                "--user",
                "-o",
                "json",
                "--since",
                "90 days ago",
                "--no-pager",
                "-n",
                &limit.to_string(),
                "-g",
                pat,
            ])
            .output();
        let Ok(out) = out else { continue };
        if !out.status.success() {
            continue;
        }
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let msg = v
                .get("MESSAGE")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let relevant = match kind {
                "lock" => msg.contains("lock") && !msg.contains("unlock"),
                "unlock" => msg.contains("unlock"),
                _ => true,
            };
            if !relevant {
                continue;
            }
            if let Some(ts) = parse_journal_ts_value(&v) {
                let _ = db.insert_system_event(ts, kind, Some(pat), false);
                n += 1;
            }
        }
    }
    Ok(n)
}

fn parse_journal_ts_line(line: &str) -> Option<i64> {
    let v: Value = serde_json::from_str(line).ok()?;
    parse_journal_ts_value(&v)
}

fn parse_journal_ts_value(v: &Value) -> Option<i64> {
    // __REALTIME_TIMESTAMP is microseconds since epoch as string
    if let Some(us) = v
        .get("__REALTIME_TIMESTAMP")
        .and_then(|x| x.as_str())
        .and_then(|s| s.parse::<i64>().ok())
    {
        return Some(us / 1000);
    }
    if let Some(us) = v.get("__REALTIME_TIMESTAMP").and_then(|x| x.as_i64()) {
        return Some(us / 1000);
    }
    None
}

pub fn local_day_bounds(date: &str) -> Result<(i64, i64), String> {
    let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| format!("invalid date: {e}"))?;
    let start = d
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| "bad start".to_string())?;
    let end = (d + chrono::Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| "bad end".to_string())?;
    let start_ms = Local
        .from_local_datetime(&start)
        .single()
        .ok_or_else(|| "ambiguous start".to_string())?
        .timestamp_millis();
    let end_ms = Local
        .from_local_datetime(&end)
        .single()
        .ok_or_else(|| "ambiguous end".to_string())?
        .timestamp_millis();
    Ok((start_ms, end_ms))
}

pub fn today_local() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}
