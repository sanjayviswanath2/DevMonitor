pub mod kill;
pub mod memory_tools;
pub mod ports;
pub mod processes;
pub mod stats;

use std::sync::Mutex;
use sysinfo::System;

/// Shared system handle so CPU usage can be computed across refreshes.
pub struct AppState {
    pub system: Mutex<System>,
}

impl AppState {
    pub fn new() -> Self {
        // Avoid System::new_all() — it eagerly opens handles for every process
        // and can pin thousands of /proc FDs on a loaded desktop session.
        let mut system = System::new();
        system.refresh_memory();
        system.refresh_cpu_all();
        Self {
            system: Mutex::new(system),
        }
    }
}
