use std::thread;
use std::time::Duration;

use chrono::Local;

use super::db::ActivityDb;
use super::probes::{self, Sample};
use super::types::{STATE_ACTIVE, STATE_AVAILABLE, STATE_IDLE, STATE_LOCKED};

pub struct Collector {
    db: ActivityDb,
    last: Option<Tracked>,
    ticks: u64,
}

struct Tracked {
    ts_ms: i64,
    state: String,
    app_id: Option<String>,
    app_name: Option<String>,
    was_active_input: bool,
}

impl Collector {
    pub fn new(db: ActivityDb) -> Self {
        Self {
            db,
            last: None,
            ticks: 0,
        }
    }

    pub fn run_forever(&mut self) {
        loop {
            let settings = self.db.get_settings();
            if settings.tracking_enabled {
                self.tick(settings.idle_threshold_secs);
            }
            self.ticks += 1;
            // Journal ingest every ~2 minutes
            if self.ticks % 24 == 1 {
                let _ = probes::ingest_journal_events(&self.db);
            }
            // Retention hourly
            if self.ticks % 720 == 1 {
                let _ = self.db.prune_older_than_days(90);
            }
            thread::sleep(Duration::from_secs(5));
        }
    }

    fn tick(&mut self, idle_threshold_secs: u32) {
        let now = Local::now().timestamp_millis();
        let day = Local::now().format("%Y-%m-%d").to_string();
        let sample = probes::take_sample_once();
        let state = classify(&sample, idle_threshold_secs);
        let input_active = matches!(
            sample.idle_ms,
            Some(ms) if ms < (idle_threshold_secs as u64) * 1000
        ) && sample.locked != Some(true);

        let prev_locked = self
            .last
            .as_ref()
            .map(|l| l.state == STATE_LOCKED);

        if let Some(prev) = self.last.take() {
            let gap = now - prev.ts_ms;
            // Large gap → monitoring interruption / sleep without journal yet
            if gap > 90_000 {
                let _ = self.db.insert_system_event(
                    prev.ts_ms,
                    "monitor_interrupted",
                    Some("Gap between samples exceeded 90s"),
                    true,
                );
                let mid_day = chrono::DateTime::from_timestamp_millis(prev.ts_ms)
                    .map(|t| t.with_timezone(&Local).format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| day.clone());
                let _ = self.db.upsert_state_interval(
                    &mid_day,
                    prev.ts_ms,
                    now.min(prev.ts_ms + gap),
                    "unknown",
                    "estimated",
                    Some("Monitoring gap"),
                );
            } else {
                let _ = self.db.upsert_state_interval(
                    &day_for(prev.ts_ms, &day),
                    prev.ts_ms,
                    now,
                    &prev.state,
                    "measured",
                    None,
                );
                if let (Some(id), Some(name)) = (&prev.app_id, &prev.app_name) {
                    let active_ms = if prev.was_active_input && prev.state == STATE_ACTIVE
                    {
                        now - prev.ts_ms
                    } else {
                        0
                    };
                    let _ = self.db.upsert_app_interval(
                        &day_for(prev.ts_ms, &day),
                        prev.ts_ms,
                        now,
                        id,
                        name,
                        "foreground",
                        active_ms,
                    );
                    let _ = self.db.touch_open_app(id, name, now);

                    // Background estimate: other recently open apps
                    for (oid, oname, _first, last) in self.db.list_open_apps() {
                        if oid == *id {
                            continue;
                        }
                        if now - last > 120_000 {
                            continue;
                        }
                        let _ = self.db.upsert_app_interval(
                            &day,
                            prev.ts_ms,
                            now,
                            &oid,
                            &oname,
                            "background",
                            0,
                        );
                    }
                }
            }
        } else {
            let _ = self.db.insert_system_event(
                now,
                "monitor_start",
                Some("Sampler tick started"),
                false,
            );
        }

        // Lock transitions as events
        if let Some(locked) = sample.locked {
            let changed = prev_locked.map(|was| was != locked).unwrap_or(true);
            if changed {
                let kind = if locked { "lock" } else { "unlock" };
                let _ = self.db.insert_system_event(now, kind, None, false);
            }
        }

        self.last = Some(Tracked {
            ts_ms: now,
            state,
            app_id: sample.app_id,
            app_name: sample.app_name,
            was_active_input: input_active,
        });

        // Drop stale open apps (not seen for 10 minutes)
        let _ = self.db.prune_open_apps_before(now - 600_000);
    }
}

fn classify(sample: &Sample, idle_threshold_secs: u32) -> String {
    if sample.locked == Some(true) {
        return STATE_LOCKED.into();
    }
    match sample.idle_ms {
        Some(ms) if ms >= (idle_threshold_secs as u64) * 1000 => STATE_IDLE.into(),
        Some(_) => STATE_ACTIVE.into(),
        None => {
            // No idle probe — mark available (not inventing active/idle)
            STATE_AVAILABLE.into()
        }
    }
}

fn day_for(ts_ms: i64, fallback: &str) -> String {
    chrono::DateTime::from_timestamp_millis(ts_ms)
        .map(|t| t.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| fallback.to_string())
}
