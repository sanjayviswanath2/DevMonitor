use std::io::Write;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::FixResult;

/// Company GitLab (self-hosted). Used for glab auth, tokens, API, and SSH.
const GITLAB_HOST: &str = "gitlab.teamta.net";
const GITLAB_API: &str = "https://gitlab.teamta.net/api/v4/user";
const GITLAB_TOKEN_URL: &str =
    "https://gitlab.teamta.net/-/user_settings/personal_access_tokens";
const GITLAB_LABEL: &str = "TeamTA GitLab";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub installed: bool,
    /// installed | not_installed | update_available | error
    pub install_status: String,
    pub version: Option<String>,
    /// Short semver-ish string for display (e.g. "2.43.0").
    pub version_short: Option<String>,
    /// Candidate from apt when an upgrade is available.
    pub latest_version: Option<String>,
    pub update_available: bool,
    /// Always from `git config --global` (not local repo).
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub identity_ok: bool,
    pub github: GitHostAccount,
    pub gitlab: GitHostAccount,
    /// Selected project repository remotes / local identity, if any.
    pub repository: Option<GitRepositoryInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHostAccount {
    pub host: String,
    pub label: String,
    pub cli: Option<String>,
    pub signed_in: bool,
    /// connected | not_connected | unknown — auth only, not commit identity.
    pub auth_status: String,
    pub login: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryInfo {
    pub path: String,
    pub name: String,
    pub is_git_repo: bool,
    pub remote_name: Option<String>,
    pub remote_url: Option<String>,
    pub provider: Option<String>,
    pub local_user_name: Option<String>,
    pub local_user_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshDiagnosis {
    pub host: String,
    pub ok: bool,
    pub summary: String,
    pub detail: String,
}

pub fn get_git_status(project_path: Option<&str>) -> GitStatus {
    let installed = which("git");
    let version = if installed {
        Command::new("git")
            .arg("--version")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    } else {
        None
    };
    let version_short = version.as_deref().and_then(parse_git_version);
    let (latest_version, apt_update) = apt_git_update();
    // Only show update when git is installed AND apt Installed != Candidate.
    let update_available = installed && apt_update;
    let install_status = if !installed {
        "not_installed".into()
    } else if update_available {
        "update_available".into()
    } else if version.is_some() {
        "installed".into()
    } else {
        "error".into()
    };

    // Global identity only — never conflate with account auth or local repo config.
    let user_name = git_config_global("user.name");
    let user_email = git_config_global("user.email");
    let identity_ok = user_name.as_ref().map(|s| !s.is_empty()).unwrap_or(false)
        && user_email.as_ref().map(|s| !s.is_empty()).unwrap_or(false);

    let github = probe_host("github");
    let gitlab = probe_host("gitlab");
    let repository = project_path
        .filter(|p| !p.is_empty())
        .and_then(inspect_repository);

    GitStatus {
        installed,
        install_status,
        version,
        version_short,
        latest_version: if installed {
            latest_version
        } else {
            None
        },
        update_available,
        user_name,
        user_email,
        identity_ok,
        github,
        gitlab,
        repository,
    }
}

/// Remove git from the system (apt). Does not clear identity by itself.
pub fn uninstall_git() -> Result<FixResult, String> {
    if !which("git") {
        return Ok(FixResult {
            success: true,
            message: "Git is not installed".into(),
        });
    }
    let output = Command::new("pkexec")
        .args(["apt-get", "remove", "-y", "git"])
        .output()
        .map_err(|e| format!("Could not start uninstaller: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Could not remove Git. Approve the password prompt if shown.\n{err}"
        ));
    }
    let _ = unset_global_identity();
    Ok(FixResult {
        success: true,
        message: "Git removed from this system · global identity cleared".into(),
    })
}

/// Clear global user.name / user.email only.
pub fn clear_git_identity() -> Result<FixResult, String> {
    unset_global_identity()?;
    Ok(FixResult {
        success: true,
        message: "Cleared global git user.name and user.email".into(),
    })
}

/// Store HTTPS credentials for any git host via git-credential (not kept in DevMonitor).
/// If credentials already exist for the host, requires `overwrite: true`.
pub fn store_git_credentials(
    host: String,
    username: String,
    password: String,
    overwrite: bool,
) -> Result<FixResult, String> {
    let host = host
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let username = username.trim().to_string();
    let password = password.trim().to_string();
    if host.is_empty() || username.is_empty() || password.is_empty() {
        return Err("Host, username, and password/token are required".into());
    }
    if !which("git") {
        return Err("Install Git first".into());
    }

    if !overwrite && credential_exists_for_host(&host) {
        return Err(format!(
            "Credentials already exist for https://{host}. Confirm overwrite to replace them."
        ));
    }

    let payload = format!(
        "protocol=https\nhost={host}\nusername={username}\npassword={password}\n\n"
    );
    let mut child = Command::new("git")
        .args(["credential", "approve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git credential failed: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes());
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git credential failed: {e}"))?;
    if out.status.success() {
        Ok(FixResult {
            success: true,
            message: format!("Saved credentials for https://{host} (system credential helper)"),
        })
    } else {
        Err(format!(
            "Could not store credentials: {}",
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

fn credential_exists_for_host(host: &str) -> bool {
    // Bound wait — `git credential fill` can block if no helper answers.
    let payload = format!("protocol=https\nhost={host}\n\n");
    let child = Command::new("timeout")
        .args(["2", "git", "credential", "fill"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok();
    let Some(mut child) = child else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes());
    }
    child
        .wait_with_output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines().any(|l| {
                l.starts_with("password=") && l.len() > "password=".len()
            })
        })
        .unwrap_or(false)
}

/// Re-probe host CLI auth. Returns connected / not_connected / unknown.
pub fn test_host_connection(host: &str) -> Result<FixResult, String> {
    let account = probe_host(host);
    match account.auth_status.as_str() {
        "connected" => {
            let who = account
                .login
                .or(account.name)
                .unwrap_or_else(|| "authenticated".into());
            Ok(FixResult {
                success: true,
                message: format!("{} connection OK · {who}", account.label),
            })
        }
        "unknown" => Ok(FixResult {
            success: false,
            message: format!("{} status unknown — could not verify authentication", account.label),
        }),
        _ => Ok(FixResult {
            success: false,
            message: format!("{} is not connected", account.label),
        }),
    }
}

/// Install git if missing, or upgrade to the apt candidate when available.
pub fn install_or_update_git() -> Result<FixResult, String> {
    let before = get_git_status(None);
    let action = if !before.installed {
        "install"
    } else if before.update_available {
        "update"
    } else {
        return Ok(FixResult {
            success: true,
            message: format!(
                "Git is up to date ({})",
                before
                    .version_short
                    .or(before.version)
                    .unwrap_or_else(|| "installed".into())
            ),
        });
    };

    let output = Command::new("pkexec")
        .args(["apt-get", "install", "-y", "git"])
        .output()
        .map_err(|e| format!("Could not start installer: {e}"))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Git {action} failed. Try: sudo apt-get install -y git\n{err}"
        ));
    }

    let after = get_git_status(None);
    let ver = after
        .version_short
        .or(after.version)
        .unwrap_or_else(|| "installed".into());
    Ok(FixResult {
        success: true,
        message: if action == "install" {
            format!("Git installed · {ver}")
        } else {
            format!("Git updated · {ver}")
        },
    })
}

fn parse_git_version(raw: &str) -> Option<String> {
    // "git version 2.43.0" → "2.43.0"
    raw.split_whitespace()
        .find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .map(|s| s.trim_matches(|c: char| !c.is_ascii_digit() && c != '.').to_string())
        .filter(|s| !s.is_empty())
}

fn apt_git_update() -> (Option<String>, bool) {
    let out = Command::new("apt-cache")
        .args(["policy", "git"])
        .output()
        .ok();
    let Some(out) = out.filter(|o| o.status.success()) else {
        return (None, false);
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut installed_pkg: Option<String> = None;
    let mut candidate_pkg: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Installed:") {
            let v = rest.trim();
            if !v.is_empty() && v != "(none)" {
                installed_pkg = Some(v.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("Candidate:") {
            let v = rest.trim();
            if !v.is_empty() && v != "(none)" {
                candidate_pkg = Some(v.to_string());
            }
        }
    }
    let latest_short = candidate_pkg.as_deref().map(extract_pkg_version);
    let update = match (&installed_pkg, &candidate_pkg) {
        (Some(i), Some(c)) => i != c,
        (None, Some(_)) => true,
        _ => false,
    };
    (latest_short, update)
}

fn extract_pkg_version(pkg: &str) -> String {
    // Strip epoch "1:" then take until packaging revision.
    let s = pkg.split_once(':').map(|(_, r)| r).unwrap_or(pkg);
    s.split('-').next().unwrap_or(s).to_string()
}

pub fn set_git_identity(
    name: String,
    email: String,
    global: bool,
    project_path: Option<String>,
) -> Result<FixResult, String> {
    let name = name.trim().to_string();
    let email = email.trim().to_string();
    if name.is_empty() || email.is_empty() || !email.contains('@') {
        return Err("Provide a non-empty name and a valid email".into());
    }

    let mut cmd_name = Command::new("git");
    let mut cmd_email = Command::new("git");
    if global {
        cmd_name.args(["config", "--global", "user.name", &name]);
        cmd_email.args(["config", "--global", "user.email", &email]);
    } else {
        let path = project_path.ok_or_else(|| {
            "Local git config requires an open project folder".to_string()
        })?;
        cmd_name.current_dir(&path).args(["config", "user.name", &name]);
        cmd_email
            .current_dir(&path)
            .args(["config", "user.email", &email]);
    }

    let out1 = cmd_name.output().map_err(|e| e.to_string())?;
    let out2 = cmd_email.output().map_err(|e| e.to_string())?;
    if out1.status.success() && out2.status.success() {
        Ok(FixResult {
            success: true,
            message: format!(
                "Set git user.name/email ({})",
                if global { "global" } else { "local" }
            ),
        })
    } else {
        Ok(FixResult {
            success: false,
            message: format!(
                "git config failed: {} {}",
                String::from_utf8_lossy(&out1.stderr),
                String::from_utf8_lossy(&out2.stderr)
            ),
        })
    }
}

pub fn diagnose_git_ssh(host: String) -> SshDiagnosis {
    let host = match host.as_str() {
        "gitlab.com" | "gitlab" | "gitlab.teamta.net" => GITLAB_HOST,
        "bitbucket.org" => "bitbucket.org",
        _ => "github.com",
    };
    let target = format!("git@{host}");
    let output = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "ConnectTimeout=8",
            "-T",
            &target,
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = format!("{stdout}{stderr}").to_lowercase();
            // GitHub returns exit 1 even on success with "successfully authenticated"
            let ok = combined.contains("successfully authenticated")
                || combined.contains("welcome to gitlab")
                || combined.contains("authenticated via ssh");
            let summary = if ok {
                "SSH authentication succeeded".into()
            } else if combined.contains("permission denied") {
                "SSH authentication failed (permission denied)".into()
            } else if combined.contains("could not resolve") {
                "Could not resolve SSH host".into()
            } else if combined.contains("connection timed out") || combined.contains("timed out") {
                "SSH connection timed out".into()
            } else if combined.contains("no such file") || combined.contains("identity file") {
                "SSH key may be missing".into()
            } else {
                "SSH probe did not confirm authentication".into()
            };
            SshDiagnosis {
                host: host.into(),
                ok,
                summary,
                detail: format!("{stdout}{stderr}").trim().to_string(),
            }
        }
        Err(e) => SshDiagnosis {
            host: host.into(),
            ok: false,
            summary: "ssh command failed to run".into(),
            detail: e.to_string(),
        },
    }
}

/// Authenticate with the host CLI in the browser.
/// Does NOT change global git user.name / user.email.
pub fn use_host_account(host: &str) -> Result<FixResult, String> {
    let account = login_host(host)?;
    let who = account
        .login
        .or(account.name)
        .unwrap_or_else(|| account.label.clone());
    Ok(FixResult {
        success: true,
        message: format!("Connected to {} as {who}", account.label),
    })
}

/// Authenticate with a one-shot token (not stored in DevMonitor).
/// Does NOT change global git identity.
pub fn use_host_token(host: &str, token: &str) -> Result<FixResult, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("Paste a personal access token first".into());
    }
    let account = fetch_host_with_token(host, token)?;
    let who = account
        .login
        .or(account.name)
        .unwrap_or_else(|| account.label.clone());
    Ok(FixResult {
        success: true,
        message: format!(
            "Verified {} account {who}. Token was not saved in DevMonitor.",
            account.label
        ),
    })
}

/// Sign out of the host CLI on this machine.
/// Does NOT clear global git user.name / user.email.
pub fn remove_host_account(host: &str) -> Result<FixResult, String> {
    let host = normalize_host(host);
    let label = if host == "gitlab" {
        GITLAB_LABEL
    } else {
        "GitHub"
    };

    match host {
        "gitlab" => logout_gitlab()?,
        _ => logout_github()?,
    }

    Ok(FixResult {
        success: true,
        message: format!("Disconnected {label} authentication on this system"),
    })
}

fn logout_github() -> Result<(), String> {
    let _ = reject_git_credentials("github.com");
    if !which("gh") {
        return Ok(());
    }
    if gh_signed_in() {
        let child = Command::new("gh")
            .args(["auth", "logout", "--hostname", "github.com"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok();
        if let Some(mut child) = child {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(b"y\n");
            }
            let _ = child.wait_with_output();
        }
    }
    // Always clear local host config as a fallback.
    clear_gh_host_config("github.com");
    Ok(())
}

fn logout_gitlab() -> Result<(), String> {
    let _ = reject_git_credentials(GITLAB_HOST);
    // Older glab (e.g. 1.36) has no `auth logout` — clear config ourselves.
    if which("glab") {
        let out = Command::new("glab")
            .args(["auth", "logout", "--hostname", GITLAB_HOST])
            .output();
        if let Ok(out) = out {
            if out.status.success() {
                clear_glab_host_config(GITLAB_HOST);
                return Ok(());
            }
        }
    }
    clear_glab_host_config(GITLAB_HOST);
    Ok(())
}

fn reject_git_credentials(host: &str) -> Result<(), String> {
    if !which("git") {
        return Ok(());
    }
    let payload = format!("protocol=https\nhost={host}\n\n");
    let mut child = Command::new("git")
        .args(["credential", "reject"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.as_bytes());
    }
    let _ = child.wait();
    Ok(())
}

fn glab_config_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let p = std::path::PathBuf::from(home).join(".config/glab-cli/config.yml");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

fn gh_hosts_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let p = std::path::PathBuf::from(home).join(".config/gh/hosts.yml");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}

/// Remove a hostname block from glab config (works without `auth logout`).
fn clear_glab_host_config(hostname: &str) {
    let Some(path) = glab_config_path() else {
        return;
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let cleaned = remove_yaml_host_block(&raw, hostname);
    let _ = std::fs::write(&path, cleaned);
}

fn clear_gh_host_config(hostname: &str) {
    let Some(path) = gh_hosts_path() else {
        return;
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return;
    };
    let cleaned = remove_yaml_host_block(&raw, hostname);
    let _ = std::fs::write(&path, cleaned);
}

/// Drop an indented host key and its nested properties from simple YAML.
fn remove_yaml_host_block(raw: &str, hostname: &str) -> String {
    let markers = [
        format!("    {hostname}:"),
        format!("  {hostname}:"),
        format!("{hostname}:"),
    ];
    let lines: Vec<&str> = raw.lines().collect();
    let mut start = None;
    let mut host_indent = 0usize;
    for (i, line) in lines.iter().enumerate() {
        for m in &markers {
            if line.starts_with(m.as_str()) || *line == m.as_str() {
                start = Some(i);
                host_indent = line.chars().take_while(|c| *c == ' ').count();
                break;
            }
        }
        if start.is_some() {
            break;
        }
    }
    let Some(start) = start else {
        return raw.to_string();
    };
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.chars().take_while(|c| *c == ' ').count();
        // Next key at same or shallower indent ends the block.
        if indent <= host_indent && line.contains(':') {
            end = i;
            break;
        }
    }
    let mut out = Vec::with_capacity(lines.len());
    out.extend_from_slice(&lines[..start]);
    out.extend_from_slice(&lines[end..]);
    let mut s = out.join("\n");
    if raw.ends_with('\n') && !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

fn unset_global_identity() -> Result<(), String> {
    let _ = Command::new("git")
        .args(["config", "--global", "--unset", "user.name"])
        .output();
    let _ = Command::new("git")
        .args(["config", "--global", "--unset", "user.email"])
        .output();
    Ok(())
}

fn auth_status_for(signed_in: bool) -> String {
    if signed_in {
        "connected".into()
    } else {
        "not_connected".into()
    }
}

/// Install the host CLI if missing (1-click fix — never tell the user to install manually).
fn ensure_cli(bin: &str) -> Result<(), String> {
    if which(bin) {
        return Ok(());
    }
    // Prefer apt, then snap.
    let apt = Command::new("pkexec")
        .args(["apt-get", "install", "-y", bin])
        .output();
    if let Ok(out) = apt {
        if out.status.success() && which(bin) {
            return Ok(());
        }
    }
    let snap = Command::new("pkexec")
        .args(["snap", "install", bin])
        .output();
    if let Ok(out) = snap {
        if out.status.success() && which(bin) {
            return Ok(());
        }
    }
    // Last resort: official install scripts (network).
    if bin == "gh" {
        let script = Command::new("bash")
            .args([
                "-c",
                "curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | pkexec tee /usr/share/keyrings/githubcli-archive-keyring.gpg >/dev/null && echo \"deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main\" | pkexec tee /etc/apt/sources.list.d/github-cli.list >/dev/null && pkexec apt-get update && pkexec apt-get install -y gh",
            ])
            .output();
        if script.map(|o| o.status.success()).unwrap_or(false) && which("gh") {
            return Ok(());
        }
    }
    if bin == "glab" {
        // Binary release via glab install helper when apt/snap unavailable.
        let script = Command::new("bash")
            .args([
                "-c",
                "curl -sL https://raw.githubusercontent.com/upciti/wakemeops/main/assets/install_repository | pkexec bash && pkexec apt-get install -y glab",
            ])
            .output();
        if script.map(|o| o.status.success()).unwrap_or(false) && which("glab") {
            return Ok(());
        }
        // Direct github release install for linux amd64
        let direct = Command::new("bash")
            .args([
                "-c",
                r#"set -e
TMP=$(mktemp -d)
VER=$(curl -fsSL https://gitlab.com/api/v4/projects/gitlab-org%2Fcli/releases/permalink/latest | grep -oP '"tag_name":"\Kv[^"]+' | head -1 || echo v1.48.0)
curl -fsSL "https://gitlab.com/gitlab-org/cli/-/releases/${VER}/downloads/glab_${VER#v}_linux_amd64.tar.gz" -o "$TMP/glab.tgz" \
  || curl -fsSL "https://github.com/profclems/glab/releases/latest/download/glab_Linux_x86_64.tar.gz" -o "$TMP/glab.tgz"
tar -xzf "$TMP/glab.tgz" -C "$TMP"
BIN=$(find "$TMP" -type f -name glab | head -1)
pkexec install -m 755 "$BIN" /usr/local/bin/glab
"#,
            ])
            .output();
        if direct.map(|o| o.status.success()).unwrap_or(false) && which("glab") {
            return Ok(());
        }
    }
    Err(format!(
        "Could not install {bin} automatically. Approve the permission prompt if shown, then try Sign in again."
    ))
}

pub fn open_token_page(host: &str) -> Result<FixResult, String> {
    let url = match normalize_host(host) {
        "gitlab" => GITLAB_TOKEN_URL,
        _ => "https://github.com/settings/tokens/new?scopes=read:user,user:email&description=DevMonitor",
    };
    let ok = Command::new("xdg-open")
        .arg(url)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        Ok(FixResult {
            success: true,
            message: format!("Opened {url} in your browser"),
        })
    } else {
        Err(format!("Could not open browser. Visit {url}"))
    }
}

fn login_host(host: &str) -> Result<GitHostAccount, String> {
    match normalize_host(host) {
        "gitlab" => login_gitlab(),
        _ => login_github(),
    }
}

fn login_github() -> Result<GitHostAccount, String> {
    ensure_cli("gh")?;
    if let Ok(account) = fetch_github_via_gh() {
        return Ok(account);
    }
    let mut child = Command::new("gh")
        .args([
            "auth",
            "login",
            "--web",
            "--hostname",
            "github.com",
            "--git-protocol",
            "https",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start gh: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"\n");
    }
    let _ = child.wait_with_output();
    fetch_github_via_gh()
}

fn login_gitlab() -> Result<GitHostAccount, String> {
    ensure_cli("glab")?;
    // Always verify with a real profile fetch — glab status can lie (exit 0 when not logged in).
    if let Ok(account) = fetch_gitlab_via_glab() {
        return Ok(account);
    }
    run_glab_web_login()?;
    fetch_gitlab_via_glab()
}

fn run_glab_web_login() -> Result<(), String> {
    let mut child = Command::new("glab")
        .args(["auth", "login", "--hostname", GITLAB_HOST, "--web"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start glab: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"\n");
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("glab auth login failed: {e}"))?;
    if fetch_gitlab_via_glab().is_ok() {
        return Ok(());
    }
    if out.status.success() {
        // Login command returned but profile still unavailable.
        return Ok(());
    }
    Err(format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_string())
}

fn probe_host(host: &str) -> GitHostAccount {
    match normalize_host(host) {
        "gitlab" => {
            if which("glab") {
                // Only "signed in" if we can actually load the user profile.
                fetch_gitlab_via_glab().unwrap_or_else(|_| {
                    empty_account("gitlab", Some("glab"))
                })
            } else {
                empty_account("gitlab", None)
            }
        }
        _ => {
            if which("gh") {
                fetch_github_via_gh()
                    .unwrap_or_else(|_| empty_account("github", Some("gh")))
            } else {
                empty_account("github", None)
            }
        }
    }
}

fn empty_account(host: &str, cli: Option<&str>) -> GitHostAccount {
    GitHostAccount {
        host: host.into(),
        label: if host == "gitlab" {
            GITLAB_LABEL.into()
        } else {
            "GitHub".into()
        },
        cli: cli.map(|s| s.into()),
        signed_in: false,
        auth_status: "not_connected".into(),
        login: None,
        name: None,
        email: None,
    }
}

fn fetch_github_via_gh() -> Result<GitHostAccount, String> {
    let user = gh_json(&["api", "user"])?;
    let login = json_str(&user, "login");
    let name = json_str(&user, "name").or_else(|| login.clone());
    let mut email = json_str(&user, "email");
    if email.is_none() {
        if let Ok(emails) = gh_json(&["api", "user/emails"]) {
            email = primary_github_email(&emails);
        }
    }
    Ok(GitHostAccount {
        host: "github".into(),
        label: "GitHub".into(),
        cli: Some("gh".into()),
        signed_in: true,
        auth_status: auth_status_for(true),
        login,
        name,
        email,
    })
}

fn fetch_gitlab_via_glab() -> Result<GitHostAccount, String> {
    // Prefer host-scoped API so self-hosted profiles resolve correctly.
    let user = glab_json(&["api", "--hostname", GITLAB_HOST, "user"])?;
    let login = json_str(&user, "username");
    if login.is_none() {
        return Err("GitLab profile incomplete".into());
    }
    Ok(GitHostAccount {
        host: "gitlab".into(),
        label: GITLAB_LABEL.into(),
        cli: Some("glab".into()),
        signed_in: true,
        auth_status: auth_status_for(true),
        login: login.clone(),
        name: json_str(&user, "name").or(login),
        email: json_str(&user, "email").or_else(|| json_str(&user, "public_email")),
    })
}

fn fetch_host_with_token(host: &str, token: &str) -> Result<GitHostAccount, String> {
    match normalize_host(host) {
        "gitlab" => {
            let user = curl_json(
                GITLAB_API,
                &[("Authorization", &format!("Bearer {token}"))],
            )?;
            Ok(GitHostAccount {
                host: "gitlab".into(),
                label: GITLAB_LABEL.into(),
                cli: None,
                signed_in: true,
                auth_status: auth_status_for(true),
                login: json_str(&user, "username"),
                name: json_str(&user, "name").or_else(|| json_str(&user, "username")),
                email: json_str(&user, "email").or_else(|| json_str(&user, "public_email")),
            })
        }
        _ => {
            let user = curl_json(
                "https://api.github.com/user",
                &[
                    ("Authorization", &format!("Bearer {token}")),
                    ("Accept", "application/vnd.github+json"),
                    ("User-Agent", "DevMonitor"),
                ],
            )?;
            let emails = curl_json(
                "https://api.github.com/user/emails",
                &[
                    ("Authorization", &format!("Bearer {token}")),
                    ("Accept", "application/vnd.github+json"),
                    ("User-Agent", "DevMonitor"),
                ],
            )
            .ok();
            Ok(GitHostAccount {
                host: "github".into(),
                label: "GitHub".into(),
                cli: None,
                signed_in: true,
                auth_status: auth_status_for(true),
                login: json_str(&user, "login"),
                name: json_str(&user, "name").or_else(|| json_str(&user, "login")),
                email: json_str(&user, "email")
                    .or_else(|| emails.as_ref().and_then(primary_github_email)),
            })
        }
    }
}

fn gh_signed_in() -> bool {
    Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn glab_json(args: &[&str]) -> Result<serde_json::Value, String> {
    let out = Command::new("glab")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(stderr.trim().to_string());
    }
    // Some glab builds exit 0 while printing an error to stdout.
    let trimmed = stdout.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("Get \"")
        || trimmed.to_ascii_lowercase().contains("not authenticated")
        || trimmed.starts_with("error")
    {
        return Err(if trimmed.is_empty() {
            stderr.trim().to_string()
        } else {
            trimmed.to_string()
        });
    }
    serde_json::from_str(trimmed).map_err(|e| e.to_string())
}

fn gh_json(args: &[&str]) -> Result<serde_json::Value, String> {
    let out = Command::new("gh")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())
}

fn curl_json(url: &str, headers: &[(&str, &str)]) -> Result<serde_json::Value, String> {
    let mut cmd = Command::new("curl");
    cmd.args(["-sfS", url]);
    for (k, v) in headers {
        cmd.args(["-H", &format!("{k}: {v}")]);
    }
    let out = cmd.output().map_err(|e| format!("curl failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "Host API error: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("Unexpected API response: {e}"))
}

fn json_str(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn primary_github_email(value: &serde_json::Value) -> Option<String> {
    let arr = value.as_array()?;
    let primary = arr.iter().find(|e| e.get("primary").and_then(|v| v.as_bool()) == Some(true));
    let chosen = primary.or_else(|| {
        arr.iter()
            .find(|e| e.get("visibility").and_then(|v| v.as_str()) == Some("public"))
    });
    chosen
        .or(arr.first())
        .and_then(|e| e.get("email"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn normalize_host(host: &str) -> &str {
    match host.trim().to_ascii_lowercase().as_str() {
        "gitlab" | "gitlab.com" | "gitlab.teamta.net" | "teamta" => "gitlab",
        _ => "github",
    }
}

fn git_config_global(key: &str) -> Option<String> {
    Command::new("git")
        .args(["config", "--global", "--get", key])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn git_config_local(key: &str, project_path: &str) -> Option<String> {
    Command::new("git")
        .current_dir(project_path)
        .args(["config", "--local", "--get", key])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn inspect_repository(path: &str) -> Option<GitRepositoryInfo> {
    let path_buf = std::path::Path::new(path);
    if !path_buf.exists() {
        return None;
    }
    let name = path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string();

    let is_git_repo = Command::new("git")
        .current_dir(path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false);

    if !is_git_repo {
        return Some(GitRepositoryInfo {
            path: path.to_string(),
            name,
            is_git_repo: false,
            remote_name: None,
            remote_url: None,
            provider: None,
            local_user_name: None,
            local_user_email: None,
        });
    }

    let remote_name = Command::new("git")
        .current_dir(path)
        .args(["remote"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(str::to_string)
        });

    let remote_url = remote_name.as_ref().and_then(|remote| {
        Command::new("git")
            .current_dir(path)
            .args(["remote", "get-url", remote])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    });

    let provider = remote_url.as_deref().map(detect_provider);

    Some(GitRepositoryInfo {
        path: path.to_string(),
        name,
        is_git_repo: true,
        remote_name,
        remote_url,
        provider,
        local_user_name: git_config_local("user.name", path),
        local_user_email: git_config_local("user.email", path),
    })
}

fn detect_provider(url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    if lower.contains("github.com") {
        "GitHub".into()
    } else if lower.contains("gitlab.teamta.net") {
        GITLAB_LABEL.into()
    } else if lower.contains("gitlab") {
        "GitLab".into()
    } else if lower.contains("bitbucket") {
        "Bitbucket".into()
    } else if lower.contains("dev.azure.com") || lower.contains("visualstudio.com") {
        "Azure DevOps".into()
    } else if lower.contains("gitea") || lower.contains("codeberg") {
        "Gitea".into()
    } else {
        "Custom".into()
    }
}

fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_version_string() {
        assert_eq!(
            parse_git_version("git version 2.43.0"),
            Some("2.43.0".into())
        );
        assert_eq!(parse_git_version("not a version"), None);
    }

    #[test]
    fn detects_remote_providers() {
        assert_eq!(
            detect_provider("https://github.com/acme/app.git"),
            "GitHub"
        );
        assert_eq!(
            detect_provider("git@gitlab.teamta.net:group/repo.git"),
            GITLAB_LABEL
        );
        assert_eq!(
            detect_provider("https://gitlab.com/org/repo.git"),
            "GitLab"
        );
        assert_eq!(
            detect_provider("https://git.example.com/x.git"),
            "Custom"
        );
    }

    #[test]
    fn auth_status_reflects_signed_in() {
        assert_eq!(auth_status_for(true), "connected");
        assert_eq!(auth_status_for(false), "not_connected");
    }

    #[test]
    fn normalize_host_aliases() {
        assert_eq!(normalize_host("GitHub"), "github");
        assert_eq!(normalize_host("gitlab.teamta.net"), "gitlab");
        assert_eq!(normalize_host("teamta"), "gitlab");
    }
}
