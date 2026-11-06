import { useCallback, useEffect, useState } from "react";
import { getDockerDisk, pruneDocker } from "../../lib/api";
import type { DockerDisk } from "../../lib/types";

type Props = {
  onToast: (kind: "success" | "error" | "info", message: string) => void;
};

export function DockerDoctorView({ onToast }: Props) {
  const [disk, setDisk] = useState<DockerDisk | null>(null);
  const [loading, setLoading] = useState(true);
  const [pruning, setPruning] = useState(false);
  const [confirm, setConfirm] = useState<"safe" | "cache" | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setDisk(await getDockerDisk());
    } catch (e) {
      onToast("error", e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [onToast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const runPrune = async (includeBuildCache: boolean) => {
    setPruning(true);
    setConfirm(null);
    try {
      const result = await pruneDocker(includeBuildCache);
      onToast(result.success ? "success" : "error", result.message);
      await refresh();
    } catch (e) {
      onToast("error", e instanceof Error ? e.message : String(e));
    } finally {
      setPruning(false);
    }
  };

  if (loading && !disk) {
    return (
      <div className="skeleton-stack">
        {Array.from({ length: 4 }).map((_, i) => (
          <div key={i} className="skeleton" />
        ))}
      </div>
    );
  }

  if (!disk) {
    return (
      <div className="state-empty">
        <strong>Unable to read Docker disk usage</strong>
      </div>
    );
  }

  if (!disk.available) {
    return (
      <div className="state-empty">
        <strong>Docker not available</strong>
        <p>{disk.note}</p>
      </div>
    );
  }

  return (
    <div className="node-doctor">
      <div className="view-toolbar">
        <div>
          <h3 className="issues-greeting">Docker disk</h3>
          <p className="issues-sub">{disk.note}</p>
        </div>
        <div className="issues-hero-actions">
          <button className="btn" onClick={() => void refresh()} disabled={loading}>
            Refresh
          </button>
          <button
            className="btn btn-danger"
            disabled={pruning}
            onClick={() => setConfirm("safe")}
          >
            Safe prune
          </button>
          <button
            className="btn btn-danger"
            disabled={pruning}
            onClick={() => setConfirm("cache")}
          >
            Prune + cache
          </button>
        </div>
      </div>

      <div className="stats-grid">
        <div className="stat-box">
          <label>Images</label>
          <strong className="mono" style={{ fontSize: 14 }}>
            {disk.images ?? "—"}
          </strong>
        </div>
        <div className="stat-box">
          <label>Containers</label>
          <strong className="mono" style={{ fontSize: 14 }}>
            {disk.containers ?? "—"}
          </strong>
        </div>
        <div className="stat-box">
          <label>Volumes</label>
          <strong className="mono" style={{ fontSize: 14 }}>
            {disk.volumes ?? "—"}
          </strong>
        </div>
        <div className="stat-box">
          <label>Build cache</label>
          <strong className="mono" style={{ fontSize: 14 }}>
            {disk.buildCache ?? "—"}
          </strong>
        </div>
      </div>

      <section className="table-shell group-box" style={{ minHeight: 200, border: "none", borderRadius: 0 }}>
        <div className="table-toolbar group-box-head">
          <h3>Raw usage</h3>
        </div>
        <pre
          className="mono group-box-body"
          style={{
            margin: 0,
            padding: 10,
            fontSize: 14,
            whiteSpace: "pre-wrap",
            color: "var(--text-muted)",
          }}
        >
          {disk.raw || "(empty)"}
        </pre>
      </section>

      {confirm && (
        <div className="overlay" onClick={() => setConfirm(null)} role="presentation">
          <div
            className="dialog"
            role="dialog"
            aria-modal="true"
            onClick={(e) => e.stopPropagation()}
          >
            <h2>{confirm === "cache" ? "Prune + build cache" : "Safe Docker prune"}</h2>
            <div className="dialog-meta">
              <div>
                {confirm === "cache"
                  ? "Runs `docker system prune -f` then `docker builder prune -f`. Unused containers, networks, dangling images, and build cache are removed."
                  : "Runs `docker system prune -f`. Unused containers, networks, and dangling images are removed."}
              </div>
              <div style={{ marginTop: 8, fontWeight: 600 }}>
                Volumes are never deleted.
              </div>
            </div>
            <div className="dialog-actions">
              <button className="btn" onClick={() => setConfirm(null)}>
                Cancel
              </button>
              <button
                className="btn btn-danger"
                disabled={pruning}
                onClick={() => void runPrune(confirm === "cache")}
              >
                {pruning ? "Pruning…" : "Continue"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
