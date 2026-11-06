//! Activity Tracker — local system/session/app usage monitoring.
//!
//! Continuous focus/idle/lock sampling runs while the DevMonitor process is alive
//! (including when the window is closed to the system tray).
//! Boot, suspend, resume, and some lock events are also reconstructed from the
//! systemd journal. Gaps are marked `unknown`.

mod collector;
mod db;
mod probes;
mod summarize;
mod types;

pub use types::*;

use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use collector::Collector;
use db::ActivityDb;

static DB_PATH: OnceLock<std::path::PathBuf> = OnceLock::new();

fn db_path() -> &'static std::path::PathBuf {
    DB_PATH.get_or_init(|| {
        let base = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("devmonitor");
        let _ = std::fs::create_dir_all(&base);
        base.join("activity.db")
    })
}

pub fn open_db() -> Result<ActivityDb, String> {
    ActivityDb::open(db_path())
}

/// Start the background sampler (once). Safe to call from Tauri setup.
pub fn start_collector() {
    thread::Builder::new()
        .name("activity-collector".into())
        .spawn(|| {
            // Brief delay so the UI can finish startup.
            thread::sleep(Duration::from_secs(2));
            let Ok(db) = open_db() else {
                return;
            };
            let _ = db.insert_system_event(
                chrono::Local::now().timestamp_millis(),
                "monitor_start",
                Some("Activity collector started"),
                false,
            );
            let mut collector = Collector::new(db);
            collector.run_forever();
        })
        .ok();
}

pub fn get_day(date: Option<String>) -> Result<ActivityDay, String> {
    let db = open_db()?;
    // Refresh journal into DB (best-effort) before summarizing.
    let _ = probes::ingest_journal_events(&db);
    let _ = db.prune_older_than_days(90);
    summarize::build_day(&db, date.as_deref())
}

pub fn get_settings() -> Result<ActivitySettings, String> {
    let db = open_db()?;
    Ok(db.get_settings())
}

pub fn set_settings(settings: ActivitySettings) -> Result<ActivitySettings, String> {
    let db = open_db()?;
    db.set_settings(&settings)?;
    Ok(db.get_settings())
}

pub fn clear_history() -> Result<FixOk, String> {
    let db = open_db()?;
    db.clear_all()?;
    let _ = db.insert_system_event(
        chrono::Local::now().timestamp_millis(),
        "monitor_start",
        Some("History cleared; monitoring continues"),
        false,
    );
    Ok(FixOk {
        success: true,
        message: "Activity history cleared (last 90 days)".into(),
    })
}

pub fn available_dates() -> Result<Vec<String>, String> {
    let db = open_db()?;
    Ok(db.list_dates(90))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixOk {
    pub success: bool,
    pub message: String,
}
