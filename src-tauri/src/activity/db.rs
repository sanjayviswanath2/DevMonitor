use std::path::Path;
use std::sync::Mutex;

use chrono::{Duration, Local, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};

use super::types::ActivitySettings;

pub struct ActivityDb {
    conn: Mutex<Connection>,
}

impl ActivityDb {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(|e| format!("activity db open: {e}"))?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            PRAGMA foreign_keys=ON;

            CREATE TABLE IF NOT EXISTS settings (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS system_events (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              ts_ms INTEGER NOT NULL,
              kind TEXT NOT NULL,
              detail TEXT,
              inferred INTEGER NOT NULL DEFAULT 0,
              UNIQUE(ts_ms, kind)
            );

            CREATE TABLE IF NOT EXISTS state_intervals (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              day TEXT NOT NULL,
              start_ms INTEGER NOT NULL,
              end_ms INTEGER NOT NULL,
              state TEXT NOT NULL,
              source TEXT NOT NULL,
              detail TEXT
            );

            CREATE TABLE IF NOT EXISTS app_intervals (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              day TEXT NOT NULL,
              start_ms INTEGER NOT NULL,
              end_ms INTEGER NOT NULL,
              app_id TEXT NOT NULL,
              app_name TEXT NOT NULL,
              kind TEXT NOT NULL,
              active_ms INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS open_apps (
              app_id TEXT PRIMARY KEY,
              app_name TEXT NOT NULL,
              first_ms INTEGER NOT NULL,
              last_ms INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_events_ts ON system_events(ts_ms);
            CREATE INDEX IF NOT EXISTS idx_state_day ON state_intervals(day);
            CREATE INDEX IF NOT EXISTS idx_app_day ON app_intervals(day);
            "#,
        )
        .map_err(|e| format!("activity db migrate: {e}"))?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        // Seed defaults if missing.
        let s = db.get_settings();
        let _ = db.set_settings(&s);
        Ok(db)
    }

    pub fn get_settings(&self) -> ActivitySettings {
        let conn = self.conn.lock().unwrap();
        let mut s = ActivitySettings::default();
        let mut stmt = match conn.prepare("SELECT key, value FROM settings") {
            Ok(st) => st,
            Err(_) => return s,
        };
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        });
        if let Ok(rows) = rows {
            for r in rows.flatten() {
                match r.0.as_str() {
                    "idle_threshold_secs" => {
                        if let Ok(v) = r.1.parse() {
                            s.idle_threshold_secs = v;
                        }
                    }
                    "store_window_titles" => s.store_window_titles = r.1 == "1" || r.1 == "true",
                    "browser_domain_tracking" => {
                        s.browser_domain_tracking = r.1 == "1" || r.1 == "true"
                    }
                    "tracking_enabled" => s.tracking_enabled = r.1 != "0" && r.1 != "false",
                    _ => {}
                }
            }
        }
        s
    }

    pub fn set_settings(&self, s: &ActivitySettings) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        let pairs = [
            ("idle_threshold_secs", s.idle_threshold_secs.to_string()),
            (
                "store_window_titles",
                if s.store_window_titles { "1" } else { "0" }.into(),
            ),
            (
                "browser_domain_tracking",
                if s.browser_domain_tracking { "1" } else { "0" }.into(),
            ),
            (
                "tracking_enabled",
                if s.tracking_enabled { "1" } else { "0" }.into(),
            ),
        ];
        for (k, v) in pairs {
            conn.execute(
                "INSERT INTO settings(key,value) VALUES(?1,?2)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![k, v],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn insert_system_event(
        &self,
        ts_ms: i64,
        kind: &str,
        detail: Option<&str>,
        inferred: bool,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO system_events(ts_ms, kind, detail, inferred)
             VALUES(?1, ?2, ?3, ?4)",
            params![ts_ms, kind, detail, if inferred { 1 } else { 0 }],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn events_between(&self, start_ms: i64, end_ms: i64) -> Vec<(i64, String, Option<String>, bool)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT ts_ms, kind, detail, inferred FROM system_events
             WHERE ts_ms >= ?1 AND ts_ms < ?2 ORDER BY ts_ms ASC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![start_ms, end_ms], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get::<_, i64>(3)? != 0,
            ))
        })
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn upsert_state_interval(
        &self,
        day: &str,
        start_ms: i64,
        end_ms: i64,
        state: &str,
        source: &str,
        detail: Option<&str>,
    ) -> Result<(), String> {
        if end_ms <= start_ms {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        // Merge into open interval with same state/day if contiguous.
        let open: Option<(i64, i64)> = conn
            .query_row(
                "SELECT id, end_ms FROM state_intervals
                 WHERE day=?1 AND state=?2 AND source=?3
                 ORDER BY end_ms DESC LIMIT 1",
                params![day, state, source],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if let Some((id, prev_end)) = open {
            if (start_ms - prev_end).abs() <= 8_000 {
                conn.execute(
                    "UPDATE state_intervals SET end_ms=?1, detail=COALESCE(?2, detail) WHERE id=?3",
                    params![end_ms.max(prev_end), detail, id],
                )
                .map_err(|e| e.to_string())?;
                return Ok(());
            }
        }

        conn.execute(
            "INSERT INTO state_intervals(day, start_ms, end_ms, state, source, detail)
             VALUES(?1,?2,?3,?4,?5,?6)",
            params![day, start_ms, end_ms, state, source, detail],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn state_intervals_for_day(
        &self,
        day: &str,
    ) -> Vec<(i64, i64, String, String, Option<String>)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT start_ms, end_ms, state, source, detail FROM state_intervals
             WHERE day=?1 ORDER BY start_ms ASC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![day], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn upsert_app_interval(
        &self,
        day: &str,
        start_ms: i64,
        end_ms: i64,
        app_id: &str,
        app_name: &str,
        kind: &str,
        active_ms: i64,
    ) -> Result<(), String> {
        if end_ms <= start_ms {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let open: Option<(i64, i64, i64)> = conn
            .query_row(
                "SELECT id, end_ms, active_ms FROM app_intervals
                 WHERE day=?1 AND app_id=?2 AND kind=?3
                 ORDER BY end_ms DESC LIMIT 1",
                params![day, app_id, kind],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;

        if let Some((id, prev_end, prev_active)) = open {
            if (start_ms - prev_end).abs() <= 8_000 {
                conn.execute(
                    "UPDATE app_intervals SET end_ms=?1, active_ms=?2 WHERE id=?3",
                    params![end_ms.max(prev_end), prev_active + active_ms, id],
                )
                .map_err(|e| e.to_string())?;
                return Ok(());
            }
        }

        conn.execute(
            "INSERT INTO app_intervals(day, start_ms, end_ms, app_id, app_name, kind, active_ms)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![day, start_ms, end_ms, app_id, app_name, kind, active_ms],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn app_intervals_for_day(
        &self,
        day: &str,
    ) -> Vec<(i64, i64, String, String, String, i64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT start_ms, end_ms, app_id, app_name, kind, active_ms FROM app_intervals
             WHERE day=?1 ORDER BY start_ms ASC",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![day], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn touch_open_app(&self, app_id: &str, app_name: &str, ts_ms: i64) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO open_apps(app_id, app_name, first_ms, last_ms)
             VALUES(?1,?2,?3,?3)
             ON CONFLICT(app_id) DO UPDATE SET
               app_name=excluded.app_name,
               last_ms=excluded.last_ms",
            params![app_id, app_name, ts_ms],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_open_apps(&self) -> Vec<(String, String, i64, i64)> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT app_id, app_name, first_ms, last_ms FROM open_apps",
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    pub fn prune_open_apps_before(&self, cutoff_ms: i64) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM open_apps WHERE last_ms < ?1", params![cutoff_ms])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_dates(&self, max_days: i64) -> Vec<String> {
        let cutoff_date = (Local::now() - Duration::days(max_days)).date_naive();
        let cutoff = cutoff_date.format("%Y-%m-%d").to_string();
        let cutoff_ms = Local
            .from_local_datetime(&cutoff_date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .map(|d| d.timestamp_millis())
            .unwrap_or(0);
        let conn = self.conn.lock().unwrap();
        let mut dates = std::collections::BTreeSet::new();

        if let Ok(mut stmt) = conn.prepare(
            "SELECT DISTINCT day FROM state_intervals WHERE day >= ?1
             UNION SELECT DISTINCT day FROM app_intervals WHERE day >= ?1",
        ) {
            if let Ok(rows) = stmt.query_map(params![cutoff.as_str()], |row| row.get::<_, String>(0))
            {
                for d in rows.flatten() {
                    dates.insert(d);
                }
            }
        }

        if let Ok(mut stmt) = conn.prepare(
            "SELECT DISTINCT strftime('%Y-%m-%d', ts_ms/1000, 'unixepoch', 'localtime')
             FROM system_events WHERE ts_ms >= ?1",
        ) {
            if let Ok(rows) = stmt.query_map(params![cutoff_ms], |row| row.get::<_, String>(0)) {
                for d in rows.flatten() {
                    dates.insert(d);
                }
            }
        }

        dates.insert(Local::now().format("%Y-%m-%d").to_string());
        dates.into_iter().rev().collect()
    }

    pub fn prune_older_than_days(&self, days: i64) -> Result<u64, String> {
        let cutoff = Local::now() - Duration::days(days);
        let cutoff_ms = cutoff.timestamp_millis();
        let cutoff_day = cutoff.format("%Y-%m-%d").to_string();
        let conn = self.conn.lock().unwrap();
        let mut n = 0u64;
        n += conn
            .execute(
                "DELETE FROM system_events WHERE ts_ms < ?1",
                params![cutoff_ms],
            )
            .map_err(|e| e.to_string())? as u64;
        n += conn
            .execute(
                "DELETE FROM state_intervals WHERE day < ?1",
                params![cutoff_day],
            )
            .map_err(|e| e.to_string())? as u64;
        n += conn
            .execute(
                "DELETE FROM app_intervals WHERE day < ?1",
                params![cutoff_day],
            )
            .map_err(|e| e.to_string())? as u64;
        Ok(n)
    }

    pub fn clear_all(&self) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "DELETE FROM system_events;
             DELETE FROM state_intervals;
             DELETE FROM app_intervals;
             DELETE FROM open_apps;",
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

}
