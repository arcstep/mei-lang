import { COCKPIT_TYPE } from "../cockpit/tokens.js";
import { fallbackColor, fallbackFont } from "./theme-fallback.js";

function tokenKey(name) {
  return String(name ?? "")
    .trim()
    .replace(/_/g, "-");
}

/** Semantic color token → CSS var (fallback from theme-fallback.js only). */
export function color(name) {
  const key = tokenKey(name);
  if (!key) {
    return "inherit";
  }
  return `var(--mei-color-${key}, ${fallbackColor(name)})`;
}

/** Named gradient token → CSS var. */
export function gradient(name) {
  const key = tokenKey(name);
  if (!key) {
    return "none";
  }
  return `var(--mei-gradient-${key})`;
}

/** Typography role → CSS var chain from COCKPIT_TYPE. */
export function font(role) {
  const key = String(role ?? "").trim();
  if (!key) {
    return `var(--mei-font-2, ${fallbackFont("2")})`;
  }
  if (COCKPIT_TYPE[key]) {
    return COCKPIT_TYPE[key];
  }
  if (/^\d+$/.test(key)) {
    return `var(--mei-font-${key}, ${fallbackFont(key)})`;
  }
  return `var(--mei-metric-${key}-font-size, var(--mei-font-2, ${fallbackFont("2")}))`;
}

export { color as themeColorStrict };
