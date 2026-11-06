use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::project::ProjectInfo;
use super::FixResult;

pub fn clean_reinstall(project: &ProjectInfo) -> Result<FixResult, String> {
    if !project.has_package_json {
        return Ok(FixResult {
            success: false,
            message: "No package.json in project".into(),
        });
    }

    let root = PathBuf::from(&project.path);
    let modules = root.join("node_modules");
    if modules.exists() {
        fs::remove_dir_all(&modules).map_err(|e| format!("Failed to remove node_modules: {e}"))?;
    }

    // Also remove package-manager specific stale artifacts carefully
    let _ = fs::remove_file(root.join(".package-lock.json.bak"));

    let (cmd, args): (String, Vec<String>) = match project.package_manager.as_str() {
        "pnpm" => {
            if root.join("pnpm-lock.yaml").exists() {
                ("pnpm".into(), vec!["install".into(), "--frozen-lockfile".into()])
            } else {
                ("pnpm".into(), vec!["install".into()])
            }
        }
        "yarn" => {
            if root.join("yarn.lock").exists() {
                ("yarn".into(), vec!["install".into(), "--frozen-lockfile".into()])
            } else {
                ("yarn".into(), vec!["install".into()])
            }
        }
        "bun" => ("bun".into(), vec!["install".into()]),
        _ => {
            if root.join("package-lock.json").exists() {
                ("npm".into(), vec!["ci".into()])
            } else {
                ("npm".into(), vec!["install".into()])
            }
        }
    };

    let output = Command::new(&cmd)
        .args(&args)
        .current_dir(&root)
        .output()
        .map_err(|e| format!("Failed to run {cmd}: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() {
        Ok(FixResult {
            success: true,
            message: format!(
                "Clean reinstall with `{cmd} {}` completed.",
                args.join(" ")
            ),
        })
    } else {
        Ok(FixResult {
            success: false,
            message: format!(
                "Clean reinstall failed:\n{stderr}\n{stdout}"
            ),
        })
    }
}

pub fn deps_look_broken(project: &ProjectInfo) -> bool {
    if !project.has_package_json {
        return false;
    }
    // Missing node_modules while lockfile exists is a strong signal
    if project.lockfile.is_some() && !project.has_node_modules {
        return true;
    }
    // node_modules exists but is empty-ish
    if project.has_node_modules {
        let modules = PathBuf::from(&project.path).join("node_modules");
        if let Ok(mut rd) = fs::read_dir(&modules) {
            if rd.next().is_none() {
                return true;
            }
        }
    }
    false
}
