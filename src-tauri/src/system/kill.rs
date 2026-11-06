use std::fs;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KillResult {
    pub success: bool,
    pub message: String,
}

pub fn kill_process(pid: u32) -> Result<KillResult, String> {
    if pid == 0 || pid == 1 {
        return Ok(KillResult {
            success: false,
            message: "Refusing to terminate system process".into(),
        });
    }

    let proc_path = PathBuf::from(format!("/proc/{pid}"));
    if !proc_path.exists() {
        return Ok(KillResult {
            success: false,
            message: format!("Process {pid} not found"),
        });
    }

    // Validate it looks like a real process directory
    let _ = fs::read_to_string(proc_path.join("stat")).map_err(|_| {
        format!("Cannot access process {pid} (permission denied or gone)")
    })?;

    // Best-effort name check against protected system processes
    if let Ok(comm) = fs::read_to_string(proc_path.join("comm")) {
        let name = comm.trim();
        if crate::system::processes::is_protected_process(pid, name) {
            return Ok(KillResult {
                success: false,
                message: format!(
                    "Refusing to end “{name}” — it looks like a protected system process"
                ),
            });
        }
    }

    let result = unsafe { libc::kill(pid as i32, libc::SIGTERM) };

    if result == 0 {
        Ok(KillResult {
            success: true,
            message: format!("Sent SIGTERM to process {pid}"),
        })
    } else {
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EPERM) => Ok(KillResult {
                success: false,
                message: format!(
                    "Permission denied: cannot terminate process {pid} without elevated privileges"
                ),
            }),
            Some(libc::ESRCH) => Ok(KillResult {
                success: false,
                message: format!("Process {pid} not found"),
            }),
            _ => Ok(KillResult {
                success: false,
                message: format!("Failed to terminate process {pid}: {err}"),
            }),
        }
    }
}
