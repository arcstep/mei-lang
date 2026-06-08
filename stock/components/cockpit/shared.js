/** 驾驶舱 Web Component 通用工具（props 解析、转义、资源路径等）。 */

const PARSED_DATA_PROPS_CACHE = new WeakMap();

export function parseProps(element) {
  if (!(element instanceof Element)) {
    return {};
  }
  const raw = String(element.getAttribute("data-props") || "");
  const cached = PARSED_DATA_PROPS_CACHE.get(element);
  if (cached && cached.raw === raw) {
    return cached.value;
  }
  let value = {};
  try {
    value = raw ? JSON.parse(raw) : {};
  } catch {
    value = {};
  }
  PARSED_DATA_PROPS_CACHE.set(element, { raw, value });
  return value;
}

export function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

/** JSON for `data-props="..."` */
export function escapeAttr(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

export function rowsOf(prop) {
  const d = prop?.dataset ?? prop;
  if (!d || typeof d !== "object") return [];
  return Array.isArray(d.rows) ? d.rows : [];
}

export function pad2(n) {
  return String(n).padStart(2, "0");
}

export function formatNowParts() {
  const d = new Date();
  const weekdays = ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"];
  return {
    time: `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`,
    date: `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`,
    weekday: weekdays[d.getDay()],
  };
}

export function cockpitAsset(props, key) {
  const assets = props?.assets;
  if (!assets || typeof assets !== "object") {
    return "";
  }
  const value = assets[key];
  return typeof value === "string" ? value : "";
}
