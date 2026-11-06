mod activity;
mod doctor;

pub use activity::*;
pub use doctor::*;

use tauri::{AppHandle, Manager};

use crate::doctor::FixResult;
use crate::system::kill::{self, KillResult};
use crate::system::memory_tools;
use crate::system::ports::{self, ListeningPortsResult};
use crate::system::processes::{self, ProcessInfo};
use crate::system::stats::{self, SystemStats};
use crate::system::AppState;

fn with_state<T, F>(app: &AppHandle, f: F) -> Result<T, String>
where
    F: FnOnce(&AppState) -> Result<T, String>,
{
    let state = app.state::<AppState>();
    f(&*state)
}

/// Heavy sysinfo work must not run on the GTK main thread or the window freezes.
#[tauri::command]
pub async fn get_system_stats(app: AppHandle) -> Result<SystemStats, String> {
    tauri::async_runtime::spawn_blocking(move || with_state(&app, stats::get_system_stats))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_processes(app: AppHandle) -> Result<Vec<ProcessInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || with_state(&app, processes::get_processes))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_listening_ports(app: AppHandle) -> Result<ListeningPortsResult, String> {
    tauri::async_runtime::spawn_blocking(move || with_state(&app, ports::get_listening_ports))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn kill_process(pid: u32) -> Result<KillResult, String> {
    kill::kill_process(pid)
}

#[tauri::command]
pub async fn enable_swap(size_gb: Option<u32>) -> Result<FixResult, String> {
    tauri::async_runtime::spawn_blocking(move || memory_tools::create_or_enable_swap(size_gb))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn drop_memory_caches() -> Result<FixResult, String> {
    tauri::async_runtime::spawn_blocking(memory_tools::drop_memory_caches)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn clear_developer_storage(include_apt: Option<bool>) -> Result<FixResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        memory_tools::clear_developer_storage(include_apt.unwrap_or(false))
    })
    .await
    .map_err(|e| e.to_string())?
}
