import { useCallback, useEffect, useMemo, useState } from "react";
import { getNodeEnvironment, listNodeVersions, useNodeVersion } from "../../lib/api";
import type {
  NodeEnvironment,
  NodeUsePhase,
  NodeVersionCatalog,
  NodeVersionRow,
} from "../../lib/types";

type Props = {
  node: NodeEnvironment | null;
  loading: boolean;
  onRepairPath: () => void;
  onRepairNpm: () => void;
  applying: boolean;
  onToast?: (kind: "success" | "error" | "info", message: string) => void;
  onNodeChanged?: () => void;
};

type Filter = "all" | "lts" | "installed";

export function NodeDoctorView({
  node: nodeProp,
  loading,
  onRepairPath,
  onRepairNpm,
  applying,
  onToast,
  onNodeChanged,
}: Props) {
  const [node, setNode] = useState<NodeEnvironment | null>(nodeProp);
  const [catalog, setCatalog] = useState<NodeVersionCatalog | null>(null);
  const [listPhase, setListPhase] = useState<NodeUsePhase>("idle");
  const [listError, setListError] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [busyVersion, setBusyVersion] = useState<string | null>(null);
  const [busyPhase, setBusyPhase] = useState<NodeUsePhase>("idle");
  const [statusLine, setStatusLine] = useState<string | null>(null);

  useEffect(() => {
    setNode(nodeProp);
  }, [nodeProp]);

  const refreshCatalog = useCallback(async () => {
    setListPhase("checking");
    setListError(null);
    try {
      const [cat, env] = await Promise.all([
        listNodeVersions(),
        getNodeEnvironment(),
      ]);
      setCatalog(cat);
      setNode(env);
      setListPhase("done");
    } catch (e) {
      setListError(e instanceof Error ? e.message : String(e));
      setListPhase("error");
    }
  }, []);

  useEffect(() => {
    void refreshCatalog();
  }, [refreshCatalog]);

  const rows = useMemo(() => {
    const list = catalog?.versions ?? [];
    const q = query.trim().toLowerCase();
    return list.filter((r) => {
      if (filter === "lts" && !r.lts) return false;
      if (filter === "installed" && !r.installed) return false;
      if (!q) return true;
      return (
        r.version.toLowerCase().includes(q) ||
        (r.lts?.toLowerCase().includes(q) ?? false)
      );
    });
  }, [catalog, filter, query]);

  const onUse = async (row: NodeVersionRow) => {
    if (busyVersion) return;
    setBusyVersion(row.version);
    setBusyPhase(row.installed ? "switching" : "installing");
    setStatusLine(
      row.installed
        ? `Switching to Node ${row.version}…`
        : `Installing Node ${row.version} from nodejs.org…`,
    );
    try {
      // Soft phase progression while backend runs as one call
      const phaseTimer = window.setTimeout(() => {
        setBusyPhase("switching");
        setStatusLine(`Switching active Node to ${row.version}…`);
      }, 1200);
      const phaseTimer2 = window.setTimeout(() => {
        setBusyPhase("setting-current");
        setStatusLine(`Setting Node ${row.version} as current default…`);
      }, 2400);

      const result = await useNodeVersion(row.version);
      window.clearTimeout(phaseTimer);
      window.clearTimeout(phaseTimer2);

      if (!result.success) {
        setBusyPhase("error");
        setStatusLine(result.message);
        onToast?.("error", result.message);
        return;
      }

      setBusyPhase("done");
      setStatusLine(result.message);
      onToast?.("success", result.message);
      await refreshCatalog();
      onNodeChanged?.();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setBusyPhase("error");
      setStatusLine(msg);
      onToast?.("error", msg);
    } finally {
      window.setTimeout(() => {
        setBusyVersion(null);
        setBusyPhase("idle");
      }, 1600);
    }
  };

  if (loading && !node) {
    return (
      <div className="skeleton-stack">
        {Array.from({ length: 5 }).map((_, i) => (
          <div key={i} className="skeleton" />
        ))}
      </div>
    );
  }

  if (!node) {
    return (
      <div className="state-empty">
        <strong>Unable to read Node environment</strong>
      </div>
    );
  }

  const managerLabel =
    catalog?.manager ?? node.manager ?? node.installMethod ?? "none";

  return (
    <div className="node-doctor">
      <div className="node-hero">
        <div className="node-hero-main">
          <p className="node-kicker">Node.js</p>
          <h3 className="node-hero-title">
            {node.activeNode ? `v${node.activeNode}` : "Not on PATH"}
          </h3>
          <p className="node-hero-sub">
            {node.whichNode ?? "node not found"} · managed by{" "}
            <strong>{managerLabel}</strong>
          </p>
        </div>
        <div className="node-hero-meta">
          <div className="node-pill">
            <span>npm</span>
            <strong>{node.activeNpm ? `v${node.activeNpm}` : "—"}</strong>
          </div>
          <div className="node-pill">
            <span>Local installs</span>
            <strong>{node.installations.length}</strong>
          </div>
        </div>
      </div>

      {statusLine && (
        <div
          className={`node-status-banner tone-${
            busyPhase === "error" ? "error" : busyPhase === "done" ? "ok" : "busy"
          }`}
          role="status"
        >
          <span className="node-status-dot" aria-hidden />
          <div>
            <strong>{phaseLabel(busyPhase, listPhase)}</strong>
            <p>{statusLine}</p>
          </div>
        </div>
      )}

      <section className="node-versions card-soft">
        <div className="node-versions-head">
          <div>
            <h4>Versions</h4>
            <p>
              Latest release per major from nodejs.org — Use installs if needed,
              then switches and sets default.
            </p>
          </div>
          <div className="node-versions-actions">
            <button
              type="button"
              className="btn"
              onClick={() => void refreshCatalog()}
              disabled={listPhase === "checking" || !!busyVersion}
            >
              {listPhase === "checking" ? "Checking…" : "Refresh"}
            </button>
          </div>
        </div>

        <div className="node-toolbar">
          <div className="node-filters" role="tablist" aria-label="Filter versions">
            {(
              [
                ["all", "All"],
                ["lts", "LTS"],
                ["installed", "Installed"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                role="tab"
                aria-selected={filter === id}
                className={`node-filter ${filter === id ? "active" : ""}`}
                onClick={() => setFilter(id)}
              >
                {label}
              </button>
            ))}
          </div>
          <input
            className="dialog-input node-search"
            placeholder="Filter by version…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="Filter Node versions"
          />
        </div>

        {listPhase === "checking" && !catalog && (
          <div className="node-list-state">Checking releases on nodejs.org…</div>
        )}
        {listPhase === "error" && (
          <div className="node-list-state tone-error">
            {listError ?? "Could not load versions"}
            <button type="button" className="btn" onClick={() => void refreshCatalog()}>
              Retry
            </button>
          </div>
        )}

        {catalog && (
          <ul className="node-version-list">
            {rows.length === 0 ? (
              <li className="node-list-state">No versions match this filter</li>
            ) : (
              rows.map((row) => {
                const isBusy = busyVersion === row.version;
                const phase = isBusy ? busyPhase : "idle";
                return (
                  <li
                    key={row.version}
                    className={`node-version-row ${row.current ? "is-current" : ""} ${
                      isBusy ? "is-busy" : ""
                    }`}
                  >
                    <div className="node-version-main">
                      <div className="node-version-title">
                        <span className="node-version-num">v{row.version}</span>
                        {row.lts && (
                          <span className="node-badge lts">LTS · {row.lts}</span>
                        )}
                        {row.current && (
                          <span className="node-badge current">Current</span>
                        )}
                        {row.installed && !row.current && (
                          <span className="node-badge installed">Installed</span>
                        )}
                        {!row.installed && (
                          <span className="node-badge remote">Not installed</span>
                        )}
                      </div>
                      <div className="node-version-meta">
                        {row.date ? `Released ${row.date}` : "Local only"}
                        {row.manager ? ` · ${row.manager}` : ""}
                        {isBusy ? ` · ${phaseLabel(phase, "idle")}` : ""}
                      </div>
                    </div>
                    <button
                      type="button"
                      className={`btn ${row.current ? "" : "btn-primary"} node-use-btn`}
                      disabled={
                        !!busyVersion || applying || row.current || listPhase === "checking"
                      }
                      onClick={() => void onUse(row)}
                    >
                      {row.current
                        ? "In use"
                        : isBusy
                          ? busyButtonLabel(phase)
                          : row.installed
                            ? "Use"
                            : "Install & use"}
                    </button>
                  </li>
                );
              })
            )}
          </ul>
        )}
      </section>

      <div className="node-footer">
        <p>
          Switches use <strong>nvm</strong> or <strong>fnm</strong> (nvm is
          installed automatically if missing). System apt/snap Node is not
          overwritten.
        </p>
        <div className="node-footer-actions">
          <button
            className="btn"
            onClick={onRepairPath}
            disabled={applying || !!busyVersion}
          >
            Repair PATH
          </button>
          <button
            className="btn"
            onClick={onRepairNpm}
            disabled={applying || !!busyVersion}
          >
            Repair npm prefix
          </button>
        </div>
      </div>
    </div>
  );
}

function phaseLabel(busy: NodeUsePhase, list: NodeUsePhase): string {
  if (busy === "installing") return "Installing";
  if (busy === "switching") return "Switching";
  if (busy === "setting-current") return "Setting current";
  if (busy === "done") return "Ready";
  if (busy === "error") return "Failed";
  if (list === "checking") return "Checking";
  return "Status";
}

function busyButtonLabel(phase: NodeUsePhase): string {
  if (phase === "installing") return "Installing…";
  if (phase === "switching") return "Switching…";
  if (phase === "setting-current") return "Setting…";
  if (phase === "done") return "Done";
  if (phase === "error") return "Failed";
  return "Working…";
}
