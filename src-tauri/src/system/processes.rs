use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::{ProcessesToUpdate, ProcessRefreshKind, System, UpdateKind};

use super::AppState;

const PROCESS_LIMIT: usize = 250;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProcessCategory {
    App,
    Dev,
    Safe,
    System,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub display_name: String,
    pub cpu_usage: f32,
    /// Resident RAM (RSS), the same figure GNOME System Monitor shows per process.
    pub memory: u64,
    /// Proportional share (PSS). App rows add this so shared pages are counted once.
    pub unique_memory: u64,
    pub category: ProcessCategory,
    pub safe_to_kill: bool,
    pub is_user: bool,
    pub exe_path: Option<String>,
    pub reason: String,
    /// Stable key for grouping related processes under one app.
    pub group_id: String,
    pub group_label: String,
    pub parent_pid: Option<u32>,
}

pub fn get_processes(state: &AppState) -> Result<Vec<ProcessInfo>, String> {
    let mut system = state.system.lock().map_err(|e| e.to_string())?;

    // Do not ask sysinfo for per-process CPU here. A second refresh in the
    // same poll divides a tiny time slice and reports thousands of percent.
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_memory()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_user(UpdateKind::OnlyIfNotSet),
    );

    let my_uid = unsafe { libc::getuid() };
    let desktop_apps = desktop_app_names();

    let mut processes: Vec<ProcessInfo> = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let name = process.name().to_string_lossy().to_string();
            if name.is_empty() {
                return None;
            }
            let exe = process
                .exe()
                .map(|p| p.to_string_lossy().to_string());
            let uid = process.user_id().map(|u| **u);
            let is_user = uid == Some(my_uid);
            let (category, safe_to_kill, reason, display_name) =
                classify(&name, exe.as_deref(), is_user, pid.as_u32(), desktop_apps);
            let (group_id, group_label) =
                group_for(&name, exe.as_deref(), desktop_apps, &display_name);
            let parent_pid = process.parent().map(|p| p.as_u32());

            let pid_u32 = pid.as_u32();
            Some(ProcessInfo {
                pid: pid_u32,
                name: name.clone(),
                display_name,
                cpu_usage: 0.0,
                // RSS for the process row. PSS is filled after truncate so we
                // do not read smaps_rollup for every process on the machine.
                memory: process.memory(),
                unique_memory: process.memory(),
                category,
                safe_to_kill,
                is_user,
                exe_path: exe,
                reason,
                group_id,
                group_label,
                parent_pid,
            })
        })
        .collect();

    // Prefer useful rows first, then CPU
    processes.sort_by(|a, b| {
        category_rank(&a.category)
            .cmp(&category_rank(&b.category))
            .then_with(|| {
                b.cpu_usage
                    .partial_cmp(&a.cpu_usage)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| b.memory.cmp(&a.memory))
    });

    processes.truncate(PROCESS_LIMIT);

    let shares = machine_cpu_shares(processes.iter().map(|p| p.pid));
    for p in &mut processes {
        p.cpu_usage = shares.get(&p.pid).copied().unwrap_or(0.0);
    }

    // Unique share for app totals. Process rows keep RSS.
    for p in &mut processes {
        p.unique_memory = unique_memory_bytes(p.pid, p.memory);
    }

    Ok(processes)
}

pub fn find_process_by_pid(system: &System, pid: u32) -> Option<ProcessInfo> {
    let pid = sysinfo::Pid::from_u32(pid);
    let process = system.process(pid)?;
    let name = process.name().to_string_lossy().to_string();
    let exe = process.exe().map(|p| p.to_string_lossy().to_string());
    let my_uid = unsafe { libc::getuid() };
    let is_user = process.user_id().map(|u| **u) == Some(my_uid);
    let desktop_apps = desktop_app_names();
    let (category, safe_to_kill, reason, display_name) =
        classify(&name, exe.as_deref(), is_user, pid.as_u32(), desktop_apps);
    let (group_id, group_label) =
        group_for(&name, exe.as_deref(), desktop_apps, &display_name);
    let pid_u32 = pid.as_u32();
    let rss = process.memory();
    Some(ProcessInfo {
        pid: pid_u32,
        name,
        display_name,
        cpu_usage: cached_cpu_share(pid_u32),
        memory: rss,
        unique_memory: unique_memory_bytes(pid_u32, rss),
        category,
        safe_to_kill,
        is_user,
        exe_path: exe,
        reason,
        group_id,
        group_label,
        parent_pid: process.parent().map(|p| p.as_u32()),
    })
}

struct CpuTracker {
    at: Option<Instant>,
    total: u64,
    procs: HashMap<u32, u64>,
    shares: HashMap<u32, f32>,
}

fn cpu_tracker() -> &'static Mutex<CpuTracker> {
    static TRACKER: OnceLock<Mutex<CpuTracker>> = OnceLock::new();
    TRACKER.get_or_init(|| {
        Mutex::new(CpuTracker {
            at: None,
            total: 0,
            procs: HashMap::new(),
            shares: HashMap::new(),
        })
    })
}

/// Percent of this computer, 0–100. 100 means the whole machine, not one core.
fn machine_cpu_shares(pids: impl Iterator<Item = u32>) -> HashMap<u32, f32> {
    let pids: Vec<u32> = pids.collect();
    let Ok(mut tracker) = cpu_tracker().lock() else {
        return HashMap::new();
    };
    let now = Instant::now();
    if tracker
        .at
        .is_some_and(|at| now.duration_since(at) < Duration::from_millis(700))
        && !tracker.shares.is_empty()
    {
        return tracker.shares.clone();
    }

    let Some(total) = read_total_jiffies() else {
        return tracker.shares.clone();
    };
    let elapsed_ticks = total.saturating_sub(tracker.total);
    let mut procs = HashMap::with_capacity(pids.len());
    let mut shares = HashMap::with_capacity(pids.len());
    let have_baseline = tracker.total > 0 && elapsed_ticks > 0;
    for pid in pids {
        let Some(ticks) = read_proc_jiffies(pid) else {
            continue;
        };
        if have_baseline {
            if let Some(prev) = tracker.procs.get(&pid) {
                let used = ticks.saturating_sub(*prev);
                let share = (used as f32 / elapsed_ticks as f32) * 100.0;
                shares.insert(pid, share.clamp(0.0, 100.0));
            }
        }
        procs.insert(pid, ticks);
    }
    if shares.is_empty() {
        shares = tracker.shares.clone();
    }
    tracker.at = Some(now);
    tracker.total = total;
    tracker.procs = procs;
    tracker.shares = shares.clone();
    shares
}

fn cached_cpu_share(pid: u32) -> f32 {
    cpu_tracker()
        .lock()
        .ok()
        .and_then(|tracker| tracker.shares.get(&pid).copied())
        .unwrap_or(0.0)
}

/// Sum of every CPU's time from `/proc/stat`, in the same ticks as process stat.
fn read_total_jiffies() -> Option<u64> {
    let content = fs::read_to_string("/proc/stat").ok()?;
    let line = content.lines().next()?;
    let mut sum = 0u64;
    let mut any = false;
    for part in line.split_whitespace().skip(1) {
        let Ok(value) = part.parse::<u64>() else {
            continue;
        };
        sum = sum.saturating_add(value);
        any = true;
    }
    any.then_some(sum)
}

/// utime + stime from `/proc/<pid>/stat`.
fn read_proc_jiffies(pid: u32) -> Option<u64> {
    let content = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = content.rsplit_once(')')?.1;
    let mut fields = rest.split_whitespace();
    // 0 state … 11 utime, 12 stime
    let utime: u64 = fields.nth(11)?.parse().ok()?;
    let stime: u64 = fields.next()?.parse().ok()?;
    Some(utime.saturating_add(stime))
}

/// Proportional set size (PSS) when available — shared pages split across processes.
/// Falls back to RSS if `/proc/<pid>/smaps_rollup` is missing or unreadable.
fn unique_memory_bytes(pid: u32, rss: u64) -> u64 {
    read_pss_bytes(pid).unwrap_or(rss)
}

fn read_pss_bytes(pid: u32) -> Option<u64> {
    let content = fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?;
    for line in content.lines() {
        let Some(rest) = line.strip_prefix("Pss:") else {
            continue;
        };
        let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
        return Some(kb.saturating_mul(1024));
    }
    None
}

/// Group related processes (browser helpers, electron children, snap apps).
fn group_for(
    name: &str,
    exe: Option<&str>,
    desktop_apps: &HashSet<String>,
    display: &str,
) -> (String, String) {
    let lower = name.to_lowercase();
    let base = exe
        .and_then(|p| Path::new(p).file_name())
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_lowercase();

    if let Some(exe) = exe {
        // /snap/<name>/...
        if let Some(rest) = exe.strip_prefix("/snap/") {
            let snap = rest.split('/').next().unwrap_or(rest);
            if !snap.is_empty() {
                return (format!("snap:{snap}"), title_case(snap));
            }
        }
        // flatpak app ids in path
        if exe.contains("/flatpak/app/") {
            if let Some(app) = exe.split("/flatpak/app/").nth(1) {
                let id = app.split('/').next().unwrap_or(app);
                let label = id.split('.').next_back().unwrap_or(id);
                return (format!("flatpak:{id}"), title_case(label));
            }
        }
        // node project tools share node binary — group by first meaningful path segment
        if is_dev_tool(&lower, &base, Some(exe)) {
            if exe.contains("/node_modules/") {
                // group by project root above node_modules
                if let Some(idx) = exe.find("/node_modules/") {
                    let root = &exe[..idx];
                    let proj = Path::new(root)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("node");
                    return (format!("dev:{root}"), format!("{proj} (Node)"));
                }
            }
            if base == "node" || base == "nodejs" {
                return ("dev:node".into(), "Node.js".into());
            }
            if matches!(base.as_str(), "npm" | "npx" | "yarn" | "pnpm" | "bun") {
                return (format!("dev:{base}"), title_case(&base));
            }
            if matches!(base.as_str(), "cargo" | "rustc" | "rust-analyzer") {
                return ("dev:rust".into(), "Rust".into());
            }
            return (format!("dev:{base}"), title_case(&base));
        }
    }

    // Desktop-installed apps
    for candidate in [&base, &lower] {
        let stem = candidate.split('.').next().unwrap_or(candidate);
        let normalized = normalize_app_stem(stem);
        if desktop_apps.contains(stem)
            || desktop_apps.contains(&normalized)
            || desktop_apps.contains(candidate)
        {
            let label = title_case(&normalized);
            return (format!("app:{normalized}"), label);
        }
    }

    // Browser / Electron helpers → main app
    let normalized = normalize_app_stem(&base);
    let from_name = normalize_app_stem(&lower);
    let key = if normalized.len() >= 2 {
        normalized
    } else {
        from_name
    };
    if key.len() >= 2 {
        let label = title_case(&key);
        return (format!("proc:{key}"), label);
    }

    (format!("pid-lone:{}", display.to_lowercase()), display.to_string())
}

fn normalize_app_stem(raw: &str) -> String {
    let mut s = raw.to_lowercase();
    for suffix in [
        " helper",
        " helper (renderer)",
        " helper (gpu)",
        " helper (plugin)",
        " helper (renderer)",
        "-helper",
        "_helper",
        " web content",
        " gpu process",
        " rdd process",
        " privileged content",
        " content proc",
        ".bin",
    ] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.to_string();
        }
    }
    // chrome_crashpad_handler → chrome
    if s.contains("crashpad") {
        if s.contains("chrome") || s.contains("chromium") {
            return "chrome".into();
        }
        if s.contains("code") {
            return "code".into();
        }
    }
    if s.starts_with("chrome") || s.starts_with("google-chrome") {
        return "chrome".into();
    }
    if s.starts_with("chromium") {
        return "chromium".into();
    }
    if s.starts_with("firefox") {
        return "firefox".into();
    }
    if s.starts_with("code") || s == "code-oss" {
        return "code".into();
    }
    if s.starts_with("cursor") {
        return "cursor".into();
    }
    if s.starts_with("slack") {
        return "slack".into();
    }
    if s.starts_with("discord") {
        return "discord".into();
    }
    if s.starts_with("spotify") {
        return "spotify".into();
    }
    s
}

fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

pub fn is_protected_process(pid: u32, name: &str) -> bool {
    if pid <= 2 {
        return true;
    }
    let lower = name.to_lowercase();
    is_kernel_thread(&lower) || protected_name(&lower)
}

fn category_rank(c: &ProcessCategory) -> u8 {
    match c {
        ProcessCategory::App => 0,
        ProcessCategory::Dev => 1,
        ProcessCategory::Safe => 2,
        ProcessCategory::Other => 3,
        ProcessCategory::System => 4,
    }
}

fn classify(
    name: &str,
    exe: Option<&str>,
    is_user: bool,
    pid: u32,
    desktop_apps: &HashSet<String>,
) -> (ProcessCategory, bool, String, String) {
    let lower = name.to_lowercase();
    let base = exe
        .and_then(|p| Path::new(p).file_name())
        .and_then(|s| s.to_str())
        .unwrap_or(name)
        .to_lowercase();
    let display = pretty_name(name, exe);

    if pid <= 2 || is_kernel_thread(&lower) || protected_name(&lower) || protected_exe(exe) {
        return (
            ProcessCategory::System,
            false,
            "Critical system process — ending it can freeze or crash the session".into(),
            display,
        );
    }

    if is_dev_tool(&lower, &base, exe) {
        return (
            ProcessCategory::Dev,
            is_user,
            if is_user {
                "Developer tool — usually safe to end if you started it".into()
            } else {
                "Developer-related process owned by another user".into()
            },
            display,
        );
    }

    if is_installed_app(&lower, &base, exe, desktop_apps) {
        return (
            ProcessCategory::App,
            is_user,
            if is_user {
                "Installed / desktop app — safe to end from here".into()
            } else {
                "Desktop app owned by another user".into()
            },
            display,
        );
    }

    if is_user && !looks_like_system_helper(&lower, exe) {
        return (
            ProcessCategory::Safe,
            true,
            "Your process — generally safe to end".into(),
            display,
        );
    }

    if looks_like_system_helper(&lower, exe) {
        return (
            ProcessCategory::System,
            false,
            "System service helper — leave running".into(),
            display,
        );
    }

    (
        ProcessCategory::Other,
        false,
        "Unclassified — end only if you recognize it".into(),
        display,
    )
}

fn is_kernel_thread(name: &str) -> bool {
    name.starts_with('[') && name.ends_with(']')
}

fn protected_name(name: &str) -> bool {
    const PROTECTED: &[&str] = &[
        "systemd",
        "init",
        "kthreadd",
        "dbus-daemon",
        "dbus-broker",
        "networkmanager",
        "snapd",
        "containerd",
        "dockerd",
        "containerd-shim",
        "xorg",
        "xwayland",
        "gdm",
        "gdm3",
        "gdm-session-worker",
        "lightdm",
        "sddm",
        "gnome-shell",
        "gnome-session",
        "gnome-session-binary",
        "pipewire",
        "pipewire-pulse",
        "wireplumber",
        "pulseaudio",
        "sshd",
        "cron",
        "crond",
        "rsyslogd",
        "systemd-journald",
        "systemd-logind",
        "systemd-resolved",
        "systemd-udevd",
        "polkitd",
        "udisksd",
        "accounts-daemon",
        "fwupd",
        "packagekitd",
        "cupsd",
        "bluetoothd",
        "modemmanager",
        "iio-sensor-proxy",
        "rtkit-daemon",
        "colord",
        "upowerd",
        "login",
    ];
    if PROTECTED.iter().any(|p| name == *p) {
        return true;
    }
    name.starts_with("systemd-")
        || name.starts_with("kworker")
        || name.starts_with("ksoftirq")
        || name.starts_with("migration/")
        || name.starts_with("rcu_")
        || name.starts_with("gdm-")
}

fn protected_exe(exe: Option<&str>) -> bool {
    let Some(exe) = exe else { return false };
    exe.starts_with("/usr/lib/systemd/")
        || exe.starts_with("/lib/systemd/")
        || exe.contains("/gdm/")
        || exe.contains("/dbus-1/")
}

fn is_dev_tool(name: &str, base: &str, exe: Option<&str>) -> bool {
    const DEV: &[&str] = &[
        "node", "nodejs", "npm", "npx", "yarn", "pnpm", "bun", "deno", "vite", "esbuild",
        "webpack", "webpack-dev-server", "next-server", "next", "turbo", "cargo", "rustc",
        "rust-analyzer", "go", "python", "python3", "pip", "pip3", "ruby", "php", "java",
        "javac", "mvn", "gradle", "docker", "docker-compose", "compose", "podman", "code",
        "cursor", "code-insiders", "electron", "tsserver", "eslint", "prettier", "watchman",
        "ngrok", "cloudflared",
    ];
    if DEV.iter().any(|d| name == *d || base == *d || name.starts_with(&format!("{d}-"))) {
        return true;
    }
    let Some(exe) = exe else { return false };
    exe.contains("/node_modules/")
        || exe.contains("/.nvm/")
        || exe.contains("/.fnm/")
        || exe.contains("/.cargo/")
        || exe.contains("/.local/share/fnm/")
        || exe.contains("/DevMonitor/")
        || exe.contains("/target/debug/")
        || exe.contains("/target/release/")
}

fn is_installed_app(
    name: &str,
    base: &str,
    exe: Option<&str>,
    desktop_apps: &HashSet<String>,
) -> bool {
    if desktop_apps.contains(name) || desktop_apps.contains(base) {
        return true;
    }
    // Strip .exe-like snap revisions: firefox, chromium, etc.
    let stripped = base.split('.').next().unwrap_or(base);
    if desktop_apps.contains(stripped) {
        return true;
    }
    let Some(exe) = exe else { return false };
    if exe.contains("/snap/") || exe.contains("/flatpak/") || exe.starts_with("/opt/") {
        return true;
    }
    // Common GUI locations with a matching .desktop basename
    if (exe.starts_with("/usr/bin/") || exe.starts_with("/usr/local/bin/"))
        && desktop_apps.contains(base)
    {
        return true;
    }
    false
}

fn looks_like_system_helper(name: &str, exe: Option<&str>) -> bool {
    if name.ends_with("d") && name.len() > 3 {
        // many daemons end with d — treat carefully when not user apps
        if name.contains("daemon") || name.ends_with("helper") {
            return true;
        }
    }
    let Some(exe) = exe else { return false };
    exe.starts_with("/usr/lib/")
        || exe.starts_with("/usr/libexec/")
        || exe.starts_with("/lib/")
        || exe.contains("/xdg-desktop-portal")
}

fn pretty_name(name: &str, exe: Option<&str>) -> String {
    if let Some(exe) = exe {
        if let Some(file) = Path::new(exe).file_name().and_then(|s| s.to_str()) {
            if !file.is_empty() && file != name {
                return file.to_string();
            }
        }
    }
    name.to_string()
}

fn desktop_app_names() -> &'static HashSet<String> {
    static APPS: OnceLock<HashSet<String>> = OnceLock::new();
    APPS.get_or_init(scan_desktop_apps)
}

fn scan_desktop_apps() -> HashSet<String> {
    let mut out = HashSet::new();
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
        PathBuf::from(format!("{home}/.local/share/applications")),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        PathBuf::from(format!(
            "{home}/.local/share/flatpak/exports/share/applications"
        )),
    ];

    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            // Skip hidden / NoDisplay
            if text.lines().any(|l| {
                let t = l.trim();
                t.eq_ignore_ascii_case("NoDisplay=true") || t.eq_ignore_ascii_case("Hidden=true")
            }) {
                continue;
            }
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("Exec=") {
                    if let Some(bin) = exec_basename(rest) {
                        out.insert(bin);
                    }
                }
                if let Some(rest) = line.strip_prefix("TryExec=") {
                    if let Some(bin) = exec_basename(rest) {
                        out.insert(bin);
                    }
                }
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                // firefox_firefox.desktop → firefox
                let stem = stem.split('_').next().unwrap_or(stem).to_lowercase();
                out.insert(stem);
            }
        }
    }

    // Always include common apps even if .desktop parse missed them
    for extra in [
        "firefox", "chromium", "google-chrome", "chrome", "code", "cursor", "slack",
        "discord", "spotify", "nautilus", "files", "gnome-terminal", "kitty", "alacritty",
        "tilix", "vlc", "evince", "eog", "gedit", "libreoffice", "thunderbird", "zoom",
        "teams", "postman", "insomnia", "obs", "gimp", "inkscape", "devmonitor",
    ] {
        out.insert(extra.into());
    }
    out
}

fn exec_basename(exec: &str) -> Option<String> {
    // Exec may be: env FOO=1 /usr/bin/foo --bar %U
    let mut parts = exec.split_whitespace();
    while let Some(p) = parts.next() {
        if p.contains('=') && !p.starts_with('/') {
            continue; // env assignments
        }
        if p == "env" {
            continue;
        }
        let path = p.trim_matches('"');
        let base = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())?
            .to_lowercase();
        if base.is_empty() {
            return None;
        }
        return Some(base);
    }
    None
}
