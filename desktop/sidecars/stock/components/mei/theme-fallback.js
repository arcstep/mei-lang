/**
 * Centralized theme fallbacks (builtin page preset). Consumer code must not embed hex/rgba.
 */
export const THEME_FALLBACK_COLORS = {
  text_primary: "#e2e8f0",
  text_muted: "#94a3b8",
  text_body: "#cbd5e1",
  text_unit: "#7dd3fc",
  text_inverse: "#f8fafc",
  text_accent: "#f8fafc",
  text_value: "#f0f9ff",
  text_highlight: "#e0f2fe",
  status_error: "#fca5a5",
  surface_bg: "rgba(2,6,23,.32)",
  border_default: "rgba(59,130,246,.18)",
  chart_1: "#d1fae5",
  chart_2: "#a7f3d0",
  chart_3: "#6ee7b7",
  chart_4: "#34d399",
  chart_5: "#10b981",
  chart_6: "#059669",
  warning_level_red: "#E53935",
  warning_level_yellow: "#FFB300",
  warning_level_blue: "#1E88E5",
  warning_level_grey: "#90A4AE",
  tone_blue: "#38bdf8",
  tone_cyan: "#67e8f9",
  tone_green: "#4ade80",
  tone_orange: "#fb923c",
  tone_red: "#f87171",
  tone_slate: "#cbd5e1",
  tone_violet: "#c4b5fd",
  tone_yellow: "#facc15",
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
