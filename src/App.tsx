import { useEffect, useRef, useState } from "react";
import { AppHeader } from "./components/AppHeader";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { Sidebar } from "./components/Sidebar";
import { ToastStack } from "./components/ToastStack";
import { ActivityTrackerView } from "./components/views/ActivityTrackerView";
import { DockerDoctorView } from "./components/views/DockerDoctorView";
import { GitDoctorView } from "./components/views/GitDoctorView";
import { IssuesView } from "./components/views/IssuesView";
import { NodeDoctorView } from "./components/views/NodeDoctorView";
import { PortsView } from "./components/views/PortsView";
import { SystemView } from "./components/views/SystemView";
import { useDoctor } from "./hooks/useDoctor";
import { useSystemMonitor } from "./hooks/useSystemMonitor";
import { useTheme } from "./hooks/useTheme";
import type { AppView } from "./lib/types";
import "./styles/app.css";

const DOCTOR_VIEWS: AppView[] = ["issues", "node", "docker", "git"];

export default function App() {
  const monitor = useSystemMonitor();
  const doctor = useDoctor();
  const theme = useTheme();
  const [view, setView] = useState<AppView>("activity");
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const typing =
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable);

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
        return;
      }

      if (!typing && e.key === "/") {
        e.preventDefault();
        searchRef.current?.focus();
        return;
      }

      if (e.key === "Escape") {
        if (doctor.identityModal) {
          doctor.closeIdentityModal();
          return;
        }
        if (doctor.pendingAction) {
          doctor.cancelFix();
          return;
        }
        if (monitor.pendingKill) {
          monitor.cancelKill();
          return;
        }
        if (document.activeElement === searchRef.current && monitor.query) {
          monitor.setQuery("");
          return;
        }
        monitor.setSelection(null);
        (document.activeElement as HTMLElement | null)?.blur?.();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [monitor, doctor]);

  const navigate = (next: AppView) => setView(next);

  const selectProcessAndOpen = (pid: number) => {
    monitor.setSelection({ type: "process", pid });
    setView("system");
  };

  const isDoctorView = DOCTOR_VIEWS.includes(view);
  const refreshing = isDoctorView ? doctor.scanning : monitor.refreshing;

  const onRefresh = () => {
    if (isDoctorView) void doctor.refresh();
    else void monitor.refresh(true);
  };

  const toasts = [
    ...monitor.toasts,
    ...doctor.toasts.map((t) => ({ ...t, id: t.id + 100000 })),
  ];

  return (
    <div className="app-shell">
      <Sidebar
        view={view}
        onNavigate={navigate}
        themeMode={theme.mode}
        onThemeChange={theme.setMode}
        live={monitor.live}
        onLiveToggle={() => monitor.setLive(!monitor.live)}
        issueCount={doctor.issues.length}
      />

      <div className="main-column">
        <AppHeader
          view={view}
          query={monitor.query}
          onQueryChange={monitor.setQuery}
          inputRef={searchRef}
          refreshing={refreshing}
          onRefresh={onRefresh}
        />

        <main className="content">
          {monitor.error && !monitor.stats && (
            <div className="banner">{monitor.error}</div>
          )}
          {doctor.error && isDoctorView && (
            <div className="banner">{doctor.error}</div>
          )}
          {monitor.portsWarning && view === "ports" && (
            <div className="banner">{monitor.portsWarning}</div>
          )}

          {view === "issues" && (
            <IssuesView
              issues={doctor.issues}
              loading={doctor.loading}
              scanning={doctor.scanning}
              applying={doctor.applying}
              onFix={doctor.requestFix}
              onRefresh={() => void doctor.refresh()}
            />
          )}

          {view === "system" && (
            <SystemView
              cpu={{
                stats: monitor.stats,
                history: monitor.cpuHistory,
                processes: monitor.processes,
                onSelectProcess: selectProcessAndOpen,
              }}
              memory={{
                stats: monitor.stats,
                history: monitor.memHistory,
                onToast: doctor.pushToast,
                onRefresh: () => void monitor.refresh(true),
              }}
              processes={{
                processes: monitor.processes,
                loading: monitor.loading,
                selection: monitor.selection,
                selectedDetail: monitor.selectedDetail,
                sort: monitor.processSort,
                onSort: monitor.toggleProcessSort,
                onSelect: monitor.setSelection,
                onKill: monitor.requestKill,
                filter: monitor.processFilter,
                onFilter: monitor.setProcessFilter,
                filterCounts: monitor.processFilterCounts,
              }}
            />
          )}

          {view === "ports" && (
            <PortsView
              ports={monitor.ports}
              loading={monitor.loading}
              selection={monitor.selection}
              selectedDetail={monitor.selectedDetail}
              sort={monitor.portSort}
              onSort={monitor.togglePortSort}
              onSelect={monitor.setSelection}
              onKill={monitor.requestKill}
            />
          )}

          {view === "activity" && (
            <ActivityTrackerView onToast={doctor.pushToast} />
          )}

          {view === "node" && (
            <NodeDoctorView
              node={doctor.node}
              loading={doctor.loading}
              applying={doctor.applying != null}
              onToast={doctor.pushToast}
              onNodeChanged={() => void doctor.refresh()}
              onRepairPath={() => {
                const fake = doctor.issues.find((i) => i.kind === "node_path");
                if (fake?.action) doctor.requestFix(fake);
                else {
                  doctor.requestFix({
                    id: "manual-path",
                    kind: "node_path",
                    severity: "info",
                    title: "Repair PATH",
                    summary: "",
                    details: [],
                    action: {
                      id: "repair_node_path",
                      label: "Repair PATH",
                      confirm: false,
                      confirmMessage: null,
                    },
                    meta: {},
                  });
                }
              }}
              onRepairNpm={() => {
                doctor.requestFix({
                  id: "manual-npm",
                  kind: "npm_permissions",
                  severity: "info",
                  title: "Repair npm",
                  summary: "",
                  details: [],
                  action: {
                    id: "repair_npm_permissions",
                    label: "Repair npm setup",
                    confirm: true,
                    confirmMessage: "Set a user-owned npm global prefix?",
                  },
                  meta: {},
                });
              }}
            />
          )}

          {view === "docker" && (
            <DockerDoctorView onToast={doctor.pushToast} />
          )}

          {view === "git" && (
            <GitDoctorView
              projectPath={doctor.projectPath}
              onToast={doctor.pushToast}
            />
          )}
        </main>
      </div>

      {monitor.pendingKill && (
        <ConfirmDialog
          name={monitor.pendingKill.name}
          pid={monitor.pendingKill.pid}
          port={monitor.pendingKill.port}
          memory={monitor.pendingKill.memory}
          killing={monitor.killing}
          onCancel={monitor.cancelKill}
          onConfirm={() => void monitor.confirmKill()}
        />
      )}

      {doctor.pendingAction && (
        <div className="overlay" onClick={doctor.cancelFix} role="presentation">
          <div
            className="dialog"
            role="dialog"
            aria-modal="true"
            onClick={(e) => e.stopPropagation()}
          >
            <h2>{doctor.pendingAction.issue.action?.label ?? "Confirm"}</h2>
            <div className="dialog-meta">
              <div>
                {doctor.pendingAction.issue.action?.confirmMessage ??
                  doctor.pendingAction.issue.summary}
              </div>
            </div>
            <div className="dialog-actions">
              <button className="btn" onClick={doctor.cancelFix}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={() => void doctor.confirmFix()}
                disabled={doctor.applying != null}
              >
                {doctor.applying ? "Working…" : "Continue"}
              </button>
            </div>
          </div>
        </div>
      )}

      {doctor.identityModal && (
        <div
          className="overlay"
          onClick={doctor.closeIdentityModal}
          role="presentation"
        >
          <div
            className="dialog"
            role="dialog"
            aria-modal="true"
            onClick={(e) => e.stopPropagation()}
          >
            <h2>Git identity</h2>
            <div className="dialog-meta" style={{ display: "grid", gap: 10 }}>
              <label style={{ display: "grid", gap: 4, fontSize: 14 }}>
                Name
                <input
                  className="dialog-input"
                  value={doctor.identityDraft.name}
                  onChange={(e) =>
                    doctor.setIdentityDraft({
                      ...doctor.identityDraft,
                      name: e.target.value,
                    })
                  }
                  placeholder="Your Name"
                  autoFocus
                />
              </label>
              <label style={{ display: "grid", gap: 4, fontSize: 14 }}>
                Email
                <input
                  className="dialog-input"
                  value={doctor.identityDraft.email}
                  onChange={(e) =>
                    doctor.setIdentityDraft({
                      ...doctor.identityDraft,
                      email: e.target.value,
                    })
                  }
                  placeholder="you@example.com"
                />
              </label>
              <p className="resource-note">Saved as your global Git identity.</p>
            </div>
            <div className="dialog-actions">
              <button className="btn" onClick={doctor.closeIdentityModal}>
                Cancel
              </button>
              <button
                className="btn btn-primary"
                disabled={doctor.applying != null}
                onClick={() => void doctor.submitIdentity()}
              >
                {doctor.applying ? "Saving…" : "Save"}
              </button>
            </div>
          </div>
        </div>
      )}

      <ToastStack
        toasts={toasts}
        onDismiss={(id) => {
          if (id >= 100000) doctor.dismissToast(id - 100000);
          else monitor.dismissToast(id);
        }}
      />
    </div>
  );
}
