import { usageColorVar, usageLevel } from "../lib/usageLevel";

type Props = {
  values: number[];
  max?: number;
  label?: string;
  /** hero = performance pane; mini = overview tiles */
  variant?: "hero" | "mini";
  windowLabel?: string;
  formatValue?: (v: number) => string;
};

export function UsageChart({
  values,
  max = 100,
  label,
  variant = "mini",
  windowLabel = "60 sec",
  formatValue = (v) => `${Math.round(v)}%`,
}: Props) {
  const isHero = variant === "hero";
  const width = 720;
  const height = isHero ? 200 : 56;
  const padX = 2;
  const padY = isHero ? 8 : 4;
  const hGrid = isHero ? 4 : 2;
  const vGrid = isHero ? 6 : 3;
  const latest = values.length ? values[values.length - 1] : 0;
  const pct = max > 0 ? (latest / max) * 100 : 0;
  const stroke = usageColorVar(pct);
  const level = usageLevel(pct);
  const gradId = `cg-${variant}-${level}`;
  const plotH = height - padY * 2;
  const empty = values.length < 2;

  const points = empty
    ? []
    : values.map((v, i) => {
        const x = padX + (i / (values.length - 1)) * (width - padX * 2);
        const y =
          padY + plotH - (Math.min(max, Math.max(0, v)) / max) * plotH;
        return { x, y };
      });

  const line = points.map((p, i) => `${i === 0 ? "M" : "L"}${p.x},${p.y}`).join(" ");
  const area = empty
    ? ""
    : `${line} L${points[points.length - 1].x},${padY + plotH} L${points[0].x},${padY + plotH} Z`;

  return (
    <div
      className={`usage-chart usage-chart-${variant}`}
      data-level={level}
    >
      {isHero && (
        <div className="usage-chart-yaxis" aria-hidden>
          <span>{formatValue(max)}</span>
          <span>{formatValue(max * 0.5)}</span>
          <span>{formatValue(0)}</span>
        </div>
      )}

      <div className="usage-chart-plot">
        <svg
          viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="none"
          role="img"
          aria-label={label ?? "Usage history"}
        >
          <defs>
            <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={stroke} stopOpacity="0.34" />
              <stop offset="55%" stopColor={stroke} stopOpacity="0.1" />
              <stop offset="100%" stopColor={stroke} stopOpacity="0.02" />
            </linearGradient>
          </defs>

          {Array.from({ length: hGrid + 1 }).map((_, i) => {
            const y = padY + (i / hGrid) * plotH;
            return (
              <line
                key={`h${i}`}
                x1={padX}
                y1={y}
                x2={width - padX}
                y2={y}
                stroke="var(--chart-grid)"
                strokeWidth="1"
                vectorEffect="non-scaling-stroke"
              />
            );
          })}

          {Array.from({ length: vGrid + 1 }).map((_, i) => {
            const x = padX + (i / vGrid) * (width - padX * 2);
            return (
              <line
                key={`v${i}`}
                x1={x}
                y1={padY}
                x2={x}
                y2={padY + plotH}
                stroke="var(--chart-grid)"
                strokeWidth="1"
                vectorEffect="non-scaling-stroke"
                opacity={0.65}
              />
            );
          })}

          {!empty && (
            <>
              <path d={area} fill={`url(#${gradId})`} />
              <path
                d={line}
                fill="none"
                stroke={stroke}
                strokeWidth={isHero ? 1.35 : 1.1}
                strokeLinejoin="round"
                strokeLinecap="round"
                vectorEffect="non-scaling-stroke"
              />
              <circle
                cx={points[points.length - 1].x}
                cy={points[points.length - 1].y}
                r={isHero ? 3 : 2.25}
                fill={stroke}
              />
            </>
          )}

          {empty && (
            <line
              x1={padX}
              y1={height / 2}
              x2={width - padX}
              y2={height / 2}
              stroke="var(--border-strong)"
              strokeWidth="1"
              strokeDasharray="4 4"
              vectorEffect="non-scaling-stroke"
            />
          )}
        </svg>

        {!empty && (
          <div className="usage-chart-live" style={{ color: stroke }}>
            {formatValue(latest)}
          </div>
        )}
      </div>

      {isHero && (
        <div className="usage-chart-meta">
          <span>{windowLabel} ago</span>
          <span>Now</span>
        </div>
      )}
    </div>
  );
}

/** @deprecated Prefer UsageChart */
export function Sparkline(props: Omit<Props, "variant">) {
  return <UsageChart {...props} variant="mini" />;
}
