use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::FixResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheItem {
    pub id: String,
    pub label: String,
    pub path: String,
    pub size_bytes: u64,
    pub safe: bool,
}

pub fn list_safe_caches() -> Vec<CacheItem> {
    let home = std::env::var("HOME").unwrap_or_default();
    let specs = [
        ("npm-cache", "npm cache", format!("{home}/.npm/_cacache")),
        ("yarn-cache", "Yarn cache", format!("{home}/.cache/yarn")),
        (
            "pnpm-store",
            "pnpm store",
            format!("{home}/.local/share/pnpm/store"),
        ),
        ("vite-cache", "Vite cache", format!("{home}/.cache/vite")),
        (
            "next-cache",
            "Next.js cache",
            format!("{home}/.cache/next-swc"),
        ),
        (
            "cargo-cache",
            "Cargo registry cache",
            format!("{home}/.cargo/registry/cache"),
        ),
        (
            "node-gyp",
            "node-gyp cache",
            format!("{home}/.cache/node-gyp"),
        ),
        (
            "typescript",
            "TypeScript cache",
            format!("{home}/.cache/typescript"),
        ),
    ];

    let mut items = Vec::new();
    for (id, label, path) in specs {
        let p = PathBuf::from(&path);
        if p.exists() {
            let size = dir_size(&p);
            if size > 0 {
                items.push(CacheItem {
                    id: id.into(),
                    label: label.into(),
                    path,
                    size_bytes: size,
                    safe: true,
                });
            }
        }
    }
    items.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    items
}

pub fn clean_caches(ids: &[String]) -> Result<FixResult, String> {
    let all = list_safe_caches();
    let home = std::env::var("HOME").unwrap_or_default();
    let mut cleaned = 0u64;
    let mut removed = Vec::new();

    for id in ids {
        if let Some(item) = all.iter().find(|c| &c.id == id) {
            if !item.safe {
                continue;
            }
            let path = PathBuf::from(&item.path);
            if !path.starts_with(&home) {
                continue;
            }
            if path.is_dir() {
                fs::remove_dir_all(&path)
                    .map_err(|e| format!("Failed to remove {}: {e}", item.path))?;
                cleaned += item.size_bytes;
                removed.push(item.label.clone());
            }
        }
    }

    Ok(FixResult {
        success: true,
        message: if removed.is_empty() {
            "No caches removed.".into()
        } else {
            format!(
                "Cleaned {} ({:.1} MB).",
                removed.join(", "),
                cleaned as f64 / 1_048_576.0
            )
        },
    })
}

pub fn disk_usage_percent(path: &str) -> Option<(u64, u64, f32)> {
    let output = std::process::Command::new("df")
        .args(["-B1", "--output=size,avail,pcent", path])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().nth(1)?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let total: u64 = parts[0].parse().ok()?;
    let avail: u64 = parts[1].parse().ok()?;
    let pct: f32 = parts[2].trim_end_matches('%').parse().ok()?;
    Some((total, avail, pct))
}

fn dir_size(path: &PathBuf) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}
