import {
  deferUntilDisplayed,
  fetchPanelRuntimeMetrics,
  fetchRuntimeMetrics,
  findRuntimeMetricInResults,
  parseProps,
  resolveRuntimeMetricRef,
  runtimeCallerMeta,
  subscribeQueryState,
} from "../dataset/runtime-query.js";
import {
  ANALYSIS_OPEN_EVENT_NAME,
  DRILLDOWN_EVENT_NAME,
  POPUP_OPEN_EVENT_NAME,
  tableDrilldownMeta,
} from "../cockpit/drilldown-meta.js";
import { formatMetricNumber } from "./metric-number-format.js";

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

function metricDisplayFromContent(props) {
  const content = metricContentObject(props);
  if (!content) {
    return null;
  }
  if (content.shape === "scalar") {
    return metricDisplayFromScalar(content, props);
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
  return "";
}

function justifyContentFromTextAlign(align) {
  if (align === "left") return "flex-start";
  if (align === "right") return "flex-end";
  return "center";
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

function fontSizeVar(props) {
  const key = String(props.font || "").trim();
  if (!key) {
    const role = String(props.metric_role || "").trim().toLowerCase();
    if (role === "label" || role === "value" || role === "unit" || role === "desc") {
      const tier =
        String(props.metric_variant || "").trim().toLowerCase() === "sub"
          ? "metric-sub"
          : "metric";
      return `var(--mei-${tier}-${role}-font-size, inherit)`;
    }
    return "inherit";
  }
  if (/^\d+(\.\d+)?(px|rem|em|%)$/.test(key)) {
    return key;
  }
  return `var(--mei-font-${key}, 14px)`;
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
  const rawBg = shell.background ?? "#C9E9F8";
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
    align-items: stretch;
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
  return `
    ${metricBodyClipCss(role)}
    font-size: ${fontSizeVar(props)};
    font-family: var(${prefix}-font-family, inherit);
    color: var(${prefix}-color, var(--mei-color-text-primary, #e2e8f0));
    font-weight: var(${prefix}-font-weight, inherit);
    letter-spacing: var(${prefix}-letter-spacing, normal);
    line-height: ${lineHeight || `var(${prefix}-line-height, 1.2)`};
    text-align: ${align || `var(${prefix}-text-align, inherit)`};
  `;
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
  }

  connectedCallback() {
    this._bind();
  }

  disconnectedCallback() {
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
    const runtimeRef = resolveRuntimeMetricRef(initialProps);
    if (!runtimeRef) {
      return;
    }
    const queryStateId = String(initialProps.query_state ?? initialProps.queryState ?? "page").trim() || "page";
    const refresh = () => {
      if (!this.isConnected) return;
      const props = parseProps(this);
      if (!this._isMetricContent(props)) {
        return;
      }
      this._renderMetric(props);
      this._refreshMetricRuntime(props, queryStateId);
    };
    this._previewHandler = refresh;
    window.addEventListener("meilang:preview-updated", refresh);
    this._queryUnsubscribe = subscribeQueryState(queryStateId, refresh);
    deferUntilDisplayed(this, refresh);
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
    this.dispatchEvent(
      new CustomEvent(DRILLDOWN_EVENT_NAME, {
        bubbles: true,
        composed: true,
        detail,
      }),
    );
    this.dispatchEvent(
      new CustomEvent(ANALYSIS_OPEN_EVENT_NAME, {
        bubbles: true,
        composed: true,
        detail,
      }),
    );
    this.dispatchEvent(
      new CustomEvent(POPUP_OPEN_EVENT_NAME, {
        bubbles: true,
        composed: true,
        detail,
      }),
    );
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

    const baseTypography = metricRole
      ? typography
      : `
          line-height: ${lineHeight || "1.5"};
          font-size: ${fontSize};
          color: var(--mei-color-text-primary, #e2e8f0);
          ${textAlign ? `text-align: ${textAlign}; width: 100%;` : ""}
        `;

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          ${metricRole ? hostLayout : "display: block; width: 100%; box-sizing: border-box;"}
          ${drilldownClickable ? "cursor: pointer;" : ""}
        }
        :host([data-mei-drilldown-active="true"]:focus-visible) {
          outline: 1px solid rgba(125, 211, 252, 0.9);
          outline-offset: 2px;
          border-radius: 4px;
        }
        .mei-text-body {
          margin: 0;
          padding: 0;
          word-break: break-word;
          ${baseTypography}
          ${descShell}
        }
        .mei-text-body :where(p, h1, h2, h3, ul, ol) {
          margin: 0 0 0.5em;
        }
        .mei-text-body :where(p:last-child, h1:last-child, h2:last-child, h3:last-child) {
          margin-bottom: 0;
        }
      </style>
      <div class="mei-text-body"></div>
    `;

    const body = this.shadowRoot.querySelector(".mei-text-body");
    if (!body) return;

    if (format === "html") {
      body.innerHTML = content;
    } else {
      body.textContent = content;
    }
  }
}

if (!customElements.get("mei-text")) {
  customElements.define("mei-text", MeiText);
}
