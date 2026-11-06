use std::process::Command;

use serde::{Deserialize, Serialize};

use super::project::ProjectInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonEnvironment {
    pub available: bool,
    pub version: Option<String>,
    pub which: Option<String>,
    pub required: Option<String>,
    pub mismatch: bool,
    pub guidance: String,
}

pub fn get_python_environment(project: Option<&ProjectInfo>) -> PythonEnvironment {
    let which = Command::new("which")
        .arg("python3")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());

    let version = Command::new("python3")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            );
            text.split_whitespace()
                .find(|t| t.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
                .map(|s| s.to_string())
        });

    let required = project.and_then(|p| p.python_required.clone());
    let mismatch = match (&version, &required) {
        (Some(active), Some(req)) => version_mismatch(active, req),
        (None, Some(_)) => true,
        _ => false,
    };

    let guidance = if mismatch {
        "Install or activate the required Python (pyenv/apt). DevMonitor will not replace system Python."
            .into()
    } else if version.is_none() {
        "python3 not found. Install with: sudo apt-get install -y python3".into()
    } else {
        "Python looks fine for this project.".into()
    };

    PythonEnvironment {
        available: version.is_some(),
        version,
        which,
        required,
        mismatch,
        guidance,
    }
}

fn version_mismatch(active: &str, required: &str) -> bool {
    let req = required.trim().trim_start_matches('v');
    // Exact / major.minor from .python-version "3.11"
    if !req.contains('>') && !req.contains('<') && !req.contains('=') {
        let major_minor: String = req.split('.').take(2).collect::<Vec<_>>().join(".");
        let active_mm: String = active.split('.').take(2).collect::<Vec<_>>().join(".");
        return active_mm != major_minor;
    }
    if let Some(rest) = req.strip_prefix(">=") {
        let min = rest
            .split([',', ' '])
            .next()
            .unwrap_or(rest)
            .trim()
            .trim_matches('"');
        return compare(active, min) < 0;
    }
    false
}

fn compare(a: &str, b: &str) -> i32 {
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
