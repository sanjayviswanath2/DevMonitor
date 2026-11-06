use std::process::Command;

use serde::{Deserialize, Serialize};

use super::FixResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainStatus {
    pub python3: bool,
    pub python_version: Option<String>,
    pub make: bool,
    pub gcc: bool,
    pub gpp: bool,
    pub complete: bool,
    pub missing: Vec<String>,
    pub apt_command: String,
}

pub fn check_native_toolchain() -> ToolchainStatus {
    let python_version = run_version("python3", &["--version"]);
    let python3 = python_version.is_some();
    let make = which("make");
    let gcc = which("gcc");
    let gpp = which("g++");

    let mut missing = Vec::new();
    if !python3 {
        missing.push("python3".into());
    }
    if !make {
        missing.push("make".into());
    }
    if !gcc {
        missing.push("gcc".into());
    }
    if !gpp {
        missing.push("g++".into());
    }

    ToolchainStatus {
        python3,
        python_version,
        make,
        gcc,
        gpp,
        complete: missing.is_empty(),
        missing: missing.clone(),
        apt_command: "sudo apt-get install -y build-essential python3".into(),
    }
}

pub fn install_build_tools() -> Result<FixResult, String> {
    let status = check_native_toolchain();
    if status.complete {
        return Ok(FixResult {
            success: true,
            message: "Native build tools already present.".into(),
        });
    }

    // Prefer pkexec for GUI elevation
    let output = Command::new("pkexec")
        .args([
            "apt-get",
            "install",
            "-y",
            "build-essential",
            "python3",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => Ok(FixResult {
            success: true,
            message: format!(
                "Installed build tools. Previously missing: {}",
                status.missing.join(", ")
            ),
        }),
        Ok(out) => Ok(FixResult {
            success: false,
            message: format!(
                "Could not install automatically.\n{}\nRun:\n  {}",
                String::from_utf8_lossy(&out.stderr),
                status.apt_command
            ),
        }),
        Err(_) => Ok(FixResult {
            success: false,
            message: format!(
                "pkexec unavailable. Install manually:\n  {}",
                status.apt_command
            ),
        }),
    }
}

fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_version(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    // Python 3.12.3
    let ver = text
        .split_whitespace()
        .find(|t| t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
        .map(|s| s.trim_start_matches('v').to_string())?;
    Some(ver)
}
