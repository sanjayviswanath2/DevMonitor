import { useState } from "react";
import {
  clearDeveloperStorage,
  dropMemoryCaches,
  enableSwap,
} from "../../lib/api";
import { formatBytes, formatPercent } from "../../lib/format";
import { usageLevel } from "../../lib/usageLevel";
import type { SystemStats } from "../../lib/types";
import { UsageChart } from "../Sparkline";

type Props = {
  stats: SystemStats | null;
  history: number[];
  onToast: (kind: "success" | "error" | "info", message: string) => void;
  onRefresh: () => void;
};

type Action = "swap" | "cache" | "storage";

const CONFIRM: Record<Action, { title: string; body: string }> = {
  swap: {
    title: "Add 4 GB swap?",
    body: "Creates /swapfile.devmonitor and turns it on. Ubuntu will ask for your password.",
  },
  cache: {
    title: "Free the cache?",
    body: "Clears memory Linux was keeping so files open faster. Your apps stay open. Ubuntu will ask for your password.",
  },
  storage: {
    title: "Clear caches and Trash?",
    body: "Removes safe developer caches and empties Trash. Project source files stay.",
  },
};

export function MemoryView({
  stats,
  history,
  onToast,
  onRefresh,
}: Props) {
  const used = stats?.memoryUsed ?? 0;
  const total = stats?.memoryTotal ?? 0;
  const available = stats?.memoryAvailable ?? 0;
  const cached = Math.max(0, available - (stats?.memoryFree ?? 0));
  const pct = stats?.memoryPercent ?? 0;
  const swapTotal = stats?.swapTotal ?? 0;
  const swapUsed = stats?.swapUsed ?? 0;
  const heap = stats?.suggestedHeapMb ?? 0;

  const usedPct = total > 0 ? (used / total) * 100 : 0;

  const [pending, setPending] = useState<Action | null>(null);
  const [busy, setBusy] = useState(false);

  const run = async (action: Action) => {
    setBusy(true);
    try {
      const result =
        action === "swap"
          ? await enableSwap(4)
          : action === "cache"
            ? await dropMemoryCaches()
            : await clearDeveloperStorage(false);
      onToast(result.success ? "success" : "error", result.message);
      if (result.success) onRefresh();
    } catch (e) {
      onToast("error", e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
      setPending(null);
    }
  };

  return (
    <div className="resource-layout resource-layout-perf memory-layout">
      <header className="resource-head">
        <div>
          <span className="resource-kicker memory">Resource</span>
          <h3>Memory</h3>
          <p className="resource-sub">Last 60 seconds</p>
        </div>
        <div className="resource-head-stats">
          <strong className="mono">{stats ? formatBytes(total) : "—"}</strong>
          <span>{formatPercent(pct, 0)} used</span>
        </div>
      </header>

      {stats?.memoryHigh && (
        <div className="banner memory-alert" role="status">
          Memory is {formatPercent(pct, 0)} full. Free the cache or add swap
          before builds start failing.
        </div>
      )}

      <section className="resource-chart-panel">
        <UsageChart
          values={history}
          label="Memory history"
          variant="hero"
          windowLabel="60 sec"
        />
      </section>

      <div
        className="composition-bar"
        aria-label={
          stats
            ? `${formatBytes(used)} used, ${formatBytes(available)} free`
            : "Memory"
        }
      >
        <span
          className={`comp-used ${usageLevel(pct)}`}
          style={{ width: `${usedPct}%` }}
          title="Used"
        />
        <span
          className="comp-free"
          style={{ width: `${Math.max(0, 100 - usedPct)}%` }}
          title="Free"
        />
      </div>
      <p className="memory-explain">
        {cached >= 50 * 1024 * 1024
          ? `${formatBytes(cached)} of this free memory is set aside so files open faster. It comes back when an app needs it.`
          : "Free is memory apps can still use."}
      </p>

      <div className="stats-grid stats-grid-perf stats-grid-2">
        <div className="stat-box">
          <label>Used</label>
          <strong className="mono">{formatBytes(used)}</strong>
          <span className="resource-note">Apps are using this</span>
        </div>
        <div className="stat-box">
          <label>Free</label>
          <strong className="mono">{formatBytes(available)}</strong>
          <span className="resource-note">Still open for apps</span>
        </div>
        <div className="stat-box">
          <label>Swap</label>
          <strong className="mono">
            {swapTotal > 0 ? `${formatBytes(swapUsed)} of ${formatBytes(swapTotal)}` : "Off"}
          </strong>
          <span className="resource-note">Spare memory on disk</span>
        </div>
        <div className="stat-box">
          <label>Node.js</label>
          <strong className="mono">{heap ? `${heap} MB` : "—"}</strong>
          <span className="resource-note">Safe limit for one process</span>
        </div>
      </div>

      <section className="memory-actions">
        <p>
          Swap is spare room on disk. Freeing the cache gives RAM back and
          leaves your apps open.
        </p>
        <button
          type="button"
          className="btn"
          disabled={busy || swapTotal > 0}
          onClick={() => setPending("swap")}
        >
          {swapTotal > 0 ? "Swap on" : "Add 4 GB swap"}
        </button>
        <button
          type="button"
          className="btn"
          disabled={busy}
          onClick={() => setPending("cache")}
        >
          Free the cache
        </button>
        <button
          type="button"
          className="btn"
          disabled={busy}
          onClick={() => setPending("storage")}
        >
          Clear caches & Trash
        </button>
      </section>

      {pending && (
        <div className="overlay" onClick={() => !busy && setPending(null)} role="presentation">
          <div
            className="dialog"
            role="dialog"
            aria-modal="true"
            onClick={(e) => e.stopPropagation()}
          >
            <h2>{CONFIRM[pending].title}</h2>
            <p>{CONFIRM[pending].body}</p>
            <div className="dialog-actions">
              <button
                type="button"
                className="btn"
                disabled={busy}
                onClick={() => setPending(null)}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={busy}
                onClick={() => void run(pending)}
              >
                {busy ? "Working…" : "Continue"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
