use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use super::deps;
use super::disk;
use super::docker;
use super::envfile;
use super::git;
use super::heap;
use super::node;
use super::project;
use super::python;
use super::toolchain;
use super::watchers;
use super::{DoctorIssue, IssueAction, IssueSeverity, ScanResult};
use crate::system::ports;
use crate::system::AppState;

const DEV_PORT_HINTS: &[u16] = &[
    3000, 3001, 4000, 4173, 4200, 4321, 5000, 5173, 5174, 6006, 8000, 8080, 8888,
];

pub fn scan_issues(state: &AppState, project_path: Option<&str>) -> Result<ScanResult, String> {
    let mut issues = Vec::new();

    let project = match project_path {
        Some(p) if !p.is_empty() => Some(project::inspect_project(p)?),
        _ => None,
    };

    let node_env = node::detect_node_environment();

    // 1) Dev ports in use
    if let Ok(ports_result) = ports::get_listening_ports(state) {
        for port in ports_result.ports {
            let is_dev_port = DEV_PORT_HINTS.contains(&port.port);
            let name = port.process_name.as_deref().unwrap_or("").to_lowercase();
            let looks_dev = ["node", "vite", "next", "ng", "python", "uvicorn", "webpack", "deno", "bun"]
                .iter()
                .any(|n| name.contains(n));
            if !(is_dev_port || looks_dev) {
                continue;
            }
            if port.pid.is_none() && port.permission_limited {
                continue;
            }
            let proc_label = port
                .process_name
                .clone()
                .unwrap_or_else(|| "Unknown process".into());
            let mem = port
                .memory
                .map(|m| format!("{:.0} MB", m as f64 / 1_048_576.0))
                .unwrap_or_else(|| "—".into());
            issues.push(DoctorIssue {
                id: format!("port-{}", port.port),
                kind: "port_in_use".into(),
                severity: IssueSeverity::Warning,
                title: format!("Port {} is occupied", port.port),
                summary: format!("{proc_label} is listening on port {}", port.port),
                details: vec![
                    format!("Port {}", port.port),
                    format!("Process {proc_label}"),
                    format!("PID {}", port.pid.map(|p| p.to_string()).unwrap_or_else(|| "—".into())),
                    format!("Memory {mem}"),
                ],
                action: port.pid.map(|_| IssueAction {
                    id: "free_port".into(),
                    label: "Free Port".into(),
                    confirm: true,
                    confirm_message: Some(format!(
                        "Terminate the process using port {} ({proc_label})?",
                        port.port
                    )),
                }),
                meta: json!({ "port": port.port, "pid": port.pid, "processName": port.process_name }),
            });
        }
    }

    // 2) Node missing / PATH
    if !node_env.node_available {
        issues.push(DoctorIssue {
            id: "node-missing".into(),
            kind: "node_path".into(),
            severity: IssueSeverity::Error,
            title: "Node.js is not available".into(),
            summary: node_env
                .path_issue_detail
                .clone()
                .unwrap_or_else(|| "node was not found on PATH.".into()),
            details: vec![
                format!("Installations found: {}", node_env.installations.len()),
                format!("Manager: {}", node_env.manager.clone().unwrap_or_else(|| "none".into())),
            ],
            action: if !node_env.installations.is_empty() {
                Some(IssueAction {
                    id: "repair_node_path".into(),
                    label: "Repair PATH".into(),
                    confirm: false,
                    confirm_message: None,
                })
            } else {
                None
            },
            meta: json!({}),
        });
    } else if node_env.path_issue {
        issues.push(DoctorIssue {
            id: "node-path".into(),
            kind: "node_path".into(),
            severity: IssueSeverity::Warning,
            title: "Node PATH looks broken".into(),
            summary: node_env
                .path_issue_detail
                .clone()
                .unwrap_or_else(|| "PATH issue detected.".into()),
            details: vec![
                format!("which node: {}", node_env.which_node.clone().unwrap_or_else(|| "—".into())),
            ],
            action: Some(IssueAction {
                id: "repair_node_path".into(),
                label: "Repair PATH".into(),
                confirm: false,
                confirm_message: None,
            }),
            meta: json!({}),
        });
    }

    // 3) Node version mismatch (needs project)
    if let Some(ref proj) = project {
        if let (Some(active), Some(required)) =
            (node_env.active_node.clone(), proj.required_node.clone())
        {
            if node::version_mismatch(&active, &required) {
                let can_switch = matches!(node_env.manager.as_deref(), Some("nvm") | Some("fnm"));
                let target = normalize_target_version(&required);
                issues.push(DoctorIssue {
                    id: "node-mismatch".into(),
                    kind: "node_mismatch".into(),
                    severity: IssueSeverity::Warning,
                    title: "Node version mismatch".into(),
                    summary: format!("Project requires Node {required}; you are running {active}"),
                    details: vec![
                        format!("Active Node {active}"),
                        format!("Project requires {required}"),
                        format!(
                            "Managed by {}",
                            node_env.manager.clone().unwrap_or_else(|| node_env.install_method.clone())
                        ),
                    ],
                    action: if can_switch {
                        Some(IssueAction {
                            id: "switch_node".into(),
                            label: format!("Switch to Node {target}"),
                            confirm: true,
                            confirm_message: Some(format!(
                                "Switch active Node to {target} using {}?",
                                node_env.manager.clone().unwrap_or_else(|| "version manager".into())
                            )),
                        })
                    } else {
                        None
                    },
                    meta: json!({ "required": required, "active": active, "target": target }),
                });
            }
        }

        // Package manager mismatch
        if let Some((preferred, reason)) = project::pm_mismatch(proj) {
            let install_cmd = project::install_command(&preferred);
            issues.push(DoctorIssue {
                id: "pm-mismatch".into(),
                kind: "pm_mismatch".into(),
                severity: IssueSeverity::Warning,
                title: format!("Use {preferred} for this project"),
                summary: reason,
                details: vec![
                    format!("Preferred package manager: {preferred}"),
                    format!("Install with: {install_cmd}"),
                    "DevMonitor will not rewrite lockfiles or change global defaults.".into(),
                ],
                action: Some(IssueAction {
                    id: "copy_pm_install".into(),
                    label: format!("Copy `{install_cmd}`"),
                    confirm: false,
                    confirm_message: None,
                }),
                meta: json!({
                    "packageManager": preferred,
                    "installCommand": install_cmd,
                }),
            });
        }

        // Missing .env vars
        if let Some(gaps) = envfile::scan_env_gaps(proj) {
            let preview: Vec<String> = gaps.missing_keys.iter().take(8).cloned().collect();
            let more = gaps.missing_keys.len().saturating_sub(preview.len());
            let mut details = vec![
                format!("Example file: {}", gaps.example_file),
                format!(
                    "Present env files: {}",
                    if gaps.present_files.is_empty() {
                        "none".into()
                    } else {
                        gaps.present_files.join(", ")
                    }
                ),
                format!("Missing keys: {}", preview.join(", ")),
            ];
            if more > 0 {
                details.push(format!("…and {more} more"));
            }
            details.push("DevMonitor never writes secrets — copy keys from the example file.".into());
            issues.push(DoctorIssue {
                id: "env-gaps".into(),
                kind: "env_gaps".into(),
                severity: IssueSeverity::Warning,
                title: format!(
                    "{} missing env var{}",
                    gaps.missing_keys.len(),
                    if gaps.missing_keys.len() == 1 { "" } else { "s" }
                ),
                summary: format!(
                    "Keys in {} are not present in your .env files",
                    gaps.example_file
                ),
                details,
                action: None,
                meta: json!({
                    "exampleFile": gaps.example_file,
                    "missingKeys": gaps.missing_keys,
                    "projectPath": proj.path,
                }),
            });
        }

        // Python mismatch
        if proj.python_required.is_some() {
            let py = python::get_python_environment(Some(proj));
            if py.mismatch {
                issues.push(DoctorIssue {
                    id: "python-mismatch".into(),
                    kind: "python_mismatch".into(),
                    severity: IssueSeverity::Warning,
                    title: "Python version mismatch".into(),
                    summary: format!(
                        "Project wants Python {}; active is {}",
                        py.required.clone().unwrap_or_else(|| "?".into()),
                        py.version.clone().unwrap_or_else(|| "missing".into())
                    ),
                    details: vec![
                        py.guidance.clone(),
                        "Use pyenv or apt to install the required version; system Python is not replaced."
                            .into(),
                    ],
                    action: None,
                    meta: json!({
                        "required": py.required,
                        "active": py.version,
                    }),
                });
            }
        }

        // node-gyp toolchain when native deps hinted
        if proj.has_native_deps_hint || proj.has_package_json {
            let tc = toolchain::check_native_toolchain();
            if !tc.complete && proj.has_native_deps_hint {
                issues.push(DoctorIssue {
                    id: "toolchain-missing".into(),
                    kind: "native_toolchain".into(),
                    severity: IssueSeverity::Warning,
                    title: "Native build tools missing".into(),
                    summary: format!(
                        "node-gyp may fail — missing: {}",
                        tc.missing.join(", ")
                    ),
                    details: vec![
                        format!("Install with: {}", tc.apt_command),
                        "Prefers pkexec for elevated install when available.".into(),
                    ],
                    action: Some(IssueAction {
                        id: "install_build_tools".into(),
                        label: "Install build tools".into(),
                        confirm: true,
                        confirm_message: Some(format!(
                            "Install build-essential and python3 via pkexec?\n\n{}",
                            tc.apt_command
                        )),
                    }),
                    meta: json!({ "missing": tc.missing, "aptCommand": tc.apt_command }),
                });
            }
        }

        // 5) Broken dependencies
        if deps::deps_look_broken(proj) {
            issues.push(DoctorIssue {
                id: "deps-broken".into(),
                kind: "deps_broken".into(),
                severity: IssueSeverity::Warning,
                title: "Dependencies look inconsistent".into(),
                summary: if !proj.has_node_modules {
                    "Lockfile present but node_modules is missing.".into()
                } else {
                    "node_modules appears empty or incomplete.".into()
                },
                details: vec![
                    format!("Package manager: {}", proj.package_manager),
                    format!(
                        "Lockfile: {}",
                        proj.lockfile.clone().unwrap_or_else(|| "none".into())
                    ),
                    format!("node_modules: {}", if proj.has_node_modules { "present" } else { "missing" }),
                ],
                action: Some(IssueAction {
                    id: "clean_reinstall".into(),
                    label: "Clean & Reinstall".into(),
                    confirm: true,
                    confirm_message: Some(format!(
                        "Delete node_modules and reinstall with {}?",
                        proj.package_manager
                    )),
                }),
                meta: json!({ "packageManager": proj.package_manager }),
            });
        }

        // 6) Heap suggestion when memory pressure + node project
        let mut system = state.system.lock().map_err(|e| e.to_string())?;
        system.refresh_memory();
        let avail = system.available_memory();
        let total = system.total_memory();
        drop(system);
        if total > 0 {
            let used_pct = (1.0 - (avail as f64 / total as f64)) * 100.0;
            if used_pct > 85.0 && proj.has_package_json {
                let mb = heap::suggested_heap_mb(state);
                issues.push(DoctorIssue {
                    id: "heap-pressure".into(),
                    kind: "heap_oom".into(),
                    severity: IssueSeverity::Info,
                    title: "Memory pressure — Node builds may OOM".into(),
                    summary: format!(
                        "System memory is {:.0}% used. A higher Node heap may help large builds.",
                        used_pct
                    ),
                    details: vec![
                        format!("Suggested heap: {mb} MB"),
                        "Writes NODE_OPTIONS into .env.development.local".into(),
                    ],
                    action: Some(IssueAction {
                        id: "apply_heap".into(),
                        label: format!("Apply {mb} MB heap"),
                        confirm: true,
                        confirm_message: Some(format!(
                            "Set NODE_OPTIONS=--max-old-space-size={mb} in .env.development.local?"
                        )),
                    }),
                    meta: json!({ "mb": mb }),
                });
            }
        }
    }

    // 4) npm EACCES
    if node_env.npm_prefix_owner_root {
        issues.push(DoctorIssue {
            id: "npm-eacces".into(),
            kind: "npm_permissions".into(),
            severity: IssueSeverity::Warning,
            title: "npm permission risk (EACCES)".into(),
            summary: "Global npm prefix appears root-owned. Global installs may fail without sudo."
                .into(),
            details: vec![
                format!(
                    "npm prefix: {}",
                    node_env.npm_prefix.clone().unwrap_or_else(|| "—".into())
                ),
                "Preferred fix: use nvm/fnm or a user-owned npm prefix.".into(),
            ],
            action: Some(IssueAction {
                id: "repair_npm_permissions".into(),
                label: "Repair npm setup".into(),
                confirm: true,
                confirm_message: Some(
                    "Set a user-owned npm global prefix (~/.npm-global)?".into(),
                ),
            }),
            meta: json!({}),
        });
    }

    // 7) File watchers
    let watchers = watchers::watcher_status();
    if watchers.low {
        issues.push(DoctorIssue {
            id: "watchers-low".into(),
            kind: "watchers".into(),
            severity: IssueSeverity::Warning,
            title: "File watcher limit is low".into(),
            summary: format!(
                "inotify max_user_watches is {} (recommended {}).",
                watchers.current, watchers.recommended
            ),
            details: vec![
                "This can break Vite/Webpack/IDE file watching on Linux.".into(),
                format!("Will raise to {}", watchers.recommended),
            ],
            action: Some(IssueAction {
                id: "fix_watchers".into(),
                label: "Fix watchers".into(),
                confirm: true,
                confirm_message: Some(format!(
                    "Raise fs.inotify.max_user_watches from {} to {}?",
                    watchers.current, watchers.recommended
                )),
            }),
            meta: json!({
                "current": watchers.current,
                "recommended": watchers.recommended
            }),
        });
    }

    // 8) Low disk
    if let Some((total, avail, pct)) = disk::disk_usage_percent("/") {
        if pct >= 90.0 {
            let caches = disk::list_safe_caches();
            let cache_total: u64 = caches.iter().map(|c| c.size_bytes).sum();
            issues.push(DoctorIssue {
                id: "disk-low".into(),
                kind: "disk_low".into(),
                severity: if pct >= 95.0 {
                    IssueSeverity::Error
                } else {
                    IssueSeverity::Warning
                },
                title: "Low disk space".into(),
                summary: format!(
                    "Root filesystem is {pct:.0}% used ({:.1} GB free).",
                    avail as f64 / 1_073_741_824.0
                ),
                details: vec![
                    format!("Total {:.1} GB", total as f64 / 1_073_741_824.0),
                    format!(
                        "Safe developer caches: {:.1} MB",
                        cache_total as f64 / 1_048_576.0
                    ),
                ],
                action: if cache_total > 0 {
                    Some(IssueAction {
                        id: "clean_caches".into(),
                        label: "Clean safe caches".into(),
                        confirm: true,
                        confirm_message: Some(
                            "Delete known developer caches under your home directory?".into(),
                        ),
                    })
                } else {
                    None
                },
                meta: json!({
                    "percent": pct,
                    "cacheIds": caches.iter().map(|c| &c.id).collect::<Vec<_>>(),
                }),
            });
        }
    }

    // 9) Git identity
    let git_status = git::get_git_status(project_path);
    if git_status.installed && !git_status.identity_ok {
        issues.push(DoctorIssue {
            id: "git-identity".into(),
            kind: "git_identity".into(),
            severity: IssueSeverity::Warning,
            title: "Git identity not set".into(),
            summary: "git user.name and/or user.email is empty — commits will fail or use a blank author."
                .into(),
            details: vec![
                format!(
                    "user.name: {}",
                    git_status.user_name.clone().unwrap_or_else(|| "(empty)".into())
                ),
                format!(
                    "user.email: {}",
                    git_status.user_email.clone().unwrap_or_else(|| "(empty)".into())
                ),
            ],
            action: Some(IssueAction {
                id: "set_git_identity".into(),
                label: "Set identity".into(),
                confirm: false,
                confirm_message: None,
            }),
            meta: json!({}),
        });
    }

    // 10) Docker disk when Docker exists and reclaimable-looking usage
    let docker_disk = docker::get_docker_disk();
    if docker_disk.available && !docker_disk.raw.is_empty() {
        // Surface when docker is present so users can prune from Issues / Docker page
        let looks_heavy = docker_disk
            .raw
            .to_lowercase()
            .contains("gb")
            || docker_disk.build_cache.is_some();
        if looks_heavy {
            issues.push(DoctorIssue {
                id: "docker-disk".into(),
                kind: "docker_disk".into(),
                severity: IssueSeverity::Info,
                title: "Docker disk usage".into(),
                summary: "Docker is installed — unused images/containers can reclaim space.".into(),
                details: vec![
                    docker_disk
                        .images
                        .clone()
                        .unwrap_or_else(|| "Images: —".into()),
                    docker_disk
                        .containers
                        .clone()
                        .unwrap_or_else(|| "Containers: —".into()),
                    docker_disk.note.clone(),
                ],
                action: Some(IssueAction {
                    id: "prune_docker".into(),
                    label: "Safe prune".into(),
                    confirm: true,
                    confirm_message: Some(
                        "Run `docker system prune -f`?\n\nRemoves unused containers, networks, and dangling images. Volumes are never deleted.".into(),
                    ),
                }),
                meta: json!({ "includeBuildCache": false }),
            });
        }
    }

    // Limit noisy port issues to avoid flooding home
    let mut port_count = 0;
    issues.retain(|i| {
        if i.kind == "port_in_use" {
            port_count += 1;
            port_count <= 6
        } else {
            true
        }
    });

    let scanned_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(ScanResult {
        issues,
        project,
        node: node_env,
        scanned_at,
    })
}

fn normalize_target_version(required: &str) -> String {
    let cleaned: String = required
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let major = cleaned.split('.').next().unwrap_or("22");
    if major.is_empty() {
        "22".into()
    } else {
        major.into()
    }
}
