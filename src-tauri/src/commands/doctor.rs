use tauri::{AppHandle, Manager, State};

use crate::doctor::deps;
use crate::doctor::disk::{self, CacheItem};
use crate::doctor::docker::{self, DockerDisk};
use crate::doctor::envfile::{self, EnvGaps};
use crate::doctor::git::{self, GitStatus, SshDiagnosis};
use crate::doctor::heap;
use crate::doctor::node;
use crate::doctor::project;
use crate::doctor::python::{self, PythonEnvironment};
use crate::doctor::scan;
use crate::doctor::toolchain::{self, ToolchainStatus};
use crate::doctor::watchers;
use crate::doctor::{FixResult, ScanResult};
use crate::system::kill;
use crate::system::ports;
use crate::system::AppState;

#[tauri::command]
pub fn pick_project_folder() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Open project folder")
        .pick_folder()
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn inspect_project(path: String) -> Result<project::ProjectInfo, String> {
    project::inspect_project(&path)
}

#[tauri::command]
pub async fn scan_issues(
    app: AppHandle,
    project_path: Option<String>,
) -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        scan::scan_issues(&*state, project_path.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn free_port(state: State<'_, AppState>, port: u16) -> Result<FixResult, String> {
    let result = ports::get_listening_ports(&state)?;
    let match_port = result
        .ports
        .into_iter()
        .find(|p| p.port == port)
        .ok_or_else(|| format!("No listener found on port {port}"))?;

    let pid = match_port
        .pid
        .ok_or_else(|| format!("No PID available for port {port} (permission limited)"))?;

    let kill_result = kill::kill_process(pid)?;
    Ok(FixResult {
        success: kill_result.success,
        message: if kill_result.success {
            format!("Freed port {port} (terminated PID {pid})")
        } else {
            kill_result.message
        },
    })
}

#[tauri::command]
pub fn switch_node(version: String) -> Result<FixResult, String> {
    node::switch_node(&version)
}

#[tauri::command]
pub fn list_node_versions() -> Result<node::NodeVersionCatalog, String> {
    node::list_node_versions()
}

#[tauri::command]
pub fn use_node_version(version: String) -> Result<FixResult, String> {
    node::use_node_version(&version)
}

#[tauri::command]
pub fn repair_node_path() -> Result<FixResult, String> {
    node::repair_node_path()
}

#[tauri::command]
pub fn repair_npm_permissions() -> Result<FixResult, String> {
    node::repair_npm_permissions()
}

#[tauri::command]
pub fn clean_reinstall_deps(project_path: String) -> Result<FixResult, String> {
    let info = project::inspect_project(&project_path)?;
    deps::clean_reinstall(&info)
}

#[tauri::command]
pub fn apply_node_heap(project_path: String, mb: u32) -> Result<FixResult, String> {
    let info = project::inspect_project(&project_path)?;
    heap::apply_node_heap(&info, mb)
}

#[tauri::command]
pub fn fix_file_watchers() -> Result<FixResult, String> {
    watchers::fix_file_watchers()
}

#[tauri::command]
pub fn list_safe_caches() -> Vec<CacheItem> {
    disk::list_safe_caches()
}

#[tauri::command]
pub fn clean_caches(ids: Vec<String>) -> Result<FixResult, String> {
    disk::clean_caches(&ids)
}

#[tauri::command]
pub fn get_node_environment() -> node::NodeEnvironment {
    node::detect_node_environment()
}

#[tauri::command]
pub fn get_docker_disk() -> DockerDisk {
    docker::get_docker_disk()
}

#[tauri::command]
pub fn prune_docker(include_build_cache: bool) -> Result<FixResult, String> {
    docker::prune_docker(include_build_cache)
}

#[tauri::command]
pub fn check_native_toolchain() -> ToolchainStatus {
    toolchain::check_native_toolchain()
}

#[tauri::command]
pub fn install_build_tools() -> Result<FixResult, String> {
    toolchain::install_build_tools()
}

#[tauri::command]
pub fn get_python_environment(project_path: Option<String>) -> Result<PythonEnvironment, String> {
    let project = match project_path.as_deref() {
        Some(p) if !p.is_empty() => Some(project::inspect_project(p)?),
        _ => None,
    };
    Ok(python::get_python_environment(project.as_ref()))
}

#[tauri::command]
pub fn get_git_status(project_path: Option<String>) -> GitStatus {
    git::get_git_status(project_path.as_deref())
}

#[tauri::command]
pub fn set_git_identity(
    name: String,
    email: String,
    global: bool,
    project_path: Option<String>,
) -> Result<FixResult, String> {
    git::set_git_identity(name, email, global, project_path)
}

#[tauri::command]
pub fn diagnose_git_ssh(host: String) -> SshDiagnosis {
    git::diagnose_git_ssh(host)
}

#[tauri::command]
pub async fn use_git_host_account(host: String) -> Result<FixResult, String> {
    tauri::async_runtime::spawn_blocking(move || git::use_host_account(&host))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn use_git_host_token(host: String, token: String) -> Result<FixResult, String> {
    tauri::async_runtime::spawn_blocking(move || git::use_host_token(&host, &token))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn open_git_token_page(host: String) -> Result<FixResult, String> {
    git::open_token_page(&host)
}

#[tauri::command]
pub async fn remove_git_host_account(host: String) -> Result<FixResult, String> {
    tauri::async_runtime::spawn_blocking(move || git::remove_host_account(&host))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn install_or_update_git() -> Result<FixResult, String> {
    tauri::async_runtime::spawn_blocking(git::install_or_update_git)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn uninstall_git() -> Result<FixResult, String> {
    tauri::async_runtime::spawn_blocking(git::uninstall_git)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn clear_git_identity() -> Result<FixResult, String> {
    git::clear_git_identity()
}

#[tauri::command]
pub fn store_git_credentials(
    host: String,
    username: String,
    password: String,
    overwrite: Option<bool>,
) -> Result<FixResult, String> {
    git::store_git_credentials(host, username, password, overwrite.unwrap_or(false))
}

#[tauri::command]
pub fn test_git_host_connection(host: String) -> Result<FixResult, String> {
    git::test_host_connection(&host)
}

#[tauri::command]
pub fn scan_env_gaps(project_path: String) -> Result<Option<EnvGaps>, String> {
    let info = project::inspect_project(&project_path)?;
    Ok(envfile::scan_env_gaps(&info))
}
