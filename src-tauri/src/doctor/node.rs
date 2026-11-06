use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInstall {
    pub path: String,
    pub version: Option<String>,
    pub manager: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeEnvironment {
    pub node_available: bool,
    pub npm_available: bool,
    pub active_node: Option<String>,
    pub active_npm: Option<String>,
    pub which_node: Option<String>,
    pub which_npm: Option<String>,
    pub install_method: String,
    pub manager: Option<String>,
    pub installations: Vec<NodeInstall>,
    pub npm_prefix: Option<String>,
    pub npm_prefix_owner_root: bool,
    pub path_issue: bool,
    pub path_issue_detail: Option<String>,
}

pub fn detect_node_environment() -> NodeEnvironment {
    let which_node = which("node");
    let which_npm = which("npm");
    let active_node = run_version("node");
    let active_npm = run_version("npm");

    let mut installations = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();

    // nvm versions
    let nvm_dir = std::env::var("NVM_DIR").unwrap_or_else(|_| format!("{home}/.nvm"));
    let nvm_versions = PathBuf::from(&nvm_dir).join("versions/node");
    if nvm_versions.is_dir() {
        if let Ok(entries) = fs::read_dir(&nvm_versions) {
            for entry in entries.flatten() {
                let bin = entry.path().join("bin/node");
                if bin.exists() {
                    let ver = entry.file_name().to_string_lossy().trim_start_matches('v').to_string();
                    installations.push(NodeInstall {
                        path: bin.display().to_string(),
                        version: Some(ver),
                        manager: "nvm".into(),
                    });
                }
            }
        }
    }

    // fnm
    let fnm_dir = PathBuf::from(&home).join(".local/share/fnm/node-versions");
    if fnm_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&fnm_dir) {
            for entry in entries.flatten() {
                let bin = entry.path().join("installation/bin/node");
                if bin.exists() {
                    let ver = entry.file_name().to_string_lossy().trim_start_matches('v').to_string();
                    installations.push(NodeInstall {
                        path: bin.display().to_string(),
                        version: Some(ver),
                        manager: "fnm".into(),
                    });
                }
            }
        }
    }

    if let Some(ref path) = which_node {
        if !installations.iter().any(|i| &i.path == path) {
            let manager = classify_path(path);
            installations.push(NodeInstall {
                path: path.clone(),
                version: active_node.clone(),
                manager,
            });
        }
    }

    let install_method = which_node
        .as_ref()
        .map(|p| classify_path(p))
        .unwrap_or_else(|| "missing".into());

    let manager = if PathBuf::from(&nvm_dir).join("nvm.sh").exists() {
        Some("nvm".into())
    } else if which("fnm").is_some() {
        Some("fnm".into())
    } else if install_method == "snap" {
        Some("snap".into())
    } else if install_method == "apt" || install_method == "system" {
        Some("system".into())
    } else {
        None
    };

    let npm_prefix = run_capture("npm", &["prefix", "-g"]);
    let npm_prefix_owner_root = npm_prefix
        .as_ref()
        .map(|p| is_root_owned(PathBuf::from(p)))
        .unwrap_or(false);

    let (path_issue, path_issue_detail) = diagnose_path(&which_node, &active_node, &installations);

    NodeEnvironment {
        node_available: which_node.is_some() && active_node.is_some(),
        npm_available: which_npm.is_some() && active_npm.is_some(),
        active_node,
        active_npm,
        which_node,
        which_npm,
        install_method,
        manager,
        installations,
        npm_prefix,
        npm_prefix_owner_root,
        path_issue,
        path_issue_detail,
    }
}

fn diagnose_path(
    which_node: &Option<String>,
    active_node: &Option<String>,
    installations: &[NodeInstall],
) -> (bool, Option<String>) {
    if which_node.is_none() && !installations.is_empty() {
        return (
            true,
            Some(
                "Node is installed but not available in PATH for this session.".into(),
            ),
        );
    }
    if which_node.is_some() && active_node.is_none() {
        return (
            true,
            Some("node is on PATH but failed to run (`node -v`).".into()),
        );
    }
    // Multiple installs: active may differ from newest
    if installations.len() > 1 {
        if let (Some(active), Some(path)) = (active_node, which_node) {
            let others: Vec<_> = installations
                .iter()
                .filter(|i| &i.path != path)
                .filter_map(|i| i.version.as_ref())
                .collect();
            if !others.is_empty() && others.iter().any(|v| *v != active) {
                // Not necessarily an error — informational mismatch handled elsewhere
            }
        }
    }
    (false, None)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeVersionRow {
    pub version: String,
    pub lts: Option<String>,
    pub date: Option<String>,
    pub installed: bool,
    pub current: bool,
    pub manager: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeVersionCatalog {
    pub manager: Option<String>,
    pub active_node: Option<String>,
    pub versions: Vec<NodeVersionRow>,
    pub source: String,
}

/// Fetch Node releases from nodejs.org and merge with local installs.
pub fn list_node_versions() -> Result<NodeVersionCatalog, String> {
    let env = detect_node_environment();
    let remote = fetch_nodejs_index()?;
    let mut by_major: std::collections::BTreeMap<u32, serde_json::Value> =
        std::collections::BTreeMap::new();

    if let Some(arr) = remote.as_array() {
        for entry in arr {
            let ver = entry
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim_start_matches('v');
            let major = ver
                .split('.')
                .next()
                .and_then(|m| m.parse::<u32>().ok())
                .unwrap_or(0);
            if major == 0 {
                continue;
            }
            // index.json is newest-first; keep first seen per major
            by_major.entry(major).or_insert_with(|| entry.clone());
        }
    }

    let installed_map: std::collections::HashMap<String, &NodeInstall> = env
        .installations
        .iter()
        .filter_map(|i| {
            i.version
                .as_ref()
                .map(|v| (normalize_ver(v), i))
        })
        .collect();

    let active = env.active_node.as_deref().map(normalize_ver);

    let mut versions: Vec<NodeVersionRow> = by_major
        .into_iter()
        .rev() // newest majors first
        .map(|(_major, entry)| {
            let version = entry
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim_start_matches('v')
                .to_string();
            let key = normalize_ver(&version);
            let lts = match entry.get("lts") {
                Some(serde_json::Value::String(s)) if !s.is_empty() => Some(s.clone()),
                Some(serde_json::Value::Bool(true)) => Some("LTS".into()),
                _ => None,
            };
            let date = entry
                .get("date")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let maj = key.split('.').next().unwrap_or("");
            let inst = installed_map.get(&key).copied().or_else(|| {
                env.installations.iter().find(|i| {
                    i.version
                        .as_ref()
                        .map(|v| {
                            let nv = normalize_ver(v);
                            nv == maj || nv.starts_with(&format!("{maj}."))
                        })
                        .unwrap_or(false)
                })
            });
            let installed = inst.is_some();
            let current = active
                .as_ref()
                .map(|a| a.split('.').next() == Some(maj))
                .unwrap_or(false);

            NodeVersionRow {
                version,
                lts,
                date,
                installed,
                current,
                manager: inst.map(|i| i.manager.clone()).or_else(|| env.manager.clone()),
                path: inst.map(|i| i.path.clone()),
            }
        })
        .collect();

    // Include any local installs not represented in remote majors
    for inst in &env.installations {
        let Some(ver) = &inst.version else { continue };
        let key = normalize_ver(ver);
        let maj = key.split('.').next().unwrap_or(&key).to_string();
        if !versions
            .iter()
            .any(|r| r.version.split('.').next() == Some(maj.as_str()))
        {
            versions.push(NodeVersionRow {
                version: key.clone(),
                lts: None,
                date: None,
                installed: true,
                current: active
                    .as_ref()
                    .map(|a| a.split('.').next() == Some(maj.as_str()))
                    .unwrap_or(false),
                manager: Some(inst.manager.clone()),
                path: Some(inst.path.clone()),
            });
        }
    }

    Ok(NodeVersionCatalog {
        manager: resolve_switch_manager(&env),
        active_node: env.active_node,
        versions,
        source: "https://nodejs.org/dist/index.json".into(),
    })
}

fn fetch_nodejs_index() -> Result<serde_json::Value, String> {
    let out = Command::new("curl")
        .args([
            "-sfSL",
            "--max-time",
            "25",
            "https://nodejs.org/dist/index.json",
        ])
        .output()
        .map_err(|e| format!("Could not reach nodejs.org ({e}). Check network."))?;
    if !out.status.success() {
        return Err(format!(
            "Could not download Node version list: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("Invalid Node release index: {e}"))
}

fn normalize_ver(v: &str) -> String {
    v.trim().trim_start_matches('v').to_string()
}

fn resolve_switch_manager(env: &NodeEnvironment) -> Option<String> {
    if env.manager.as_deref() == Some("nvm") || nvm_available() {
        return Some("nvm".into());
    }
    if env.manager.as_deref() == Some("fnm") || which("fnm").is_some() {
        return Some("fnm".into());
    }
    None
}

fn nvm_available() -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let nvm_dir = std::env::var("NVM_DIR").unwrap_or_else(|_| format!("{home}/.nvm"));
    PathBuf::from(nvm_dir).join("nvm.sh").exists()
}

/// Install if needed, switch, and set as default (nvm/fnm). Bootstraps nvm when missing.
pub fn use_node_version(version: &str) -> Result<super::FixResult, String> {
    let version = normalize_ver(version);
    if version.is_empty()
        || !version
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
    {
        return Err("Invalid Node version".into());
    }

    let mut steps: Vec<String> = Vec::new();
    let manager = ensure_version_manager(&mut steps)?;

    let result = match manager.as_str() {
        "nvm" => use_via_nvm(&version, &mut steps)?,
        "fnm" => use_via_fnm(&version, &mut steps)?,
        other => {
            return Ok(super::FixResult {
                success: false,
                message: format!("Unsupported manager '{other}'"),
            });
        }
    };

    // Refresh ~/.local/bin link so GUI/session PATH can see the new default
    let _ = repair_node_path();

    Ok(super::FixResult {
        success: result.success,
        message: if steps.is_empty() {
            result.message
        } else {
            format!("{} · {}", steps.join(" → "), result.message)
        },
    })
}

fn ensure_version_manager(steps: &mut Vec<String>) -> Result<String, String> {
    if nvm_available() {
        steps.push("Using nvm".into());
        return Ok("nvm".into());
    }
    if which("fnm").is_some() {
        steps.push("Using fnm".into());
        return Ok("fnm".into());
    }

    steps.push("Installing nvm".into());
    let home = std::env::var("HOME").unwrap_or_default();
    let script = r#"
set -e
export NVM_DIR="$HOME/.nvm"
if [ ! -s "$NVM_DIR/nvm.sh" ]; then
  curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
fi
"#;
    let out = Command::new("bash")
        .args(["-lc", script])
        .output()
        .map_err(|e| format!("Could not install nvm: {e}"))?;
    if !out.status.success() || !nvm_available() {
        let err = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // last resort: try fnm via cargo if present
        if which("fnm").is_some() {
            steps.push("Falling back to fnm".into());
            return Ok("fnm".into());
        }
        return Err(format!(
            "Could not set up nvm for version switching. Install nvm or fnm manually.\n{err}"
        ));
    }
    let _ = home;
    Ok("nvm".into())
}

fn use_via_nvm(version: &str, steps: &mut Vec<String>) -> Result<super::FixResult, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let nvm_dir = std::env::var("NVM_DIR").unwrap_or_else(|_| format!("{home}/.nvm"));
    let already = PathBuf::from(&nvm_dir)
        .join("versions/node")
        .join(format!("v{version}"))
        .join("bin/node")
        .exists()
        || PathBuf::from(&nvm_dir)
            .join("versions/node")
            .read_dir()
            .ok()
            .map(|rd| {
                rd.flatten().any(|e| {
                    let name = e.file_name().to_string_lossy().trim_start_matches('v').to_string();
                    name == version
                        || name.starts_with(&format!("{version}."))
                        || (version.split('.').count() == 1
                            && name.split('.').next() == Some(version))
                })
            })
            .unwrap_or(false);

    if already {
        steps.push("Already installed".into());
    } else {
        steps.push(format!("Installing {version}"));
    }
    steps.push("Switching".into());

    let script = format!(
        r#"
set -e
export NVM_DIR="{nvm_dir}"
# shellcheck disable=SC1090
. "$NVM_DIR/nvm.sh"
nvm install {version}
nvm alias default {version}
nvm use {version}
hash -r 2>/dev/null || true
node -v
npm -v
"#
    );
    let output = Command::new("bash")
        .args(["-lc", &script])
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        let node_v = stdout
            .lines()
            .rev()
            .find(|l| l.trim().starts_with('v'))
            .unwrap_or(version);
        steps.push("Set as current".into());
        Ok(super::FixResult {
            success: true,
            message: format!("Now using Node {node_v} (nvm default)"),
        })
    } else {
        Ok(super::FixResult {
            success: false,
            message: format!("nvm failed: {stderr}{stdout}"),
        })
    }
}

fn use_via_fnm(version: &str, steps: &mut Vec<String>) -> Result<super::FixResult, String> {
    steps.push(format!("Installing {version}"));
    steps.push("Switching".into());
    let script = format!(
        r#"
set -e
eval "$(fnm env)"
fnm install {version}
fnm default {version}
fnm use {version}
node -v
"#
    );
    let output = Command::new("bash")
        .args(["-lc", &script])
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        steps.push("Set as current".into());
        Ok(super::FixResult {
            success: true,
            message: format!(
                "Now using Node {} (fnm default)",
                stdout.lines().last().unwrap_or(version)
            ),
        })
    } else {
        Ok(super::FixResult {
            success: false,
            message: format!("fnm failed: {stderr}{stdout}"),
        })
    }
}

pub fn switch_node(version: &str) -> Result<super::FixResult, String> {
    use_node_version(version)
}

pub fn repair_node_path() -> Result<super::FixResult, String> {
    let env = detect_node_environment();
    let home = std::env::var("HOME").unwrap_or_default();
    let local_bin = PathBuf::from(&home).join(".local/bin");
    fs::create_dir_all(&local_bin).map_err(|e| e.to_string())?;

    // Prefer nvm default / latest installation
    let target = env
        .installations
        .iter()
        .find(|i| i.manager == "nvm")
        .or_else(|| env.installations.first())
        .ok_or_else(|| "No Node installation found to link into PATH".to_string())?;

    let node_src = PathBuf::from(&target.path);
    let npm_src = node_src.parent().map(|p| p.join("npm"));

    let node_dst = local_bin.join("node");
    let _ = fs::remove_file(&node_dst);
    std::os::unix::fs::symlink(&node_src, &node_dst).map_err(|e| e.to_string())?;

    if let Some(npm) = npm_src {
        if npm.exists() {
            let npm_dst = local_bin.join("npm");
            let _ = fs::remove_file(&npm_dst);
            let _ = std::os::unix::fs::symlink(&npm, &npm_dst);
        }
    }

    // Ensure ~/.local/bin note in a small helper file
    let tip = format!(
        "Linked {} -> {}\nEnsure ~/.local/bin is early in PATH (usually already is on Ubuntu).",
        node_src.display(),
        node_dst.display()
    );

    Ok(super::FixResult {
        success: true,
        message: tip,
    })
}

pub fn repair_npm_permissions() -> Result<super::FixResult, String> {
    let env = detect_node_environment();
    if env.manager.as_deref() == Some("nvm") || env.manager.as_deref() == Some("fnm") {
        return Ok(super::FixResult {
            success: true,
            message: format!(
                "Node is managed by {}. Global installs should go into your user version manager prefix — avoid sudo npm -g.",
                env.manager.unwrap()
            ),
        });
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let prefix = PathBuf::from(&home).join(".npm-global");
    fs::create_dir_all(prefix.join("bin")).map_err(|e| e.to_string())?;

    let output = Command::new("npm")
        .args(["config", "set", "prefix", &prefix.display().to_string()])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Ok(super::FixResult {
            success: false,
            message: format!(
                "Failed to set npm prefix: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    // Symlink note: ensure bin on path via ~/.local/bin is already common; also write a tiny env snippet
    let snippet_path = PathBuf::from(&home).join(".config/devmonitor/npm-path.sh");
    if let Some(parent) = snippet_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let snippet = format!(
        "export PATH=\"{}:$PATH\"\n",
        prefix.join("bin").display()
    );
    let _ = fs::write(&snippet_path, snippet);

    Ok(super::FixResult {
        success: true,
        message: format!(
            "Set npm global prefix to {}. Add `source {}` to your shell profile if global CLIs are still missing.",
            prefix.display(),
            snippet_path.display()
        ),
    })
}

fn classify_path(path: &str) -> String {
    if path.contains("/.nvm/") {
        "nvm".into()
    } else if path.contains("/fnm/") || path.contains("/.local/share/fnm/") {
        "fnm".into()
    } else if path.contains("/snap/") {
        "snap".into()
    } else if path.starts_with("/usr/bin") || path.starts_with("/usr/local") {
        "system".into()
    } else {
        "unknown".into()
    }
}

fn which(cmd: &str) -> Option<String> {
    Command::new("which")
        .arg(cmd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn run_version(cmd: &str) -> Option<String> {
    Command::new(cmd)
        .arg("-v")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .trim_start_matches('v')
                .to_string()
        })
        .filter(|s| !s.is_empty())
}

fn run_capture(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn is_root_owned(path: PathBuf) -> bool {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(path)
        .map(|m| m.uid() == 0)
        .unwrap_or(false)
}

/// Return true if active version does not satisfy a simple required specifier.
pub fn version_mismatch(active: &str, required: &str) -> bool {
    let req = required.trim().trim_start_matches('v');
    // Exact or major match for .nvmrc style "22" / "22.11.0"
    if let Some(major) = req.split('.').next() {
        if major.chars().all(|c| c.is_ascii_digit()) && !req.contains('>') && !req.contains('<') && !req.contains('=') && !req.contains('|') && !req.contains(' ') {
            return !active.starts_with(major)
                && active.split('.').next() != Some(major);
        }
    }
    // engines like ">=18" / ">=20 <23"
    if let Some(rest) = req.strip_prefix(">=") {
        let min = rest.split_whitespace().next().unwrap_or(rest);
        return compare_semver(active, min) < 0;
    }
    if let Some(exact) = req.strip_prefix('=') {
        return active != exact.trim();
    }
    // Fallback: compare major digits present in required
    let digits: String = req.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    if let Some(major) = digits.split('.').next() {
        if !major.is_empty() {
            return active.split('.').next() != Some(major);
        }
    }
    false
}

fn compare_semver(a: &str, b: &str) -> i32 {
    let pa: Vec<u32> = a.split('.').filter_map(|x| x.parse().ok()).collect();
    let pb: Vec<u32> = b.split('.').filter_map(|x| x.parse().ok()).collect();
    for i in 0..3 {
        let ai = pa.get(i).copied().unwrap_or(0);
        let bi = pb.get(i).copied().unwrap_or(0);
        if ai < bi {
            return -1;
        }
        if ai > bi {
            return 1;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::version_mismatch;
    #[test]
    fn detects_major_mismatch() {
        assert!(version_mismatch("20.19.0", "22"));
        assert!(!version_mismatch("22.11.0", "22"));
        assert!(version_mismatch("18.0.0", ">=20"));
        assert!(!version_mismatch("20.1.0", ">=20"));
    }
}
