use crate::activity::{self, ActivityDay, ActivitySettings, FixOk};

#[tauri::command]
pub fn get_activity_day(date: Option<String>) -> Result<ActivityDay, String> {
    activity::get_day(date)
}

#[tauri::command]
pub fn get_activity_settings() -> Result<ActivitySettings, String> {
    activity::get_settings()
}

#[tauri::command]
pub fn set_activity_settings(settings: ActivitySettings) -> Result<ActivitySettings, String> {
    // Clamp idle threshold to supported presets / range
    let mut s = settings;
    if ![60, 180, 300, 600, 900].contains(&s.idle_threshold_secs) {
        // Snap to nearest allowed
        let allowed = [60u32, 180, 300, 600, 900];
        s.idle_threshold_secs = allowed
            .iter()
            .min_by_key(|v| (**v as i64 - s.idle_threshold_secs as i64).abs())
            .copied()
            .unwrap_or(300);
    }
    // Domain tracking requires an extension we do not ship — keep false unless explicitly true
    // (UI explains limitation).
    activity::set_settings(s)
}

#[tauri::command]
pub fn clear_activity_history() -> Result<FixOk, String> {
    activity::clear_history()
}

#[tauri::command]
pub fn list_activity_dates() -> Result<Vec<String>, String> {
    activity::available_dates()
}
