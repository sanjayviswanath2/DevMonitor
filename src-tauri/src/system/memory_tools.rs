//! Swap, reclaimable cache drop, and safe storage cleanup for the Memory tab.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::doctor::disk;
use crate::doctor::FixResult;

const SWAP_PATH: &str = "/swapfile.devmonitor";
const DEFAULT_SWAP_GB: u32 = 4;

pub fn create_or_enable_swap(size_gb: Option<u32>) -> Result<FixResult, String> {
    let gb = size_gb.unwrap_or(DEFAULT_SWAP_GB).clamp(1, 32);
    let path = SWAP_PATH;

    // Already active?
    if swap_active_for(path) {
        return Ok(FixResult {
            success: true,
            message: format!("Swap already active at {path}"),
        });
    }

    // If file exists, just swapon
    if PathBuf::from(path).exists() {
        let out = Command::new("pkexec")
            .args(["swapon", path])
            .output()
            .map_err(|e| format!("Could not enable swap: {e}"))?;
        if out.status.success() {
            return Ok(FixResult {
                success: true,
                message: format!("Enabled existing swap file {path}"),
            });
        }
    }

    let bytes = (gb as u64) * 1024 * 1024 * 1024;
    let script = format!(
        r#"set -e
PATH_FILE='{path}'
SIZE={bytes}
if [ ! -f "$PATH_FILE" ]; then
  fallocate -l "$SIZE" "$PATH_FILE" || dd if=/dev/zero of="$PATH_FILE" bs=1M count=$((SIZE/1048576)) status=none
  chmod 600 "$PATH_FILE"
  mkswap "$PATH_FILE"
fi
swapon "$PATH_FILE" || true
# Persist across reboots if missing from fstab
if ! grep -qF "$PATH_FILE" /etc/fstab 2>/dev/null; then
  echo "$PATH_FILE none swap sw 0 0" >> /etc/fstab
fi
"#
    );

    let mut child = Command::new("pkexec")
        .args(["bash", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start swap setup (authorize when prompted): {e}"))?;

    let out = child
        .wait_with_output()
        .map_err(|e| format!("Swap setup failed: {e}"))?;

    if !out.status.success() {
        let err = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        return Err(format!(
            "Could not create swap. Approve the password prompt if shown.\n{err}"
        ));
    }

    Ok(FixResult {
        success: true,
        message: format!(
            "Added {gb} GB swap at {path} and enabled it (persisted in fstab)"
        ),
    })
}

fn swap_active_for(path: &str) -> bool {
    fs::read_to_string("/proc/swaps")
        .map(|s| s.lines().any(|l| l.starts_with(path)))
        .unwrap_or(false)
}

/// Drop page cache / dentries / inodes so reclaimable RAM becomes free.
/// Does not kill apps. Requires authorization.
pub fn drop_memory_caches() -> Result<FixResult, String> {
    let _ = Command::new("sync").status();
    let mut child = Command::new("pkexec")
        .args(["tee", "/proc/sys/vm/drop_caches"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not drop caches: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"3\n");
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("drop_caches failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "Could not free reclaimable memory. Approve the password prompt if shown.\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(FixResult {
        success: true,
        message: "Freed reclaimable page cache (apps were not closed)".into(),
    })
}

/// Clear safe developer caches + user Trash. Optional apt cache via pkexec.
pub fn clear_developer_storage(include_apt: bool) -> Result<FixResult, String> {
    let caches = disk::list_safe_caches();
    let ids: Vec<String> = caches.iter().map(|c| c.id.clone()).collect();
    let cache_result = if ids.is_empty() {
        FixResult {
            success: true,
            message: "No developer caches found".into(),
        }
    } else {
        disk::clean_caches(&ids)?
    };

    let mut parts = vec![cache_result.message];
    let mut freed_extra = 0u64;

    // Empty Trash
    if let Ok(home) = std::env::var("HOME") {
        let trash = PathBuf::from(format!("{home}/.local/share/Trash/files"));
        let trash_info = PathBuf::from(format!("{home}/.local/share/Trash/info"));
        for dir in [&trash, &trash_info] {
            if dir.is_dir() {
                let before = dir_size_quick(dir);
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        let _ = if p.is_dir() {
                            fs::remove_dir_all(&p)
                        } else {
                            fs::remove_file(&p)
                        };
                    }
                }
                freed_extra += before;
            }
        }
        if freed_extra > 0 {
            parts.push(format!(
                "Emptied Trash (~{:.1} MB)",
                freed_extra as f64 / 1_048_576.0
            ));
        }
    }

    if include_apt {
        let out = Command::new("pkexec")
            .args(["apt-get", "clean"])
            .output();
        if let Ok(out) = out {
            if out.status.success() {
                parts.push("Cleared apt package cache".into());
            }
        }
    }

    Ok(FixResult {
        success: true,
        message: parts.join(" · "),
    })
}

fn dir_size_quick(path: &PathBuf) -> u64 {
    walkdir::WalkDir::new(path)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Suggest a safer Node heap for the current machine (also used to fix oversized heaps).
pub fn safe_heap_mb(available_memory: u64, total_memory: u64) -> u32 {
    let avail_mb = (available_memory / (1024 * 1024)) as u32;
    let total_mb = (total_memory / (1024 * 1024)) as u32;
    // Cap at ~40% of total RAM, prefer available-aware suggestion
    let by_avail = if avail_mb >= 10000 {
        6144
    } else if avail_mb >= 6000 {
        4096
    } else if avail_mb >= 4000 {
        3072
    } else if avail_mb >= 2500 {
        2048
    } else {
        1536
    };
    let by_total = (total_mb as f32 * 0.4) as u32;
    by_avail.min(by_total).clamp(1024, 8192)
}
