/**
 * Centralized theme fallbacks (builtin page preset). Consumer code must not embed hex/rgba.
 */
export const THEME_FALLBACK_COLORS = {
  text_primary: "#e2e8f0",
  text_muted: "#94a3b8",
  text_body: "#cbd5e1",
  text_inverse: "#f8fafc",
  text_accent: "#f8fafc",
  text_value: "#f0f9ff",
  text_highlight: "#e0f2fe",
  status_error: "#fca5a5",
  surface_bg: "rgba(2,6,23,.32)",
  border_default: "rgba(59,130,246,.18)",
  chart_2: "#38bdf8",
  chart_3: "#0ea5e9",
  chart_5: "#62beeb",
  tone_orange: "#fb923c",
};

export const THEME_FALLBACK_FONTS = {
  1: "12px",
  2: "14px",
  3: "16px",
  4: "20px",
  5: "24px",
};

export const THEME_FALLBACK_SHADOWS = {
  header_title: "0 20px 30px #0091ff, 0 0 4px #0d74c2",
  panel_title: "0 0 10px rgba(0, 145, 255, 0.55), 0 0 2px rgba(13, 116, 194, 0.9)",
};

export function fallbackColor(name) {
  const key = String(name ?? "").trim();
  return THEME_FALLBACK_COLORS[key] ?? "inherit";
}

export function fallbackFont(scale) {
  const key = String(scale ?? "").trim();
  return THEME_FALLBACK_FONTS[key] ?? "14px";
}

export function fallbackShadow(name) {
  const key = String(name ?? "").trim();
  return THEME_FALLBACK_SHADOWS[key] ?? "none";
}
