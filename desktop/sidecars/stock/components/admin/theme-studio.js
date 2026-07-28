/**
 * Admin theme studio: key colors + typography with local surface preview.
 * Platform-shared; apps mount via component("admin.theme-studio").
 *
 * NOTE: Manifest loads `runtime.js` (same as other admin bricks). The live
 * custom element is registered there — keep the ThemeStudio section in
 * `runtime.js` in sync when editing this file, or edit `runtime.js` directly.
 */

function parseProps(element) {
  const raw = element.getAttribute("data-props");
  if (!raw) return {};
  try {
    return JSON.parse(raw);
  } catch (_error) {
    return {};
  }
}

function providerRefId(value) {
  if (typeof value === "string") return value.trim();
  if (!value || typeof value !== "object") return "";
  if (value.__ref === "provider_ref" || value.__call === "provider_ref") {
    return String(value.__args?.arg0 || value.__args?.id || "").trim();
  }
  return value.kind === "provider_ref" ? String(value.id || "").trim() : "";
}

async function studioRequestJson(url, options = {}) {
  const response = await fetch(url, { credentials: "same-origin", ...options });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(payload.message || payload.error || `request failed: ${response.status}`);
  }
  return payload;
}

function studioAdminContext() {
  const match = window.location.pathname.match(/^\/admin\/apps\/([^/]+)\/([^/]+)\/([^/]+)\/?$/);
  if (!match) return null;
  return {
    appId: decodeURIComponent(match[1]),
    resourceId: decodeURIComponent(match[2]),
    moduleId: decodeURIComponent(match[3]),
  };
}

async function studioResolveProvider(reference) {
  if (typeof resolveProvider === "function") return resolveProvider(reference);
  const bindingId = providerRefId(reference);
  if (!bindingId) throw new Error("provider_ref is missing");
  const context = studioAdminContext();
  if (!context) throw new Error("Admin route context is unavailable");
  const catalog = await studioRequestJson(
    `/api/admin/resources?app_id=${encodeURIComponent(context.appId)}`,
  );
  const resource = (catalog.resources || []).find(
    (entry) =>
      entry.registryEntry?.resourceId === context.resourceId &&
      entry.registryEntry?.moduleId === context.moduleId,
  );
  if (!resource) throw new Error("Admin resource not found");
  const binding = (resource.pageProgram?.provider_bindings || []).find(
    (entry) => entry.bindingId === bindingId,
  );
  if (!binding) throw new Error(`ProviderBinding not found: ${bindingId}`);
  return {
    route: context,
    binding,
    context: {
      ...context,
      providerId: binding.providerId,
      method: binding.method,
      target: binding.target,
    },
  };
}

function studioProviderEndpoint(route, providerId) {
  return `/api/admin/apps/${encodeURIComponent(route.appId)}/${encodeURIComponent(
    route.resourceId,
  )}/${encodeURIComponent(route.moduleId)}/providers/${providerId}`;
}

async function studioReadProvider(reference) {
  if (typeof readProvider === "function") return readProvider(reference);
  const resolved = await studioResolveProvider(reference);
  const query = new URLSearchParams(resolved.context);
  return studioRequestJson(`${studioProviderEndpoint(resolved.route, resolved.binding.providerId)}?${query}`);
}

async function studioPutConfigRecord(reference, payload, revision) {
  if (typeof putConfigRecord === "function") return putConfigRecord(reference, payload, revision);
  const resolved = await studioResolveProvider(reference);
  return studioRequestJson(studioProviderEndpoint(resolved.route, resolved.binding.providerId), {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      ...resolved.context,
      revision,
      idempotencyKey: globalThis.crypto?.randomUUID?.() || `theme-studio-${Date.now()}`,
      payload,
    }),
  });
}

const COLOR_GROUPS = [
  {
    id: "stage",
    title: "舞台背景",
    keys: [
      ["color", "surface_bg", "表面底色"],
      ["color", "viewport_canvas", "视口画布"],
      ["gradient", "home_frame_bg", "大背景渐变"],
    ],
  },
  {
    id: "title",
    title: "标题 / 顶栏",
    keys: [
      ["gradient", "panel_title_bar", "板块标题条"],
      ["gradient", "header_band_bg", "大标题带"],
      ["color", "panel_title", "标题字色"],
    ],
  },
  {
    id: "panel",
    title: "板块面板",
    keys: [
      ["gradient", "panel_glow_bg", "面板辉光底"],
      ["color", "section_border", "板块描边"],
    ],
  },
  {
    id: "drilldown",
    title: "二级屏",
    keys: [
      ["color", "drilldown_backdrop", "半透明遮罩"],
      ["color", "drilldown_panel_top", "壳顶栏"],
      ["color", "drilldown_panel_bottom", "壳底色"],
      ["color", "drilldown_body_bg", "内容区"],
      ["color", "drilldown_tab_bg", "页签底"],
    ],
  },
  {
    id: "filter_table",
    title: "筛选 / 表格",
    keys: [
      ["color", "filter_panel_bg", "筛选面板"],
      ["color", "filter_panel_border", "筛选边框"],
      ["color", "table_head_bg", "表头底"],
      ["color", "table_row_hover", "行悬停"],
    ],
  },
  {
    id: "chart_text",
    title: "图表 / 文字",
    keys: [
      ["color", "chart_1", "图表色 1"],
      ["color", "chart_2", "图表色 2"],
      ["color", "chart_3", "图表色 3"],
      ["color", "chart_4", "图表色 4"],
      ["color", "chart_5", "图表色 5"],
      ["color", "chart_6", "图表色 6"],
      ["color", "text_primary", "主文字"],
      ["color", "text_body", "正文"],
      ["color", "text_muted", "次要文字"],
      ["color", "text_value", "数值字色"],
    ],
  },
];

const FONT_STEPS = ["1", "2", "3", "4", "5", "6", "7"];

/** ThemeDecl text-role recipes (Host emits --mei-{role}-* + .mei-text-*). */
const TEXT_ROLES = [
  { key: "header_title", label: "顶栏大标题", prefix: "mei-header-title" },
  { key: "panel_head", label: "面板标题", prefix: "mei-panel-head" },
  { key: "body", label: "正文", prefix: "mei-body" },
  { key: "muted", label: "次要说明", prefix: "mei-muted" },
  { key: "metric_label", label: "指标标签", prefix: "mei-metric-label" },
  { key: "metric_value", label: "指标数值", prefix: "mei-metric-value" },
  { key: "metric_unit", label: "指标单位", prefix: "mei-metric-unit" },
  { key: "metric_desc", label: "指标说明", prefix: "mei-metric-desc" },
  { key: "metric_sub_label", label: "子指标标签", prefix: "mei-metric-sub-label" },
  { key: "metric_sub_value", label: "子指标数值", prefix: "mei-metric-sub-value" },
  { key: "metric_sub_unit", label: "子指标单位", prefix: "mei-metric-sub-unit" },
  { key: "chart_title", label: "图表标题", prefix: "mei-chart-title" },
  { key: "chart_label", label: "图表标签", prefix: "mei-chart-label" },
  { key: "table_head", label: "表头", prefix: "mei-table-head" },
  { key: "table_body", label: "表体", prefix: "mei-table-body" },
  { key: "filter_panel", label: "筛选面板", prefix: "mei-filter-panel" },
];

const FONT_WEIGHT_OPTIONS = [
  { value: "regular", label: "常规" },
  { value: "medium", label: "中等" },
  { value: "bold", label: "加粗" },
];

const TEXT_ROLE_DEFAULTS = {
  header_title: { font: "5", color: "text_primary", font_weight: "bold" },
  panel_head: { font: "4", color: "panel_title", font_weight: "medium" },
  body: { font: "2", color: "text_body", font_weight: "regular" },
  muted: { font: "1", color: "text_muted", font_weight: "regular" },
  metric_label: { font: "2", color: "text_muted", font_weight: "regular" },
  metric_value: { font: "4", color: "text_value", font_weight: "bold" },
  metric_unit: { font: "1", color: "text_unit", font_weight: "regular" },
  metric_desc: { font: "1", color: "text_muted", font_weight: "regular" },
  metric_sub_label: { font: "1", color: "text_muted", font_weight: "regular" },
  metric_sub_value: { font: "3", color: "text_value", font_weight: "bold" },
  metric_sub_unit: { font: "1", color: "text_unit", font_weight: "regular" },
  chart_title: { font: "2", color: "text_primary", font_weight: "medium" },
  chart_label: { font: "1", color: "text_muted", font_weight: "regular" },
  table_head: { font: "2", color: "text_primary", font_weight: "medium" },
  table_body: { font: "2", color: "text_body", font_weight: "regular" },
  filter_panel: { font: "2", color: "text_body", font_weight: "regular" },
};

/** Gradients edited as structured glow-bar / dual-stop recipes. */
const STRUCTURED_GRADIENT_KEYS = new Set(["panel_title_bar", "header_band_bg", "panel_glow_bg", "home_frame_bg"]);

const THEME_STUDIO_ACTIVE_KEY = (appId) => `mei-theme-studio-active:${appId}`;

function deepClone(value) {
  return JSON.parse(JSON.stringify(value ?? {}));
}

function escapeAttr(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;");
}

function tokenCssName(kind, key) {
  const slug = String(key).replace(/_/g, "-");
  return kind === "gradient" ? `--mei-gradient-${slug}` : `--mei-color-${slug}`;
}

function readToken(theme, kind, key) {
  const bag = theme?.tokens?.[kind] || {};
  return bag[key] ?? "";
}

function writeToken(theme, kind, key, value) {
  if (!theme.tokens) theme.tokens = {};
  if (!theme.tokens[kind]) theme.tokens[kind] = {};
  theme.tokens[kind][key] = value;
}

function ensureFont(theme) {
  if (!theme.font || typeof theme.font !== "object") theme.font = {};
  for (const step of FONT_STEPS) {
    if (!theme.font[step]) theme.font[step] = step === "1" ? "16px" : `${14 + Number(step) * 2}px`;
  }
  if (!theme.tokens) theme.tokens = {};
  if (!theme.tokens.typography) {
    theme.tokens.typography = {
      family: 'system-ui, "PingFang SC", "Microsoft YaHei", sans-serif',
      weight_regular: "400",
      weight_medium: "500",
      weight_bold: "700",
    };
  }
  for (const role of TEXT_ROLES) {
    const defaults = TEXT_ROLE_DEFAULTS[role.key] || {};
    if (!theme[role.key] || typeof theme[role.key] !== "object") {
      theme[role.key] = { ...defaults };
    } else {
      for (const [k, v] of Object.entries(defaults)) {
        if (theme[role.key][k] == null || theme[role.key][k] === "") {
          theme[role.key][k] = v;
        }
      }
    }
  }
}

function resolveRoleColorValue(raw) {
  const value = String(raw || "").trim();
  if (!value) return "";
  if (
    value.startsWith("#") ||
    value.startsWith("rgb") ||
    value.startsWith("hsl") ||
    value.startsWith("var(")
  ) {
    return value;
  }
  return `var(--mei-color-${value.replace(/_/g, "-")})`;
}

function resolveRoleWeightValue(raw) {
  const value = String(raw || "").trim().toLowerCase();
  if (value === "regular" || value === "normal") {
    return "var(--mei-typography-weight-regular, 400)";
  }
  if (value === "medium") return "var(--mei-typography-weight-medium, 500)";
  if (value === "bold") return "var(--mei-typography-weight-bold, 700)";
  return value;
}

function applyTextRoleVars(root, theme) {
  for (const role of TEXT_ROLES) {
    const entry = theme?.[role.key];
    if (!entry || typeof entry !== "object") continue;
    const font = String(entry.font || "").trim();
    if (font) {
      root.style.setProperty(
        `--${role.prefix}-font-size`,
        /^\d+(\.\d+)?(px|rem|em|%)$/.test(font) ? font : `var(--mei-font-${font})`,
      );
    }
    const color = resolveRoleColorValue(entry.color);
    if (color) root.style.setProperty(`--${role.prefix}-color`, color);
    const weight = resolveRoleWeightValue(entry.font_weight);
    if (weight) root.style.setProperty(`--${role.prefix}-font-weight`, weight);
    const family = String(entry.font_family || "").trim();
    if (family) root.style.setProperty(`--${role.prefix}-font-family`, family);
    const style = String(entry.font_style || "").trim();
    if (style) root.style.setProperty(`--${role.prefix}-font-style`, style);
  }
}

function applyPreviewVars(root, theme) {
  if (!(root instanceof HTMLElement)) return;
  const colors = theme?.tokens?.color || {};
  const gradients = theme?.tokens?.gradient || {};
  Object.entries(colors).forEach(([key, value]) => {
    if (typeof value === "string" && value.trim()) {
      root.style.setProperty(tokenCssName("color", key), value);
    }
  });
  Object.entries(gradients).forEach(([key, value]) => {
    if (typeof value === "string" && value.trim()) {
      root.style.setProperty(tokenCssName("gradient", key), value);
    }
  });
  const font = theme?.font || {};
  Object.entries(font).forEach(([key, value]) => {
    if (typeof value === "string" && value.trim()) {
      root.style.setProperty(`--mei-font-${key}`, value);
    }
  });
  const typography = theme?.tokens?.typography || {};
  if (typography.family) root.style.setProperty("--mei-typography-family", typography.family);
  if (typography.weight_regular) {
    root.style.setProperty("--mei-typography-weight-regular", typography.weight_regular);
  }
  if (typography.weight_medium) {
    root.style.setProperty("--mei-typography-weight-medium", typography.weight_medium);
  }
  if (typography.weight_bold) {
    root.style.setProperty("--mei-typography-weight-bold", typography.weight_bold);
  }
  applyTextRoleVars(root, theme);
  root.style.fontFamily = `var(--mei-typography-family, system-ui, sans-serif)`;
}

function rememberActiveThemeId(appId, themeId) {
  const id = String(themeId || "").trim();
  if (!appId || !id || id.length > 64 || /[{}\[\]]/.test(id)) return;
  const key = THEME_STUDIO_ACTIVE_KEY(appId);
  try {
    // Purge any legacy oversized payload that once used this key.
    for (const store of [window.sessionStorage, window.localStorage]) {
      const existing = store.getItem(key);
      if (existing && existing.length > 64) store.removeItem(key);
    }
    sessionStorage.setItem(key, id);
  } catch {
    try {
      sessionStorage.removeItem(key);
      localStorage.removeItem(key);
    } catch {
      /* ignore quota / private mode */
    }
  }
}

function readRememberedThemeId(appId) {
  if (!appId) return "";
  const key = THEME_STUDIO_ACTIVE_KEY(appId);
  for (const store of [window.sessionStorage, window.localStorage]) {
    try {
      const stored = store.getItem(key);
      if (!stored) continue;
      if (stored.length > 64 || /[{}\[\]]/.test(stored)) {
        store.removeItem(key);
        continue;
      }
      return stored.trim();
    } catch {
      /* ignore */
    }
  }
  return "";
}

function clamp01(n) {
  const x = Number(n);
  if (!Number.isFinite(x)) return 0;
  return Math.min(1, Math.max(0, x));
}

function toHex2(n) {
  return Math.round(Math.min(255, Math.max(0, Number(n) || 0)))
    .toString(16)
    .padStart(2, "0");
}

function rgbaCss({ r, g, b, a }) {
  const alpha = Math.round(clamp01(a) * 1000) / 1000;
  return `rgba(${Math.round(r)}, ${Math.round(g)}, ${Math.round(b)}, ${alpha})`;
}

function colorToHex({ r, g, b }) {
  return `#${toHex2(r)}${toHex2(g)}${toHex2(b)}`;
}

function parseCssColor(raw) {
  const text = String(raw || "").trim();
  if (!text || text === "transparent") {
    return { r: 0, g: 0, b: 0, a: 0, hex: "#000000" };
  }
  const hex6 = text.match(/^#([0-9a-f]{6})$/i);
  if (hex6) {
    return {
      r: parseInt(hex6[1].slice(0, 2), 16),
      g: parseInt(hex6[1].slice(2, 4), 16),
      b: parseInt(hex6[1].slice(4, 6), 16),
      a: 1,
      hex: `#${hex6[1].toLowerCase()}`,
    };
  }
  const hex3 = text.match(/^#([0-9a-f]{3})$/i);
  if (hex3) {
    const r = hex3[1][0];
    const g = hex3[1][1];
    const b = hex3[1][2];
    return parseCssColor(`#${r}${r}${g}${g}${b}${b}`);
  }
  const rgba = text.match(
    /rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)(?:\s*,\s*([\d.]+))?\s*\)/i,
  );
  if (rgba) {
    const r = Number(rgba[1]);
    const g = Number(rgba[2]);
    const b = Number(rgba[3]);
    const a = rgba[4] == null ? 1 : Number(rgba[4]);
    return { r, g, b, a: clamp01(a), hex: colorToHex({ r, g, b }) };
  }
  return { r: 0, g: 33, b: 104, a: 1, hex: "#002168" };
}

function approxHex(cssColor) {
  return parseCssColor(cssColor).hex;
}

function splitCssLayers(css) {
  const input = String(css || "").trim();
  if (!input) return [];
  const layers = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < input.length; i += 1) {
    const ch = input[i];
    if (ch === "(") depth += 1;
    else if (ch === ")") depth = Math.max(0, depth - 1);
    else if (ch === "," && depth === 0) {
      layers.push(input.slice(start, i).trim());
      start = i + 1;
    }
  }
  layers.push(input.slice(start).trim());
  return layers.filter(Boolean);
}

function extractRgbaStops(layer) {
  const stops = [];
  const re =
    /(rgba?\(\s*[\d.]+\s*,\s*[\d.]+\s*,\s*[\d.]+(?:\s*,\s*[\d.]+)?\s*\)|#[0-9a-fA-F]{3,8}|transparent)(?:\s+([\d.]+%)?)?/gi;
  let match;
  while ((match = re.exec(layer))) {
    stops.push({
      color: parseCssColor(match[1]),
      pos: match[2] || "",
    });
  }
  return stops;
}

function sameRgb(a, b, tol = 2) {
  return (
    Math.abs(a.r - b.r) <= tol && Math.abs(a.g - b.g) <= tol && Math.abs(a.b - b.b) <= tol
  );
}

/**
 * Title-bar style: glow stripe (same RGB, alpha peak) + optional base fill.
 * Example: linear-gradient(90deg, rgba(C,0), rgba(C,.28), rgba(C,0)), linear-gradient(180deg, top, bottom)
 */
function parseGlowBarGradient(css) {
  const layers = splitCssLayers(css);
  if (!layers.length) return null;
  const glowLayer =
    layers.find((layer) => /linear-gradient\(\s*90deg/i.test(layer)) || layers[0];
  const glowStops = extractRgbaStops(glowLayer);
  if (glowStops.length < 2) return null;
  const mid =
    glowStops.find((s) => s.pos.includes("50")) ||
    glowStops.reduce((best, s) => (s.color.a > (best?.color.a || -1) ? s : best), null);
  if (!mid) return null;
  const ends = glowStops.filter((s) => s !== mid);
  const looksLikeGlow =
    ends.length >= 1 &&
    ends.every((s) => sameRgb(s.color, mid.color) && s.color.a <= mid.color.a + 0.05);
  if (!looksLikeGlow && glowStops.length !== 3) {
    // still accept 3-stop same-hue bars even if ends aren't near-zero
    if (!(glowStops.length === 3 && glowStops.every((s) => sameRgb(s.color, mid.color)))) {
      return null;
    }
  }
  const baseLayer =
    layers.find((layer) => layer !== glowLayer && /linear-gradient\(\s*180deg/i.test(layer)) ||
    layers.find((layer) => layer !== glowLayer);
  const baseStops = baseLayer ? extractRgbaStops(baseLayer) : [];
  const baseTop = baseStops[0]?.color || { r: 15, g: 36, b: 56, a: 0.55, hex: "#0f2438" };
  const baseBottom =
    baseStops[baseStops.length - 1]?.color || { r: 8, g: 28, b: 52, a: 0.2, hex: "#081c34" };
  return {
    kind: "glow-bar",
    glow: { r: mid.color.r, g: mid.color.g, b: mid.color.b, hex: mid.color.hex },
    peak: clamp01(mid.color.a),
    baseTop,
    baseBottom,
  };
}

function buildGlowBarGradient(model) {
  const glow = model.glow;
  const peak = clamp01(model.peak);
  const top = model.baseTop;
  const bottom = model.baseBottom;
  return [
    `linear-gradient(90deg, ${rgbaCss({ ...glow, a: 0 })} 0%, ${rgbaCss({
      ...glow,
      a: peak,
    })} 50%, ${rgbaCss({ ...glow, a: 0 })} 100%)`,
    `linear-gradient(180deg, ${rgbaCss(top)} 0%, ${rgbaCss(bottom)} 100%)`,
  ].join(", ");
}

/** Soft halo + accent fade + solid/base fill (header_band / panel_glow style). */
function parseHaloBandGradient(css) {
  const layers = splitCssLayers(css);
  if (layers.length < 2) return null;
  const colors = [];
  for (const layer of layers) {
    const stops = extractRgbaStops(layer);
    if (!stops.length) continue;
    const peak = stops.reduce((best, s) => (s.color.a > (best?.color.a || -1) ? s : best), null);
    if (peak) colors.push(peak.color);
  }
  if (colors.length < 2) return null;
  while (colors.length < 3) colors.push(colors[colors.length - 1]);
  return {
    kind: "halo-band",
    halo: colors[0],
    accent: colors[1],
    base: colors[2],
  };
}

function buildHaloBandGradient(model) {
  const halo = model.halo;
  const accent = model.accent;
  const base = model.base;
  return [
    `radial-gradient(ellipse at 50% 0%, ${rgbaCss(halo)}, transparent 55%)`,
    `linear-gradient(0deg, ${rgbaCss({ ...accent, a: clamp01(accent.a) })} 0%, ${rgbaCss({
      ...accent,
      a: 0,
    })} 50%)`,
    rgbaCss(base),
  ].join(", ");
}

/** Two-stop linear / radial (home_frame or simple panel glow). */
function parseDualStopGradient(css) {
  const layers = splitCssLayers(css);
  if (layers.length !== 1) return null;
  const layer = layers[0];
  if (/^url\(/i.test(layer)) return { kind: "raw", css: String(css || "") };
  const stops = extractRgbaStops(layer);
  if (stops.length < 2) return null;
  const isRadial = /radial-gradient/i.test(layer);
  return {
    kind: isRadial ? "radial-dual" : "linear-dual",
    start: stops[0].color,
    end: stops[stops.length - 1].color,
  };
}

function buildDualStopGradient(model) {
  if (model.kind === "radial-dual") {
    return `radial-gradient(circle at 50% 50%, ${rgbaCss(model.start)} 0%, ${rgbaCss(
      model.end,
    )} 50%)`;
  }
  return `linear-gradient(180deg, ${rgbaCss(model.start)} 0%, ${rgbaCss(model.end)} 100%)`;
}

function parseStructuredGradient(key, css) {
  const raw = String(css || "").trim();
  if (!raw) return { kind: "raw", css: "" };
  if (key === "panel_title_bar") {
    return parseGlowBarGradient(raw) || { kind: "raw", css: raw };
  }
  if (key === "header_band_bg") {
    return (
      parseHaloBandGradient(raw) ||
      parseGlowBarGradient(raw) ||
      parseDualStopGradient(raw) || { kind: "raw", css: raw }
    );
  }
  if (key === "panel_glow_bg") {
    return (
      parseDualStopGradient(raw) ||
      parseHaloBandGradient(raw) ||
      parseGlowBarGradient(raw) || { kind: "raw", css: raw }
    );
  }
  if (key === "home_frame_bg") {
    return parseDualStopGradient(raw) || { kind: "raw", css: raw };
  }
  return { kind: "raw", css: raw };
}

function buildStructuredGradient(model) {
  if (!model || model.kind === "raw") return model?.css || "";
  if (model.kind === "glow-bar") return buildGlowBarGradient(model);
  if (model.kind === "halo-band") return buildHaloBandGradient(model);
  if (model.kind === "linear-dual" || model.kind === "radial-dual") {
    return buildDualStopGradient(model);
  }
  return model.css || "";
}

function studioStyles() {
  const roleUtilityCss = TEXT_ROLES.map((role) => {
    const cls = `.mei-text-${role.prefix.replace(/^mei-/, "").replace(/_/g, "-")}`;
    return `
    ${cls} {
      font-size: var(--${role.prefix}-font-size, inherit);
      color: var(--${role.prefix}-color, inherit);
      font-weight: var(--${role.prefix}-font-weight, var(--mei-typography-weight-regular, 400));
      font-family: var(--${role.prefix}-font-family, var(--mei-typography-family, system-ui, sans-serif));
      font-style: var(--${role.prefix}-font-style, normal);
    }`;
  }).join("");
  return `
    :host {
      display: block;
      color: var(--mei-color-text-body, #e2e8f0);
      font-size: var(--mei-font-1, 16px);
      font-family: var(--mei-typography-family, system-ui, "PingFang SC", "Microsoft YaHei", sans-serif);
      font-weight: var(--mei-typography-weight-regular, 400);
      line-height: 1.45;
    }${roleUtilityCss}
    .studio {
      display: grid;
      gap: 14px;
      max-width: 1280px;
      margin: 0 auto;
      padding: 0 28px 24px;
      box-sizing: border-box;
    }
    .toolbar {
      display: flex; flex-wrap: wrap; gap: 10px; align-items: center;
      padding: 12px 14px; border-radius: 0;
      background: rgba(15, 23, 42, 0.72);
      border: 1px solid var(--mei-color-section-border, rgba(56, 160, 240, 0.28));
    }
    .toolbar label {
      display: grid; gap: 4px;
      font-size: calc(var(--mei-font-1, 16px) * 0.85);
      color: var(--mei-color-text-muted, #94a3b8);
    }
    .toolbar select, .toolbar input, .toolbar button {
      min-height: 32px; border-radius: 0;
      border: 1px solid rgba(56, 160, 240, 0.35);
      background: rgba(8, 28, 58, 0.92);
      color: var(--mei-color-text-body, #e2e8f0);
      padding: 0 10px;
      font-size: var(--mei-font-1, 16px);
      font-family: inherit;
    }
    .toolbar button.primary {
      background: rgba(14, 116, 178, 0.45); border-color: rgba(56, 189, 248, 0.55); cursor: pointer;
    }
    .toolbar button:disabled { opacity: 0.55; cursor: not-allowed; }
    .swatch-row { display: inline-flex; gap: 4px; margin-left: 8px; vertical-align: middle; }
    .swatch-dot {
      width: 14px; height: 14px; border-radius: 999px;
      border: 1px solid rgba(255,255,255,0.35); display: inline-block;
    }
    .status {
      color: var(--mei-color-chart-2, #7dd3fc);
      font-size: calc(var(--mei-font-1, 16px) * 0.85);
      min-height: 1.2em;
    }
    .status.error { color: #fca5a5; }
    .sections { display: grid; gap: 14px; }
    .section {
      border-radius: 0; padding: 14px;
      background: rgba(10, 28, 52, 0.88);
      border: 1px solid var(--mei-color-section-border, rgba(56, 160, 240, 0.22));
      box-shadow: inset 0 1px 0 rgba(125, 211, 252, 0.08);
    }
    .section h3 {
      margin: 0 0 12px;
      font-size: var(--mei-font-2, 18px);
      font-weight: var(--mei-typography-weight-bold, 700);
      color: var(--mei-color-text-primary, #f8fafc);
    }
    .section-split {
      display: grid;
      grid-template-columns: minmax(280px, 1.15fr) minmax(240px, 0.85fr);
      gap: 16px;
      align-items: start;
    }
    @media (max-width: 960px) {
      .section-split { grid-template-columns: 1fr; }
    }
    .section-config, .section-preview { min-width: 0; }
    .field {
      display: grid; grid-template-columns: 110px minmax(0, 1fr) 36px;
      gap: 8px; align-items: center; margin-bottom: 6px;
    }
    .field label { color: var(--mei-color-text-body, #cbd5e1); font-size: calc(var(--mei-font-1, 16px) * 0.85); }
    .field input[type="text"], .field input[type="range"], .field textarea {
      width: 100%; min-height: 30px; box-sizing: border-box;
      border-radius: 0; border: 1px solid rgba(56, 160, 240, 0.28);
      background: rgba(2, 12, 32, 0.65);
      color: var(--mei-color-text-body, #e2e8f0);
      padding: 0 8px;
      font-size: calc(var(--mei-font-1, 16px) * 0.9);
      font-family: inherit;
    }
    .field input[type="color"] {
      width: 36px; height: 30px; padding: 0; border: 0; background: transparent; cursor: pointer;
    }
    .field.gradient-field {
      grid-template-columns: 110px minmax(0, 1fr);
      align-items: start;
    }
    .grad-editor {
      display: grid; gap: 8px; min-width: 0;
      padding: 10px; border-radius: 0;
      background: rgba(2, 12, 32, 0.45);
      border: 1px solid rgba(56, 160, 240, 0.18);
    }
    .grad-preview {
      height: 36px; border-radius: 0;
      border: 1px solid rgba(125, 211, 252, 0.28);
      background-size: cover;
    }
    .grad-row {
      display: grid; grid-template-columns: 72px minmax(0, 1fr) 36px 64px;
      gap: 8px; align-items: center;
    }
    .grad-row label {
      color: var(--mei-color-text-muted, #94a3b8);
      font-size: calc(var(--mei-font-1, 16px) * 0.8);
    }
    .grad-row input[type="range"] { padding: 0; }
    .grad-row .alpha {
      font-variant-numeric: tabular-nums;
      color: var(--mei-color-text-muted, #94a3b8);
      font-size: calc(var(--mei-font-1, 16px) * 0.8);
      text-align: right;
    }
    .grad-advanced summary {
      cursor: pointer;
      color: var(--mei-color-text-muted, #94a3b8);
      font-size: calc(var(--mei-font-1, 16px) * 0.8);
    }
    .grad-advanced textarea {
      margin-top: 6px; min-height: 64px; resize: vertical;
      font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      font-size: 12px; line-height: 1.35; padding: 8px;
    }
    .preview-block {
      border-radius: 0; overflow: hidden;
      border: 1px solid rgba(125, 211, 252, 0.28);
      background: var(--mei-color-viewport-canvas, #002168);
    }
    .preview-stage {
      min-height: 96px;
      background-image: var(--mei-gradient-home-frame-bg, radial-gradient(circle, #2273e7, #002168));
      background-color: var(--mei-color-surface-bg, #002168);
      display: grid; place-items: center;
      color: var(--mei-color-text-primary, #fff);
      font-size: var(--mei-font-4, 32px);
      font-weight: var(--mei-typography-weight-bold, 700);
    }
    .preview-title-bar {
      margin: 10px; padding: 8px 12px; border-radius: 0;
      background-image: var(--mei-gradient-panel-title-bar, linear-gradient(90deg, transparent, rgba(0,217,255,.5), transparent));
      color: var(--mei-color-panel-title, rgba(255,255,255,.8));
      font-size: var(--mei-font-2, 18px);
      font-weight: var(--mei-typography-weight-medium, 500);
      text-align: center;
    }
    .preview-panel {
      margin: 10px; padding: 12px; border-radius: 0;
      border: 1px solid var(--mei-color-section-border, rgba(98,190,235,.55));
      background-image: var(--mei-gradient-panel-glow-bg, linear-gradient(#002977, #002168));
      color: var(--mei-color-text-body, #c9e9f8);
      font-size: var(--mei-font-1, 16px);
    }
    .preview-header-band {
      margin: 10px; padding: 14px 12px; border-radius: 0;
      background-image: var(--mei-gradient-header-band-bg, linear-gradient(#094194, #002977));
      color: var(--mei-color-text-primary, #fff);
      font-size: var(--mei-font-3, 26px);
      font-weight: var(--mei-typography-weight-bold, 700);
      text-align: center;
    }
    .preview-drill {
      margin: 10px; border-radius: 0; overflow: hidden;
      border: 1px solid rgba(0,217,255,.28);
      background: linear-gradient(
        180deg,
        var(--mei-color-drilldown-panel-top, rgba(9,65,148,.98)),
        var(--mei-color-drilldown-panel-bottom, rgba(0,41,119,.98))
      );
    }
    .preview-drill-mask {
      padding: 8px; background: var(--mei-color-drilldown-backdrop, rgba(0,33,104,.58));
    }
    .preview-drill-chrome {
      padding: 8px 10px; font-size: calc(var(--mei-font-1, 16px) * 0.85);
      background: var(--mei-color-drilldown-tab-bg, #094194);
      color: var(--mei-color-text-body, #e2e8f0);
    }
    .preview-drill-body {
      padding: 10px; min-height: 40px;
      background: var(--mei-color-drilldown-body-bg, rgba(0,41,119,.92));
      color: var(--mei-color-text-muted, #94a3b8);
      font-size: calc(var(--mei-font-1, 16px) * 0.85);
    }
    .preview-filter {
      margin: 10px; padding: 10px; border-radius: 0;
      background: var(--mei-color-filter-panel-bg, rgba(0,41,119,.88));
      border: 1px solid var(--mei-color-filter-panel-border, rgba(98,190,235,.22));
    }
    .preview-filter-control {
      display: inline-block; min-width: 120px; padding: 6px 10px; border-radius: 0;
      background: var(--mei-color-drilldown-tab-bg, #094194);
      border: 1px solid var(--mei-color-drilldown-tab-border, rgba(113,241,234,.3));
      color: var(--mei-color-text-body, #e2e8f0);
      font-size: var(--mei-font-1, 16px);
    }
    .preview-table {
      margin: 10px; border-radius: 0; overflow: hidden;
      border: 1px solid rgba(0,217,255,.2);
    }
    .preview-table .head {
      padding: 8px 10px;
      background: var(--mei-color-table-head-bg, rgba(0,41,119,.92));
      color: var(--mei-color-text-primary, #fff);
      font-size: var(--mei-font-2, 18px);
    }
    .preview-table .row {
      padding: 8px 10px;
      color: var(--mei-color-text-body, #c9e9f8);
      font-size: var(--mei-font-1, 16px);
      border-top: 1px solid rgba(201,233,248,.12);
    }
    .preview-table .row:hover {
      background: var(--mei-color-table-row-hover, rgba(98,190,235,.18));
    }
    .preview-charts { display: flex; gap: 6px; padding: 10px; }
    .preview-charts span { flex: 1; height: 28px; border-radius: 0; }
    .preview-type {
      padding: 10px; display: grid; gap: 6px;
      color: var(--mei-color-text-body, #e2e8f0);
    }
    .preview-type .step {
      display: flex; justify-content: space-between; gap: 8px; align-items: baseline;
      border-bottom: 1px dashed rgba(148,163,184,.25); padding-bottom: 4px;
    }
    .preview-type .muted { color: var(--mei-color-text-muted, #94a3b8); }
    .preview-type .value {
      color: var(--mei-color-text-value, #71f1ea);
      font-weight: var(--mei-typography-weight-bold, 700);
    }
    .preview-text-stack {
      padding: 10px; display: grid; gap: 8px;
      color: var(--mei-color-text-body, #c9e9f8);
    }
    .mei-text-header-title {
      font-size: var(--mei-header-title-font-size, var(--mei-font-5, 32px));
      color: var(--mei-header-title-color, var(--mei-color-text-primary, #fff));
      font-weight: var(--mei-header-title-font-weight, var(--mei-typography-weight-bold, 700));
      font-style: var(--mei-header-title-font-style, normal);
    }
    .mei-text-panel-head {
      font-size: var(--mei-panel-head-font-size, var(--mei-font-4, 24px));
      color: var(--mei-panel-head-color, var(--mei-color-panel-title, #ecfeff));
      font-weight: var(--mei-panel-head-font-weight, var(--mei-typography-weight-medium, 500));
      font-style: var(--mei-panel-head-font-style, normal);
    }
    .mei-text-body {
      font-size: var(--mei-body-font-size, var(--mei-font-2, 14px));
      color: var(--mei-body-color, var(--mei-color-text-body, #c9e9f8));
      font-weight: var(--mei-body-font-weight, var(--mei-typography-weight-regular, 400));
      font-style: var(--mei-body-font-style, normal);
    }
    .mei-text-muted {
      font-size: var(--mei-muted-font-size, var(--mei-font-1, 12px));
      color: var(--mei-muted-color, var(--mei-color-text-muted, #94a3b8));
      font-weight: var(--mei-muted-font-weight, var(--mei-typography-weight-regular, 400));
      font-style: var(--mei-muted-font-style, normal);
    }
    .mei-text-metric-value {
      font-size: var(--mei-metric-value-font-size, var(--mei-font-4, 24px));
      color: var(--mei-metric-value-color, var(--mei-color-text-value, #71f1ea));
      font-weight: var(--mei-metric-value-font-weight, var(--mei-typography-weight-bold, 700));
      font-style: var(--mei-metric-value-font-style, normal);
    }
    .role-grid { display: grid; gap: 10px; margin-bottom: 12px; }
    .role-card {
      padding: 10px; border: 1px solid rgba(56, 160, 240, 0.18);
      background: rgba(2, 12, 32, 0.35);
    }
    .role-card-head {
      display: flex; justify-content: space-between; gap: 8px; align-items: baseline;
      margin-bottom: 8px;
    }
    .role-card-head code {
      color: var(--mei-color-text-muted, #94a3b8);
      font-size: 11px;
    }
    .role-sample {
      margin-top: 6px; padding: 8px 10px;
      border: 1px dashed rgba(125, 211, 252, 0.25);
      background: rgba(0, 20, 48, 0.45);
    }
    .field select {
      width: 100%; min-height: 30px; box-sizing: border-box;
      border-radius: 0; border: 1px solid rgba(56, 160, 240, 0.28);
      background: rgba(2, 12, 32, 0.65);
      color: var(--mei-color-text-body, #e2e8f0);
      padding: 0 8px;
      font-size: calc(var(--mei-font-1, 16px) * 0.9);
      font-family: inherit;
    }
    .font-advanced {
      margin-top: 8px; padding-top: 8px;
      border-top: 1px solid rgba(56, 160, 240, 0.16);
    }
    .font-advanced summary {
      cursor: pointer;
      color: var(--mei-color-text-muted, #94a3b8);
      font-size: calc(var(--mei-font-1, 16px) * 0.85);
      margin-bottom: 8px;
    }
  `;
}

class ThemeStudio extends HTMLElement {
  static observedAttributes = ["data-props"];

  connectedCallback() {
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.syncPropsFromDom();
    if (this._bootStarted) return;
    this._bootStarted = true;
    this._themes = [];
    this._themeId = "";
    this._draft = null;
    this._dirty = false;
    this._status = "";
    this._error = "";
    this._busy = false;
    this.renderSkeleton();
    this.bootstrap();
  }

  attributeChangedCallback(name, oldValue, newValue) {
    if (name !== "data-props" || oldValue === newValue) return;
    // Thin-shell mounts the element before data-props arrives.
    this.syncPropsFromDom();
  }

  syncPropsFromDom() {
    this._props = typeof parseProps === "function" ? parseProps(this) : {};
    return this._props || {};
  }

  currentAdminAppId() {
    this.syncPropsFromDom();
    const match = String(location.pathname || "").match(/\/admin\/apps\/([^/]+)/);
    return match?.[1] || this._props.app_id || this._props.appId || "";
  }

  renderSkeleton() {
    this.shadowRoot.innerHTML = `<style>${studioStyles()}</style><div class="studio"><p class="status">加载主题库…</p></div>`;
  }

  async bootstrap() {
    try {
      const catalogRes = await fetch("/api/ops/scene-themes", { credentials: "same-origin" });
      if (!catalogRes.ok) throw new Error(`catalog HTTP ${catalogRes.status}`);
      const catalog = await catalogRes.json();
      this._themes = Array.isArray(catalog.themes) ? catalog.themes : [];
      let active = catalog.default || this._themes[0]?.id || "cockpit";
      const urlTheme = new URLSearchParams(location.search).get("theme");
      if (urlTheme) active = urlTheme;
      active = await this.resolveActiveThemeId(active);
      await this.loadTheme(active);
    } catch (error) {
      this._error = error?.message || String(error);
      this.render();
    }
  }

  async resolveActiveThemeId(fallback) {
    this.syncPropsFromDom();
    const getRef = this._props.selection_get || this._props.selectionGet;
    if (getRef) {
      try {
        const payload = await studioReadProvider(getRef);
        const active = String(payload?.payload?.active || payload?.active || "").trim();
        if (active) return active;
      } catch {
        /* fall through */
      }
    }
    const appId = this.currentAdminAppId();
    const stored = readRememberedThemeId(appId);
    if (stored && this._themes.some((t) => t.id === stored)) return stored;
    return this._themes.find((t) => t.id === fallback)?.id || this._themes[0]?.id || fallback;
  }

  async loadTheme(themeId) {
    this._busy = true;
    this._status = `加载 ${themeId}…`;
    this._error = "";
    this.render();
    try {
      const res = await fetch(`/api/ops/scene-themes/${encodeURIComponent(themeId)}`, {
        credentials: "same-origin",
      });
      if (!res.ok) throw new Error(`theme HTTP ${res.status}`);
      const payload = await res.json();
      this._themeId = String(payload.id || themeId).trim();
      this._draft = deepClone(payload.theme || {});
      ensureFont(this._draft);
      this._dirty = false;
      this._status = `已加载「${this._draft.label || this._themeId}」`;
      rememberActiveThemeId(this.currentAdminAppId(), this._themeId);
      await this.refreshLiveCss();
    } catch (error) {
      this._error = error?.message || String(error);
    } finally {
      this._busy = false;
      this.render();
    }
  }

  onThemePick(themeId) {
    if (this._dirty && !confirm("有未保存修改，切换主题将丢弃本地草稿。继续？")) return;
    this.loadTheme(themeId);
  }

  onColorChange(kind, key, value) {
    if (!this._draft) return;
    writeToken(this._draft, kind, key, value);
    this._dirty = true;
    this._status = "已修改（未保存）";
    this.applyStudioThemeVars();
    this.renderPreviewOnly();
    this.updateDirtyChrome();
  }

  onFontChange(step, value) {
    if (!this._draft) return;
    ensureFont(this._draft);
    this._draft.font[step] = value;
    this._dirty = true;
    this._status = "已修改（未保存）";
    this.applyStudioThemeVars();
    this.renderPreviewOnly();
    this.updateDirtyChrome();
  }

  onTypographyChange(field, value) {
    if (!this._draft) return;
    ensureFont(this._draft);
    this._draft.tokens.typography[field] = value;
    this._dirty = true;
    this._status = "已修改（未保存）";
    this.applyStudioThemeVars();
    this.renderPreviewOnly();
    this.updateDirtyChrome();
  }

  onTextRoleChange(roleKey, field, value) {
    if (!this._draft) return;
    ensureFont(this._draft);
    if (!this._draft[roleKey] || typeof this._draft[roleKey] !== "object") {
      this._draft[roleKey] = { ...(TEXT_ROLE_DEFAULTS[roleKey] || {}) };
    }
    if (field === "font_style") {
      if (value) this._draft[roleKey].font_style = "italic";
      else delete this._draft[roleKey].font_style;
    } else {
      this._draft[roleKey][field] = value;
    }
    this._dirty = true;
    this._status = "已修改（未保存）";
    this.applyStudioThemeVars();
    this.renderPreviewOnly();
    this.updateDirtyChrome();
  }

  applyStudioThemeVars() {
    if (!this._draft) return;
    applyPreviewVars(this, this._draft);
    const studio = this.shadowRoot?.querySelector(".studio");
    if (studio) applyPreviewVars(studio, this._draft);
  }

  updateDirtyChrome() {
    const status = this.shadowRoot.querySelector("[data-status]");
    if (status) {
      status.textContent = this._error || this._status;
      status.classList.toggle("error", Boolean(this._error));
    }
    const saveBtn = this.shadowRoot.querySelector("[data-save]");
    if (saveBtn) saveBtn.disabled = this._busy || !this._dirty;
  }

  renderPreviewOnly() {
    this.shadowRoot.querySelectorAll("[data-preview-root]").forEach((preview) => {
      if (this._draft) applyPreviewVars(preview, this._draft);
    });
    this.shadowRoot.querySelectorAll("[data-grad-preview]").forEach((node) => {
      const key = node.getAttribute("data-grad-preview");
      const value = readToken(this._draft, "gradient", key);
      node.style.backgroundImage = value || "none";
    });
  }

  async saveTheme() {
    if (!this._draft || !this._themeId) return;
    this._busy = true;
    this._error = "";
    this._status = "保存中…";
    this.updateDirtyChrome();
    try {
      const res = await fetch(`/api/ops/scene-themes/${encodeURIComponent(this._themeId)}`, {
        method: "PUT",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          patch: {
            label: this._draft.label,
            font: this._draft.font,
            tokens: {
              color: this._draft.tokens?.color || {},
              gradient: this._draft.tokens?.gradient || {},
              typography: this._draft.tokens?.typography || {},
            },
            ...Object.fromEntries(
              TEXT_ROLES.map((role) => [role.key, this._draft[role.key] || {}]),
            ),
          },
        }),
      });
      const payload = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(payload.error || `save HTTP ${res.status}`);
      this._draft = deepClone(payload.theme || this._draft);
      ensureFont(this._draft);
      this._dirty = false;
      this._status = "已保存到工作区主题库";
      await this.refreshLiveCss();
      const catalogRes = await fetch("/api/ops/scene-themes", { credentials: "same-origin" });
      if (catalogRes.ok) {
        const catalog = await catalogRes.json();
        this._themes = Array.isArray(catalog.themes) ? catalog.themes : this._themes;
      }
    } catch (error) {
      this._error = error?.message || String(error);
    } finally {
      this._busy = false;
      this.render();
    }
  }

  async applyToApp() {
    const appId = this.currentAdminAppId();
    if (!appId || !this._themeId) return;
    this._busy = true;
    this._error = "";
    this._status = "应用到当前应用…";
    this.updateDirtyChrome();
    try {
      this.syncPropsFromDom();
      const putRef = this._props.selection_put || this._props.selectionPut;
      if (!putRef) throw new Error("缺少 selection_put provider");
      let revision = 0;
      const getRef = this._props.selection_get || this._props.selectionGet;
      if (getRef) {
        try {
          const current = await studioReadProvider(getRef);
          revision = Number(current?.revision || 0);
        } catch {
          revision = 0;
        }
      }
      await studioPutConfigRecord(putRef, { active: this._themeId }, revision);
      this._status = `已将当前应用主题设为「${this._themeId}」`;
      rememberActiveThemeId(appId, this._themeId);
      if (typeof refreshSceneThemeCss === "function") {
        await refreshSceneThemeCss(this._themeId);
      } else {
        await this.refreshLiveCss();
      }
    } catch (error) {
      this._error = error?.message || String(error);
    } finally {
      this._busy = false;
      this.render();
    }
  }

  async refreshLiveCss() {
    const appId = this.currentAdminAppId();
    if (!appId) return;
    try {
      const query = this._themeId ? `?theme=${encodeURIComponent(this._themeId)}` : "";
      const response = await fetch(`/api/ops/theme/style/${encodeURIComponent(appId)}${query}`, {
        credentials: "same-origin",
      });
      if (!response.ok) return;
      const cssVarsStyle = await response.text();
      const styleEl =
        document.getElementById("mei-scene-theme-style") ||
        (() => {
          const el = document.createElement("style");
          el.id = "mei-scene-theme-style";
          document.head.appendChild(el);
          return el;
        })();
      if (cssVarsStyle.includes("{")) {
        styleEl.textContent = cssVarsStyle;
      } else {
        styleEl.textContent = `:root, .mei-compose-scene-root, #mei-compose-root { ${cssVarsStyle} }`;
      }
      window.dispatchEvent?.(
        new CustomEvent("meilang:preview-updated", {
          bubbles: true,
          detail: { reason: "theme-studio-save", resetRuntimeQueryCache: false },
        }),
      );
    } catch {
      /* ignore */
    }
  }

  renderColorChannel(label, path, color, { showAlpha = true } = {}) {
    const alphaPct = Math.round(clamp01(color.a) * 100);
    return `
      <div class="grad-row">
        <label>${label}</label>
        <input type="range" min="0" max="100" value="${alphaPct}" data-grad-path="${path}" data-grad-part="alpha" ${
          showAlpha ? "" : "disabled hidden"
        } />
        <input type="color" value="${color.hex}" data-grad-path="${path}" data-grad-part="hex" title="${label}" />
        <span class="alpha">${showAlpha ? `${alphaPct}%` : ""}</span>
      </div>
    `;
  }

  renderGradientEditor(key, label, value) {
    const model = parseStructuredGradient(key, value);
    const previewCss = escapeAttr(value || "none");
    if (model.kind === "glow-bar") {
      return `
        <div class="field gradient-field">
          <label>${label}</label>
          <div class="grad-editor" data-grad-key="${key}" data-grad-kind="glow-bar">
            <div class="grad-preview" data-grad-preview="${key}" style="background-image:${previewCss}"></div>
            ${this.renderColorChannel("高光色", "glow", { ...model.glow, a: model.peak }, { showAlpha: true })}
            ${this.renderColorChannel("底色上", "baseTop", model.baseTop)}
            ${this.renderColorChannel("底色下", "baseBottom", model.baseBottom)}
            <details class="grad-advanced">
              <summary>原始 CSS</summary>
              <textarea data-token-kind="gradient" data-token-key="${key}">${escapeAttr(value)}</textarea>
            </details>
          </div>
        </div>
      `;
    }
    if (model.kind === "halo-band") {
      return `
        <div class="field gradient-field">
          <label>${label}</label>
          <div class="grad-editor" data-grad-key="${key}" data-grad-kind="halo-band">
            <div class="grad-preview" data-grad-preview="${key}" style="background-image:${previewCss}"></div>
            ${this.renderColorChannel("光晕", "halo", model.halo)}
            ${this.renderColorChannel("强调", "accent", model.accent)}
            ${this.renderColorChannel("底色", "base", model.base)}
            <details class="grad-advanced">
              <summary>原始 CSS</summary>
              <textarea data-token-kind="gradient" data-token-key="${key}">${escapeAttr(value)}</textarea>
            </details>
          </div>
        </div>
      `;
    }
    if (model.kind === "linear-dual" || model.kind === "radial-dual") {
      return `
        <div class="field gradient-field">
          <label>${label}</label>
          <div class="grad-editor" data-grad-key="${key}" data-grad-kind="${model.kind}">
            <div class="grad-preview" data-grad-preview="${key}" style="background-image:${previewCss}"></div>
            ${this.renderColorChannel("起点", "start", model.start)}
            ${this.renderColorChannel("终点", "end", model.end)}
            <details class="grad-advanced">
              <summary>原始 CSS</summary>
              <textarea data-token-kind="gradient" data-token-key="${key}">${escapeAttr(value)}</textarea>
            </details>
          </div>
        </div>
      `;
    }
    return `
      <div class="field gradient-field">
        <label>${label}</label>
        <div class="grad-editor" data-grad-key="${key}" data-grad-kind="raw">
          <div class="grad-preview" data-grad-preview="${key}" style="background-image:${previewCss}"></div>
          <textarea data-token-kind="gradient" data-token-key="${key}">${escapeAttr(value)}</textarea>
        </div>
      </div>
    `;
  }

  readGradColorFromEditor(editor, path) {
    const hexInput = editor.querySelector(`[data-grad-path="${path}"][data-grad-part="hex"]`);
    const alphaInput = editor.querySelector(`[data-grad-path="${path}"][data-grad-part="alpha"]`);
    const parsed = parseCssColor(hexInput?.value || "#000000");
    const alpha = alphaInput ? Number(alphaInput.value) / 100 : parsed.a;
    return { ...parsed, a: clamp01(alpha) };
  }

  syncGradientEditor(editor) {
    const key = editor.getAttribute("data-grad-key");
    const kind = editor.getAttribute("data-grad-kind");
    let css = "";
    if (kind === "glow-bar") {
      const glow = this.readGradColorFromEditor(editor, "glow");
      css = buildGlowBarGradient({
        glow: { r: glow.r, g: glow.g, b: glow.b, hex: glow.hex },
        peak: glow.a,
        baseTop: this.readGradColorFromEditor(editor, "baseTop"),
        baseBottom: this.readGradColorFromEditor(editor, "baseBottom"),
      });
    } else if (kind === "halo-band") {
      css = buildHaloBandGradient({
        halo: this.readGradColorFromEditor(editor, "halo"),
        accent: this.readGradColorFromEditor(editor, "accent"),
        base: this.readGradColorFromEditor(editor, "base"),
      });
    } else if (kind === "linear-dual" || kind === "radial-dual") {
      css = buildDualStopGradient({
        kind,
        start: this.readGradColorFromEditor(editor, "start"),
        end: this.readGradColorFromEditor(editor, "end"),
      });
    } else {
      return;
    }
    writeToken(this._draft, "gradient", key, css);
    const raw = editor.querySelector(`textarea[data-token-key="${key}"]`);
    if (raw) raw.value = css;
    editor.querySelectorAll(".grad-row").forEach((row) => {
      const alphaInput = row.querySelector('[data-grad-part="alpha"]');
      const alphaLabel = row.querySelector(".alpha");
      if (alphaInput && alphaLabel) alphaLabel.textContent = `${alphaInput.value}%`;
    });
    this._dirty = true;
    this._status = "已修改（未保存）";
    this.applyStudioThemeVars();
    this.renderPreviewOnly();
    this.updateDirtyChrome();
  }

  bindEvents() {
    const root = this.shadowRoot;
    root.querySelector("[data-theme-select]")?.addEventListener("change", (event) => {
      this.onThemePick(event.target.value);
    });
    root.querySelector("[data-save]")?.addEventListener("click", () => this.saveTheme());
    root.querySelector("[data-apply]")?.addEventListener("click", () => this.applyToApp());

    root.querySelectorAll("[data-grad-key]").forEach((editor) => {
      editor.querySelectorAll("[data-grad-path]").forEach((input) => {
        const handler = () => this.syncGradientEditor(editor);
        input.addEventListener("input", handler);
        input.addEventListener("change", handler);
      });
    });

    root.querySelectorAll("[data-token-kind]").forEach((input) => {
      const kind = input.getAttribute("data-token-kind");
      const key = input.getAttribute("data-token-key");
      const apply = () => this.onColorChange(kind, key, input.value);
      input.addEventListener("change", apply);
      if (input.tagName === "TEXTAREA") {
        input.addEventListener("change", () => {
          const editor = input.closest("[data-grad-key]");
          if (editor) {
            const preview = editor.querySelector("[data-grad-preview]");
            if (preview) preview.style.backgroundImage = input.value || "none";
          }
        });
      }
      input.addEventListener("input", () => {
        if (input.type === "color") {
          const text = root.querySelector(
            `input[data-token-kind="${kind}"][data-token-key="${key}"][type="text"]`,
          );
          if (text && text !== input) text.value = input.value;
          this.onColorChange(kind, key, input.value);
        }
      });
    });
    root.querySelectorAll("[data-font-step]").forEach((input) => {
      input.addEventListener("change", () => {
        this.onFontChange(input.getAttribute("data-font-step"), input.value);
      });
    });
    root.querySelectorAll("[data-typo]").forEach((input) => {
      input.addEventListener("change", () => {
        this.onTypographyChange(input.getAttribute("data-typo"), input.value);
      });
    });
    root.querySelectorAll("[data-text-role]").forEach((input) => {
      const apply = () => {
        const role = input.getAttribute("data-text-role");
        const field = input.getAttribute("data-text-field");
        const value =
          input.type === "checkbox" ? (input.checked ? "italic" : "") : input.value;
        this.onTextRoleChange(role, field, value);
      };
      input.addEventListener("change", apply);
    });
  }

  renderGroupFields(group) {
    return group.keys
      .map(([kind, key, label]) => {
        const value = readToken(this._draft, kind, key);
        if (kind === "gradient" && STRUCTURED_GRADIENT_KEYS.has(key)) {
          return this.renderGradientEditor(key, label, value);
        }
        if (kind === "gradient") {
          return `
            <div class="field gradient-field">
              <label>${label}</label>
              <div class="grad-editor">
                <div class="grad-preview" data-grad-preview="${key}" style="background-image:${escapeAttr(
                  value || "none",
                )}"></div>
                <textarea data-token-kind="gradient" data-token-key="${key}">${escapeAttr(value)}</textarea>
              </div>
            </div>
          `;
        }
        const colorValue = approxHex(value);
        return `
          <div class="field">
            <label>${label}</label>
            <input type="text" data-token-kind="${kind}" data-token-key="${key}" value="${escapeAttr(value)}" />
            <input type="color" data-token-kind="${kind}" data-token-key="${key}" value="${colorValue}" title="${label}" />
          </div>
        `;
      })
      .join("");
  }

  renderFontFields() {
    ensureFont(this._draft);
    const colorKeys = Object.keys(this._draft.tokens?.color || {}).sort();
    const roleFields = TEXT_ROLES.map((role) => {
      const entry = this._draft[role.key] || {};
      const colorOptions = colorKeys
        .map(
          (key) =>
            `<option value="${escapeAttr(key)}" ${
              entry.color === key ? "selected" : ""
            }>${escapeAttr(key)}</option>`,
        )
        .join("");
      const fontOptions = FONT_STEPS.map(
        (step) =>
          `<option value="${step}" ${String(entry.font) === step ? "selected" : ""}>字阶 ${step}</option>`,
      ).join("");
      const weightOptions = FONT_WEIGHT_OPTIONS.map(
        (opt) =>
          `<option value="${opt.value}" ${
            String(entry.font_weight || "regular") === opt.value ? "selected" : ""
          }>${opt.label}</option>`,
      ).join("");
      const italic = String(entry.font_style || "").toLowerCase() === "italic";
      const className = `.mei-text-${role.prefix.replace(/^mei-/, "").replace(/_/g, "-")}`;
      return `
        <div class="role-card">
          <div class="role-card-head">
            <strong>${role.label}</strong>
            <code>${className}</code>
          </div>
          <div class="field">
            <label>字阶</label>
            <select data-text-role="${role.key}" data-text-field="font">${fontOptions}</select>
            <span></span>
          </div>
          <div class="field">
            <label>字色</label>
            <select data-text-role="${role.key}" data-text-field="color">
              <option value="">（继承）</option>
              ${colorOptions}
            </select>
            <span></span>
          </div>
          <div class="field">
            <label>字重</label>
            <select data-text-role="${role.key}" data-text-field="font_weight">${weightOptions}</select>
            <span></span>
          </div>
          <div class="field role-italic">
            <label>斜体</label>
            <input type="checkbox" data-text-role="${role.key}" data-text-field="font_style" ${
              italic ? "checked" : ""
            } />
            <span></span>
          </div>
          <div class="role-sample ${className.slice(1)}">示例 Aa 监督 12,580</div>
        </div>
      `;
    }).join("");
    const steps = FONT_STEPS.map(
      (step) => `
      <div class="field">
        <label>字号 ${step}</label>
        <input type="text" data-font-step="${step}" value="${escapeAttr(this._draft.font[step] || "")}" />
        <span></span>
      </div>
    `,
    ).join("");
    const typo = this._draft.tokens.typography;
    return `
      <div class="role-grid">${roleFields}</div>
      <details class="font-advanced">
        <summary>基线字阶与全局字体（高级）</summary>
        ${steps}
        <div class="field">
          <label>字体族</label>
          <input type="text" data-typo="family" value="${escapeAttr(typo.family || "")}" />
          <span></span>
        </div>
        <div class="field">
          <label>常规字重</label>
          <input type="text" data-typo="weight_regular" value="${escapeAttr(typo.weight_regular || "")}" />
          <span></span>
        </div>
        <div class="field">
          <label>中等字重</label>
          <input type="text" data-typo="weight_medium" value="${escapeAttr(typo.weight_medium || "")}" />
          <span></span>
        </div>
        <div class="field">
          <label>加粗字重</label>
          <input type="text" data-typo="weight_bold" value="${escapeAttr(typo.weight_bold || "")}" />
          <span></span>
        </div>
      </details>
    `;
  }

  renderSectionPreview(sectionId) {
    const charts = [1, 2, 3, 4, 5, 6]
      .map((n) => `<span style="background: var(--mei-color-chart-${n}, #38bdf8)"></span>`)
      .join("");
    const typeSteps = FONT_STEPS.map(
      (step) => `
      <div class="step">
        <span style="font-size: var(--mei-font-${step}); font-weight: var(--mei-typography-weight-regular, 400);">字阶 ${step} 示例 Aa 监督</span>
        <span class="muted">${this._draft?.font?.[step] || ""}</span>
      </div>
    `,
    ).join("");

    const bodies = {
      stage: `<div class="preview-stage">舞台预览</div>`,
      title: `
        <div class="preview-header-band">大标题带</div>
        <div class="preview-title-bar">板块标题</div>
      `,
      panel: `<div class="preview-panel">面板内容 · 指标与图表容器</div>`,
      drilldown: `
        <div class="preview-drill">
          <div class="preview-drill-mask">
            <div class="preview-drill-chrome">二级屏顶栏 / 页签</div>
            <div class="preview-drill-body">遮罩 + 内容区</div>
          </div>
        </div>
      `,
      filter_table: `
        <div class="preview-filter">
          <span class="preview-filter-control">筛选控件</span>
        </div>
        <div class="preview-table">
          <div class="head">表头</div>
          <div class="row">数据行（悬停）</div>
        </div>
      `,
      chart_text: `
        <div class="preview-charts">${charts}</div>
        <div class="preview-text-stack">
          <div class="mei-text-header-title">顶栏大标题</div>
          <div class="mei-text-panel-head">面板标题</div>
          <div class="mei-text-body">正文内容 · 指标说明</div>
          <div class="mei-text-muted">次要文字 · 辅助说明</div>
          <div class="mei-text-metric-value">数值强调 12,580</div>
        </div>
      `,
      typography: `
        <div class="preview-type">
          <div class="mei-text-header-title">顶栏大标题</div>
          <div class="mei-text-panel-head">面板标题</div>
          <div class="mei-text-metric-value">指标数值 12,580</div>
          <div class="mei-text-body">正文示例</div>
          <div class="mei-text-muted">次要说明</div>
          ${typeSteps}
        </div>
      `,
    };

    return `
      <div class="preview-block" data-preview-root data-preview-section="${sectionId}">
        ${bodies[sectionId] || ""}
      </div>
    `;
  }

  renderSection(title, sectionId, fieldsHtml) {
    return `
      <section class="section" data-section="${sectionId}">
        <h3>${title}</h3>
        <div class="section-split">
          <div class="section-config">${fieldsHtml}</div>
          <div class="section-preview">
            ${this.renderSectionPreview(sectionId)}
          </div>
        </div>
      </section>
    `;
  }

  renderSections() {
    const colorSections = COLOR_GROUPS.map((group) =>
      this.renderSection(group.title, group.id, this.renderGroupFields(group)),
    ).join("");
    const fontSection = this.renderSection("文字样式", "typography", this.renderFontFields());
    return `<div class="sections">${colorSections}${fontSection}</div>`;
  }

  render() {
    if (!this.shadowRoot) return;
    if (this._error && !this._draft) {
      this.shadowRoot.innerHTML = `<style>${studioStyles()}</style><div class="studio"><p class="status error">${this._error}</p></div>`;
      return;
    }
    if (!this._draft) {
      this.shadowRoot.innerHTML = `<style>${studioStyles()}</style><div class="studio"><p class="status">${this._status || "加载中…"}</p></div>`;
      return;
    }
    const options = this._themes
      .map((theme) => {
        const selected = theme.id === this._themeId ? "selected" : "";
        return `<option value="${theme.id}" ${selected}>${theme.label || theme.id}</option>`;
      })
      .join("");
    const activeSwatches = this._themes.find((t) => t.id === this._themeId)?.swatches || {};
    const dots = ["surface_bg", "chart_1", "drilldown_panel_top", "text_primary"]
      .map((key) => {
        const color = activeSwatches[key] || readToken(this._draft, "color", key) || "#334155";
        return `<span class="swatch-dot" style="background:${color}" title="${key}"></span>`;
      })
      .join("");

    this.shadowRoot.innerHTML = `
      <style>${studioStyles()}</style>
      <div class="studio">
        <div class="toolbar">
          <label>当前主题
            <select data-theme-select>${options}</select>
          </label>
          <span class="swatch-row">${dots}</span>
          <button type="button" class="primary" data-save ${this._busy || !this._dirty ? "disabled" : ""}>保存主题</button>
          <button type="button" data-apply ${this._busy ? "disabled" : ""}>应用到当前应用</button>
          <span class="status ${this._error ? "error" : ""}" data-status>${this._error || this._status}</span>
        </div>
        ${this.renderSections()}
      </div>
    `;
    this.applyStudioThemeVars();
    this.renderPreviewOnly();
    this.bindEvents();
  }
}

if (!customElements.get("mei-admin-theme-studio")) {
  customElements.define("mei-admin-theme-studio", ThemeStudio);
}
