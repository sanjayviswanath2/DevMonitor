import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyNodeHeap,
  cleanCaches,
  cleanReinstallDeps,
  fixFileWatchers,
  freePort,
  installBuildTools,
  pruneDocker,
  repairNodePath,
  repairNpmPermissions,
  scanIssues,
  setGitIdentity,
  switchNode,
} from "../lib/api";
import type { DoctorIssue, FixResult, ProjectInfo } from "../lib/types";

export type DoctorToast = {
  id: number;
  kind: "success" | "error" | "info";
  message: string;
};

export type GitIdentityDraft = {
  name: string;
  email: string;
  global: boolean;
};

export function useDoctor() {
  const [scan, setScan] = useState<Awaited<ReturnType<typeof scanIssues>> | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [applying, setApplying] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<{
    issue: DoctorIssue;
  } | null>(null);
  const [identityModal, setIdentityModal] = useState(false);
  const [identityDraft, setIdentityDraft] = useState<GitIdentityDraft>({
    name: "",
    email: "",
    global: true,
  });
  const [toasts, setToasts] = useState<DoctorToast[]>([]);
  const toastId = useRef(0);
  const mounted = useRef(true);

  const pushToast = useCallback((kind: DoctorToast["kind"], message: string) => {
    const id = ++toastId.current;
    setToasts((prev) => [...prev, { id, kind, message }]);
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3800);
  }, []);

  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const refresh = useCallback(async () => {
    setScanning(true);
    try {
      const result = await scanIssues(null);
      if (!mounted.current) return;
      setScan(result);
      setError(null);
    } catch (e) {
      if (!mounted.current) return;
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (!mounted.current) return;
      setLoading(false);
      setScanning(false);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    return () => {
      mounted.current = false;
    };
  }, [refresh]);

  const runFix = useCallback(
    async (issue: DoctorIssue, identity?: GitIdentityDraft) => {
      if (!issue.action) return;
      setApplying(issue.id);
      setPendingAction(null);
      try {
        const result = await applyFix(issue, null, null, identity);
        pushToast(result.success ? "success" : "error", result.message);
        if (result.success && issue.action.id === "set_git_identity") {
          setIdentityModal(false);
        }
        await refresh();
      } catch (e) {
        pushToast("error", e instanceof Error ? e.message : String(e));
      } finally {
        setApplying(null);
      }
    },
    [pushToast, refresh],
  );

  const requestFix = useCallback(
    (issue: DoctorIssue) => {
      if (!issue.action) return;
      if (issue.action.id === "set_git_identity") {
        setIdentityDraft({ name: "", email: "", global: true });
        setIdentityModal(true);
        return;
      }
      if (issue.action.id === "copy_pm_install") {
        const cmd = String(issue.meta.installCommand ?? "npm install");
        void navigator.clipboard.writeText(cmd).then(
          () => pushToast("success", `Copied: ${cmd}`),
          () => pushToast("info", `Run: ${cmd}`),
        );
        return;
      }
      if (issue.action.confirm) {
        setPendingAction({ issue });
        return;
      }
      void runFix(issue);
    },
    [runFix, pushToast],
  );

  const confirmFix = useCallback(async () => {
    if (!pendingAction) return;
    await runFix(pendingAction.issue);
  }, [pendingAction, runFix]);

  const cancelFix = useCallback(() => setPendingAction(null), []);

  const submitIdentity = useCallback(async () => {
    const fakeIssue: DoctorIssue = {
      id: "git-identity",
      kind: "git_identity",
      severity: "warning",
      title: "Git identity",
      summary: "",
      details: [],
      action: {
        id: "set_git_identity",
        label: "Set identity",
        confirm: false,
        confirmMessage: null,
      },
      meta: {},
    };
    await runFix(fakeIssue, identityDraft);
  }, [runFix, identityDraft]);

  return {
    projectPath: null as string | null,
    project: null as ProjectInfo | null,
    node: scan?.node ?? null,
    issues: scan?.issues ?? [],
    loading,
    scanning,
    applying,
    error,
    pendingAction,
    identityModal,
    identityDraft,
    setIdentityDraft,
    closeIdentityModal: () => setIdentityModal(false),
    submitIdentity,
    toasts,
    dismissToast,
    pushToast,
    refresh,
    requestFix,
    confirmFix,
    cancelFix,
  };
}

async function applyFix(
  issue: DoctorIssue,
  projectPath: string | null,
  project: ProjectInfo | null,
  identity?: GitIdentityDraft,
): Promise<FixResult> {
  const action = issue.action?.id;
  switch (action) {
    case "free_port": {
      const port = Number(issue.meta.port);
      return freePort(port);
    }
    case "switch_node": {
      const target = String(issue.meta.target ?? "22");
      return switchNode(target);
    }
    case "repair_node_path":
      return repairNodePath();
    case "repair_npm_permissions":
      return repairNpmPermissions();
    case "clean_reinstall": {
      const path = projectPath ?? project?.path;
      if (!path) return { success: false, message: "No project selected" };
      return cleanReinstallDeps(path);
    }
    case "apply_heap": {
      const path = projectPath ?? project?.path;
      if (!path) return { success: false, message: "No project selected" };
      const mb = Number(issue.meta.mb ?? 4096);
      return applyNodeHeap(path, mb);
    }
    case "fix_watchers":
      return fixFileWatchers();
    case "clean_caches": {
      const ids = (issue.meta.cacheIds as string[] | undefined) ?? [];
      if (ids.length === 0) {
        const { listSafeCaches } = await import("../lib/api");
        const caches = await listSafeCaches();
        return cleanCaches(caches.map((c) => c.id));
      }
      return cleanCaches(ids);
    }
    case "install_build_tools":
      return installBuildTools();
    case "prune_docker": {
      const includeBuildCache = Boolean(issue.meta.includeBuildCache);
      return pruneDocker(includeBuildCache);
    }
    case "set_git_identity": {
      if (!identity) {
        return { success: false, message: "Identity details required" };
      }
      return setGitIdentity(
        identity.name,
        identity.email,
        identity.global,
        identity.global ? null : projectPath,
      );
    }
    case "copy_pm_install":
      return {
        success: true,
        message: `Use: ${String(issue.meta.installCommand ?? "npm install")}`,
      };
    default:
      return { success: false, message: `Unknown action: ${action}` };
  }
}
