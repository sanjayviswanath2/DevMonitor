import { formatBytes, formatCpu, formatPercent } from "../../lib/format";
import { usageLabel, usageLevel } from "../../lib/usageLevel";
import type { ProcessRow, SystemStats } from "../../lib/types";
import { UsageChart } from "../Sparkline";

type Props = {
  stats: SystemStats | null;
  history: number[];
  processes: ProcessRow[];
  onSelectProcess: (pid: number) => void;
  /** Hide the per-process table when the parent already lists processes. */
  embedded?: boolean;
};

export function CpuView({
  stats,
  history,
  processes,
  onSelectProcess,
  embedded = false,
}: Props) {
  const cpu = stats?.cpuUsage ?? 0;
  const top = processes.slice(0, 12);
  const status = usageLabel(cpu);

  return (
    <div className="resource-layout resource-layout-perf">
      <header className="resource-head">
        <div>
          <span className="resource-kicker cpu">Resource</span>
          <h3>CPU</h3>
          <p className="resource-sub">Last 60 seconds · {status}</p>
        </div>
        <div className="resource-head-stats">
          <strong className="mono">{formatPercent(cpu, 0)}</strong>
          <span>Live</span>
        </div>
      </header>

      <section className="resource-chart-panel">
        <UsageChart
          values={history}
          label="CPU history"
          variant="hero"
          windowLabel="60 sec"
        />
      </section>

      <div className="composition-bar" aria-label="CPU load">
        <span
          className={`comp-used ${usageLevel(cpu)}`}
          style={{ width: `${Math.min(100, cpu)}%` }}
        />
        <span className="comp-free" style={{ width: `${Math.max(0, 100 - cpu)}%` }} />
      </div>

      <div className="stats-grid stats-grid-perf stats-grid-3">
        <div className="stat-box">
          <label>Utilization</label>
          <strong className="mono">{formatPercent(cpu, 1)}</strong>
        </div>
        <div className="stat-box">
          <label>Status</label>
          <strong>{status}</strong>
        </div>
        <div className="stat-box">
          <label>Top process</label>
          <strong className="mono">
            {top[0] ? `${top[0].name} · ${formatCpu(top[0].cpuUsage)}` : "—"}
          </strong>
        </div>
      </div>

      {!embedded && (
      <section className="table-shell resource-table">
        <div className="table-toolbar">
          <h3>Per-process CPU</h3>
          <span className="table-meta">Top consumers</span>
        </div>
        <div className="table-scroll">
          <table className="data-table">
            <thead>
              <tr>
                <th style={{ width: "36%" }}>Process</th>
                <th style={{ width: "16%" }}>PID</th>
                <th style={{ width: "16%" }}>CPU</th>
                <th style={{ width: "16%" }}>Memory</th>
                <th style={{ width: "16%" }}>Port</th>
              </tr>
            </thead>
            <tbody>
              {top.map((p) => (
                <tr key={p.pid} onClick={() => onSelectProcess(p.pid)}>
                  <td className="col-name">{p.name}</td>
                  <td className="col-mono col-muted">{p.pid}</td>
                  <td className="col-mono">{formatCpu(p.cpuUsage)}</td>
                  <td className="col-mono col-muted">
                    {formatBytes(p.memory)}
                  </td>
                  <td className="col-mono">
                    {p.ports.length > 0 ? p.ports.join(", ") : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
      )}
    </div>
  );
}
