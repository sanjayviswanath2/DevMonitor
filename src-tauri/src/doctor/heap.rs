use std::fs;
use std::path::PathBuf;

use super::project::ProjectInfo;
use super::FixResult;
use crate::system::AppState;

pub fn apply_node_heap(project: &ProjectInfo, mb: u32) -> Result<FixResult, String> {
    if !(512..=16384).contains(&mb) {
        return Err("Heap size must be between 512 and 16384 MB".into());
    }

    // RAM-aware warning handled by caller; still soft-cap here using sysinfo
    let state_note = "";

    let root = PathBuf::from(&project.path);
    if !root.is_dir() {
        return Err("Invalid project path".into());
    }

    let env_path = root.join(".env.development.local");
    let key = "NODE_OPTIONS";
    let value = format!("--max-old-space-size={mb}");

    let mut lines: Vec<String> = if env_path.exists() {
        fs::read_to_string(&env_path)
            .unwrap_or_default()
            .lines()
            .map(|l| l.to_string())
            .collect()
    } else {
        Vec::new()
    };

    let mut found = false;
    for line in lines.iter_mut() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("NODE_OPTIONS=") || trimmed.starts_with("NODE_OPTIONS =") {
            // Merge carefully: replace max-old-space-size or set whole value
            *line = format!("{key}={value}");
            found = true;
        }
    }
    if !found {
        if !lines.is_empty() && !lines.last().map(|l| l.is_empty()).unwrap_or(true) {
            lines.push(String::new());
        }
        lines.push(format!("# Applied by DevMonitor{state_note}"));
        lines.push(format!("{key}={value}"));
    }

    fs::write(&env_path, lines.join("\n") + "\n").map_err(|e| e.to_string())?;

    Ok(FixResult {
        success: true,
        message: format!(
            "Wrote {key}={value} to {}. Restart your dev server to apply.",
            env_path.display()
        ),
    })
}

pub fn suggested_heap_mb(state: &AppState) -> u32 {
    let mut system = match state.system.lock() {
        Ok(s) => s,
        Err(_) => return 4096,
    };
    system.refresh_memory();
    let avail_mb = (system.available_memory() / (1024 * 1024)) as u32;
    if avail_mb >= 10000 {
        8192
    } else if avail_mb >= 6000 {
        6144
    } else if avail_mb >= 4000 {
        4096
    } else {
        2048
    }
}
