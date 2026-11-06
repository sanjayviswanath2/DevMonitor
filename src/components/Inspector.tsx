import { formatBytes, formatCpu } from "../lib/format";
import type { ListeningPort, ProcessRow } from "../lib/types";

type Detail =
  | {
      kind: "process";
      process: ProcessRow | null;
      ports: ListeningPort[];
    }
  | {
      kind: "port";
      port: ListeningPort | null;
      process: ProcessRow | null;
    }
  | null;

type Props = {
  detail: Detail;
  onKill: (pid: number, name: string, port?: number, memory?: number) => void;
};

export function Inspector({ detail, onKill }: Props) {
  if (!detail) {
    return (
      <aside className="inspector">
        <div className="inspector-empty">
          <strong>No selection</strong>
          <span>Select a process or port to inspect details.</span>
        </div>
      </aside>
    );
  }

  if (detail.kind === "port") {
    const port = detail.port;
    const process = detail.process;
    const name = process?.name ?? port?.processName ?? "Unknown process";
    const pid = process?.pid ?? port?.pid ?? null;
    const memory = process?.memory ?? port?.memory ?? undefined;
    const cpu = process?.cpuUsage ?? port?.cpuUsage ?? null;
    const limited = port?.permissionLimited && !process;

    return (
      <aside className="inspector">
        <div className="inspector-head">
          <div className="inspector-kicker">Listening port</div>
          <h3 className="inspector-title">:{port?.port ?? "—"}</h3>
        </div>
        <div className="inspector-body">
          <div className="inspector-row">
            <span>Process</span>
            <strong>{name}</strong>
          </div>
          <div className="inspector-row">
            <span>PID</span>
            <strong className="mono">{pid ?? "—"}</strong>
          </div>
          <div className="inspector-row">
            <span>CPU</span>
            <strong className="mono">{cpu != null ? formatCpu(cpu) : "—"}</strong>
          </div>
          <div className="inspector-row">
            <span>Memory</span>
            <strong className="mono">
              {memory != null ? formatBytes(memory) : "—"}
            </strong>
          </div>
          <div className="inspector-row">
            <span>Status</span>
            <strong>
              <span className={`status-pill ${limited ? "limited" : ""}`}>
                {limited ? "Limited access" : "Active"}
              </span>
            </strong>
          </div>
          {limited && (
            <div className="inspector-row">
              <span>Note</span>
              <strong style={{ fontWeight: 400, color: "var(--text-secondary)" }}>
                Process details require elevated permissions
              </strong>
            </div>
          )}
        </div>
        <div className="inspector-foot">
          <button
            className="btn btn-danger"
            disabled={pid == null}
            onClick={() => {
              if (pid != null) onKill(pid, name, port?.port, memory);
            }}
          >
            Terminate Process
          </button>
        </div>
      </aside>
    );
  }

  const process = detail.process;
  const ports = detail.ports;
  const name = process?.displayName ?? process?.name ?? "Unknown process";
  const pid = process?.pid ?? ports[0]?.pid ?? null;
  const canKill = process?.safeToKill ?? false;

  return (
    <aside className="inspector">
      <div className="inspector-head">
        <div className="inspector-kicker">Process</div>
        <h3 className="inspector-title">{name}</h3>
      </div>
      <div className="inspector-body">
        <div className="inspector-row">
          <span>PID</span>
          <strong className="mono">{pid ?? "—"}</strong>
        </div>
        <div className="inspector-row">
          <span>Kind</span>
          <strong style={{ textTransform: "capitalize" }}>
            {process?.category ?? "—"}
          </strong>
        </div>
        <div className="inspector-row">
          <span>CPU</span>
          <strong className="mono">
            {process ? formatCpu(process.cpuUsage) : "—"}
          </strong>
        </div>
        <div className="inspector-row">
          <span>Memory</span>
          <strong className="mono">
            {process ? formatBytes(process.memory) : "—"}
          </strong>
        </div>
        <div className="inspector-row">
          <span>Port</span>
          <strong className="mono">
            {ports.length > 0 ? ports.map((p) => p.port).join(", ") : "—"}
          </strong>
        </div>
        <div className="inspector-row">
          <span>End?</span>
          <strong>
            <span className={`status-pill ${canKill ? "" : "limited"}`}>
              {canKill ? "Safe to end" : "Protected"}
            </span>
          </strong>
        </div>
        {process?.reason && (
          <div className="inspector-row">
            <span>Note</span>
            <strong
              style={{
                fontWeight: 400,
                color: "var(--text-secondary)",
                textAlign: "right",
              }}
            >
              {process.reason}
            </strong>
          </div>
        )}
      </div>
      <div className="inspector-foot">
        <button
          className="btn btn-danger"
          disabled={pid == null || !canKill}
          title={
            canKill
              ? "End this process"
              : process?.reason || "Protected process"
          }
          onClick={() => {
            if (pid != null && canKill)
              onKill(pid, name, ports[0]?.port, process?.memory);
          }}
        >
          {canKill ? "End process" : "Protected"}
        </button>
      </div>
    </aside>
  );
}
