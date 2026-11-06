export interface SystemStats {
  cpuUsage: number;
  memoryTotal: number;
  memoryUsed: number;
  memoryAvailable: number;
  memoryFree: number;
  memoryPercent: number;
  swapTotal: number;
  swapUsed: number;
  swapFree: number;
  suggestedHeapMb: number;
  memoryHigh: boolean;
}

export type ProcessCategory = "app" | "dev" | "safe" | "system" | "other";

export interface ProcessInfo {
  pid: number;
  name: string;
  displayName: string;
  cpuUsage: number;
  /** Resident RAM for this process, matching System Monitor. */
  memory: number;
  /** Proportional share. App rows add this so shared pages are counted once. */
  uniqueMemory: number;
  category: ProcessCategory;
  safeToKill: boolean;
  isUser: boolean;
  exePath: string | null;
  reason: string;
  groupId: string;
  groupLabel: string;
  parentPid: number | null;
}

export type ProcessFilter =
  | "useful"
  | "apps"
  | "dev"
  | "safe"
  | "all";

export interface ListeningPort {
  port: number;
  address: string;
  pid: number | null;
  processName: string | null;
  cpuUsage: number | null;
  memory: number | null;
  permissionLimited: boolean;
}

export interface ListeningPortsResult {
  ports: ListeningPort[];
  warning: string | null;
}

export interface KillResult {
  success: boolean;
  message: string;
}

export type Selection =
  | { type: "process"; pid: number }
  | { type: "port"; port: number; pid: number | null };

export type AppView =
  | "issues"
  | "system"
  | "ports"
  | "activity"
  | "node"
  | "docker"
  | "git";

export interface ActivitySettings {
  idleThresholdSecs: number;
  storeWindowTitles: boolean;
  browserDomainTracking: boolean;
  trackingEnabled: boolean;
}

export interface ActivityDay {
  date: string;
  timezone: string;
  tracking: TrackingCoverage;
  overview: DayOverview;
  timeline: TimelineSegment[];
  apps: AppUsageRow[];
  events: SystemEventRow[];
  legend: LegendItem[];
  notes: string[];
}

export interface TrackingCoverage {
  monitoredRatio: number;
  hasJournalEvents: boolean;
  hasSamples: boolean;
  incomplete: boolean;
  status: string;
}

export interface DayOverview {
  uptimeMs: number;
  firstBootMs: number | null;
  lastEndMs: number | null;
  lastEndKind: string | null;
  activeMs: number;
  idleMs: number;
  lockedMs: number;
  suspendedMs: number;
  poweredOffMs: number;
  unknownMs: number;
  foregroundMs: number;
  backgroundMs: number;
  activeIsEstimate: boolean;
  backgroundIsEstimate: boolean;
}

export interface TimelineSegment {
  startMs: number;
  endMs: number;
  durationMs: number;
  state: string;
  label: string;
  source: string;
  detail: string | null;
}

export interface AppUsageRow {
  appId: string;
  appName: string;
  firstMs: number;
  lastMs: number;
  sessions: number;
  foregroundMs: number;
  backgroundMs: number;
  activeMs: number;
  backgroundIsEstimate: boolean;
}

export interface SystemEventRow {
  tsMs: number;
  kind: string;
  detail: string | null;
  inferred: boolean;
}

export interface LegendItem {
  state: string;
  label: string;
  colorToken: string;
}

export type ThemeMode = "system" | "light" | "dark";

export type ProcessSortKey = "name" | "pid" | "cpu" | "memory" | "port";
export type PortSortKey = "port" | "name" | "pid" | "memory";
export type SortDir = "asc" | "desc";

export interface ProcessRow extends ProcessInfo {
  ports: number[];
  status: "running" | "limited" | "protected";
}

export type IssueSeverity = "info" | "warning" | "error";

export interface IssueAction {
  id: string;
  label: string;
  confirm: boolean;
  confirmMessage: string | null;
}

export interface DoctorIssue {
  id: string;
  kind: string;
  severity: IssueSeverity;
  title: string;
  summary: string;
  details: string[];
  action: IssueAction | null;
  meta: Record<string, unknown>;
}

export interface ProjectInfo {
  path: string;
  name: string;
  packageManager: string;
  packageManagerField: string | null;
  hasPackageJson: boolean;
  hasNodeModules: boolean;
  lockfile: string | null;
  lockfileCount: number;
  enginesNode: string | null;
  nvmrc: string | null;
  nodeVersionFile: string | null;
  requiredNode: string | null;
  envExample: string | null;
  pythonRequired: string | null;
  hasNativeDepsHint: boolean;
}

export interface NodeInstall {
  path: string;
  version: string | null;
  manager: string;
}

export interface NodeEnvironment {
  nodeAvailable: boolean;
  npmAvailable: boolean;
  activeNode: string | null;
  activeNpm: string | null;
  whichNode: string | null;
  whichNpm: string | null;
  installMethod: string;
  manager: string | null;
  installations: NodeInstall[];
  npmPrefix: string | null;
  npmPrefixOwnerRoot: boolean;
  pathIssue: boolean;
  pathIssueDetail: string | null;
}

export interface NodeVersionRow {
  version: string;
  lts: string | null;
  date: string | null;
  installed: boolean;
  current: boolean;
  manager: string | null;
  path: string | null;
}

export interface NodeVersionCatalog {
  manager: string | null;
  activeNode: string | null;
  versions: NodeVersionRow[];
  source: string;
}

/** UI phase while using / installing a Node version */
export type NodeUsePhase =
  | "idle"
  | "checking"
  | "installing"
  | "switching"
  | "setting-current"
  | "done"
  | "error";

export interface ScanResult {
  issues: DoctorIssue[];
  project: ProjectInfo | null;
  node: NodeEnvironment;
  scannedAt: number;
}

export interface FixResult {
  success: boolean;
  message: string;
}

export interface CacheItem {
  id: string;
  label: string;
  path: string;
  sizeBytes: number;
  safe: boolean;
}

export interface DockerDisk {
  available: boolean;
  raw: string;
  images: string | null;
  containers: string | null;
  volumes: string | null;
  buildCache: string | null;
  note: string;
}

export interface ToolchainStatus {
  python3: boolean;
  pythonVersion: string | null;
  make: boolean;
  gcc: boolean;
  gpp: boolean;
  complete: boolean;
  missing: string[];
  aptCommand: string;
}

export interface PythonEnvironment {
  available: boolean;
  version: string | null;
  which: string | null;
  required: string | null;
  mismatch: boolean;
  guidance: string;
}

export interface GitStatus {
  installed: boolean;
  /** installed | not_installed | update_available | error */
  installStatus: string;
  version: string | null;
  versionShort: string | null;
  latestVersion: string | null;
  updateAvailable: boolean;
  /** Always from git config --global */
  userName: string | null;
  userEmail: string | null;
  identityOk: boolean;
  github: GitHostAccount;
  gitlab: GitHostAccount;
  repository: GitRepositoryInfo | null;
}

export interface GitHostAccount {
  host: string;
  label: string;
  cli: string | null;
  signedIn: boolean;
  /** connected | not_connected | unknown — auth only */
  authStatus: string;
  login: string | null;
  name: string | null;
  email: string | null;
}

export interface GitRepositoryInfo {
  path: string;
  name: string;
  isGitRepo: boolean;
  remoteName: string | null;
  remoteUrl: string | null;
  provider: string | null;
  localUserName: string | null;
  localUserEmail: string | null;
}

export interface SshDiagnosis {
  host: string;
  ok: boolean;
  summary: string;
  detail: string;
}
