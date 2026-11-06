import type {
  ActivityDay,
  ActivitySettings,
  CacheItem,
  DockerDisk,
  FixResult,
  GitStatus,
  KillResult,
  ListeningPortsResult,
  NodeEnvironment,
  NodeVersionCatalog,
  ProcessInfo,
  ProjectInfo,
  PythonEnvironment,
  ScanResult,
  SshDiagnosis,
  SystemStats,
  ToolchainStatus,
} from "./types";
import { invoke } from "@tauri-apps/api/core";

export function getSystemStats(): Promise<SystemStats> {
  return invoke("get_system_stats");
}

export function enableSwap(sizeGb?: number): Promise<FixResult> {
  return invoke("enable_swap", { sizeGb: sizeGb ?? null });
}

export function dropMemoryCaches(): Promise<FixResult> {
  return invoke("drop_memory_caches");
}

export function clearDeveloperStorage(includeApt = false): Promise<FixResult> {
  return invoke("clear_developer_storage", { includeApt });
}

export function getProcesses(): Promise<ProcessInfo[]> {
  return invoke("get_processes");
}

export function getListeningPorts(): Promise<ListeningPortsResult> {
  return invoke("get_listening_ports");
}

export function killProcess(pid: number): Promise<KillResult> {
  return invoke("kill_process", { pid });
}

export function pickProjectFolder(): Promise<string | null> {
  return invoke("pick_project_folder");
}

export function inspectProject(path: string): Promise<ProjectInfo> {
  return invoke("inspect_project", { path });
}

export function scanIssues(projectPath?: string | null): Promise<ScanResult> {
  return invoke("scan_issues", { projectPath: projectPath ?? null });
}

export function freePort(port: number): Promise<FixResult> {
  return invoke("free_port", { port });
}

export function switchNode(version: string): Promise<FixResult> {
  return invoke("switch_node", { version });
}

export function listNodeVersions(): Promise<NodeVersionCatalog> {
  return invoke("list_node_versions");
}

export function useNodeVersion(version: string): Promise<FixResult> {
  return invoke("use_node_version", { version });
}

export function repairNodePath(): Promise<FixResult> {
  return invoke("repair_node_path");
}

export function repairNpmPermissions(): Promise<FixResult> {
  return invoke("repair_npm_permissions");
}

export function cleanReinstallDeps(projectPath: string): Promise<FixResult> {
  return invoke("clean_reinstall_deps", { projectPath });
}

export function applyNodeHeap(projectPath: string, mb: number): Promise<FixResult> {
  return invoke("apply_node_heap", { projectPath, mb });
}

export function fixFileWatchers(): Promise<FixResult> {
  return invoke("fix_file_watchers");
}

export function listSafeCaches(): Promise<CacheItem[]> {
  return invoke("list_safe_caches");
}

export function cleanCaches(ids: string[]): Promise<FixResult> {
  return invoke("clean_caches", { ids });
}

export function getNodeEnvironment(): Promise<NodeEnvironment> {
  return invoke("get_node_environment");
}

export function getDockerDisk(): Promise<DockerDisk> {
  return invoke("get_docker_disk");
}

export function pruneDocker(includeBuildCache = false): Promise<FixResult> {
  return invoke("prune_docker", { includeBuildCache });
}

export function checkNativeToolchain(): Promise<ToolchainStatus> {
  return invoke("check_native_toolchain");
}

export function installBuildTools(): Promise<FixResult> {
  return invoke("install_build_tools");
}

export function getPythonEnvironment(
  projectPath?: string | null,
): Promise<PythonEnvironment> {
  return invoke("get_python_environment", { projectPath: projectPath ?? null });
}

export function getGitStatus(projectPath?: string | null): Promise<GitStatus> {
  return invoke("get_git_status", { projectPath: projectPath ?? null });
}

export function setGitIdentity(
  name: string,
  email: string,
  global: boolean,
  projectPath?: string | null,
): Promise<FixResult> {
  return invoke("set_git_identity", {
    name,
    email,
    global,
    projectPath: projectPath ?? null,
  });
}

export function diagnoseGitSsh(host = "github.com"): Promise<SshDiagnosis> {
  return invoke("diagnose_git_ssh", { host });
}

export function useGitHostAccount(host: "github" | "gitlab"): Promise<FixResult> {
  return invoke("use_git_host_account", { host });
}

export function useGitHostToken(
  host: "github" | "gitlab",
  token: string,
): Promise<FixResult> {
  return invoke("use_git_host_token", { host, token });
}

export function openGitTokenPage(host: "github" | "gitlab"): Promise<FixResult> {
  return invoke("open_git_token_page", { host });
}

export function removeGitHostAccount(
  host: "github" | "gitlab",
): Promise<FixResult> {
  return invoke("remove_git_host_account", { host });
}

export function installOrUpdateGit(): Promise<FixResult> {
  return invoke("install_or_update_git");
}

export function uninstallGit(): Promise<FixResult> {
  return invoke("uninstall_git");
}

export function clearGitIdentity(): Promise<FixResult> {
  return invoke("clear_git_identity");
}

export function storeGitCredentials(
  host: string,
  username: string,
  password: string,
  overwrite = false,
): Promise<FixResult> {
  return invoke("store_git_credentials", {
    host,
    username,
    password,
    overwrite,
  });
}

export function testGitHostConnection(
  host: "github" | "gitlab",
): Promise<FixResult> {
  return invoke("test_git_host_connection", { host });
}

export function getActivityDay(date?: string | null): Promise<ActivityDay> {
  return invoke("get_activity_day", { date: date ?? null });
}

export function getActivitySettings(): Promise<ActivitySettings> {
  return invoke("get_activity_settings");
}

export function setActivitySettings(
  settings: ActivitySettings,
): Promise<ActivitySettings> {
  return invoke("set_activity_settings", { settings });
}

export function clearActivityHistory(): Promise<FixResult> {
  return invoke("clear_activity_history");
}

export function listActivityDates(): Promise<string[]> {
  return invoke("list_activity_dates");
}
