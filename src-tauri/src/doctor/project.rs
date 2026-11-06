use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub package_manager: String,
    pub package_manager_field: Option<String>,
    pub has_package_json: bool,
    pub has_node_modules: bool,
    pub lockfile: Option<String>,
    pub lockfile_count: usize,
    pub engines_node: Option<String>,
    pub nvmrc: Option<String>,
    pub node_version_file: Option<String>,
    pub required_node: Option<String>,
    pub env_example: Option<String>,
    pub python_required: Option<String>,
    pub has_native_deps_hint: bool,
}

pub fn inspect_project(path: &str) -> Result<ProjectInfo, String> {
    let root = PathBuf::from(path);
    if !root.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }

    let package_json = root.join("package.json");
    let has_package_json = package_json.exists();
    let has_node_modules = root.join("node_modules").is_dir();

    let mut name = root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    let mut engines_node = None;
    let mut package_manager_field = None;
    let mut has_native_deps_hint = false;

    if has_package_json {
        if let Ok(raw) = fs::read_to_string(&package_json) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(n) = json.get("name").and_then(|v| v.as_str()) {
                    name = n.to_string();
                }
                engines_node = json
                    .pointer("/engines/node")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                package_manager_field = json
                    .get("packageManager")
                    .and_then(|v| v.as_str())
                    .map(|s| s.split('@').next().unwrap_or(s).to_string());
                has_native_deps_hint = deps_hint_native(&json);
            }
        }
    }

    let nvmrc = read_trimmed(root.join(".nvmrc"));
    let node_version_file = read_trimmed(root.join(".node-version"));
    let lockfiles = detect_lockfiles(&root);
    let lockfile = lockfiles.first().cloned();
    let lockfile_count = lockfiles.len();

    let package_manager = package_manager_field
        .clone()
        .or_else(|| {
            lockfile.as_deref().map(|lf| match lf {
                "pnpm-lock.yaml" => "pnpm".to_string(),
                "yarn.lock" => "yarn".to_string(),
                "bun.lock" | "bun.lockb" => "bun".to_string(),
                _ => "npm".to_string(),
            })
        })
        .unwrap_or_else(|| "npm".into());

    let required_node = nvmrc
        .clone()
        .or(node_version_file.clone())
        .or(engines_node.clone());

    let env_example = [".env.example", ".env.sample", ".env.template"]
        .iter()
        .find(|n| root.join(n).exists())
        .map(|s| s.to_string());

    let python_required = read_trimmed(root.join(".python-version"))
        .or_else(|| read_runtime_txt(&root))
        .or_else(|| read_requires_python(&root));

    Ok(ProjectInfo {
        path: root.display().to_string(),
        name,
        package_manager,
        package_manager_field,
        has_package_json,
        has_node_modules,
        lockfile,
        lockfile_count,
        engines_node,
        nvmrc,
        node_version_file,
        required_node,
        env_example,
        python_required,
        has_native_deps_hint,
    })
}

fn deps_hint_native(json: &serde_json::Value) -> bool {
    let markers = [
        "better-sqlite3",
        "sharp",
        "bcrypt",
        "canvas",
        "node-sass",
        "sqlite3",
        "fsevents",
        "@napi-rs",
        "esbuild",
    ];
    for key in ["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(obj) = json.get(key).and_then(|v| v.as_object()) {
            for (name, _) in obj {
                if markers.iter().any(|m| name.contains(m)) {
                    return true;
                }
            }
        }
    }
    false
}

fn read_trimmed(path: PathBuf) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.lines().next().unwrap_or("").trim().trim_start_matches('v').to_string())
        .filter(|s| !s.is_empty())
}

fn read_runtime_txt(root: &Path) -> Option<String> {
    let raw = fs::read_to_string(root.join("runtime.txt")).ok()?;
    // python-3.11.0
    let line = raw.lines().next()?.trim();
    line.strip_prefix("python-")
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn read_requires_python(root: &Path) -> Option<String> {
    let raw = fs::read_to_string(root.join("pyproject.toml")).ok()?;
    for line in raw.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("requires-python") {
            let val = rest
                .trim_start_matches([' ', '='])
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

fn detect_lockfiles(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for name in [
        "pnpm-lock.yaml",
        "yarn.lock",
        "bun.lock",
        "bun.lockb",
        "package-lock.json",
    ] {
        if root.join(name).exists() {
            out.push(name.to_string());
        }
    }
    out
}

pub fn pm_mismatch(project: &ProjectInfo) -> Option<(String, String)> {
    // Multiple lockfiles
    if project.lockfile_count > 1 {
        return Some((
            project.package_manager.clone(),
            format!(
                "Multiple lockfiles detected ({})",
                project.lockfile_count
            ),
        ));
    }
    // packageManager field disagrees with lockfile
    if let (Some(field), Some(lock)) = (&project.package_manager_field, &project.lockfile) {
        let from_lock = match lock.as_str() {
            "pnpm-lock.yaml" => "pnpm",
            "yarn.lock" => "yarn",
            "bun.lock" | "bun.lockb" => "bun",
            "package-lock.json" => "npm",
            _ => return None,
        };
        if field != from_lock {
            return Some((
                field.clone(),
                format!("package.json says {field} but lockfile is {lock}"),
            ));
        }
    }
    None
}

pub fn install_command(pm: &str) -> String {
    match pm {
        "pnpm" => "pnpm install".into(),
        "yarn" => "yarn install".into(),
        "bun" => "bun install".into(),
        _ => "npm install".into(),
    }
}
