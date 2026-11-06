import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
} from "react";
import {
  clearActivityHistory,
  getActivityDay,
  getActivitySettings,
  listActivityDates,
  setActivitySettings,
} from "../../lib/api";
import {
  formatClock,
  formatClockShort,
  formatDayLabel,
  formatDayShort,
  formatDuration,
  formatDurationFriendly,
} from "../../lib/format";
import type {
  ActivityDay,
  ActivitySettings,
  AppUsageRow,
  SystemEventRow,
  TimelineSegment,
} from "../../lib/types";

type Props = {
  onToast: (kind: "success" | "error" | "info", message: string) => void;
};

type AppSort =
  | "foreground"
  | "background"
  | "active"
  | "sessions"
  | "recent";

const IDLE_OPTIONS = [
  { value: 60, label: "1 minute" },
  { value: 180, label: "3 minutes" },
  { value: 300, label: "5 minutes" },
  { value: 600, label: "10 minutes" },
  { value: 900, label: "15 minutes" },
];

function todayIso(): string {
  const d = new Date();
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function shiftDate(iso: string, delta: number): string {
  const d = new Date(`${iso}T12:00:00`);
  d.setDate(d.getDate() + delta);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function earliestAllowed(): string {
  return shiftDate(todayIso(), -89);
}

/** Soften backend status for the UI; keep incomplete signal. */
function friendlyStatus(day: ActivityDay): { text: string; tone: "ok" | "warn" | "quiet" } {
  const unknownShare =
    day.overview.unknownMs /
    Math.max(
      1,
      day.overview.uptimeMs +
        day.overview.unknownMs +
        day.overview.suspendedMs +
        day.overview.poweredOffMs,
    );

  if (!day.tracking.hasSamples && !day.tracking.hasJournalEvents) {
    return {
      text: "No activity recorded for this day yet",
      tone: "quiet",
    };
  }
  if (unknownShare > 0.5 && day.tracking.hasSamples) {
    return {
      text: "Tracking started later in the day — earlier hours are unrecorded",
      tone: "warn",
    };
  }
  if (day.tracking.incomplete) {
    return {
      text: "Some periods are estimated or incomplete",
      tone: "warn",
    };
  }
  return {
    text: "Tracking looks complete for the hours DevMonitor was open",
    tone: "ok",
  };
}

/** Collapse noisy consecutive monitor_start duplicates for display. */
function tidyEvents(events: SystemEventRow[]): SystemEventRow[] {
  const out: SystemEventRow[] = [];
  for (const ev of events) {
    const prev = out[out.length - 1];
    if (
      prev &&
      prev.kind === ev.kind &&
      (ev.kind === "monitor_start" || ev.kind === "monitor_stop") &&
      Math.abs(ev.tsMs - prev.tsMs) < 60_000
    ) {
      continue;
    }
    out.push(ev);
  }
  return out;
}

export function ActivityTrackerView({ onToast }: Props) {
  const [date, setDate] = useState(todayIso);
  const [day, setDay] = useState<ActivityDay | null>(null);
  const [settings, setSettings] = useState<ActivitySettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [appSort, setAppSort] = useState<AppSort>("foreground");
  const [selectedApp, setSelectedApp] = useState<string | null>(null);
  const [hoverSeg, setHoverSeg] = useState<TimelineSegment | null>(null);
  const [hoverPos, setHoverPos] = useState<{ x: number; y: number } | null>(
    null,
  );
  const [confirmClear, setConfirmClear] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notesOpen, setNotesOpen] = useState(false);

  const load = useCallback(async (d: string) => {
    setLoading(true);
    setError(null);
    try {
      const [activity, sett] = await Promise.all([
        getActivityDay(d),
        getActivitySettings(),
        listActivityDates(),
      ]);
      setDay(activity);
      setSettings(sett);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setDay(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(date);
  }, [date, load]);

  useEffect(() => {
    if (date !== todayIso()) return;
    const id = window.setInterval(() => void load(date), 15000);
    return () => window.clearInterval(id);
  }, [date, load]);

  const today = todayIso();
  const minDate = earliestAllowed();
  const canPrev = date > minDate;
  const canNext = date < today;

  const apps = useMemo(() => {
    if (!day) return [];
    const list = [...day.apps];
    switch (appSort) {
      case "background":
        list.sort((a, b) => b.backgroundMs - a.backgroundMs);
        break;
      case "active":
        list.sort((a, b) => b.activeMs - a.activeMs);
        break;
      case "sessions":
        list.sort((a, b) => b.sessions - a.sessions);
        break;
      case "recent":
        list.sort((a, b) => b.lastMs - a.lastMs);
        break;
      default:
        list.sort((a, b) => b.foregroundMs - a.foregroundMs);
    }
    return list;
  }, [day, appSort]);

  const maxForeground = useMemo(
    () => Math.max(1, ...apps.map((a) => a.foregroundMs)),
    [apps],
  );

  const events = useMemo(
    () => (day ? tidyEvents(day.events) : []),
    [day],
  );

  const selected = apps.find((a) => a.appId === selectedApp) ?? null;
  const status = day ? friendlyStatus(day) : null;

  const saveSettings = async (next: ActivitySettings) => {
    setBusy(true);
    try {
      const saved = await setActivitySettings(next);
      setSettings(saved);
      onToast("success", "Preferences saved");
      await load(date);
    } catch (e) {
      onToast("error", e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const doClear = async () => {
    setConfirmClear(false);
    setBusy(true);
    try {
      const r = await clearActivityHistory();
      onToast("success", r.message);
      await load(date);
    } catch (e) {
      onToast("error", e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  if (loading && !day) {
    return (
      <div
        className="activity-tracker"
        aria-busy="true"
        aria-label="Loading activity"
      >
        <div className="skeleton-stack">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="skeleton activity-skeleton" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="activity-tracker">
      <header className="activity-hero">
        <div className="activity-hero-text">
          <h3 className="activity-date-label">{formatDayLabel(date)}</h3>
          <p className="activity-kicker">Your day at a glance</p>
        </div>
        <div className="activity-nav" role="group" aria-label="Date navigation">
          <button
            type="button"
            className="activity-nav-icon"
            disabled={!canPrev || busy}
            onClick={() => setDate(shiftDate(date, -1))}
            aria-label="Previous day"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" aria-hidden>
              <path d="M15 6l-6 6 6 6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
          <label className="activity-date-wrap" title="Choose a date">
            <span className="activity-date-text">{formatDayShort(date)}</span>
            <input
              type="date"
              className="activity-date-input"
              value={date}
              min={minDate}
              max={today}
              onChange={(e) => {
                const v = e.target.value;
                if (v && v <= today && v >= minDate) setDate(v);
              }}
              aria-label="Choose a date"
            />
          </label>
          <button
            type="button"
            className="activity-nav-icon"
            disabled={!canNext || busy}
            onClick={() => setDate(shiftDate(date, 1))}
            aria-label="Next day"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" aria-hidden>
              <path d="M9 6l6 6-6 6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
          {date !== today && (
            <button
              type="button"
              className="activity-nav-today"
              disabled={busy}
              onClick={() => setDate(today)}
            >
              Today
            </button>
          )}
        </div>
      </header>

      {error && <div className="banner">{error}</div>}

      {day && status && (
        <>
          <p className={`activity-status tone-${status.tone}`} role="status">
            {status.text}
          </p>

          <section className="activity-summary" aria-label="Day summary">
            <div className="activity-summary-primary">
              <MetricCard
                tone="available"
                label="System uptime"
                value={formatDurationFriendly(day.overview.uptimeMs)}
                sub="Time your computer was available"
              />
              <MetricCard
                tone="active"
                label="Active interaction"
                value={formatDurationFriendly(day.overview.activeMs)}
                sub="Keyboard or mouse activity detected"
              />
            </div>
            <div className="activity-summary-secondary">
              <MetricCard
                tone="idle"
                label="Idle"
                value={formatDurationFriendly(day.overview.idleMs)}
                sub="No input detected"
              />
              <MetricCard
                tone="locked"
                label="Locked"
                value={formatDurationFriendly(day.overview.lockedMs)}
                sub="Session locked"
              />
              <MetricCard
                tone="app"
                label="In focus"
                value={formatDurationFriendly(day.overview.foregroundMs)}
                sub="Foreground apps"
              />
              <MetricCard
                tone="bg"
                label="Background"
                value={formatDurationFriendly(day.overview.backgroundMs)}
                sub={
                  day.overview.backgroundIsEstimate
                    ? "Estimated while monitoring"
                    : "Open but not focused"
                }
              />
            </div>
          </section>

          <section className="activity-section" aria-labelledby="timeline-heading">
            <div className="activity-section-head">
              <h4 id="timeline-heading">Your day</h4>
              <p>System state across the day — zoom in to inspect busy periods</p>
            </div>
            <Timeline
              segments={day.timeline}
              date={day.date}
              onHover={(seg, pos) => {
                setHoverSeg(seg);
                setHoverPos(pos);
              }}
              onLeave={() => {
                setHoverSeg(null);
                setHoverPos(null);
              }}
            />
            <div className="activity-legend" aria-label="Timeline legend">
              {day.legend.map((item) => (
                <span key={item.state} className="activity-legend-item">
                  <span
                    className={`activity-swatch state-${item.colorToken}`}
                    aria-hidden
                  />
                  {item.label}
                </span>
              ))}
            </div>
          </section>

          {hoverSeg && hoverPos && (
            <div
              className="activity-tooltip"
              style={{ left: hoverPos.x, top: hoverPos.y }}
              role="tooltip"
            >
              <strong>{hoverSeg.label}</strong>
              <div>
                {formatClockShort(hoverSeg.startMs)} –{" "}
                {formatClockShort(hoverSeg.endMs)}
              </div>
              <div>{formatDuration(hoverSeg.durationMs)}</div>
            </div>
          )}

          <section className="activity-section" aria-labelledby="apps-heading">
            <div className="activity-section-head row">
              <div>
                <h4 id="apps-heading">Applications</h4>
                <p>Where your focus went</p>
              </div>
              <label className="activity-sort">
                <span className="sr-only">Sort by</span>
                <select
                  className="dialog-input"
                  value={appSort}
                  onChange={(e) => setAppSort(e.target.value as AppSort)}
                  aria-label="Sort applications"
                >
                  <option value="foreground">Most time in focus</option>
                  <option value="background">Most background time</option>
                  <option value="active">Most active</option>
                  <option value="sessions">Most sessions</option>
                  <option value="recent">Recently used</option>
                </select>
              </label>
            </div>

            {apps.length === 0 ? (
              <div className="activity-empty">
                <strong>No apps recorded yet</strong>
                <p>
                  Focus time appears while DevMonitor is open and can see the
                  active window.
                </p>
              </div>
            ) : (
              <ul className="activity-app-list">
                {apps.map((app) => {
                  const pct = Math.max(
                    6,
                    (app.foregroundMs / maxForeground) * 100,
                  );
                  const isSelected = selectedApp === app.appId;
                  return (
                    <li key={app.appId}>
                      <button
                        type="button"
                        className={`activity-app-row ${isSelected ? "selected" : ""}`}
                        onClick={() =>
                          setSelectedApp(isSelected ? null : app.appId)
                        }
                        aria-expanded={isSelected}
                      >
                        <span className="activity-app-icon" aria-hidden>
                          {app.appName.slice(0, 1).toUpperCase()}
                        </span>
                        <span className="activity-app-main">
                          <span className="activity-app-name-row">
                            <span className="activity-app-name">
                              {app.appName}
                            </span>
                            <span className="activity-app-sessions">
                              {app.sessions} session
                              {app.sessions === 1 ? "" : "s"}
                            </span>
                          </span>
                          <span className="activity-app-bar" aria-hidden>
                            <span style={{ width: `${pct}%` }} />
                          </span>
                        </span>
                        <span className="activity-app-duration">
                          {formatDurationFriendly(app.foregroundMs)}
                        </span>
                      </button>
                      {isSelected && selected && <AppDetail app={selected} />}
                    </li>
                  );
                })}
              </ul>
            )}
          </section>

          <div className="activity-split">
            <section
              className="activity-section"
              aria-labelledby="events-heading"
            >
              <div className="activity-section-head">
                <h4 id="events-heading">Moments</h4>
                <p>Locks, sleep, and other events</p>
              </div>
              {events.length === 0 ? (
                <div className="activity-empty">
                  <strong>No events yet</strong>
                  <p>Boot and sleep events appear from the system journal.</p>
                </div>
              ) : (
                <ol className="activity-event-list">
                  {events.map((ev, i) => (
                    <li key={`${ev.tsMs}-${ev.kind}-${i}`}>
                      <span
                        className={`activity-event-dot kind-${ev.kind}`}
                        aria-hidden
                      />
                      <time dateTime={new Date(ev.tsMs).toISOString()}>
                        {formatClockShort(ev.tsMs)}
                      </time>
                      <div className="activity-event-body">
                        <strong>{eventLabel(ev.kind)}</strong>
                        {ev.inferred && (
                          <span className="activity-event-tag">Estimated</span>
                        )}
                      </div>
                    </li>
                  ))}
                </ol>
              )}
            </section>

            <section
              className="activity-section"
              aria-labelledby="settings-heading"
            >
              <div className="activity-section-head">
                <h4 id="settings-heading">Preferences</h4>
                <p>Stored only on this computer</p>
              </div>

              {settings && (
                <div className="activity-settings">
                  <label className="activity-pref">
                    <span className="activity-pref-text">
                      <strong>Idle after</strong>
                      <span>Quiet time before idle is counted</span>
                    </span>
                    <select
                      className="dialog-input"
                      value={settings.idleThresholdSecs}
                      disabled={busy}
                      onChange={(e) =>
                        void saveSettings({
                          ...settings,
                          idleThresholdSecs: Number(e.target.value),
                        })
                      }
                    >
                      {IDLE_OPTIONS.map((o) => (
                        <option key={o.value} value={o.value}>
                          {o.label}
                        </option>
                      ))}
                    </select>
                  </label>

                  <label className="activity-pref">
                    <span className="activity-pref-text">
                      <strong>Keep tracking while open</strong>
                      <span>Pause new samples when turned off</span>
                    </span>
                    <input
                      type="checkbox"
                      className="activity-toggle"
                      checked={settings.trackingEnabled}
                      disabled={busy}
                      onChange={(e) =>
                        void saveSettings({
                          ...settings,
                          trackingEnabled: e.target.checked,
                        })
                      }
                      aria-label="Keep tracking while open"
                    />
                  </label>

                  <label className="activity-pref">
                    <span className="activity-pref-text">
                      <strong>Browser domains</strong>
                      <span>Needs a separate extension</span>
                    </span>
                    <input
                      type="checkbox"
                      className="activity-toggle"
                      checked={settings.browserDomainTracking}
                      disabled={busy}
                      onChange={(e) =>
                        void saveSettings({
                          ...settings,
                          browserDomainTracking: e.target.checked,
                        })
                      }
                      aria-label="Browser domain tracking"
                    />
                  </label>

                  <div className="activity-pref danger">
                    <span className="activity-pref-text">
                      <strong>Clear history</strong>
                      <span>Removes up to 90 days of local data</span>
                    </span>
                    <button
                      type="button"
                      className="btn btn-danger"
                      disabled={busy}
                      onClick={() => setConfirmClear(true)}
                    >
                      Clear
                    </button>
                  </div>
                </div>
              )}

              <button
                type="button"
                className="activity-notes-toggle"
                onClick={() => setNotesOpen((v) => !v)}
                aria-expanded={notesOpen}
              >
                {notesOpen ? "Hide measurement notes" : "How metrics are measured"}
              </button>
              {notesOpen && (
                <ul className="activity-notes">
                  {day.notes.map((n) => (
                    <li key={n}>{n}</li>
                  ))}
                </ul>
              )}
            </section>
          </div>
        </>
      )}

      {!loading && !day && !error && (
        <div className="activity-empty">
          <strong>Nothing here yet</strong>
          <p>
            Tracking starts when DevMonitor is running. Power events may still
            appear from the system journal.
          </p>
        </div>
      )}

      {confirmClear && (
        <div
          className="overlay"
          onClick={() => setConfirmClear(false)}
          role="presentation"
        >
          <div
            className="dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="clear-activity-title"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 id="clear-activity-title">Clear activity history?</h2>
            <div className="dialog-meta">
              Deletes up to 90 days of local tracking data. Monitoring can
              continue afterward. Nothing is sent anywhere else.
            </div>
            <div className="dialog-actions">
              <button
                type="button"
                className="btn"
                onClick={() => setConfirmClear(false)}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn btn-danger"
                onClick={() => void doClear()}
              >
                Clear history
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function MetricCard({
  label,
  value,
  sub,
  tone,
}: {
  label: string;
  value: string;
  sub: string;
  tone: string;
}) {
  return (
    <article className={`activity-metric tone-${tone}`}>
      <h5>{label}</h5>
      <p className="activity-metric-value">{value}</p>
      <p className="activity-metric-sub">{sub}</p>
    </article>
  );
}

const ZOOM_HOURS = [24, 12, 6, 3, 1] as const;
type ZoomHours = (typeof ZOOM_HOURS)[number];

const INTERESTING = new Set([
  "active",
  "idle",
  "locked",
  "available",
  "suspended",
]);

function activityBounds(
  segments: TimelineSegment[],
  dayStart: number,
  dayEnd: number,
): { start: number; end: number } | null {
  let start = Number.POSITIVE_INFINITY;
  let end = Number.NEGATIVE_INFINITY;
  for (const seg of segments) {
    if (!INTERESTING.has(seg.state)) continue;
    if (seg.durationMs < 30_000 && seg.state !== "active") continue;
    start = Math.min(start, seg.startMs);
    end = Math.max(end, seg.endMs);
  }
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) {
    return null;
  }
  const pad = Math.min(15 * 60_000, (end - start) * 0.15);
  return {
    start: Math.max(dayStart, start - pad),
    end: Math.min(dayEnd, end + pad),
  };
}

function clampWindow(
  start: number,
  windowMs: number,
  dayStart: number,
  dayEnd: number,
): number {
  const maxStart = Math.max(dayStart, dayEnd - windowMs);
  return Math.min(Math.max(start, dayStart), maxStart);
}

function tickStepMs(windowMs: number): number {
  if (windowMs <= 60 * 60_000) return 5 * 60_000;
  if (windowMs <= 3 * 60 * 60_000) return 15 * 60_000;
  if (windowMs <= 6 * 60 * 60_000) return 30 * 60_000;
  if (windowMs <= 12 * 60 * 60_000) return 60 * 60_000;
  return 3 * 60 * 60_000;
}

function formatTick(ms: number, windowMs: number): string {
  const d = new Date(ms);
  if (windowMs <= 3 * 60 * 60_000) {
    return d.toLocaleTimeString(undefined, {
      hour: "numeric",
      minute: "2-digit",
    });
  }
  return d.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: windowMs <= 12 * 60 * 60_000 ? "2-digit" : undefined,
  });
}

function Timeline({
  segments,
  date,
  onHover,
  onLeave,
}: {
  segments: TimelineSegment[];
  date: string;
  onHover: (seg: TimelineSegment, pos: { x: number; y: number }) => void;
  onLeave: () => void;
}) {
  const dayStart = useMemo(
    () => new Date(`${date}T00:00:00`).getTime(),
    [date],
  );
  const dayEnd = dayStart + 24 * 60 * 60 * 1000;
  const dayMs = dayEnd - dayStart;

  const [zoomHours, setZoomHours] = useState<ZoomHours>(24);
  const [viewStart, setViewStart] = useState(dayStart);
  const trackRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{
    pointerId: number;
    originX: number;
    originStart: number;
  } | null>(null);

  const windowMs = zoomHours * 60 * 60_000;
  const zoomed = zoomHours < 24;

  // Reset when the selected day changes.
  useEffect(() => {
    setZoomHours(24);
    setViewStart(dayStart);
  }, [dayStart]);

  useEffect(() => {
    setViewStart((prev) => clampWindow(prev, windowMs, dayStart, dayEnd));
  }, [windowMs, dayStart, dayEnd]);

  const viewEnd = viewStart + windowMs;
  const interesting = useMemo(
    () => activityBounds(segments, dayStart, dayEnd),
    [segments, dayStart, dayEnd],
  );

  const setZoom = (next: ZoomHours, centerMs?: number) => {
    const nextWindow = next * 60 * 60_000;
    const center =
      centerMs ??
      (zoomed ? viewStart + windowMs / 2 : (interesting?.start ?? dayStart) +
        ((interesting?.end ?? dayEnd) - (interesting?.start ?? dayStart)) / 2);
    const start = clampWindow(
      center - nextWindow / 2,
      nextWindow,
      dayStart,
      dayEnd,
    );
    setZoomHours(next);
    setViewStart(start);
  };

  const zoomIn = () => {
    const idx = ZOOM_HOURS.indexOf(zoomHours);
    if (idx < ZOOM_HOURS.length - 1) setZoom(ZOOM_HOURS[idx + 1]);
  };

  const zoomOut = () => {
    const idx = ZOOM_HOURS.indexOf(zoomHours);
    if (idx > 0) setZoom(ZOOM_HOURS[idx - 1]);
  };

  const fitActivity = () => {
    if (!interesting) {
      setZoom(24);
      return;
    }
    const spanH = (interesting.end - interesting.start) / (60 * 60_000);
    const fit =
      ZOOM_HOURS.find((h) => h >= spanH * 1.25) ??
      ZOOM_HOURS[ZOOM_HOURS.length - 1];
    setZoom(fit, (interesting.start + interesting.end) / 2);
  };

  const panBy = (deltaMs: number) => {
    setViewStart((prev) =>
      clampWindow(prev + deltaMs, windowMs, dayStart, dayEnd),
    );
  };

  const onWheel = (e: ReactWheelEvent<HTMLDivElement>) => {
    // Ctrl/Cmd+scroll zooms; when zoomed, plain scroll pans the window.
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      if (e.deltaY < 0) zoomIn();
      else zoomOut();
      return;
    }
    if (!zoomed) return;
    e.preventDefault();
    const track = trackRef.current;
    if (!track) return;
    const ratio = e.deltaY / Math.max(1, track.clientWidth);
    panBy(ratio * windowMs * 0.85);
  };

  const onPointerDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!zoomed || e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest(".activity-seg")) return;
    dragRef.current = {
      pointerId: e.pointerId,
      originX: e.clientX,
      originStart: viewStart,
    };
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const onPointerMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointerId !== e.pointerId) return;
    const track = trackRef.current;
    if (!track) return;
    const dx = e.clientX - drag.originX;
    const deltaMs = -(dx / Math.max(1, track.clientWidth)) * windowMs;
    setViewStart(
      clampWindow(drag.originStart + deltaMs, windowMs, dayStart, dayEnd),
    );
  };

  const onPointerUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (dragRef.current?.pointerId === e.pointerId) {
      dragRef.current = null;
    }
  };

  const ticks = useMemo(() => {
    const step = tickStepMs(windowMs);
    const first = Math.ceil(viewStart / step) * step;
    const out: number[] = [];
    for (let t = first; t <= viewEnd; t += step) {
      if (t >= dayStart && t <= dayEnd) out.push(t);
    }
    return out;
  }, [viewStart, viewEnd, windowMs, dayStart, dayEnd]);

  const visibleSegs = useMemo(
    () =>
      segments.filter((seg) => seg.endMs > viewStart && seg.startMs < viewEnd),
    [segments, viewStart, viewEnd],
  );

  const rangeLabel = `${formatClockShort(viewStart)} – ${formatClockShort(Math.min(viewEnd, dayEnd))}`;

  return (
    <div className="activity-timeline" onMouseLeave={onLeave}>
      <div className="activity-timeline-toolbar">
        <div className="activity-zoom-group" role="group" aria-label="Zoom">
          <button
            type="button"
            className="btn activity-zoom-btn"
            onClick={zoomOut}
            disabled={zoomHours === 24}
            aria-label="Zoom out"
            title="Zoom out"
          >
            −
          </button>
          <div className="activity-zoom-presets">
            {ZOOM_HOURS.map((h) => (
              <button
                key={h}
                type="button"
                className={`activity-zoom-chip ${zoomHours === h ? "active" : ""}`}
                onClick={() => setZoom(h)}
                aria-pressed={zoomHours === h}
              >
                {h === 24 ? "Day" : `${h}h`}
              </button>
            ))}
          </div>
          <button
            type="button"
            className="btn activity-zoom-btn"
            onClick={zoomIn}
            disabled={zoomHours === 1}
            aria-label="Zoom in"
            title="Zoom in"
          >
            +
          </button>
        </div>

        <div className="activity-timeline-actions">
          {interesting && (
            <button
              type="button"
              className="btn activity-zoom-fit"
              onClick={fitActivity}
            >
              Fit activity
            </button>
          )}
          {zoomed && (
            <>
              <button
                type="button"
                className="btn activity-zoom-btn"
                onClick={() => panBy(-windowMs * 0.5)}
                aria-label="Pan earlier"
              >
                ‹
              </button>
              <button
                type="button"
                className="btn activity-zoom-btn"
                onClick={() => panBy(windowMs * 0.5)}
                aria-label="Pan later"
              >
                ›
              </button>
            </>
          )}
          <span className="activity-range-label">{rangeLabel}</span>
        </div>
      </div>

      <div
        className={`activity-timeline-viewport ${zoomed ? "zoomed" : ""}`}
        ref={trackRef}
        onWheel={onWheel}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
      >
        <div
          className="activity-timeline-track"
          role="img"
          aria-label={`Activity timeline showing ${rangeLabel}`}
        >
          {visibleSegs.map((seg, i) => {
            const left =
              ((Math.max(seg.startMs, viewStart) - viewStart) / windowMs) * 100;
            const right =
              ((Math.min(seg.endMs, viewEnd) - viewStart) / windowMs) * 100;
            // Keep a hairline gap so rounded ends don't visually merge.
            const gap = zoomed ? 0.12 : 0.06;
            const raw = Math.max(right - left, 0);
            const width = Math.max(raw - gap, zoomed ? 0.18 : 0.12);
            const short = raw < (zoomed ? 1.2 : 0.6);
            return (
              <button
                key={`${seg.startMs}-${seg.state}-${i}`}
                type="button"
                className={`activity-seg state-${seg.state} source-${seg.source}${short ? " short" : ""}`}
                style={{ left: `${left}%`, width: `${width}%` }}
                title={`${seg.label}\n${formatClockShort(seg.startMs)} – ${formatClockShort(seg.endMs)}\n${formatDuration(seg.durationMs)}`}
                aria-label={`${seg.label}, ${formatClock(seg.startMs)} to ${formatClock(seg.endMs)}, ${formatDuration(seg.durationMs)}`}
                onFocus={(e) => {
                  const rect = e.currentTarget.getBoundingClientRect();
                  onHover(seg, { x: rect.left, y: rect.bottom + 8 });
                }}
                onBlur={onLeave}
                onMouseEnter={(e) =>
                  onHover(seg, { x: e.clientX + 12, y: e.clientY + 12 })
                }
                onMouseMove={(e) =>
                  onHover(seg, { x: e.clientX + 12, y: e.clientY + 12 })
                }
              />
            );
          })}
        </div>
      </div>

      <div className="activity-hours" aria-hidden>
        {ticks.map((t) => (
          <span
            key={t}
            className="activity-tick"
            style={{ left: `${((t - viewStart) / windowMs) * 100}%` }}
          >
            <i />
            {formatTick(t, windowMs)}
          </span>
        ))}
      </div>

      <div
        className="activity-daymap"
        aria-label="Full day overview"
        onClick={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const ratio = (e.clientX - rect.left) / Math.max(1, rect.width);
          const center = dayStart + ratio * dayMs;
          setViewStart(
            clampWindow(center - windowMs / 2, windowMs, dayStart, dayEnd),
          );
          if (!zoomed) setZoom(6, center);
        }}
      >
        <div className="activity-daymap-track">
          {segments.map((seg, i) => {
            const left = ((seg.startMs - dayStart) / dayMs) * 100;
            const width = Math.max(
              ((seg.endMs - seg.startMs) / dayMs) * 100,
              0.15,
            );
            return (
              <span
                key={`ov-${seg.startMs}-${seg.state}-${i}`}
                className={`activity-daymap-seg state-${seg.state}`}
                style={{ left: `${left}%`, width: `${width}%` }}
              />
            );
          })}
          <span
            className="activity-daymap-window"
            style={{
              left: `${((viewStart - dayStart) / dayMs) * 100}%`,
              width: `${(windowMs / dayMs) * 100}%`,
            }}
          />
        </div>
        <div className="activity-daymap-hint">
          {zoomed
            ? "Drag to pan · click the map to jump · Ctrl+scroll to zoom"
            : "Zoom in or click the map where activity appears"}
        </div>
      </div>
    </div>
  );
}

function AppDetail({ app }: { app: AppUsageRow }) {
  return (
    <div className="activity-app-detail">
      <div className="activity-app-detail-grid">
        <span>
          <em>In focus</em>
          {formatDuration(app.foregroundMs)}
        </span>
        <span>
          <em>Active</em>
          {formatDuration(app.activeMs)}
        </span>
        <span>
          <em>Background</em>
          {formatDuration(app.backgroundMs)}
          {app.backgroundIsEstimate ? " est." : ""}
        </span>
      </div>
      <p>
        Seen {formatClockShort(app.firstMs)} – {formatClockShort(app.lastMs)}
      </p>
    </div>
  );
}

function eventLabel(kind: string): string {
  switch (kind) {
    case "boot":
      return "System started";
    case "shutdown":
      return "System shut down";
    case "suspend":
      return "Went to sleep";
    case "resume":
      return "Woke up";
    case "lock":
      return "Session locked";
    case "unlock":
      return "Session unlocked";
    case "monitor_start":
      return "Tracking started";
    case "monitor_stop":
      return "Tracking stopped";
    case "monitor_interrupted":
      return "Tracking interrupted";
    default:
      return kind;
  }
}
