import { formatBytes } from "../../lib/format";
import type {
  ListeningPort,
  PortSortKey,
  Selection,
  SortDir,
} from "../../lib/types";
import { Inspector } from "../Inspector";

type Props = {
  ports: ListeningPort[];
  loading: boolean;
  selection: Selection | null;
  selectedDetail: ReturnType<
    typeof import("../../hooks/useSystemMonitor").useSystemMonitor
  >["selectedDetail"];
  sort: { key: PortSortKey; dir: SortDir };
  onSort: (key: PortSortKey) => void;
  onSelect: (selection: Selection) => void;
  onKill: (pid: number, name: string, port?: number, memory?: number) => void;
};

function SortHeader({
  label,
  sortKey,
  current,
  onSort,
  width,
}: {
  label: string;
  sortKey: PortSortKey;
  current: { key: PortSortKey; dir: SortDir };
  onSort: (key: PortSortKey) => void;
  width?: string;
}) {
  const active = current.key === sortKey;
  return (
    <th
      className={`sortable ${active ? "active" : ""}`}
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

export function PortsView({
  ports,
  loading,
  selection,
  selectedDetail,
  sort,
  onSort,
  onSelect,
  onKill,
}: Props) {
  return (
    <div className="table-view">
      <section className="table-shell">
        <div className="table-toolbar">
          <h3>Listening ports</h3>
          <span className="table-meta">{ports.length} ports</span>
        </div>
        <div className="table-scroll">
          {loading && ports.length === 0 ? (
            <div className="skeleton-stack">
              {Array.from({ length: 8 }).map((_, i) => (
                <div key={i} className="skeleton" />
              ))}
            </div>
          ) : ports.length === 0 ? (
            <div className="state-empty">
              <strong>No listening ports</strong>
              <span>No TCP listeners matched your search.</span>
            </div>
          ) : (
            <table className="data-table">
              <thead>
                <tr>
                  <SortHeader
                    label="Port"
                    sortKey="port"
                    current={sort}
                    onSort={onSort}
                    width="14%"
                  />
                  <SortHeader
                    label="Process"
                    sortKey="name"
                    current={sort}
                    onSort={onSort}
                    width="28%"
                  />
                  <SortHeader
                    label="PID"
                    sortKey="pid"
                    current={sort}
                    onSort={onSort}
                    width="14%"
                  />
                  <SortHeader
                    label="Memory"
                    sortKey="memory"
                    current={sort}
                    onSort={onSort}
                    width="16%"
                  />
                  <th style={{ width: "18%" }}>Status</th>
                  <th style={{ width: "10%" }} />
                </tr>
              </thead>
              <tbody>
                {ports.map((port) => {
                  const selected =
                    selection?.type === "port" &&
                    selection.port === port.port &&
                    selection.pid === port.pid;
                  const name = port.processName ?? "Unknown process";
                  const limited = port.permissionLimited && !port.processName;
                  return (
                    <tr
                      key={`${port.address}-${port.pid ?? "x"}`}
                      className={selected ? "selected" : ""}
                      onClick={() =>
                        onSelect({
                          type: "port",
                          port: port.port,
                          pid: port.pid,
                        })
                      }
                      title={
                        limited
                          ? "Limited access — process details need elevated permissions"
                          : undefined
                      }
                    >
                      <td className="col-mono" style={{ color: "var(--accent)", fontWeight: 560 }}>
                        {port.port}
                      </td>
                      <td className="col-name">{name}</td>
                      <td className="col-mono col-muted">
                        {port.pid ?? "—"}
                      </td>
                      <td className="col-mono col-muted">
                        {port.memory != null ? formatBytes(port.memory) : "—"}
                      </td>
                      <td>
                        <span
                          className={`status-pill ${limited ? "limited" : ""}`}
                        >
                          {limited ? "Limited access" : "Active"}
                        </span>
                      </td>
                      <td>
                        <button
                          className="row-action"
                          title="Terminate"
                          disabled={port.pid == null}
                          onClick={(e) => {
                            e.stopPropagation();
                            if (port.pid == null) return;
                            onKill(
                              port.pid,
                              name,
                              port.port,
                              port.memory ?? undefined,
                            );
                          }}
                        >
                          ✕
                        </button>
                      </td>
                    </tr>
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
