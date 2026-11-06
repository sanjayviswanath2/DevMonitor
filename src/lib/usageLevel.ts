export type UsageLevel = "low" | "mid" | "warn" | "hot";

/** 0–25 green, 25–75 blue, 75–90 amber, 90+ red */
export function usageLevel(pct: number): UsageLevel {
  if (pct >= 90) return "hot";
  if (pct >= 75) return "warn";
  if (pct >= 25) return "mid";
  return "low";
}

export function usageColorVar(pct: number): string {
  switch (usageLevel(pct)) {
    case "hot":
      return "var(--usage-hot)";
    case "warn":
      return "var(--usage-warn)";
    case "mid":
      return "var(--usage-mid)";
    default:
      return "var(--usage-low)";
  }
}

export function usageLabel(pct: number): string {
  switch (usageLevel(pct)) {
    case "hot":
      return "Critical";
    case "warn":
      return "High";
    case "mid":
      return "Normal";
    default:
      return "Low";
  }
}
