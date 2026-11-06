use std::fs;
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::FixResult;

const RECOMMENDED: u64 = 524_288;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatcherStatus {
    pub current: u64,
    pub recommended: u64,
    pub low: bool,
}

pub fn watcher_status() -> WatcherStatus {
    let current = fs::read_to_string("/proc/sys/fs/inotify/max_user_watches")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    WatcherStatus {
        current,
        recommended: RECOMMENDED,
        low: current > 0 && current < RECOMMENDED,
    }
}

pub fn fix_file_watchers() -> Result<FixResult, String> {
    let status = watcher_status();
    if !status.low {
        return Ok(FixResult {
            success: true,
            message: format!(
                "Watcher limit already OK ({} ≥ {}).",
                status.current, status.recommended
            ),
        });
    }

    // Try non-interactive sysctl first
    let set = Command::new("sysctl")
        .args([
            "-w",
            &format!("fs.inotify.max_user_watches={RECOMMENDED}"),
        ])
        .output();

    if let Ok(output) = set {
        if output.status.success() {
            // Persist via pkexec if possible (optional)
            let conf = format!("fs.inotify.max_user_watches={RECOMMENDED}\n");
            let tmp = "/tmp/devmonitor-inotify.conf";
            let _ = fs::write(tmp, &conf);
            let _ = Command::new("pkexec")
                .args([
                    "install",
                    "-m",
                    "644",
                    tmp,
                    "/etc/sysctl.d/99-devmonitor-inotify.conf",
                ])
                .output();
            let _ = Command::new("pkexec")
                .args(["sysctl", "--system"])
                .output();

            return Ok(FixResult {
                success: true,
                message: format!(
                    "Raised fs.inotify.max_user_watches from {} to {}.",
                    status.current, RECOMMENDED
                ),
            });
        }
    }

    // Fallback: write user-visible instructions file + attempt pkexec sysctl -w
    let pk = Command::new("pkexec")
        .args([
            "sysctl",
            "-w",
            &format!("fs.inotify.max_user_watches={RECOMMENDED}"),
        ])
        .output();

    if let Ok(output) = pk {
        if output.status.success() {
            return Ok(FixResult {
                success: true,
                message: format!(
                    "Raised watcher limit to {} via elevated privileges. For permanence, add fs.inotify.max_user_watches={} to /etc/sysctl.d/.",
                    RECOMMENDED, RECOMMENDED
                ),
            });
        }
    }

    Ok(FixResult {
        success: false,
        message: format!(
            "Could not raise watcher limit automatically (permission denied). Run:\n  sudo sysctl -w fs.inotify.max_user_watches={RECOMMENDED}\n  echo fs.inotify.max_user_watches={RECOMMENDED} | sudo tee /etc/sysctl.d/99-devmonitor-inotify.conf"
        ),
    })
}
