/** 驾驶舱底图观察窗：focusInset 解析与 cockpitBleed 布局（map / basemap-stage 共用）。 */

import { resolveRuntimeStyleValue } from "./tokens.js";

export function cssLength(value, fallback) {
  if (value == null || value === "") {
    return fallback;
  }
  if (typeof value === "number" && Number.isFinite(value)) {
    return `${value}px`;
  }
  const text = String(value).trim();
  return text || fallback;
}

export function cssLengthToPx(length) {
  const text = String(length || "").trim();
  const match = text.match(/^([\d.]+)px$/);
  if (match) {
    return Number(match[1]);
  }
  return 0;
}

function isUnresolvedMeiRef(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  return "__var" in value || "__member" in value || "__call" in value;
}

function hasUnresolvedMeiRefs(value) {
  if (!value || typeof value !== "object") {
    return false;
  }
  if (Array.isArray(value)) {
    return value.some(hasUnresolvedMeiRefs);
  }
  if (isUnresolvedMeiRef(value)) {
    return true;
  }
  return Object.values(value).some(hasUnresolvedMeiRefs);
}

function buildFocusInsetResult({
  mode = "",
  top,
  right,
  bottom,
  left,
  showFocusGuide = false,
  focusFrameBorder = null,
  focusFrameRadius = "4px",
}) {
  if (top === "0px" && right === "0px" && bottom === "0px" && left === "0px") {
    return null;
  }
  return {
    mode,
    top,
    right,
    bottom,
    left,
    showFocusGuide,
    focusFrameBorder,
    focusFrameRadius,
    focusInsetPx: {
      top: cssLengthToPx(top),
      right: cssLengthToPx(right),
      bottom: cssLengthToPx(bottom),
      left: cssLengthToPx(left),
    },
  };
}

/** 从 T1 stage-aperture-frame 实测观察窗，弥补 SSR 未展开的 geo.FOCUS_INSET。 */
export function measureFocusInsetFromAperture(host) {
  const stage = resolveCockpitStageSurface(host);
  if (!stage) {
    return null;
  }
  const frame = stage.querySelector('[data-mei-panel-name="stage-aperture-frame"]');
  if (!(frame instanceof HTMLElement)) {
    return null;
  }
  const designW = stage.offsetWidth || 1920;
  const designH = stage.offsetHeight || 1080;
  const stageRect = stage.getBoundingClientRect();
  const frameRect = frame.getBoundingClientRect();
  if (stageRect.width <= 0 || stageRect.height <= 0) {
    return null;
  }
  const scaleX = designW / stageRect.width;
  const scaleY = designH / stageRect.height;
  const top = Math.max(0, Math.round((frameRect.top - stageRect.top) * scaleY));
  const left = Math.max(0, Math.round((frameRect.left - stageRect.left) * scaleX));
  const right = Math.max(0, Math.round((stageRect.right - frameRect.right) * scaleX));
  const bottom = Math.max(0, Math.round((stageRect.bottom - frameRect.bottom) * scaleY));
  return buildFocusInsetResult({
    mode: "cockpitBleed",
    top: `${top}px`,
    right: `${right}px`,
    bottom: `${bottom}px`,
    left: `${left}px`,
  });
}

/** 驾驶舱全幅底图 + 中间观察区：解析 focusInset */
export function resolveMapFocusInset(props, basemap = {}, host = null) {
  const mapSpec = props.mapSpec || props.map || {};
  const raw =
    props.mapViewport ||
    props.map_viewport ||
    props.mapFocusInset ||
    props.map_focus_inset ||
    mapSpec.mapViewport ||
    mapSpec.map_viewport ||
    mapSpec.focusInset ||
    mapSpec.focus_inset ||
    basemap.mapViewport ||
    basemap.focusInset;
  if (!raw || typeof raw !== "object") {
    return measureFocusInsetFromAperture(host);
  }
  const inset = raw.focusInset || raw.focus_inset || raw;
  if (hasUnresolvedMeiRefs(raw) || hasUnresolvedMeiRefs(inset)) {
    const measured = measureFocusInsetFromAperture(host);
    if (measured) {
      return measured;
    }
  }
  const top = cssLength(inset.top ?? raw.top, "0px");
  const right = cssLength(inset.right ?? raw.right, "0px");
  const bottom = cssLength(inset.bottom ?? raw.bottom, "0px");
  const left = cssLength(inset.left ?? raw.left, "0px");
  if (top === "0px" && right === "0px" && bottom === "0px" && left === "0px") {
    return measureFocusInsetFromAperture(host);
  }
  const mode = String(raw.mode || raw.layoutMode || raw.layout_mode || "").trim();
  const explicitGuideOff =
    props.showFocusGuide === false ||
    props.show_focus_guide === false ||
    raw.showFocusGuide === false ||
    raw.show_focus_guide === false;
  const explicitGuideOn =
    props.showFocusGuide === true ||
    props.show_focus_guide === true ||
    props.showFocusFrame === true ||
    raw.showFocusGuide === true ||
    raw.show_focus_guide === true ||
    raw.showFocusFrame === true;
  const frameBorderRaw =
    props.focusFrameBorder ??
    props.focus_frame_border ??
    inset.border ??
    inset.frameBorder ??
    inset.frame_border ??
    raw.focusFrameBorder ??
    raw.focus_frame_border;
  let focusFrameBorder = null;
  if (frameBorderRaw != null && String(frameBorderRaw).trim() !== "") {
    const text = String(frameBorderRaw).trim();
    if (text !== "none" && text !== "false") {
      focusFrameBorder = text;
    }
  }
  const focusFrameRadius = cssLength(
    inset.borderRadius ??
      inset.frameRadius ??
      inset.frame_radius ??
      raw.focusFrameRadius ??
      raw.focus_frame_radius,
    "4px",
  );
  const showFocusGuide =
    !explicitGuideOff && (explicitGuideOn || focusFrameBorder != null);
  if (showFocusGuide && !focusFrameBorder) {
    focusFrameBorder = "2px dashed #facc15";
  }
  return buildFocusInsetResult({
    mode,
    top,
    right,
    bottom,
    left,
    showFocusGuide,
    focusFrameBorder,
    focusFrameRadius,
  });
}

/** 访问态 contain 缩放后，将设计稿 focusInset 换算为视口坐标 */
export function resolveCockpitStageMetrics(host) {
  if (typeof window === "undefined") {
    return null;
  }
  const stage =
    host?.closest?.(".preview-stage.preview-surface") ||
    document.querySelector(".preview-stage.preview-surface");
  if (!stage) {
    return null;
  }
  const rect = stage.getBoundingClientRect();
  const stageStyle = window.getComputedStyle(stage);
  const rootStyle = window.getComputedStyle(document.documentElement);
  const designW =
    Number.parseFloat(stageStyle.getPropertyValue("--mei-viewport-design-width")) ||
    Number.parseFloat(rootStyle.getPropertyValue("--mei-viewport-design-width")) ||
    1920;
  const designH =
    Number.parseFloat(stageStyle.getPropertyValue("--mei-viewport-design-height")) ||
    Number.parseFloat(rootStyle.getPropertyValue("--mei-viewport-design-height")) ||
    1080;
  const scale = Math.min(rect.width / designW, rect.height / designH);
  const contentW = designW * scale;
  const contentH = designH * scale;
  return {
    scale,
    designW,
    designH,
    offsetX: rect.left + (rect.width - contentW) / 2,
    offsetY: rect.top + (rect.height - contentH) / 2,
  };
}

/** 将设计稿 focusInset 换算为视口坐标矩形（与 T1 contain 缩放后的观察窗对齐） */
export function focusInsetViewportRect(metrics, focusInsetPx) {
  if (!metrics || !focusInsetPx) {
    return null;
  }
  const top = Number(focusInsetPx.top) || 0;
  const right = Number(focusInsetPx.right) || 0;
  const bottom = Number(focusInsetPx.bottom) || 0;
  const left = Number(focusInsetPx.left) || 0;
  const { scale, offsetX, offsetY, designW, designH } = metrics;
  const stageRight = offsetX + designW * scale;
  const stageBottom = offsetY + designH * scale;
  return {
    top: offsetY + top * scale,
    left: offsetX + left * scale,
    right: stageRight - right * scale,
    bottom: stageBottom - bottom * scale,
  };
}

/** 访问态舞台表面（与 T0/T1 共用 transform scale 的设计稿根） */
export function resolveCockpitStageSurface(host) {
  const fromHost = host?.closest?.(".preview-stage.preview-surface");
  if (fromHost instanceof HTMLElement) {
    return fromHost;
  }
  const shell = host?.closest?.(".preview-stage-shell");
  const fromShell = shell?.querySelector?.(".preview-stage.preview-surface");
  if (fromShell instanceof HTMLElement) {
    return fromShell;
  }
  const found = document.querySelector(".preview-stage.preview-surface");
  return found instanceof HTMLElement ? found : null;
}

/** 视口坐标 → 舞台设计稿坐标（供 stage 内 overlay 定位） */
export function clientPointToStageLocal(stage, clientX, clientY) {
  if (!(stage instanceof HTMLElement)) {
    return { left: clientX, top: clientY };
  }
  const rect = stage.getBoundingClientRect();
  const designW = stage.offsetWidth || 1920;
  const designH = stage.offsetHeight || 1080;
  const scaleX = rect.width > 0 ? designW / rect.width : 1;
  const scaleY = rect.height > 0 ? designH / rect.height : 1;
  return {
    left: (clientX - rect.left) * scaleX,
    top: (clientY - rect.top) * scaleY,
  };
}

export function focusInsetCssVars(focusInset) {
  if (!focusInset) {
    return "";
  }
  return [
    `--map-focus-top:${focusInset.top}`,
    `--map-focus-right:${focusInset.right}`,
    `--map-focus-bottom:${focusInset.bottom}`,
    `--map-focus-left:${focusInset.left}`,
  ].join(";");
}

/** 观察窗描边（basemap 组件内）：实线/虚线均用 border + border-box。首层 chrome 框见 stage-aperture-frame。 */
export function focusFrameGuideStyle(border, radius = "4px", host = null) {
  const text = String(border || "").trim();
  if (!text || text === "none" || text === "false") {
    return "";
  }
  const resolvedBorder =
    host instanceof Element ? resolveRuntimeStyleValue(host, text) : text;
  const radiusText = String(radius || "4px").trim() || "4px";
  return `border:${resolvedBorder};border-radius:${radiusText};box-sizing:border-box;background:transparent;`;
}

export function applyFocusFrameGuide(guide, layout, host = null) {
  if (!guide || !layout) {
    return;
  }
  guide.hidden = !layout.cockpitBleed || !layout.showFocusGuide;
  if (guide.hidden || !layout.focusFrameBorder) {
    guide.style.cssText = "";
    return;
  }
  guide.style.cssText = focusFrameGuideStyle(
    layout.focusFrameBorder,
    layout.focusFrameRadius,
    host ?? guide.parentElement,
  );
}

export function resolveCockpitBleedLayout(props, basemap = {}) {
  const focusInset = resolveMapFocusInset(props, basemap);
  const fill =
    props.mapFill === true ||
    props.mapFill === "true" ||
    String(props.mapHeight ?? "").trim() === "100%";
  const mode = String(
    props.mapLayoutMode || props.map_layout_mode || focusInset?.mode || "",
  ).trim();
  const cockpitBleed =
    mode === "cockpitBleed" ||
    mode === "cockpit_bleed" ||
    (focusInset != null && fill);

  if (cockpitBleed && focusInset) {
    const vars = focusInsetCssVars(focusInset);
    return {
      fill: true,
      cockpitBleed: true,
      focusInset,
      focusInsetPx: focusInset.focusInsetPx,
      showFocusGuide: focusInset.showFocusGuide,
      focusFrameBorder: focusInset.focusFrameBorder,
      focusFrameRadius: focusInset.focusFrameRadius,
      host: `display:block;width:100%;height:100%;min-height:0;${vars}`,
      wrap: "position:relative;width:100%;height:100%;min-height:0;overflow:hidden;",
      content: "position:absolute;inset:0;width:100%;height:100%;overflow:hidden;",
      toolbarPos:
        "top:calc(var(--map-focus-top) + 8px);right:calc(var(--map-focus-right) + 8px);",
      statusPos:
        "left:calc(var(--map-focus-left) + 12px);bottom:calc(var(--map-focus-bottom) + 10px);",
    };
  }

  if (fill) {
    return {
      fill: true,
      cockpitBleed: false,
      host: "display:block;width:100%;height:100%;min-height:0;",
      wrap: "position:relative;width:100%;height:100%;min-height:0;overflow:hidden;",
      content: "position:absolute;inset:0;width:100%;height:100%;overflow:hidden;",
      toolbarPos: "top:8px;right:8px;",
      statusPos: "left:12px;bottom:10px;",
    };
  }

  const height = Number(props.mapHeight) > 0 ? Number(props.mapHeight) : 420;
  return {
    fill: false,
    cockpitBleed: false,
    host: "display:block;width:100%;min-width:0;",
    wrap: `position:relative;height:${height}px;overflow:hidden;`,
    content: `width:100%;height:${height}px;overflow:hidden;`,
    toolbarPos: "top:8px;right:8px;",
    statusPos: "left:12px;bottom:10px;",
  };
}
