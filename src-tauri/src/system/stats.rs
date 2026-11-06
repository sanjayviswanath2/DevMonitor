use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind};

use super::memory_tools;
use super::AppState;

static LAST_MEMORY_ALERT: Mutex<Option<Instant>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStats {
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_available: u64,
    pub memory_free: u64,
    pub memory_percent: f32,
    pub swap_total: u64,
    pub swap_used: u64,
    pub swap_free: u64,
    pub suggested_heap_mb: u32,
    pub memory_high: bool,
}

pub fn get_system_stats(state: &AppState) -> Result<SystemStats, String> {
    let mut system = state.system.lock().map_err(|e| e.to_string())?;

    system.refresh_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );

    let cpu_usage = system.global_cpu_usage();
    let memory_total = system.total_memory();
    let memory_used = system.used_memory();
    let memory_available = system.available_memory();
    let memory_free = system.free_memory();
    let memory_percent = if memory_total > 0 {
        (memory_used as f64 / memory_total as f64 * 100.0) as f32
    } else {
        0.0
    };
    let swap_total = system.total_swap();
    let swap_used = system.used_swap();
    let swap_free = system.free_swap();
    let suggested_heap_mb = memory_tools::safe_heap_mb(memory_available, memory_total);
    let memory_high = memory_percent >= 80.0;
    drop(system);

    if memory_high {
        maybe_notify_high_memory(memory_percent);
    }

    Ok(SystemStats {
        cpu_usage,
        memory_total,
        memory_used,
        memory_available,
        memory_free,
        memory_percent,
        swap_total,
        swap_used,
        swap_free,
        suggested_heap_mb,
        memory_high,
    })
}

/// Desktop notification at most once every 15 minutes while RAM stays ≥ 80%.
fn maybe_notify_high_memory(percent: f32) {
    let Ok(mut last) = LAST_MEMORY_ALERT.lock() else {
        return;
    };
    if last.is_some_and(|t| t.elapsed() < Duration::from_secs(15 * 60)) {
        return;
    }
    *last = Some(Instant::now());
    let body = format!("RAM is {percent:.0}% in use. Open Memory to free cache or add swap.");
    let _ = Command::new("notify-send")
        .args([
            "-a",
            "DevMonitor",
            "-u",
            "critical",
            "Memory is high",
            &body,
        ])
        .spawn();
}
