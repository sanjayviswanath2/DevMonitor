import type { RefObject } from "react";
import type { AppView } from "../lib/types";

const TITLES: Record<AppView, string> = {
  issues: "Issues",
  system: "System",
  ports: "Ports",
  activity: "Activity",
  node: "Node.js",
  docker: "Docker",
  git: "Git",
};

type Props = {
  view: AppView;
  query: string;
  onQueryChange: (value: string) => void;
  inputRef: RefObject<HTMLInputElement | null>;
  refreshing: boolean;
  onRefresh: () => void;
};

export function AppHeader({
  view,
  query,
  onQueryChange,
  inputRef,
  refreshing,
  onRefresh,
}: Props) {
  return (
    <header className="app-header">
      <div className="header-titles">
        <h2>{TITLES[view]}</h2>
      </div>

      <label className="search-field">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
          <circle cx="11" cy="11" r="7" />
          <path d="M20 20l-3.5-3.5" />
        </svg>
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          placeholder="Filter processes, ports, PIDs…"
          spellCheck={false}
          autoComplete="off"
        />
        <kbd className="search-kbd">Ctrl K</kbd>
      </label>

      <div className="header-actions">
        <button
          className={`icon-btn ${refreshing ? "spinning" : ""}`}
          onClick={onRefresh}
          title="Refresh now"
          aria-label="Refresh"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M21 12a9 9 0 1 1-2.6-6.4" />
            <path d="M21 3v6h-6" />
          </svg>
        </button>
      </div>
    </header>
  );
}
