use std::process::Command;

use serde::{Deserialize, Serialize};

use super::FixResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerDisk {
    pub available: bool,
    pub raw: String,
    pub images: Option<String>,
    pub containers: Option<String>,
    pub volumes: Option<String>,
    pub build_cache: Option<String>,
    pub note: String,
}

pub fn get_docker_disk() -> DockerDisk {
    if !which("docker") {
        return DockerDisk {
            available: false,
            raw: String::new(),
            images: None,
            containers: None,
            volumes: None,
            build_cache: None,
            note: "Docker CLI not found on PATH.".into(),
        };
    }

    match Command::new("docker").args(["system", "df"]).output() {
        Ok(out) if out.status.success() => {
            let raw = String::from_utf8_lossy(&out.stdout).to_string();
            let mut images = None;
            let mut containers = None;
            let mut volumes = None;
            let mut build_cache = None;
            for line in raw.lines().skip(1) {
                let lower = line.to_lowercase();
                if lower.starts_with("images") {
                    images = Some(line.to_string());
                } else if lower.starts_with("containers") {
                    containers = Some(line.to_string());
                } else if lower.starts_with("local volumes") || lower.starts_with("volumes") {
                    volumes = Some(line.to_string());
                } else if lower.starts_with("build cache") {
                    build_cache = Some(line.to_string());
                }
            }
            DockerDisk {
                available: true,
                raw,
                images,
                containers,
                volumes,
                build_cache,
                note: "Volumes are never pruned by DevMonitor's safe clean.".into(),
            }
        }
        Ok(out) => DockerDisk {
            available: true,
            raw: String::from_utf8_lossy(&out.stderr).to_string(),
            images: None,
            containers: None,
            volumes: None,
            build_cache: None,
            note: "Docker is installed but `docker system df` failed (daemon running?).".into(),
        },
        Err(e) => DockerDisk {
            available: false,
            raw: e.to_string(),
            images: None,
            containers: None,
            volumes: None,
            build_cache: None,
            note: "Failed to run docker.".into(),
        },
    }
}

/// Safe prune: never includes `--volumes`.
pub fn prune_docker(include_build_cache: bool) -> Result<FixResult, String> {
    if !which("docker") {
        return Ok(FixResult {
            success: false,
            message: "Docker not found on PATH".into(),
        });
    }

    let out = Command::new("docker")
        .args(["system", "prune", "-f"])
        .output()
        .map_err(|e| e.to_string())?;

    if !out.status.success() {
        return Ok(FixResult {
            success: false,
            message: format!(
                "docker system prune failed:\n{}",
                String::from_utf8_lossy(&out.stderr)
            ),
        });
    }

    let mut message = format!(
        "Pruned unused containers/networks/images (volumes kept).\n{}",
        String::from_utf8_lossy(&out.stdout)
    );

    if include_build_cache {
        let cache = Command::new("docker")
            .args(["builder", "prune", "-f"])
            .output()
            .map_err(|e| e.to_string())?;
        if cache.status.success() {
            message.push_str(&format!(
                "\nBuild cache pruned.\n{}",
                String::from_utf8_lossy(&cache.stdout)
            ));
        } else {
            return Ok(FixResult {
                success: false,
                message: format!(
                    "{message}\nbuilder prune failed:\n{}",
                    String::from_utf8_lossy(&cache.stderr)
                ),
            });
        }
    }

    Ok(FixResult {
        success: true,
        message,
    })
}

fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
