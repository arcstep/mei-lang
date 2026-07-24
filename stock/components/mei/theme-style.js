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

/** Typography role → size CSS var (and optional weight/color via class `.mei-text-*`). */
export function font(role) {
  const key = String(role ?? "").trim();
  if (!key) {
    return `var(--mei-body-font-size, var(--mei-font-2, ${fallbackFont("2")}))`;
  }
  if (COCKPIT_TYPE[key]) {
    return COCKPIT_TYPE[key];
  }
  if (/^\d+$/.test(key)) {
    return `var(--mei-font-${key}, ${fallbackFont(key)})`;
  }
  const kebab = key.replace(/_/g, "-");
  return `var(--mei-${kebab}-font-size, var(--mei-font-2, ${fallbackFont("2")}))`;
}

/** Apply composed text-role appearance (size + color + weight + style). */
export function textRoleStyle(role) {
  const kebab = String(role ?? "")
    .trim()
    .replace(/_/g, "-");
  if (!kebab) {
    return font("body");
  }
  const prefix = kebab.startsWith("mei-") ? kebab : `mei-${kebab}`;
  return [
    `font-size: var(--${prefix}-font-size, inherit)`,
    `color: var(--${prefix}-color, inherit)`,
    `font-weight: var(--${prefix}-font-weight, var(--mei-typography-weight-regular, 400))`,
    `font-family: var(--${prefix}-font-family, var(--mei-typography-family, system-ui, sans-serif))`,
    `font-style: var(--${prefix}-font-style, normal)`,
  ].join("; ");
}

export { color as themeColorStrict };
