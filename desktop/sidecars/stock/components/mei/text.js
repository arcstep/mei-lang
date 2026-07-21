import {
  deferUntilDisplayed,
  fetchPanelRuntimeMetrics,
  fetchRuntimeMetrics,
  findRuntimeMetricInResults,
  isDrilldownOverlayOpen,
  isRuntimeDrilldownOverlayElement,
  isStaticSkeletonDisplay,
  parseProps,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  shouldReactToPreviewUpdated,
  subscribeQueryState,
} from "../dataset/runtime-query.js";
import {
  ANALYSIS_OPEN_EVENT_NAME,
  DRILLDOWN_EVENT_NAME,
  POPUP_OPEN_EVENT_NAME,
  tableDrilldownMeta,
} from "../cockpit/drilldown-meta.js";
import { formatMetricNumber } from "./metric-number-format.js";
import {
  bindFloatingPopoverDrag,
  buildTextPopoverShellHtml,
  copyTextToClipboard,
  ensureFloatingTextPopoverStyles,
  mountFloatingPopoverOnBody,
  mountTextPopoverBackdrop,
  positionFloatingPopoverNearAnchor,
  removeTextPopoverBackdrop,
} from "./floating-text-popover.js";

/**
 * 内置 mei.text：plain 转义文本；format=html 或 props.html 渲染基本 HTML（作者态可信内容）。
 */

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function resolveContent(props) {
  if (props.content != null && String(props.content).length > 0) {
    return String(props.content);
  }
  if (props.html != null && String(props.html).length > 0) {
    return String(props.html);
  }
  const doc = props.resource?.document;
  if (doc != null && String(doc).length > 0) {
    return String(doc);
  }
  return "";
}

function resolveFormat(props, content) {
  const explicit = String(props.format || "").trim().toLowerCase();
  if (explicit === "html" || explicit === "plain") {
    return explicit;
  }
  if (props.html != null && String(props.html).length > 0) {
    return "html";
  }
  return "plain";
}

function resolveMetricValueFormat(metric, props) {
  const patch = metricPatchOf(props);
  const fromPatch = patch?.value_format ?? patch?.valueFormat;
  if (fromPatch != null) return fromPatch;
  const fromProps = props?.value_format ?? props?.valueFormat;
  if (fromProps != null) return fromProps;
  const fromMetric = metric?.value_format ?? metric?.valueFormat;
  if (fromMetric != null) return fromMetric;
  return null;
}

function formatMetricValue(value, unit = "", format = null) {
  return formatMetricNumber(value, { unit, format });
}

function metricMapOf(props) {
  const raw = props?.metricMap ?? props?.metric_map ?? {};
  return raw && typeof raw === "object" ? raw : {};
}

function metricPatchOf(props) {
  const raw = props?.metricPatch ?? props?.metric_patch ?? {};
  return raw && typeof raw === "object" ? raw : {};
}

function metricPresentationBaseOf(props) {
  const raw = props?.__mei_metric_presentation ?? props?.__meiMetricPresentation ?? {};
  return raw && typeof raw === "object" ? raw : {};
}

function metricPresentationFromMetric(metric) {
  if (!metric || typeof metric !== "object") {
    return {};
  }
  if (metric.presentation && typeof metric.presentation === "object") {
    return metric.presentation;
  }
  if (
    metric.value &&
    typeof metric.value === "object" &&
    metric.value.presentation &&
    typeof metric.value.presentation === "object"
  ) {
    return metric.value.presentation;
  }
  return {};
}

function resolveMetricPresentation(metric, props) {
  const base = metricPresentationBaseOf(props);
  const patch = metricPatchOf(props)?.presentation;
  const fromMetric = metricPresentationFromMetric(metric);
  const iconCandidate =
    fromMetric?.icon ??
    patch?.icon ??
    base?.icon;
  const icon = iconCandidate == null ? "" : String(iconCandidate).trim();
  return icon ? { icon } : {};
}

function backgroundImageCssValue(icon) {
  const raw = String(icon || "").trim();
  if (!raw) {
    return "";
  }
  if (/^url\(/i.test(raw)) {
    return raw;
  }
  return `url(${raw})`;
}

function findMetricCardHost(element) {
  let node = element;
  while (node) {
    if (
      node instanceof HTMLElement &&
      node.getAttribute?.("data-mei-metric-card") === "true"
    ) {
      return node;
    }
    node = node.parentElement;
  }
  return null;
}

function syncMetricCardPresentation(element, metric, props) {
  const role = String(props?.metric_role || "").trim().toLowerCase();
  if (role !== "value") {
    return;
  }
  const resolved = resolveMetricPresentation(metric, props);
  const icon = String(resolved?.icon || "").trim();
  if (!icon) {
    return;
  }
  const host = findMetricCardHost(element);
  if (!host) {
    return;
  }
  const imageValue = backgroundImageCssValue(icon);
  const style = host.getAttribute("style") || "";
  if (/background-image\s*:/i.test(style)) {
    host.setAttribute(
      "style",
      style.replace(/background-image\s*:\s*[^;]+;?/i, `background-image:${imageValue};`),
    );
    return;
  }
  host.setAttribute("style", `${style}background-image:${imageValue};`);
}

function slotFieldName(slot, props) {
  const mapped = metricMapOf(props)[slot];
  const name = String(mapped ?? "").trim();
  return name || slot;
}

function primitiveSlotOverride(props, slot) {
  const raw = props?.[slot];
  if (raw == null || typeof raw === "object") return undefined;
  return String(raw);
}

function applyMetricPatch(display, props) {
  const out = {
    label: String(display?.label ?? ""),
    value: String(display?.value ?? "--"),
    unit: String(display?.unit ?? ""),
    desc: String(display?.desc ?? ""),
  };
  for (const slot of ["label", "value", "unit", "desc"]) {
    const override = primitiveSlotOverride(props, slot);
    if (override !== undefined) out[slot] = override;
  }
  const patch = metricPatchOf(props);
  for (const slot of ["label", "value", "unit", "desc"]) {
    if (patch?.[slot] != null) out[slot] = String(patch[slot]);
  }
  return out;
}

function metricDisplayFromScalar(metric, props = {}) {
  if (!metric || metric.shape !== "scalar" || typeof metric.value !== "object" || metric.value === null) {
    return { label: "", value: "--", unit: "", desc: "" };
  }
  const schema = Array.isArray(metric.schema) ? metric.schema : [];
  const units = new Map(
    schema
      .filter((column) => column && typeof column.name === "string")
      .map((column) => [column.name, column.unit || ""]),
  );
  const values = metric.value || {};
  const entries = Object.entries(values);
  const metricLabel =
    typeof metric.label === "string" && metric.label.trim() ? metric.label.trim() : "";
  const metricUnit =
    typeof metric.unit === "string" && metric.unit.trim() ? metric.unit.trim() : "";
  const mappedValueKey = slotFieldName("value", props);
  const [key, rawValue] =
    (mappedValueKey && Object.prototype.hasOwnProperty.call(values, mappedValueKey)
      ? [mappedValueKey, values[mappedValueKey]]
      : entries[0]) || ["", ""];
  let label = metricLabel || key;
  const mappedLabelKey = slotFieldName("label", props);
  if (
    mappedLabelKey &&
    mappedLabelKey !== "label" &&
    Object.prototype.hasOwnProperty.call(values, mappedLabelKey)
  ) {
    label = String(values[mappedLabelKey] ?? label);
  }
  let unit = metricUnit || units.get(key) || "";
  const mappedUnitKey = slotFieldName("unit", props);
  if (
    mappedUnitKey &&
    mappedUnitKey !== "unit" &&
    Object.prototype.hasOwnProperty.call(values, mappedUnitKey)
  ) {
    unit = String(values[mappedUnitKey] ?? unit);
  }
  let desc = "";
  const mappedDescKey = slotFieldName("desc", props);
  if (mappedDescKey && Object.prototype.hasOwnProperty.call(values, mappedDescKey)) {
    desc = String(values[mappedDescKey] ?? "");
  }
  const valueFormat = resolveMetricValueFormat(metric, props);
  return applyMetricPatch(
    {
      label: String(label ?? ""),
      value: formatMetricValue(rawValue, unit, valueFormat),
      unit: String(unit ?? ""),
      desc,
    },
    props,
  );
}

function metricContentObject(props) {
  const content = props?.content;
  if (!content || typeof content !== "object" || Array.isArray(content)) {
    return null;
  }
  return content;
}

function isPlainStaticMetricDisplay(content) {
  if (!content || typeof content !== "object" || Array.isArray(content)) {
    return false;
  }
  if (content.__mei_runtime_ref || content.shape) {
    return false;
  }
  return (
    content.label != null ||
    content.value != null ||
    content.unit != null ||
    content.desc != null
  );
}

function metricDisplayFromContent(props) {
  const content = metricContentObject(props);
  if (!content) {
    return null;
  }
  if (isStaticSkeletonDisplay(props)) {
    return applyMetricPatch(
      {
        label: String(content.label ?? "指标名"),
        value: String(content.value ?? "xxxx"),
        unit: String(content.unit ?? "单位"),
        desc: String(content.desc ?? ""),
      },
      props,
    );
  }
  if (content.shape === "scalar") {
    return metricDisplayFromScalar(content, props);
  }
  if (isPlainStaticMetricDisplay(content)) {
    return applyMetricPatch(
      {
        label: String(content.label ?? ""),
        value: String(content.value ?? "--"),
        unit: String(content.unit ?? ""),
        desc: String(content.desc ?? ""),
      },
      props,
    );
  }
  return applyMetricPatch({ label: "", value: "--", unit: "", desc: "" }, props);
}

function metricSlotContent(props) {
  const role = String(props?.metric_role || "").trim().toLowerCase();
  if (!role) {
    return "";
  }
  const display = metricDisplayFromContent(props);
  if (!display) {
    return "";
  }
  return String(display[role] ?? "");
}

function metricRuntimeRef(props) {
  const content = metricContentObject(props);
  if (!content || typeof content !== "object") {
    return null;
  }
  const ref = content.__mei_runtime_ref;
  if (!ref || typeof ref !== "object" || Array.isArray(ref)) {
    return null;
  }
  return ref;
}

function drilldownTabIds(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry) => String(entry ?? "").trim())
    .filter((entry) => entry.length > 0);
}

function drilldownStringArray(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((entry) => String(entry ?? "").trim())
    .filter((entry) => entry.length > 0);
}

function drilldownExplainMetrics(value) {
  const normalizeEntry = (entry, fallbackId = "") => {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;
    const id = String(entry.id || fallbackId || "").trim();
    const kind = String(entry.kind || "").trim();
    if (!id && !kind) return null;
    return {
      ...entry,
      id: id || kind,
    };
  };
  if (Array.isArray(value)) {
    return value.map((entry) => normalizeEntry(entry)).filter(Boolean);
  }
  if (!value || typeof value !== "object") {
    return [];
  }
  return Object.entries(value)
    .map(([key, entry]) => normalizeEntry(entry, key))
    .filter(Boolean);
}

function metricDrilldownMeta(props) {
  return tableDrilldownMeta(props);
}

function textAlignCss(props) {
  const raw = String(props.align || "").trim().toLowerCase();
  if (raw === "center" || raw === "centre") {
    return "center";
  }
  if (raw === "right" || raw === "end") {
    return "right";
  }
  if (raw === "left" || raw === "start") {
    return "left";
  }
  // 横排短标签：在定宽槽内两端拉开（「金额」→「金　　额」对齐「政府采购」）。
  if (raw === "justify" || raw === "distribute") {
    return "justify";
  }
  // 横排数值：整数部右齐 + 小数部固定槽（小数点对齐）。
  if (raw === "decimal" || raw === "decimal-point" || raw === "decimal_point") {
    return "decimal";
  }
  return "";
}

function justifyContentFromTextAlign(align) {
  if (align === "left") return "flex-start";
  if (align === "right") return "flex-end";
  if (align === "justify" || align === "decimal") return "stretch";
  return "center";
}

/** Split display number into integer + fractional (incl. '.') for decimal-point align. */
function splitDecimalDisplay(text) {
  const raw = String(text ?? "").trim();
  const m = raw.match(/^([+\-]?[\d,]+)(\.\d+)?(.*)$/);
  if (!m) {
    return { intPart: raw, fracPart: "", suffix: "" };
  }
  return {
    intPart: m[1] || "",
    fracPart: m[2] || "",
    suffix: m[3] || "",
  };
}

function metricVerticalAlign(props) {
  const raw = String(props.metric_v_align || "").trim().toLowerCase();
  if (raw === "end" || raw === "bottom" || raw === "baseline") {
    return "flex-end";
  }
  if (raw === "start" || raw === "top") {
    return "flex-start";
  }
  return "center";
}

const METRIC_FONT_FALLBACK = { label: "2", value: "3", unit: "1", desc: "1" };
const METRIC_SUB_FONT_FALLBACK = { label: "1", value: "2", unit: "1", desc: "1" };

function fontSizeVar(props) {
  const key = String(props.font || "").trim();
  if (key) {
    if (/^\d+(\.\d+)?(px|rem|em|%)$/.test(key)) {
      return key;
    }
    return `var(--mei-font-${key}, 14px)`;
  }
  const explicitSize = String(props.font_size ?? props.fontSize ?? "").trim();
  if (explicitSize && /^\d+(\.\d+)?(px|rem|em|%)$/.test(explicitSize)) {
    return explicitSize;
  }
  const role = String(props.metric_role || "").trim().toLowerCase();
  if (role === "label" || role === "value" || role === "unit" || role === "desc") {
    const isSub = String(props.metric_variant || "").trim().toLowerCase() === "sub";
    const tier = isSub ? "metric-sub" : "metric";
    const table = isSub ? METRIC_SUB_FONT_FALLBACK : METRIC_FONT_FALLBACK;
    const fallback = table[role] || "2";
    return `var(--mei-${tier}-${role}-font-size, var(--mei-font-${fallback}, 14px))`;
  }
  return "var(--mei-panel-head-font-size, inherit)";
}

/** Slides: detect deck context + semantic slot leaf (hero/claim/…) for typography. */
function resolveSlidesPresentationContext(hostEl) {
  if (typeof document === "undefined") return null;
  const bodyProfile = String(document.body?.getAttribute?.("data-mei-stage-profile") || "");
  const compose = document.getElementById("mei-compose-root");
  const composeProfile = String(compose?.getAttribute?.("data-mei-stage-profile") || "");
  const underSlide = Boolean(hostEl?.closest?.('[data-mei-ui-role="slide"]'));
  const map = typeof window !== "undefined" ? window.__mei?.presentation_map : null;
  const deckSlides = map?.deck?.slides || map?.presentation_deck?.slides;
  const hasDeck = Array.isArray(deckSlides) && deckSlides.length > 0;
  const slides =
    bodyProfile === "slides" ||
    composeProfile === "slides" ||
    underSlide ||
    hasDeck;
  if (!slides) return null;

  const SLOT_LEAVES = new Set([
    "hero",
    "claim",
    "explain",
    "evidence",
    "action",
    "title",
    "steps",
    "visual",
    "col_a",
    "col_b",
    "col_c",
    "q1",
    "q2",
    "q3",
    "q4",
  ]);

  const scopeEl = hostEl?.closest?.("[data-preview-scope]");
  const scope = String(scopeEl?.getAttribute?.("data-preview-scope") || "");
  const scopeParts = scope.split("/").filter(Boolean);
  let panel = "";
  for (let i = scopeParts.length - 1; i >= 0; i -= 1) {
    const part = String(scopeParts[i] || "")
      .trim()
      .toLowerCase();
    if (SLOT_LEAVES.has(part)) {
      panel = part;
      break;
    }
  }
  if (!panel) {
    const named = hostEl?.closest?.("[data-mei-panel-name]");
    const raw = String(named?.getAttribute?.("data-mei-panel-name") || "")
      .trim()
      .toLowerCase();
    if (SLOT_LEAVES.has(raw)) panel = raw;
  }
  return { panel: panel || "body" };
}

/**
 * Shadow-DOM presentation typography for deck slots (does not affect cockpit metrics).
 */
function slidesPresentationCss(panel) {
  const role = String(panel || "").trim().toLowerCase();
  let tier = "body";
  if (role === "hero") tier = "hero";
  else if (role === "claim" || role === "title") tier = "claim";
  else if (role === "action") tier = "action";
  else if (
    role === "col_a" ||
    role === "col_b" ||
    role === "col_c" ||
    role === "q1" ||
    role === "q2" ||
    role === "q3" ||
    role === "q4"
  ) {
    tier = "column";
  }

  const sizeMap = {
    hero: {
      first: "clamp(2.8rem, 5vw, 4.25rem)",
      rest: "clamp(1.2rem, 2vw, 1.55rem)",
      weight: "700",
      align: "center",
      lh: "1.18",
      mutedSize: "clamp(1.15rem, 1.7vw, 1.4rem)",
    },
    claim: {
      first: "clamp(2.1rem, 3.4vw, 3.1rem)",
      rest: "clamp(1.1rem, 1.6vw, 1.35rem)",
      weight: "700",
      align: "left",
      lh: "1.22",
      mutedSize: "clamp(1.05rem, 1.4vw, 1.25rem)",
    },
    action: {
      first: "clamp(1.45rem, 2.1vw, 1.85rem)",
      rest: "clamp(1.35rem, 1.9vw, 1.65rem)",
      weight: "650",
      align: "left",
      lh: "1.4",
      mutedSize: "clamp(1.25rem, 1.7vw, 1.5rem)",
    },
    column: {
      first: "clamp(1.55rem, 2.3vw, 2rem)",
      rest: "clamp(1.3rem, 1.8vw, 1.55rem)",
      weight: "650",
      align: "left",
      lh: "1.4",
      mutedSize: "clamp(1.25rem, 1.7vw, 1.45rem)",
    },
    body: {
      first: "clamp(1.45rem, 2.1vw, 1.85rem)",
      rest: "clamp(1.35rem, 1.9vw, 1.65rem)",
      weight: "550",
      align: "left",
      lh: "1.5",
      mutedSize: "clamp(1.3rem, 1.8vw, 1.55rem)",
    },
  };
  const s = sizeMap[tier] || sizeMap.body;
  const hostLayout =
    tier === "hero"
      ? `display: flex; align-items: center; justify-content: center; width: 100%; height: auto; max-height: 100%; min-height: 0; min-width: 0; box-sizing: border-box; overflow: hidden; color: var(--mei-slide-claim, #f8fafc);`
      : `display: block; width: 100%; height: auto; min-height: 0; min-width: 0; box-sizing: border-box; overflow: hidden; color: var(--mei-slide-claim, #f8fafc);`;

  return `
        :host {
          ${hostLayout}
        }
        .mei-text-body,
        .mei-text-preview {
          font-family: "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei",
            "Noto Sans SC", system-ui, sans-serif;
          font-size: ${s.rest};
          font-weight: 450;
          line-height: ${s.lh};
          letter-spacing: 0.01em;
          color: var(--mei-slide-claim, #f8fafc);
          text-align: ${s.align};
          word-break: normal;
          overflow-wrap: break-word;
        }
        .mei-text-body > :first-child,
        .mei-text-preview > :first-child {
          font-size: ${s.first};
          font-weight: ${s.weight};
          line-height: ${s.lh};
          letter-spacing: 0.005em;
          color: var(--mei-slide-claim, #f8fafc);
          margin: 0 0 0.55em;
        }
        .mei-text-body > :first-child:is(ul, ol),
        .mei-text-preview > :first-child:is(ul, ol) {
          font-size: ${s.rest};
          font-weight: 450;
          color: var(--mei-slide-claim, #e2e8f0);
        }
        .mei-text-body > :first-child:last-child,
        .mei-text-preview > :first-child:last-child {
          margin-bottom: 0;
        }
        .mei-text-body > :not(:first-child),
        .mei-text-preview > :not(:first-child) {
          font-size: ${s.mutedSize};
          font-weight: 400;
          color: var(--mei-slide-muted, #cbd5e1);
        }
        .mei-text-body :where(ul, ol),
        .mei-text-preview :where(ul, ol) {
          margin: 0.35em 0 0;
          padding-left: 1.35em;
          color: var(--mei-slide-claim, #e2e8f0);
        }
        .mei-text-body :where(li),
        .mei-text-preview :where(li) {
          margin: 0.35em 0;
          font-size: ${s.rest};
          font-weight: 450;
          color: var(--mei-slide-claim, #e2e8f0);
        }
        .mei-text-body :where(code),
        .mei-text-preview :where(code) {
          font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
          font-size: 0.92em;
          color: var(--mei-slide-accent, #7dd3fc);
          background: rgba(125, 211, 252, 0.1);
          padding: 0.08em 0.35em;
          border-radius: 4px;
        }
        .mei-text-body :where(strong),
        .mei-text-preview :where(strong) {
          color: var(--mei-slide-claim, #f8fafc);
          font-weight: 650;
        }
        .mei-text-body :where(p, h1, h2, h3, ul, ol),
        .mei-text-preview :where(p, h1, h2, h3, ul, ol) {
          margin: 0 0 0.45em;
        }
        .mei-text-body :where(p:last-child, h1:last-child, h2:last-child, h3:last-child, ul:last-child, ol:last-child),
        .mei-text-preview :where(p:last-child, h1:last-child, h2:last-child, h3:last-child, ul:last-child, ol:last-child) {
          margin-bottom: 0;
        }
  `;
}

function metricDescShellBox(shell) {
  if (!shell || typeof shell !== "object") {
    return null;
  }
  const width = shell.width != null ? String(shell.width) : "80px";
  const height = shell.height != null ? String(shell.height) : "20px";
  const borderRadius = shell.border_radius ?? shell.borderRadius ?? "2px";
  const background = metricDescShellBackground(shell);
  return { width, height, background, borderRadius };
}

function metricDescShellBackground(shell) {
  const rawBg = shell.background ?? color("text_unit");
  if (String(rawBg).trim().toLowerCase().startsWith("rgba(")) {
    return String(rawBg).trim();
  }
  const opacity = shell.background_opacity ?? shell.opacity;
  if (opacity == null || String(opacity).trim() === "") {
    return String(rawBg).trim();
  }
  const hex = String(rawBg).trim().replace("#", "");
  if (hex.length === 6) {
    const r = parseInt(hex.slice(0, 2), 16);
    const g = parseInt(hex.slice(2, 4), 16);
    const b = parseInt(hex.slice(4, 6), 16);
    if ([r, g, b].every((n) => Number.isFinite(n))) {
      return `rgba(${r}, ${g}, ${b}, ${opacity})`;
    }
  }
  return String(rawBg).trim();
}

function metricDescShellCss(props) {
  const shell = props.desc_shell ?? props.metric_desc_shell;
  const box = metricDescShellBox(shell);
  if (!box) {
    return "";
  }
  const { width, height, background, borderRadius } = box;
  return `
    width: ${width};
    height: ${height};
    max-width: 100%;
    background: ${background};
    border-radius: ${borderRadius};
    box-sizing: border-box;
    flex: 0 0 auto;
  `;
}

function metricHostLayoutCss(props, { descShell = false } = {}) {
  const align = textAlignCss(props);
  const vAlign = metricVerticalAlign(props);
  if (descShell) {
    return `
      display: flex;
      flex-direction: column;
      width: 100%;
      height: 100%;
      min-width: 0;
      min-height: 0;
      box-sizing: border-box;
      overflow: hidden;
      justify-content: flex-end;
      align-items: center;
    `;
  }
  return `
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    box-sizing: border-box;
    overflow: hidden;
    justify-content: ${vAlign};
    align-items: ${align ? justifyContentFromTextAlign(align) : "stretch"};
  `;
}

function metricBodyClipCss(role) {
  const slot = String(role || "").trim().toLowerCase();
  return `
    flex: 0 0 auto;
    width: 100%;
    min-width: 0;
    max-width: 100%;
    max-height: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: ${slot === "desc" ? "normal" : "nowrap"};
    box-sizing: border-box;
  `;
}

function metricTypographyCss(props) {
  const role = String(props.metric_role || "").trim().toLowerCase();
  if (!role) {
    return "";
  }
  if (role === "desc" && (props.desc_shell || props.metric_desc_shell)) {
    const shell = props.desc_shell ?? props.metric_desc_shell;
    const fontFamily = String(
      props.font_family ??
        props.fontFamily ??
        shell?.font_family ??
        shell?.fontFamily ??
        "Microsoft YaHei, MicrosoftYaHei, PingFang SC, sans-serif",
    ).trim();
    const fontSize = String(
      props.font ?? shell?.font_size ?? shell?.fontSize ?? "12px",
    ).trim();
    const color = String(
      props.color ?? shell?.color ?? "rgba(255, 255, 255, 0.8)",
    ).trim();
    const fontWeight = String(
      props.font_weight ?? props.fontWeight ?? shell?.font_weight ?? shell?.fontWeight ?? "400",
    ).trim();
    const letterSpacing = String(
      props.letter_spacing ??
        props.letterSpacing ??
        shell?.letter_spacing ??
        shell?.letterSpacing ??
        "0",
    ).trim();
    const textAlign = textAlignCss(props) || "center";
    return `
      ${metricBodyClipCss(role)}
      font-size: ${/^\d+(\.\d+)?(px|rem|em|%)$/.test(fontSize) ? fontSize : fontSizeVar({ ...props, font: fontSize })};
      font-family: ${fontFamily};
      color: ${color};
      font-weight: ${fontWeight};
      letter-spacing: ${letterSpacing};
      line-height: 1.2;
      text-align: ${textAlign};
      display: flex;
      align-items: center;
      justify-content: center;
    `;
  }
  const tier =
    String(props.metric_variant || "").trim().toLowerCase() === "sub"
      ? "metric-sub"
      : "metric";
  const prefix = `--mei-${tier}-${role}`;
  const align = textAlignCss(props);
  const lineHeightRaw = String(props.lineHeight ?? props.line_height ?? "").trim();
  const lineHeight =
    lineHeightRaw &&
    (/^\d+(\.\d+)?$/.test(lineHeightRaw) ? lineHeightRaw : lineHeightRaw);
  const justifyLast =
    align === "justify" ? "text-align-last: justify; width: 100%;" : "";
  // decimal 对齐由 .mei-text-num-decimal 网格承担，勿把非法 text-align:decimal 写进 CSS。
  const cssAlign =
    align === "decimal" ? "left" : align || `var(${prefix}-text-align, inherit)`;
  return `
    ${metricBodyClipCss(role)}
    font-size: ${fontSizeVar(props)};
    font-family: var(${prefix}-font-family, inherit);
    color: var(${prefix}-color, var(--mei-color-text-primary, #e2e8f0));
    font-weight: var(${prefix}-font-weight, inherit);
    letter-spacing: var(${prefix}-letter-spacing, normal);
    line-height: ${lineHeight || `var(${prefix}-line-height, 1.2)`};
    text-align: ${cssAlign};
    ${justifyLast}
  `;
}

function truthyProp(value) {
  if (value === true || value === 1) return true;
  if (value === false || value === 0 || value == null) return false;
  const raw = String(value).trim().toLowerCase();
  return raw === "true" || raw === "1" || raw === "yes" || raw === "on";
}

function falsyProp(value) {
  if (value === false || value === 0) return true;
  if (value == null || value === true || value === 1) return false;
  const raw = String(value).trim().toLowerCase();
  return raw === "false" || raw === "0" || raw === "no" || raw === "off";
}

class MeiText extends HTMLElement {
  constructor() {
    super();
    this._queryUnsubscribe = null;
    this._previewHandler = null;
    this._metricAbort = null;
    this._metricRefreshSeq = 0;
    this._drilldownMeta = null;
    this._drilldownDisplay = null;
    this._drilldownClickHandler = (event) => this._emitDrilldown(event);
    this._drilldownKeyHandler = (event) => this._handleDrilldownKey(event);
    this._fullText = "";
    this._overflowExpand = false;
    this._overflowRaf = null;
    this._overflowObserver = null;
    this._textPopoverEl = null;
    this._textPopoverDocCleanup = null;
    this._textPopoverKeydown = null;
    this._textPopoverDragCleanup = null;
    this._expandClickHandler = (event) => this._openTextPopover(event);
  }

  connectedCallback() {
    this._bind();
  }

  disconnectedCallback() {
    this._cleanupOverflowPreview();
    this._closeTextPopover();
    this._cleanupMetricBinding();
    this._setDrilldownState(null, null);
  }

  _bind() {
    this._cleanupMetricBinding();
    const props = parseProps(this);
    if (this._isMetricContent(props)) {
      this._renderMetric(props);
      this._bindMetricRuntime();
      return;
    }
    const content = resolveContent(props);
    const format = resolveFormat(props, content);
    this._render(props, content, format);
  }

  _isMetricContent(props) {
    return Boolean(String(props?.metric_role || "").trim()) && metricContentObject(props) != null;
  }

  _bindMetricRuntime() {
    const initialProps = parseProps(this);
    if (!this._isMetricContent(initialProps)) {
      return;
    }
    if (isStaticSkeletonDisplay(initialProps)) {
      this.classList.add("mei-text--static-skeleton");
      return;
    }
    const runtimeRef = resolveRuntimeMetricRef(initialProps);
    if (!runtimeRef) {
      return;
    }
    const queryStateId = String(initialProps.query_state ?? initialProps.queryState ?? "page").trim() || "page";
    const refreshMetric = () => {
      if (!this.isConnected) return;
      const props = parseProps(this);
      if (!this._isMetricContent(props)) {
        return;
      }
      this._renderMetric(props);
      this._refreshMetricRuntime(props, queryStateId);
    };
    const refreshFromPreview = (event) => {
      if (!shouldReactToPreviewUpdated(event, this)) {
        return;
      }
      refreshMetric();
    };
    const refreshFromQueryState = () => {
      if (isDrilldownOverlayOpen() && !isRuntimeDrilldownOverlayElement(this)) {
        return;
      }
      refreshMetric();
    };
    this._previewHandler = refreshFromPreview;
    window.addEventListener("meilang:preview-updated", refreshFromPreview);
    this._queryUnsubscribe = subscribeQueryState(queryStateId, refreshFromQueryState);
    deferUntilDisplayed(this, refreshMetric);
  }

  async _refreshMetricRuntime(props, queryStateId) {
    const runtimeRef = resolveRuntimeMetricRef(props);
    if (!runtimeRef) {
      return;
    }
    const metricFetchMode =
      String(props.metric_fetch_mode ?? props.metricFetchMode ?? "panel").trim().toLowerCase() ||
      "panel";
    if (metricFetchMode === "slot") {
      if (this._metricAbort) {
        this._metricAbort.abort();
      }
      this._metricAbort = new AbortController();
    }
    const seq = ++this._metricRefreshSeq;
    try {
      const requestOptions = {
        queryStateId,
        signal: metricFetchMode === "slot" ? this._metricAbort.signal : undefined,
        meta: runtimeCallerMeta(this, "mei-text"),
      };
      const { metrics } =
        metricFetchMode === "slot"
          ? await fetchRuntimeMetrics(props, requestOptions)
          : await fetchPanelRuntimeMetrics(this, props, requestOptions);
      if (!this.isConnected || seq !== this._metricRefreshSeq) {
        return;
      }
      const metric = findRuntimeMetricInResults(metrics, runtimeRef);
      if (!metric) {
        return;
      }
      const display = metricDisplayFromScalar(metric, props);
      this._renderMetric(props, display);
      syncMetricCardPresentation(this, metric, props);
    } catch (error) {
      if (error?.name === "AbortError") {
        return;
      }
      this._renderMetric(props);
    }
  }

  _cleanupMetricBinding() {
    if (typeof this._queryUnsubscribe === "function") {
      this._queryUnsubscribe();
    }
    this._queryUnsubscribe = null;
    if (this._previewHandler) {
      window.removeEventListener("meilang:preview-updated", this._previewHandler);
    }
    this._previewHandler = null;
    if (this._metricAbort) {
      this._metricAbort.abort();
    }
    this._metricAbort = null;
    this._metricRefreshSeq += 1;
  }

  _renderMetric(props, display = null) {
    const role = String(props.metric_role || "").trim().toLowerCase();
    const resolvedDisplay = display ?? metricDisplayFromContent(props);
    const content = resolvedDisplay ? String(resolvedDisplay[role] ?? "") : "";
    const drilldown = role === "value" ? metricDrilldownMeta(props) : null;
    this._setDrilldownState(drilldown, resolvedDisplay);
    if (isStaticSkeletonDisplay(props)) {
      this.classList.add("mei-text--static-skeleton");
    } else {
      this.classList.remove("mei-text--static-skeleton");
    }
    this._render(
      { ...props, content, _mei_drilldown_clickable: Boolean(drilldown) },
      content,
      "plain",
    );
  }

  _setDrilldownState(meta, display) {
    this._drilldownMeta = meta || null;
    this._drilldownDisplay = display || null;
    const interactive = !!meta;
    this.toggleAttribute("data-mei-drilldown-active", interactive);
    if (interactive) {
      this.setAttribute("tabindex", "0");
      this.setAttribute("role", "button");
      this.setAttribute("aria-label", `查看指标明细：${meta.metric_id}`);
      this.setAttribute("data-mei-drilldown-scene", meta.scene_path || meta.scene_id || "");
      this.setAttribute("data-mei-drilldown-metric", meta.metric_id);
      this.setAttribute("data-mei-drilldown-dataset", meta.dataset_id);
      this.addEventListener("click", this._drilldownClickHandler);
      this.addEventListener("keydown", this._drilldownKeyHandler);
      return;
    }
    this.removeAttribute("tabindex");
    this.removeAttribute("role");
    this.removeAttribute("aria-label");
    this.removeAttribute("data-mei-drilldown-scene");
    this.removeAttribute("data-mei-drilldown-metric");
    this.removeAttribute("data-mei-drilldown-dataset");
    this.removeEventListener("click", this._drilldownClickHandler);
    this.removeEventListener("keydown", this._drilldownKeyHandler);
  }

  _handleDrilldownKey(event) {
    if (!this._drilldownMeta) {
      return;
    }
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }
    event.preventDefault();
    this._emitDrilldown(event);
  }

  _emitDrilldown(event) {
    if (!this._drilldownMeta) {
      return;
    }
    const panelId =
      this.closest?.("[data-mei-panel-id]")?.getAttribute?.("data-mei-panel-id") || "";
    const display = this._drilldownDisplay || {};
    const detail = {
      ...this._drilldownMeta,
      panel_id: String(panelId || "").trim(),
      value: String(display.value ?? ""),
      label: String(display.label ?? ""),
      unit: String(display.unit ?? ""),
      desc: String(display.desc ?? ""),
    };
    const dispatchDrilldown = (eventName) => {
      this.dispatchEvent(
        new CustomEvent(eventName, {
          bubbles: true,
          composed: true,
          detail: { ...detail },
        }),
      );
    };
    dispatchDrilldown(DRILLDOWN_EVENT_NAME);
    dispatchDrilldown(ANALYSIS_OPEN_EVENT_NAME);
    dispatchDrilldown(POPUP_OPEN_EVENT_NAME);
  }

  _render(props, content, format) {
    const metricRole = String(props.metric_role || "").trim().toLowerCase();
    const hasDescShell =
      metricRole === "desc" && (props.desc_shell || props.metric_desc_shell);
    const descShell = hasDescShell ? metricDescShellCss(props) : "";
    const typography = metricRole ? metricTypographyCss(props) : "";
    const hostLayout = metricRole ? metricHostLayoutCss(props, { descShell: hasDescShell }) : "";
    const fontSize = metricRole ? "" : fontSizeVar(props);
    const textAlign = metricRole ? "" : textAlignCss(props);
    const lineHeightRaw = String(props.lineHeight ?? props.line_height ?? "").trim();
    const lineHeight =
      lineHeightRaw &&
      (/^\d+(\.\d+)?$/.test(lineHeightRaw) ? lineHeightRaw : lineHeightRaw);
    const drilldownClickable = props._mei_drilldown_clickable === true;

    if (!this.shadowRoot) {
      this.attachShadow({ mode: "open" });
    }

    const drilldownBodyStyle = drilldownClickable
      ? "cursor: pointer; width: fit-content; max-width: 100%; margin-inline: auto;"
      : "";

    const plainColor = String(props.color || "").trim();
    const plainPadding = String(props.padding || "").trim();
    const plainBackground = String(props.background || "").trim();
    const plainBorder = String(props.border || "").trim();
    const plainRadius = String(props.radius || props.borderRadius || "").trim();
    const plainWhiteSpace = String(props.white_space || props.whiteSpace || "").trim();
    const overflowMode = String(props.overflow || props.text_overflow || "").trim().toLowerCase();
    const lineClampRaw = props.line_clamp ?? props.lineClamp ?? props.max_lines ?? props.maxLines;
    const lineClamp =
      !metricRole && lineClampRaw != null && String(lineClampRaw).trim() !== ""
        ? Math.max(1, Math.min(12, Number(lineClampRaw) || 0))
        : 0;
    const plainEllipsis =
      !metricRole &&
      lineClamp <= 0 &&
      (overflowMode === "ellipsis" ||
        props.ellipsis === true ||
        props.truncate === true);
    // 有背景/边框的芯片：chrome 画在 :host（铺满 slot），正文按内容高度居中，避免卡内大片空白。
    const fillChip = Boolean(plainBackground || plainBorder);
    const expandOptOut = falsyProp(props.expand ?? props.text_expand ?? props.textExpand);
    const expandOptIn = truthyProp(props.expand ?? props.text_expand ?? props.textExpand);
    const overflowExpand =
      !metricRole &&
      !drilldownClickable &&
      format !== "html" &&
      !expandOptOut &&
      (plainEllipsis || lineClamp > 0 || expandOptIn);
    const expandLabel =
      String(props.expand_label ?? props.expandLabel ?? "查看全文").trim() || "查看全文";
    const popoverTitle =
      String(props.popover_title ?? props.popoverTitle ?? expandLabel).trim() || "详细内容";
    const popoverVariant =
      String(props.popover_variant ?? props.popoverVariant ?? "large").trim().toLowerCase() ||
      "large";
    this._fullText = String(content ?? "");
    this._overflowExpand = overflowExpand;
    this._popoverTitle = popoverTitle;
    this._popoverVariant = popoverVariant;
    const effectiveLineHeight = lineHeight || (plainEllipsis || lineClamp ? "1.35" : "1.5");
    // 查看全文：与 table cell-shell 同构 —— 预览单行省略，… 固定在同行末尾，不换行另起一行。
    const previewCss = overflowExpand
      ? lineClamp > 1
        ? `overflow: hidden; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: ${lineClamp}; word-break: break-word; max-width: 100%; width: 100%; min-width: 0; max-height: calc(${effectiveLineHeight}em * ${lineClamp});`
        : "overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; width: 100%; min-width: 0;"
      : lineClamp
        ? `overflow: hidden; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: ${lineClamp}; word-break: break-word; max-width: 100%; width: 100%; flex: 0 1 auto; min-width: 0; min-height: 0;`
        : plainEllipsis
          ? "overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; width: 100%; flex: 0 1 auto; min-width: 0; min-height: 0;"
          : fillChip
            ? "word-break: break-word; width: 100%; flex: 0 1 auto; min-height: 0;"
            : "word-break: break-word; width: 100%; flex: 1 1 auto; min-height: 0;";
    const hostAlign = String(props.align_items || props.alignItems || "").trim().toLowerCase();
    const hostJustify = String(props.justify_content || props.justifyContent || "").trim().toLowerCase();
    const defaultChipJustify =
      fillChip || plainEllipsis || lineClamp ? "center" : "";
    const plainHostLayout = metricRole
      ? ""
      : `display: flex; flex-direction: column; width: 100%; height: 100%; min-height: 0; min-width: 0; box-sizing: border-box; overflow: hidden;${
          hostAlign ? ` align-items: ${hostAlign};` : " align-items: stretch;"
        }${
          hostJustify
            ? ` justify-content: ${hostJustify};`
            : defaultChipJustify
              ? ` justify-content: ${defaultChipJustify};`
              : ""
        }${plainPadding && fillChip ? ` padding: ${plainPadding};` : ""}${
          plainBackground && fillChip ? ` background: ${plainBackground};` : ""
        }${plainBorder && fillChip ? ` border: ${plainBorder};` : ""}${
          plainRadius && fillChip ? ` border-radius: ${plainRadius};` : ""
        }`;
    const slidesCtx = !metricRole ? resolveSlidesPresentationContext(this) : null;
    const slidesCss = slidesCtx ? slidesPresentationCss(slidesCtx.panel) : "";
    const baseTypography = metricRole
      ? typography
      : slidesCss
        ? ""
        : `
          line-height: ${effectiveLineHeight};
          font-size: ${fontSize};
          color: ${plainColor || "var(--mei-color-text-primary, #e2e8f0)"};
          ${textAlign ? `text-align: ${textAlign};` : ""}
          ${plainWhiteSpace ? `white-space: ${plainWhiteSpace};` : ""}
          ${plainPadding && !fillChip ? `padding: ${plainPadding};` : ""}
          ${plainBackground && !fillChip ? `background: ${plainBackground};` : ""}
          ${plainBorder && !fillChip ? `border: ${plainBorder};` : ""}
          ${plainRadius && !fillChip ? `border-radius: ${plainRadius};` : ""}
          ${overflowExpand ? "" : previewCss}
          box-sizing: border-box;
        `;

    this.shadowRoot.innerHTML = `
      <style>
        ${slidesCss}
        :host {
          ${
            slidesCss
              ? ""
              : metricRole
                ? hostLayout
                : plainHostLayout ||
                  "display: block; width: 100%; height: 100%; min-height: 0; min-width: 0; box-sizing: border-box; overflow: hidden;"
          }
        }
        :host([data-mei-drilldown-active="true"]:focus-visible) {
          outline: 1px solid rgba(125, 211, 252, 0.9);
          outline-offset: 2px;
          border-radius: 4px;
        }
        .mei-text-body {
          margin: 0;
          padding: 0;
          ${metricRole ? "word-break: break-word;" : ""}
          ${baseTypography}
          ${descShell}
          ${drilldownBodyStyle}
        }
        .mei-text-body.mei-text-label-distribute {
          display: flex;
          justify-content: space-between;
          align-items: center;
          width: 100%;
          white-space: nowrap;
          text-align: left;
          text-align-last: auto;
        }
        .mei-text-body.mei-text-label-distribute > span {
          flex: 0 0 auto;
        }
        .mei-text-body.mei-text-num-decimal {
          display: block;
          width: 100%;
          white-space: nowrap;
          text-align: right;
          font-variant-numeric: tabular-nums;
          overflow: visible;
        }
        .mei-text-body.mei-text-num-decimal.mei-text-num-decimal-split {
          display: grid;
          grid-template-columns: minmax(0, 1fr) 2.2ch;
          align-items: baseline;
          text-align: left;
        }
        .mei-text-body.mei-text-num-decimal-split > .mei-num-int {
          min-width: 0;
          text-align: right;
          overflow: hidden;
          text-overflow: clip;
        }
        .mei-text-body.mei-text-num-decimal-split > .mei-num-frac {
          text-align: left;
          font-variant-numeric: tabular-nums;
        }
        .mei-text-body :where(p, h1, h2, h3, ul, ol) {
          margin: 0 0 0.5em;
        }
        .mei-text-body :where(p:last-child, h1:last-child, h2:last-child, h3:last-child) {
          margin-bottom: 0;
        }
        .mei-text-shell {
          display: grid;
          grid-template-columns: minmax(0, 1fr) auto;
          align-items: ${lineClamp > 1 ? "end" : "center"};
          gap: 4px;
          width: 100%;
          max-width: 100%;
          min-width: 0;
          min-height: 0;
        }
        .mei-text-shell[data-expanded-hidden="true"] {
          grid-template-columns: minmax(0, 1fr);
        }
        .mei-text-preview {
          margin: 0;
          padding: 0;
          min-width: 0;
          ${baseTypography}
          ${previewCss}
        }
        button.mei-text-expand-btn {
          flex: 0 0 auto;
          display: none;
          align-items: center;
          justify-content: center;
          margin: 0;
          padding: 1px 7px;
          min-width: 22px;
          min-height: 20px;
          border-radius: 4px;
          border: 1px solid rgba(59, 130, 246, 0.55);
          background: rgba(37, 99, 235, 0.2);
          font: inherit;
          font-size: 12px;
          font-weight: 700;
          line-height: 1;
          letter-spacing: 0.02em;
          color: var(--mei-color-text-unit, #7dd3fc);
          cursor: pointer;
          white-space: nowrap;
        }
        button.mei-text-expand-btn[data-visible="true"] {
          display: inline-flex;
        }
        button.mei-text-expand-btn:hover {
          background: rgba(59, 130, 246, 0.38);
          border-color: rgba(147, 197, 253, 0.85);
          color: var(--mei-color-text-highlight, #e0f2fe);
        }
        button.mei-text-expand-btn:focus-visible {
          outline: 2px solid rgba(147, 197, 253, 0.9);
          outline-offset: 2px;
        }
      </style>
      ${
        overflowExpand
          ? `<div class="mei-text-shell" data-expanded-hidden="true">
              <div class="mei-text-preview"></div>
              <button type="button" class="mei-text-expand-btn" aria-label="${escapeHtml(
                expandLabel,
              )}">…</button>
            </div>`
          : `<div class="mei-text-body"></div>`
      }
    `;

    const body = this.shadowRoot.querySelector(
      overflowExpand ? ".mei-text-preview" : ".mei-text-body",
    );
    if (!body) return;

    const alignMode = textAlignCss(props);
    const labelChars =
      metricRole === "label" && alignMode === "justify" && format !== "html"
        ? Array.from(String(content ?? "").trim())
        : null;
    // 二字/三字标签拉开对齐四字槽；四字及以上保持紧排（如「政府采购」）。
    const distributeLabel = Boolean(labelChars && labelChars.length >= 2 && labelChars.length <= 3);
    const decimalValue =
      metricRole === "value" && alignMode === "decimal" && format !== "html";
    body.classList.remove(
      "mei-text-label-distribute",
      "mei-text-num-decimal",
      "mei-text-num-decimal-split",
    );
    if (format === "html") {
      body.innerHTML = content;
    } else if (distributeLabel) {
      body.classList.add("mei-text-label-distribute");
      body.innerHTML = labelChars.map((ch) => `<span>${escapeHtml(ch)}</span>`).join("");
    } else if (decimalValue) {
      const parts = splitDecimalDisplay(content);
      body.classList.add("mei-text-num-decimal");
      // 无小数：整格右齐，避免空小数槽把五位整数挤出容器；有小数再拆整数/小数槽。
      if (parts.fracPart) {
        body.classList.add("mei-text-num-decimal-split");
        body.innerHTML =
          `<span class="mei-num-int">${escapeHtml(parts.intPart)}${escapeHtml(parts.suffix)}</span>` +
          `<span class="mei-num-frac">${escapeHtml(parts.fracPart)}</span>`;
      } else {
        body.textContent = `${parts.intPart}${parts.suffix}`;
      }
    } else {
      body.textContent = content;
    }

    this._bindOverflowPreview(overflowExpand);
  }

  _cleanupOverflowPreview() {
    if (this._overflowRaf != null && typeof cancelAnimationFrame === "function") {
      cancelAnimationFrame(this._overflowRaf);
    }
    this._overflowRaf = null;
    if (this._overflowObserver) {
      try {
        this._overflowObserver.disconnect();
      } catch {
        /* ignore */
      }
    }
    this._overflowObserver = null;
    const btn = this.shadowRoot?.querySelector?.(".mei-text-expand-btn");
    if (btn) {
      btn.removeEventListener("click", this._expandClickHandler);
    }
  }

  _bindOverflowPreview(enabled) {
    this._cleanupOverflowPreview();
    if (!enabled || !this.shadowRoot) return;
    const btn = this.shadowRoot.querySelector(".mei-text-expand-btn");
    const body = this.shadowRoot.querySelector(".mei-text-preview");
    if (!btn || !body) return;
    btn.addEventListener("click", this._expandClickHandler);
    const sync = () => this._syncOverflowExpandButton();
    if (typeof ResizeObserver === "function") {
      this._overflowObserver = new ResizeObserver(() => sync());
      this._overflowObserver.observe(this);
      this._overflowObserver.observe(body);
    }
    if (typeof requestAnimationFrame === "function") {
      this._overflowRaf = requestAnimationFrame(() => {
        this._overflowRaf = requestAnimationFrame(sync);
      });
    } else {
      sync();
    }
  }

  _isTextOverflowing(node) {
    if (!(node instanceof HTMLElement) || node.clientWidth <= 0) return false;
    if (node.scrollWidth - node.clientWidth > 1) return true;
    if (node.scrollHeight - node.clientHeight > 1) return true;
    return false;
  }

  _syncOverflowExpandButton() {
    if (!this._overflowExpand || !this.shadowRoot) return;
    const shell = this.shadowRoot.querySelector(".mei-text-shell");
    const btn = this.shadowRoot.querySelector(".mei-text-expand-btn");
    const body = this.shadowRoot.querySelector(".mei-text-preview");
    if (!btn || !body) return;
    const full = String(this._fullText || "").trim();
    const show = full.length > 8 && this._isTextOverflowing(body);
    btn.dataset.visible = show ? "true" : "false";
    btn.setAttribute("aria-hidden", show ? "false" : "true");
    if (shell) {
      shell.dataset.expandedHidden = show ? "false" : "true";
    }
  }

  _openTextPopover(event) {
    event?.preventDefault?.();
    event?.stopPropagation?.();
    const fullText = String(this._fullText || "").trim();
    if (!fullText) return;
    this._closeTextPopover();
    ensureFloatingTextPopoverStyles();
    const large = String(this._popoverVariant || "large") !== "default";
    const title = String(this._popoverTitle || "详细内容");
    const backdrop = mountTextPopoverBackdrop(this);
    const pop = document.createElement("div");
    pop.className = `cell-pop${large ? " cell-pop--large" : ""}`;
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-modal", "true");
    pop.setAttribute("aria-label", title);
    pop.innerHTML = buildTextPopoverShellHtml({ title, subtitle: "", fullText }, escapeHtml);
    const defaultWidth = large ? 480 : 420;
    const anchor = event?.currentTarget || this;
    mountFloatingPopoverOnBody(pop, { width: defaultWidth });
    // 盖在同一角色层之上，避免被 backdrop 抢走命中。
    pop.style.zIndex = "calc(var(--mei-z-cockpit-text-popover, 2350) + 2)";
    this._textPopoverEl = pop;
    positionFloatingPopoverNearAnchor(pop, anchor, {
      topOffset: 8,
      defaultWidth,
    });
    this._textPopoverDragCleanup = bindFloatingPopoverDrag(
      pop,
      pop.querySelector(".cell-pop-drag-handle"),
    );

    const close = (ev) => {
      ev?.preventDefault?.();
      ev?.stopPropagation?.();
      this._closeTextPopover();
    };
    backdrop?.addEventListener("pointerdown", close);
    const onDoc = (ev) => {
      const path = ev.composedPath?.() || [];
      if (path.includes(pop) || path.includes(anchor) || path.includes(this)) return;
      this._closeTextPopover();
    };
    // 延后绑定，避免打开当下的同一次 pointerup/click 直接关闭。
    setTimeout(() => document.addEventListener("pointerdown", onDoc, true), 0);
    this._textPopoverDocCleanup = () => document.removeEventListener("pointerdown", onDoc, true);
    this._textPopoverKeydown = (ev) => {
      if (ev.key === "Escape") {
        ev.stopPropagation();
        this._closeTextPopover();
      }
    };
    document.addEventListener("keydown", this._textPopoverKeydown, true);
    pop.querySelector(".cell-pop-close")?.addEventListener("click", close);
    pop.querySelector(".cell-pop-copy")?.addEventListener("click", (ev) => {
      ev?.preventDefault?.();
      ev?.stopPropagation?.();
      copyTextToClipboard(fullText);
    });
    try {
      pop.querySelector(".cell-pop-close")?.focus();
    } catch {
      /* ignore */
    }
  }

  _closeTextPopover() {
    if (typeof this._textPopoverDragCleanup === "function") {
      try {
        this._textPopoverDragCleanup();
      } catch {
        /* ignore */
      }
      this._textPopoverDragCleanup = null;
    }
    if (typeof this._textPopoverDocCleanup === "function") {
      try {
        this._textPopoverDocCleanup();
      } catch {
        /* ignore */
      }
      this._textPopoverDocCleanup = null;
    }
    if (typeof this._textPopoverKeydown === "function") {
      try {
        document.removeEventListener("keydown", this._textPopoverKeydown, true);
      } catch {
        /* ignore */
      }
      this._textPopoverKeydown = null;
    }
    if (this._textPopoverEl?.isConnected) {
      try {
        this._textPopoverEl.remove();
      } catch {
        /* ignore */
      }
    }
    this._textPopoverEl = null;
    removeTextPopoverBackdrop(this);
  }
}

if (!customElements.get("mei-text")) {
  customElements.define("mei-text", MeiText);
}
