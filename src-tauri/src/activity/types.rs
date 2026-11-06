use serde::{Deserialize, Serialize};

/// System timeline segment states.
pub const STATE_AVAILABLE: &str = "available";
pub const STATE_ACTIVE: &str = "active";
pub const STATE_IDLE: &str = "idle";
pub const STATE_LOCKED: &str = "locked";
pub const STATE_SUSPENDED: &str = "suspended";
pub const STATE_POWERED_OFF: &str = "powered_off";
pub const STATE_UNKNOWN: &str = "unknown";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySettings {
    /// Seconds without input before idle classification.
    pub idle_threshold_secs: u32,
    /// When true, store sanitized window titles (never URLs by default).
    pub store_window_titles: bool,
    /// Opt-in browser domain tracking (not implemented without extension).
    pub browser_domain_tracking: bool,
    pub tracking_enabled: bool,
}

impl Default for ActivitySettings {
    fn default() -> Self {
        Self {
            idle_threshold_secs: 300,
            store_window_titles: false,
            browser_domain_tracking: false,
            tracking_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDay {
    /// Local calendar date YYYY-MM-DD.
    pub date: String,
    pub timezone: String,
    pub tracking: TrackingCoverage,
    pub overview: DayOverview,
    pub timeline: Vec<TimelineSegment>,
    pub apps: Vec<AppUsageRow>,
    pub events: Vec<SystemEventRow>,
    pub legend: Vec<LegendItem>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingCoverage {
    /// Continuous focus/idle samples were collected for this fraction of available time (0–1).
    pub monitored_ratio: f64,
    pub has_journal_events: bool,
    pub has_samples: bool,
    pub incomplete: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayOverview {
    pub uptime_ms: i64,
    pub first_boot_ms: Option<i64>,
    pub last_end_ms: Option<i64>,
    pub last_end_kind: Option<String>,
    pub active_ms: i64,
    pub idle_ms: i64,
    pub locked_ms: i64,
    pub suspended_ms: i64,
    pub powered_off_ms: i64,
    pub unknown_ms: i64,
    pub foreground_ms: i64,
    pub background_ms: i64,
    /// Metrics that are estimates vs measured.
    pub active_is_estimate: bool,
    pub background_is_estimate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub duration_ms: i64,
    pub state: String,
    pub label: String,
    pub source: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageRow {
    pub app_id: String,
    pub app_name: String,
    pub first_ms: i64,
    pub last_ms: i64,
    pub sessions: u32,
    pub foreground_ms: i64,
    pub background_ms: i64,
    pub active_ms: i64,
    pub background_is_estimate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemEventRow {
    pub ts_ms: i64,
    pub kind: String,
    pub detail: Option<String>,
    pub inferred: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegendItem {
    pub state: String,
    pub label: String,
    pub color_token: String,
}
