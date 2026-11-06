import type { ReactNode } from "react";
import type { AppView } from "../lib/types";

type Props = {
  view: AppView;
  onNavigate: (view: AppView) => void;
  themeMode: "system" | "light" | "dark";
  onThemeChange: (mode: "system" | "light" | "dark") => void;
  live: boolean;
  onLiveToggle: () => void;
  issueCount: number;
};

type NavItem = { id: AppView; label: string; icon: ReactNode };

const ACTIVITY: NavItem[] = [
  {
    id: "activity",
    label: "Activity",
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <path d="M3 12h4l2-7 4 14 2-7h6" />
      </svg>
    ),
  },
];

const SYSTEM: NavItem[] = [
  {
    id: "system",
    label: "System",
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <rect x="3" y="3" width="7" height="9" rx="1.5" />
        <rect x="14" y="3" width="7" height="5" rx="1.5" />
        <rect x="14" y="12" width="7" height="9" rx="1.5" />
        <rect x="3" y="16" width="7" height="5" rx="1.5" />
      </svg>
    ),
  },
  {
    id: "ports",
    label: "Ports",
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <circle cx="12" cy="12" r="3" />
        <path d="M12 3v3M12 18v3M3 12h3M18 12h3" />
      </svg>
    ),
  },
];

const TOOLS: NavItem[] = [
  {
    id: "issues",
    label: "Issues",
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <circle cx="12" cy="12" r="9" />
        <path d="M12 8v5M12 16h.01" />
      </svg>
    ),
  },
  {
    id: "node",
    label: "Node.js",
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <path d="M12 2l8 4.5v11L12 22l-8-4.5v-11L12 2z" />
        <path d="M12 22V12M12 12L4.5 7.5M12 12l7.5-4.5" />
      </svg>
    ),
  },
  {
    id: "docker",
    label: "Docker",
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <rect x="3" y="10" width="4" height="4" rx="0.5" />
        <rect x="8" y="10" width="4" height="4" rx="0.5" />
        <rect x="13" y="10" width="4" height="4" rx="0.5" />
        <rect x="8" y="5" width="4" height="4" rx="0.5" />
        <path d="M3 16h16c1.5 0 3 1 3 2.5S21 21 18 21H5" />
      </svg>
    ),
  },
  {
    id: "git",
    label: "Git",
    icon: (
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
        <circle cx="6" cy="6" r="2" />
        <circle cx="18" cy="6" r="2" />
        <circle cx="12" cy="18" r="2" />
        <path d="M6 8v4c0 2 2 4 6 4M18 8v2" />
      </svg>
    ),
  },
];

function NavGroup({
  title,
  items,
  view,
  onNavigate,
  badge,
  featured = false,
}: {
  title: string;
  items: NavItem[];
  view: AppView;
  onNavigate: (v: AppView) => void;
  badge?: Partial<Record<AppView, number>>;
  featured?: boolean;
}) {
  return (
    <div className={`nav-group ${featured ? "featured" : ""}`}>
      {title ? <div className="nav-group-title">{title}</div> : null}
      {items.map((item) => (
        <button
          key={item.id}
          className={`nav-item ${featured ? "featured" : ""} ${view === item.id ? "active" : ""}`}
          onClick={() => onNavigate(item.id)}
          title={item.label}
        >
          {item.icon}
          <span>{item.label}</span>
          {badge?.[item.id] ? (
            <span className="nav-badge">{badge[item.id]}</span>
          ) : null}
        </button>
      ))}
    </div>
  );
}

export function Sidebar({
  view,
  onNavigate,
  themeMode,
  onThemeChange,
  live,
  onLiveToggle,
  issueCount,
}: Props) {
  return (
    <aside className="sidebar">
      <div className="sidebar-brand">
        <div className="sidebar-mark" aria-hidden>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2">
            <path d="M4 14l4-8 4 5 3-3 5 6" />
          </svg>
        </div>
        <div>
          <h1>DevMonitor</h1>
        </div>
      </div>

      <nav className="nav-list" aria-label="Primary">
        <NavGroup
          title=""
          featured
          items={ACTIVITY}
          view={view}
          onNavigate={onNavigate}
        />
        <NavGroup title="System" items={SYSTEM} view={view} onNavigate={onNavigate} />
        <NavGroup
          title="Tools"
          items={TOOLS}
          view={view}
          onNavigate={onNavigate}
          badge={{ issues: issueCount }}
        />
      </nav>

      <div className="sidebar-footer">
        <div className="theme-switch" role="group" aria-label="Theme">
          {(["system", "light", "dark"] as const).map((mode) => (
            <button
              key={mode}
              className={`theme-btn ${themeMode === mode ? "active" : ""}`}
              onClick={() => onThemeChange(mode)}
            >
              {mode === "system" ? "Auto" : mode[0].toUpperCase() + mode.slice(1)}
            </button>
          ))}
        </div>
        <button
          className={`live-toggle ${live ? "on" : ""}`}
          onClick={onLiveToggle}
          title={live ? "Pause live updates" : "Resume live updates"}
        >
          <span className="live-dot" />
          <span>{live ? "Live updates" : "Paused"}</span>
        </button>
      </div>
    </aside>
  );
}
