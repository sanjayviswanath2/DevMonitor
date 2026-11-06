import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getListeningPorts,
  getProcesses,
  getSystemStats,
  killProcess,
} from "../lib/api";
import type {
  KillResult,
  ListeningPort,
  ProcessFilter,
  ProcessInfo,
  ProcessRow,
  ProcessSortKey,
  PortSortKey,
  Selection,
  SortDir,
  SystemStats,
} from "../lib/types";

const POLL_MS = 2000;
const HISTORY_LEN = 60;

export type Toast = {
  id: number;
  kind: "success" | "error" | "info";
  message: string;
};

export function useSystemMonitor() {
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [allProcesses, setAllProcesses] = useState<ProcessInfo[]>([]);
  const [allPorts, setAllPorts] = useState<ListeningPort[]>([]);
  const [portsWarning, setPortsWarning] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [selection, setSelection] = useState<Selection | null>(null);
  const [pendingKill, setPendingKill] = useState<{
    pid: number;
    name: string;
    port?: number;
    memory?: number;
  } | null>(null);
  const [killing, setKilling] = useState(false);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [live, setLive] = useState(true);
  const [cpuHistory, setCpuHistory] = useState<number[]>([]);
  const [memHistory, setMemHistory] = useState<number[]>([]);
  const [processFilter, setProcessFilter] = useState<ProcessFilter>("useful");
  const [processSort, setProcessSort] = useState<{
    key: ProcessSortKey;
    dir: SortDir;
  }>({ key: "cpu", dir: "desc" });
  const [portSort, setPortSort] = useState<{
    key: PortSortKey;
    dir: SortDir;
  }>({ key: "port", dir: "asc" });
  const toastId = useRef(0);
  const mounted = useRef(true);
  const refreshInFlight = useRef(false);

  const pushToast = useCallback((kind: Toast["kind"], message: string) => {
    const id = ++toastId.current;
    setToasts((prev) => [...prev, { id, kind, message }]);
    window.setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 3200);
  }, []);

  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const refresh = useCallback(
    async (manual = false) => {
      // Skip overlapping polls — heavy process scans must not pile up.
      if (refreshInFlight.current) return;
      refreshInFlight.current = true;
      if (manual) setRefreshing(true);
      try {
        const [nextStats, nextProcesses, nextPorts] = await Promise.all([
          getSystemStats(),
          getProcesses(),
          getListeningPorts(),
        ]);
        if (!mounted.current) return;
        setStats(nextStats);
        setAllProcesses(nextProcesses);
        setAllPorts(nextPorts.ports);
        setPortsWarning(nextPorts.warning);
        setCpuHistory((prev) =>
          [...prev, nextStats.cpuUsage].slice(-HISTORY_LEN),
        );
        setMemHistory((prev) =>
          [...prev, nextStats.memoryPercent].slice(-HISTORY_LEN),
        );
        setError(null);
      } catch (e) {
        if (!mounted.current) return;
        const message = e instanceof Error ? e.message : String(e);
        setError(message);
        if (manual) pushToast("error", message);
      } finally {
        refreshInFlight.current = false;
        if (!mounted.current) return;
        setLoading(false);
        setRefreshing(false);
      }
    },
    [pushToast],
  );

  useEffect(() => {
    mounted.current = true;
    void refresh(false);
    return () => {
      mounted.current = false;
    };
  }, [refresh]);

  useEffect(() => {
    if (!live) return;
    const id = window.setInterval(() => {
      void refresh(false);
    }, POLL_MS);
    return () => window.clearInterval(id);
  }, [live, refresh]);

  const portsByPid = useMemo(() => {
    const map = new Map<number, number[]>();
    for (const port of allPorts) {
      if (port.pid == null) continue;
      const list = map.get(port.pid) ?? [];
      if (!list.includes(port.port)) list.push(port.port);
      map.set(port.pid, list);
    }
    return map;
  }, [allPorts]);

  const processRows = useMemo<ProcessRow[]>(() => {
    return allProcesses.map((p) => ({
      ...p,
      displayName: p.displayName || p.name,
      category: p.category ?? "other",
      safeToKill: p.safeToKill ?? false,
      isUser: p.isUser ?? false,
      exePath: p.exePath ?? null,
      reason: p.reason ?? "",
      ports: portsByPid.get(p.pid) ?? [],
      status: p.safeToKill ? ("running" as const) : ("protected" as const),
    }));
  }, [allProcesses, portsByPid]);

  const processFilterCounts = useMemo(() => {
    const counts = {
      useful: 0,
      apps: 0,
      dev: 0,
      safe: 0,
      all: processRows.length,
    };
    for (const p of processRows) {
      if (p.category === "app") counts.apps += 1;
      if (p.category === "dev") counts.dev += 1;
      if (p.safeToKill) counts.safe += 1;
      if (
        p.category === "app" ||
        p.category === "dev" ||
        (p.safeToKill && p.category !== "system")
      ) {
        counts.useful += 1;
      }
    }
    return counts;
  }, [processRows]);

  const normalizedQuery = query.trim().toLowerCase();

  const filteredProcesses = useMemo(() => {
    let rows = processRows;

    switch (processFilter) {
      case "useful":
        rows = rows.filter(
          (p) =>
            p.category === "app" ||
            p.category === "dev" ||
            (p.safeToKill && p.category !== "system"),
        );
        break;
      case "apps":
        rows = rows.filter((p) => p.category === "app");
        break;
      case "dev":
        rows = rows.filter((p) => p.category === "dev");
        break;
      case "safe":
        rows = rows.filter((p) => p.safeToKill);
        break;
      case "all":
        break;
    }

    if (normalizedQuery) {
      rows = rows.filter((p) => {
        return (
          p.name.toLowerCase().includes(normalizedQuery) ||
          p.displayName.toLowerCase().includes(normalizedQuery) ||
          String(p.pid).includes(normalizedQuery) ||
          p.ports.some((port) => String(port).includes(normalizedQuery)) ||
          (p.exePath?.toLowerCase().includes(normalizedQuery) ?? false)
        );
      });
    }

    const dir = processSort.dir === "asc" ? 1 : -1;
    return [...rows].sort((a, b) => {
      switch (processSort.key) {
        case "name":
          return a.displayName.localeCompare(b.displayName) * dir;
        case "pid":
          return (a.pid - b.pid) * dir;
        case "cpu":
          return (a.cpuUsage - b.cpuUsage) * dir;
        case "memory":
          return (a.memory - b.memory) * dir;
        case "port": {
          const ap = a.ports[0] ?? -1;
          const bp = b.ports[0] ?? -1;
          return (ap - bp) * dir;
        }
        default:
          return 0;
      }
    });
  }, [processRows, normalizedQuery, processSort, processFilter]);

  const filteredPorts = useMemo(() => {
    let rows = allPorts;
    if (normalizedQuery) {
      rows = rows.filter((p) => {
        return (
          String(p.port).includes(normalizedQuery) ||
          (p.processName?.toLowerCase().includes(normalizedQuery) ?? false) ||
          (p.pid !== null && String(p.pid).includes(normalizedQuery)) ||
          p.address.toLowerCase().includes(normalizedQuery)
        );
      });
    }

    const dir = portSort.dir === "asc" ? 1 : -1;
    return [...rows].sort((a, b) => {
      switch (portSort.key) {
        case "port":
          return (a.port - b.port) * dir;
        case "name":
          return (
            (a.processName ?? "").localeCompare(b.processName ?? "") * dir
          );
        case "pid":
          return ((a.pid ?? 0) - (b.pid ?? 0)) * dir;
        case "memory":
          return ((a.memory ?? 0) - (b.memory ?? 0)) * dir;
        default:
          return 0;
      }
    });
  }, [allPorts, normalizedQuery, portSort]);

  const selectedDetail = useMemo(() => {
    if (!selection) return null;
    if (selection.type === "process") {
      const process =
        processRows.find((p) => p.pid === selection.pid) ?? null;
      const relatedPorts = allPorts.filter((p) => p.pid === selection.pid);
      if (!process && relatedPorts.length === 0) return null;
      return {
        kind: "process" as const,
        process,
        ports: relatedPorts,
      };
    }
    const port =
      allPorts.find(
        (p) => p.port === selection.port && p.pid === selection.pid,
      ) ??
      allPorts.find((p) => p.port === selection.port) ??
      null;
    const process =
      port?.pid != null
        ? processRows.find((p) => p.pid === port.pid) ?? null
        : null;
    return { kind: "port" as const, port, process };
  }, [selection, processRows, allPorts]);

  const toggleProcessSort = useCallback((key: ProcessSortKey) => {
    setProcessSort((prev) =>
      prev.key === key
        ? { key, dir: prev.dir === "asc" ? "desc" : "asc" }
        : { key, dir: key === "name" ? "asc" : "desc" },
    );
  }, []);

  const togglePortSort = useCallback((key: PortSortKey) => {
    setPortSort((prev) =>
      prev.key === key
        ? { key, dir: prev.dir === "asc" ? "desc" : "asc" }
        : { key, dir: key === "port" || key === "name" ? "asc" : "desc" },
    );
  }, []);

  const requestKill = useCallback(
    (pid: number, name: string, port?: number, memory?: number) => {
      setPendingKill({ pid, name, port, memory });
    },
    [],
  );

  const cancelKill = useCallback(() => setPendingKill(null), []);

  const confirmKill = useCallback(async () => {
    if (!pendingKill) return;
    setKilling(true);
    try {
      const result: KillResult = await killProcess(pendingKill.pid);
      if (result.success) {
        pushToast(
          "success",
          `Terminated ${pendingKill.name} (PID ${pendingKill.pid})`,
        );
        setPendingKill(null);
        setSelection(null);
        await refresh(false);
      } else {
        pushToast("error", result.message);
      }
    } catch (e) {
      pushToast("error", e instanceof Error ? e.message : String(e));
    } finally {
      setKilling(false);
    }
  }, [pendingKill, pushToast, refresh]);

  return {
    stats,
    processes: filteredProcesses,
    ports: filteredPorts,
    processCount: allProcesses.length,
    portCount: allPorts.length,
    processFilter,
    setProcessFilter,
    processFilterCounts,
    portsWarning,
    loading,
    refreshing,
    error,
    query,
    setQuery,
    selection,
    setSelection,
    selectedDetail,
    pendingKill,
    requestKill,
    cancelKill,
    confirmKill,
    killing,
    toasts,
    dismissToast,
    refresh,
    live,
    setLive,
    cpuHistory,
    memHistory,
    processSort,
    toggleProcessSort,
    portSort,
    togglePortSort,
  };
}
