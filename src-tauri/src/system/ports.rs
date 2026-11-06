use std::process::Command;

use regex::Regex;
use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};

use super::processes::find_process_by_pid;
use super::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListeningPort {
    pub port: u16,
    pub address: String,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub cpu_usage: Option<f32>,
    pub memory: Option<u64>,
    pub permission_limited: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListeningPortsResult {
    pub ports: Vec<ListeningPort>,
    pub warning: Option<String>,
}

pub fn get_listening_ports(state: &AppState) -> Result<ListeningPortsResult, String> {
    let output = Command::new("ss")
        .args(["-ltnp"])
        .output()
        .map_err(|e| format!("Failed to run ss: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ss failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut warning = None;
    let mut ports = parse_ss_output(&stdout);

    let mut system = state.system.lock().map_err(|e| e.to_string())?;
    // Names and memory only. CPU share comes from the process sampler.
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_memory()
            .with_exe(UpdateKind::OnlyIfNotSet),
    );

    let mut limited_count = 0usize;
    for port in &mut ports {
        match port.pid {
            Some(pid) => {
                if let Some(info) = find_process_by_pid(&system, pid) {
                    port.process_name = Some(info.name);
                    port.cpu_usage = Some(info.cpu_usage);
                    port.memory = Some(info.memory);
                    port.permission_limited = false;
                } else if port.process_name.is_none() {
                    port.permission_limited = true;
                    limited_count += 1;
                }
            }
            None => {
                if port.process_name.is_none() {
                    port.permission_limited = true;
                    limited_count += 1;
                }
            }
        }
    }

    if limited_count > 0 {
        warning = Some(format!(
            "{limited_count} listening port(s) have limited process info due to permissions"
        ));
    }

    ports.sort_by(|a, b| a.port.cmp(&b.port).then_with(|| a.address.cmp(&b.address)));
    ports.dedup_by(|a, b| a.port == b.port && a.pid == b.pid && a.address == b.address);

    Ok(ListeningPortsResult { ports, warning })
}

fn parse_ss_output(stdout: &str) -> Vec<ListeningPort> {
    // Local Address:Port is typically column 4; users:((("name",pid=123,fd=4))) at end
    let users_re =
        Regex::new(r#"users:\(\((?:"([^"]+)",)?pid=(\d+)"#).expect("valid regex");
    let port_re = Regex::new(r":(\d+)$").expect("valid regex");

    let mut ports = Vec::new();

    for line in stdout.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let local = parts[3];
        let Some(port_cap) = port_re.captures(local) else {
            continue;
        };
        let Ok(port) = port_cap[1].parse::<u16>() else {
            continue;
        };

        let mut pid = None;
        let mut process_name = None;
        let mut permission_limited = true;

        if let Some(users_part) = parts.iter().find(|p| p.starts_with("users:(")) {
            if let Some(caps) = users_re.captures(users_part) {
                if let Some(name) = caps.get(1) {
                    process_name = Some(name.as_str().to_string());
                }
                if let Ok(parsed) = caps[2].parse::<u32>() {
                    pid = Some(parsed);
                    permission_limited = false;
                }
            }
        } else if line.contains("users:((") {
            // Malformed / truncated users field
            permission_limited = true;
        }

        ports.push(ListeningPort {
            port,
            address: local.to_string(),
            pid,
            process_name,
            cpu_usage: None,
            memory: None,
            permission_limited,
        });
    }

    ports
}

#[cfg(test)]
mod tests {
    use super::parse_ss_output;

    #[test]
    fn parses_ss_line_with_process() {
        let sample = "Netid State Recv-Q Send-Q Local Address:Port Peer Address:Port Process\nLISTEN 0 128 127.0.0.1:5173 0.0.0.0:* users:((\"vite\",pid=9012,fd=21))\n";
        let ports = parse_ss_output(sample);
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 5173);
        assert_eq!(ports[0].pid, Some(9012));
        assert_eq!(ports[0].process_name.as_deref(), Some("vite"));
    }
}
