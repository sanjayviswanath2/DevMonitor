import { useEffect, useMemo, useState } from "react";
import { formatBytes, formatCpu } from "../../lib/format";
import type {
  ProcessCategory,
  ProcessFilter,
  ProcessRow,
  ProcessSortKey,
  Selection,
  SortDir,
} from "../../lib/types";
import { Inspector } from "../Inspector";

type Props = {
  processes: ProcessRow[];
  loading: boolean;
  selection: Selection | null;
  selectedDetail: ReturnType<
    typeof import("../../hooks/useSystemMonitor").useSystemMonitor
  >["selectedDetail"];
  sort: { key: ProcessSortKey; dir: SortDir };
  onSort: (key: ProcessSortKey) => void;
  onSelect: (selection: Selection) => void;
  onKill: (pid: number, name: string, port?: number, memory?: number) => void;
  filter: ProcessFilter;
  onFilter: (filter: ProcessFilter) => void;
  filterCounts: {
    useful: number;
    apps: number;
    dev: number;
    safe: number;
    all: number;
  };
};

type ProcessGroup = {
  id: string;
  label: string;
  processes: ProcessRow[];
  cpuUsage: number;
  memory: number;
  ports: number[];
  category: ProcessCategory;
  safeCount: number;
  protectedCount: number;
};

function SortHeader({
  label,
  sortKey,
  current,
  onSort,
  width,
  align,
}: {
  label: string;
  sortKey: ProcessSortKey;
  current: { key: ProcessSortKey; dir: SortDir };
  onSort: (key: ProcessSortKey) => void;
  width?: string;
  align?: "left" | "right";
}) {
  const active = current.key === sortKey;
  return (
    <th
      className={`sortable ${active ? "active" : ""} ${align === "right" ? "col-right" : ""}`}
      style={width ? { width } : undefined}
      onClick={() => onSort(sortKey)}
    >
      {label}
      <span className="sort-ind">
        {active ? (current.dir === "asc" ? "↑" : "↓") : "↕"}
      </span>
    </th>
  );
}

const FILTERS: { id: ProcessFilter; label: string }[] = [
  { id: "useful", label: "Useful" },
  { id: "apps", label: "Apps" },
  { id: "dev", label: "Dev tools" },
  { id: "safe", label: "Safe to end" },
  { id: "all", label: "All" },
];

function categoryLabel(c: ProcessCategory): string {
  switch (c) {
    case "app":
      return "App";
    case "dev":
      return "Dev";
    case "safe":
      return "Yours";
    case "system":
      return "System";
    default:
      return "Other";
  }
}

function categoryRank(c: ProcessCategory): number {
  switch (c) {
    case "app":
      return 0;
    case "dev":
      return 1;
    case "safe":
      return 2;
    case "other":
      return 3;
    case "system":
      return 4;
  }
}

function buildGroups(processes: ProcessRow[]): ProcessGroup[] {
  const map = new Map<string, ProcessRow[]>();
  for (const p of processes) {
    const id = p.groupId || `pid:${p.pid}`;
    const list = map.get(id);
    if (list) list.push(p);
    else map.set(id, [p]);
  }

  const groups: ProcessGroup[] = [];
  for (const [id, list] of map) {
    const sorted = [...list].sort(
      (a, b) => b.cpuUsage - a.cpuUsage || b.memory - a.memory,
    );
    const ports = [
      ...new Set(sorted.flatMap((p) => p.ports)),
    ].sort((a, b) => a - b);
    let category = sorted[0].category;
    for (const p of sorted) {
      if (categoryRank(p.category) < categoryRank(category)) {
        category = p.category;
      }
    }
    groups.push({
      id,
      label: sorted[0].groupLabel || sorted[0].displayName || sorted[0].name,
      processes: sorted,
      cpuUsage: Math.min(
        100,
        sorted.reduce((s, p) => s + p.cpuUsage, 0),
      ),
      memory: sorted.reduce((s, p) => s + (p.uniqueMemory ?? p.memory), 0),
      ports,
      category,
      safeCount: sorted.filter((p) => p.safeToKill).length,
      protectedCount: sorted.filter((p) => !p.safeToKill).length,
    });
  }
  return groups;
}

function sortGroups(
  groups: ProcessGroup[],
  sort: { key: ProcessSortKey; dir: SortDir },
): ProcessGroup[] {
  const mul = sort.dir === "asc" ? 1 : -1;
  return [...groups].sort((a, b) => {
    switch (sort.key) {
      case "name":
        return mul * a.label.localeCompare(b.label);
      case "pid": {
        const ap = a.processes[0]?.pid ?? 0;
        const bp = b.processes[0]?.pid ?? 0;
        return mul * (ap - bp);
      }
      case "cpu":
        return mul * (a.cpuUsage - b.cpuUsage);
      case "memory":
        return mul * (a.memory - b.memory);
      case "port": {
        const ap = a.ports[0] ?? Number.POSITIVE_INFINITY;
        const bp = b.ports[0] ?? Number.POSITIVE_INFINITY;
        return mul * (ap - bp);
      }
      default:
        return 0;
    }
  });
}

export function ProcessesView({
  processes,
  loading,
  selection,
  selectedDetail,
  sort,
  onSort,
  onSelect,
  onKill,
  filter,
  onFilter,
  filterCounts,
}: Props) {
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());

  const groups = useMemo(
    () => sortGroups(buildGroups(processes), sort),
    [processes, sort],
  );

  // Keep the group containing the selected process open.
  useEffect(() => {
    if (selection?.type !== "process") return;
    const group = groups.find((g) =>
      g.processes.some((p) => p.pid === selection.pid),
    );
    if (!group || group.processes.length <= 1) return;
    setExpanded((prev) => {
      if (prev.has(group.id)) return prev;
      const next = new Set(prev);
      next.add(group.id);
      return next;
    });
  }, [selection, groups]);

  const toggle = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const processCount = processes.length;
  const groupCount = groups.length;

  return (
    <div className="table-view">
      <section className="table-shell">
        <div className="table-toolbar process-toolbar">
          <div>
            <h3>Processes</h3>
            <p className="process-toolbar-sub">
              CPU is the share of this computer, from 0 to 100%. App memory is
              the real amount of RAM. Expand a row to see each process.
            </p>
          </div>
          <span className="table-meta">
            {groupCount} apps · {processCount} processes
          </span>
        </div>

        <div className="process-filters" role="tablist" aria-label="Process filters">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              type="button"
              role="tab"
              aria-selected={filter === f.id}
              className={`process-filter ${filter === f.id ? "active" : ""}`}
              onClick={() => onFilter(f.id)}
            >
              {f.label}
              <em>{filterCounts[f.id]}</em>
            </button>
          ))}
        </div>

        <div className="table-scroll">
          {loading && processes.length === 0 ? (
            <div className="skeleton-stack">
              {Array.from({ length: 10 }).map((_, i) => (
                <div key={i} className="skeleton" />
              ))}
            </div>
          ) : processes.length === 0 ? (
            <div className="state-empty">
              <strong>No matching processes</strong>
              <span>
                Try another filter, or search for a name / PID in the header.
              </span>
            </div>
          ) : (
            <table className="data-table">
              <thead>
                <tr>
                  <SortHeader
                    label="App / process"
                    sortKey="name"
                    current={sort}
                    onSort={onSort}
                    width="28%"
                  />
                  <th style={{ width: "9%" }}>Kind</th>
                  <SortHeader
                    label="PID"
                    sortKey="pid"
                    current={sort}
                    onSort={onSort}
                    width="9%"
                  />
                  <SortHeader
                    label="CPU"
                    sortKey="cpu"
                    current={sort}
                    onSort={onSort}
                    width="10%"
                  />
                  <SortHeader
                    label="Memory"
                    sortKey="memory"
                    current={sort}
                    onSort={onSort}
                    width="12%"
                  />
                  <SortHeader
                    label="Port"
                    sortKey="port"
                    current={sort}
                    onSort={onSort}
                    width="12%"
                  />
                  <th style={{ width: "12%" }}>End?</th>
                  <th style={{ width: "8%" }} />
                </tr>
              </thead>
              <tbody>
                {groups.map((g) => {
                  const multi = g.processes.length > 1;
                  const isOpen = multi && expanded.has(g.id);
                  const primary = g.processes[0];
                  const groupSelected =
                    selection?.type === "process" &&
                    g.processes.some((p) => p.pid === selection.pid);

                  if (!multi) {
                    return (
                      <ProcessFlatRow
                        key={g.id}
                        p={primary}
                        selection={selection}
                        onSelect={onSelect}
                        onKill={onKill}
                      />
                    );
                  }

                  return (
                    <GroupBlock
                      key={g.id}
                      group={g}
                      open={isOpen}
                      selected={groupSelected}
                      selection={selection}
                      onToggle={() => toggle(g.id)}
                      onSelect={onSelect}
                      onKill={onKill}
                    />
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      </section>

      <Inspector detail={selectedDetail} onKill={onKill} />
    </div>
  );
}

function ProcessFlatRow({
  p,
  selection,
  onSelect,
  onKill,
  nested,
}: {
  p: ProcessRow;
  selection: Selection | null;
  onSelect: (selection: Selection) => void;
  onKill: (pid: number, name: string, port?: number, memory?: number) => void;
  nested?: boolean;
}) {
  const selected =
    selection?.type === "process" && selection.pid === p.pid;
  return (
    <tr
      className={`${selected ? "selected" : ""} ${
        p.safeToKill ? "" : "row-protected"
      } ${nested ? "process-child-row" : ""}`}
      onClick={() => onSelect({ type: "process", pid: p.pid })}
      title={p.reason || p.exePath || undefined}
    >
      <td className="col-name" title={p.exePath ?? p.name}>
        <span className={`process-name ${nested ? "process-name-nested" : ""}`}>
          {p.displayName || p.name}
        </span>
        {p.displayName && p.displayName !== p.name && (
          <span className="process-name-sub">{p.name}</span>
        )}
      </td>
      <td>
        <span className={`process-kind kind-${p.category}`}>
          {categoryLabel(p.category)}
        </span>
      </td>
      <td className="col-mono col-muted">{p.pid}</td>
      <td className="col-mono">{formatCpu(p.cpuUsage)}</td>
      <td className="col-mono col-muted">{formatBytes(p.memory)}</td>
      <td className="col-mono">
        {p.ports.length > 0 ? p.ports.join(", ") : "—"}
      </td>
      <td>
        {p.safeToKill ? (
          <span className="status-pill">Safe</span>
        ) : (
          <span className="status-pill limited">Protected</span>
        )}
      </td>
      <td>
        {p.safeToKill ? (
          <button
            className="row-action"
            title="End process"
            onClick={(e) => {
              e.stopPropagation();
              onKill(p.pid, p.displayName || p.name, p.ports[0], p.memory);
            }}
          >
            ✕
          </button>
        ) : (
          <span
            className="row-action disabled"
            title={p.reason || "Protected process"}
            aria-hidden
          >
            ✕
          </span>
        )}
      </td>
    </tr>
  );
}

function GroupBlock({
  group,
  open,
  selected,
  selection,
  onToggle,
  onSelect,
  onKill,
}: {
  group: ProcessGroup;
  open: boolean;
  selected: boolean;
  selection: Selection | null;
  onToggle: () => void;
  onSelect: (selection: Selection) => void;
  onKill: (pid: number, name: string, port?: number, memory?: number) => void;
}) {
  const primary = group.processes[0];
  const endLabel =
    group.safeCount === 0
      ? "Protected"
      : group.protectedCount > 0
        ? `${group.safeCount} safe`
        : "Safe";

  return (
    <>
      <tr
        className={`process-group-row ${selected ? "selected" : ""} ${
          open ? "open" : ""
        }`}
        onClick={() => {
          onToggle();
          onSelect({ type: "process", pid: primary.pid });
        }}
      >
        <td className="col-name">
          <div className="process-group-cell">
            <button
              type="button"
              className="process-expand"
              aria-expanded={open}
              aria-label={open ? "Collapse group" : "Expand group"}
              onClick={(e) => {
                e.stopPropagation();
                onToggle();
              }}
            >
              {open ? "▾" : "▸"}
            </button>
            <span className="process-group-label">
              <span className="process-name">{group.label}</span>
              <span className="process-name-sub">
                {group.processes.length} processes
              </span>
            </span>
          </div>
        </td>
        <td>
          <span className={`process-kind kind-${group.category}`}>
            {categoryLabel(group.category)}
          </span>
        </td>
        <td className="col-mono col-muted">—</td>
        <td className="col-mono">{formatCpu(group.cpuUsage)}</td>
        <td className="col-mono col-muted">{formatBytes(group.memory)}</td>
        <td className="col-mono">
          {group.ports.length > 0 ? group.ports.join(", ") : "—"}
        </td>
        <td>
          <span
            className={`status-pill ${group.safeCount === 0 ? "limited" : ""}`}
          >
            {endLabel}
          </span>
        </td>
        <td>
          {group.safeCount > 0 ? (
            <button
              className="row-action"
              title={`End ${group.safeCount} safe process${group.safeCount === 1 ? "" : "es"}`}
              onClick={(e) => {
                e.stopPropagation();
                for (const p of group.processes) {
                  if (!p.safeToKill) continue;
                  onKill(p.pid, p.displayName || p.name, p.ports[0], p.memory);
                }
              }}
            >
              ✕
            </button>
          ) : (
            <span className="row-action disabled" aria-hidden>
              ✕
            </span>
          )}
        </td>
      </tr>
      {open &&
        group.processes.map((p) => (
          <ProcessFlatRow
            key={p.pid}
            p={p}
            selection={selection}
            onSelect={onSelect}
            onKill={onKill}
            nested
          />
        ))}
    </>
  );
}
