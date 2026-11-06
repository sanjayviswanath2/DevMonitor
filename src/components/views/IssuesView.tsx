import type { DoctorIssue } from "../../lib/types";

type Props = {
  issues: DoctorIssue[];
  loading: boolean;
  scanning: boolean;
  applying: string | null;
  onFix: (issue: DoctorIssue) => void;
  onRefresh: () => void;
};

export function IssuesView({
  issues,
  loading,
  scanning,
  applying,
  onFix,
  onRefresh,
}: Props) {
  const errors = issues.filter((i) => i.severity === "error").length;
  const warnings = issues.filter((i) => i.severity === "warning").length;

  const statusLabel = loading
    ? "Scanning…"
    : issues.length === 0
      ? "No issues"
      : `${issues.length} issue${issues.length === 1 ? "" : "s"}`;

  return (
    <div className="issues-view">
      <div className="view-toolbar">
        <div>
          <h3 className="issues-greeting">{statusLabel}</h3>
          <p className="issues-sub">
            Machine-wide: ports, Node, memory, Docker, and Git identity.
          </p>
        </div>
        <div className="issues-hero-actions">
          <button className="btn" onClick={onRefresh} disabled={scanning}>
            {scanning ? "Scanning…" : "Re-scan"}
          </button>
        </div>
      </div>

      {!loading && issues.length > 0 && (
        <div className="issues-summary">
          {errors > 0 && <span className="chip error">{errors} error</span>}
          {warnings > 0 && <span className="chip warning">{warnings} warning</span>}
          <span className="chip">{issues.length} total</span>
        </div>
      )}

      {loading ? (
        <div className="skeleton-stack">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} className="skeleton" style={{ height: 36 }} />
          ))}
        </div>
      ) : issues.length === 0 ? (
        <div className="healthy-card">
          <div className="healthy-icon">✓</div>
          <strong>All clear</strong>
          <span>
            No issues detected. Use System for CPU, memory, and processes, and Ports for listeners.
          </span>
        </div>
      ) : (
        <div className="issue-list">
          {issues.map((issue) => (
            <article
              key={issue.id}
              className={`issue-card severity-${issue.severity}`}
            >
              <div className="issue-card-main">
                <div className="issue-badge">{issue.severity}</div>
                <h4>{issue.title}</h4>
                <p>{issue.summary}</p>
                {issue.details.length > 0 && (
                  <ul className="issue-details">
                    {issue.details.slice(0, 3).map((d) => (
                      <li key={d}>{d}</li>
                    ))}
                  </ul>
                )}
              </div>
              {issue.action && (
                <div className="issue-card-action">
                  <button
                    className="btn btn-primary"
                    disabled={applying === issue.id}
                    onClick={() => onFix(issue)}
                  >
                    {applying === issue.id ? "Working…" : issue.action.label}
                  </button>
                </div>
              )}
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
