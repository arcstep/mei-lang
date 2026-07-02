import { color } from "../mei/theme-style.js";
import { resolveRuntimeColor } from "../cockpit/tokens.js";

/**
 * GIS layerSpec / joinSpec 共享工具（chart.geo 与 map.maplibre 共用）
 */

export const MEI_MAP_SELECTION = "mei:map-selection";

const JOIN_KEY_CANDIDATES = ["code", "id", "parkId", "enterpriseId", "name"];
const GEOJSON_CACHE = new Map();

function mapLibrePaintColor(value, tokenName, fallback = "#cbd5e1") {
  const host = typeof document !== "undefined" ? document.documentElement : null;
  const resolved = resolveRuntimeColor(host, value, tokenName);
  if (resolved && !String(resolved).startsWith("var(")) {
    return String(resolved);
  }
  const text = String(value || "");
  const match = text.match(/,\s*(#[0-9a-f]{3,8}|rgba?\([^)]+\))\s*\)/i);
  if (match) {
    return match[1];
  }
  return fallback;
}

export function resolveFeatureJoinKey(properties, preferredKey) {
  const props = properties && typeof properties === "object" ? properties : {};
  const key = String(preferredKey || "").trim();
  if (key && props[key] != null && props[key] !== "") {
    return { joinKey: key, code: String(props[key]) };
  }
  for (const candidate of JOIN_KEY_CANDIDATES) {
    if (props[candidate] != null && props[candidate] !== "") {
      return { joinKey: candidate, code: String(props[candidate]) };
    }
  }
  return { joinKey: key || "code", code: "" };
}

export function resolveLayerJoinKey(layerSpec) {
  return String(layerSpec?.joinKey || layerSpec?.idField || "").trim() || "code";
}

export async function fetchGeoJson(url) {
  const target = String(url || "").trim();
  if (!target) {
    throw new Error("缺少 geojsonUrl");
  }
  if (!GEOJSON_CACHE.has(target)) {
    GEOJSON_CACHE.set(
      target,
      fetch(target, { credentials: "same-origin" })
        .then((response) => {
          if (!response.ok) {
            throw new Error(`GeoJSON 加载失败 (${response.status}): ${target}`);
          }
          return response.json();
        })
        .then((data) => {
          if (!data || data.type !== "FeatureCollection" || !Array.isArray(data.features)) {
            throw new Error("GeoJSON 须为 FeatureCollection");
          }
          return data;
        })
        .catch((error) => {
          GEOJSON_CACHE.delete(target);
          throw error;
        }),
    );
  }
  return GEOJSON_CACHE.get(target);
}

export function ensureFeatureCollection(data) {
  if (!data || data.type !== "FeatureCollection" || !Array.isArray(data.features)) {
    throw new Error("GeoJSON 须为 FeatureCollection");
  }
  return data;
}

function resolveLayerBoundValue(props, layerSpec, keys = []) {
  const list = Array.isArray(keys) ? keys : [keys];
  for (const key of list) {
    const inlineValue = layerSpec?.[key];
    if (inlineValue != null) {
      return inlineValue;
    }
    const normalized = String(key || "")
      .replace(/[A-Z]/g, (m) => `_${m.toLowerCase()}`)
      .replace(/^_/, "");
    const keyFromProps =
      layerSpec?.[`${key}Key`] ||
      layerSpec?.[`${key}_key`] ||
      layerSpec?.[`${normalized}Key`] ||
      layerSpec?.[`${normalized}_key`];
    if (!keyFromProps) continue;
    const value = props?.[String(keyFromProps)];
    if (value != null) {
      return value;
    }
  }
  return null;
}

export function resolveLayerDataPayload(props, layerSpec = {}) {
  const payload = {};
  const layerValueByCode = resolveLayerBoundValue(props, layerSpec, ["valueByCode", "value_by_code"]);
  if (layerValueByCode && typeof layerValueByCode === "object" && !Array.isArray(layerValueByCode)) {
    payload.valueByCode = layerValueByCode;
  }
  const specValueByCode = layerSpec.valueByCode || layerSpec.value_by_code;
  if (specValueByCode && typeof specValueByCode === "object" && !Array.isArray(specValueByCode)) {
    payload.valueByCode = { ...(payload.valueByCode || {}), ...specValueByCode };
  }
  const layerData = resolveLayerBoundValue(props, layerSpec, ["value", "data", "dataset"]);
  const dataKey = String(layerSpec.dataKey || layerSpec.data_key || "").trim();
  const metricPayload =
    layerData != null
      ? layerData
      : dataKey && props?.[dataKey] != null
        ? props[dataKey]
        : null;
  if (metricPayload != null) {
    payload.value = metricPayload;
    payload.data = metricPayload;
    payload.dataset = metricPayload;
  }
  if (layerSpec.valueField || layerSpec.value_field) {
    payload.valueField = layerSpec.valueField || layerSpec.value_field;
  }
  if (layerSpec.codeField || layerSpec.code_field) {
    payload.codeField = layerSpec.codeField || layerSpec.code_field;
  }
  if (layerSpec.joinKey || layerSpec.idField) {
    payload.joinKey = layerSpec.joinKey || layerSpec.idField;
  }
  return payload;
}

function parseCoordinateValue(raw) {
  if (Array.isArray(raw) && raw.length >= 2) {
    const lng = Number(raw[0]);
    const lat = Number(raw[1]);
    return Number.isFinite(lng) && Number.isFinite(lat) ? [lng, lat] : null;
  }
  if (typeof raw !== "string") {
    return null;
  }
  const text = raw.trim();
  if (!text) {
    return null;
  }
  const parts = text
    .split(/[,\s，;；]+/)
    .map((part) => Number(part))
    .filter((value) => Number.isFinite(value));
  if (parts.length < 2) {
    return null;
  }
  return [parts[0], parts[1]];
}

function resolvePointFromRow(row, geometryMapping = {}) {
  const lngField =
    geometryMapping.lngField ||
    geometryMapping.lonField ||
    geometryMapping.longitudeField ||
    geometryMapping.lng_field ||
    geometryMapping.lon_field ||
    geometryMapping.longitude_field ||
    "lng";
  const latField =
    geometryMapping.latField ||
    geometryMapping.latitudeField ||
    geometryMapping.lat_field ||
    geometryMapping.latitude_field ||
    "lat";
  const coordField =
    geometryMapping.coordField ||
    geometryMapping.coordinateField ||
    geometryMapping.coord_field ||
    geometryMapping.coordinate_field ||
    "";
  const lng = Number(row?.[lngField]);
  const lat = Number(row?.[latField]);
  if (Number.isFinite(lng) && Number.isFinite(lat)) {
    return [lng, lat];
  }
  if (coordField) {
    return parseCoordinateValue(row?.[coordField]);
  }
  return null;
}

export function rowsToPointFeatureCollection(rows, geometryMapping = {}, layerSpec = {}) {
  const features = [];
  const sourceRows = Array.isArray(rows) ? rows : [];
  const idField = String(layerSpec.idField || layerSpec.id_field || "").trim();
  for (const row of sourceRows) {
    if (!row || typeof row !== "object") continue;
    const point = resolvePointFromRow(row, geometryMapping);
    if (!point) continue;
    const feature = {
      type: "Feature",
      geometry: {
        type: "Point",
        coordinates: point,
      },
      properties: { ...row },
    };
    if (idField && row[idField] != null && row[idField] !== "") {
      feature.id = row[idField];
    }
    features.push(feature);
  }
  return {
    type: "FeatureCollection",
    features,
  };
}

function resolveLayerFeatureMatch(layerSpec, props = {}) {
  return (
    layerSpec?.featureMatch ||
    layerSpec?.feature_match ||
    resolveLayerBoundValue(props, layerSpec, ["featureMatch", "feature_match"])
  );
}

function featureMatches(feature, matcher) {
  if (!matcher || typeof matcher !== "object" || Array.isArray(matcher)) {
    return true;
  }
  const props = feature?.properties && typeof feature.properties === "object" ? feature.properties : {};
  return Object.entries(matcher).every(([key, expected]) => {
    const actual = props[key];
    if (Array.isArray(expected)) {
      return expected.some((value) => String(actual ?? "").trim() === String(value ?? "").trim());
    }
    return String(actual ?? "").trim() === String(expected ?? "").trim();
  });
}

function filterFeatureCollection(featureCollection, matcher) {
  if (!matcher || typeof matcher !== "object" || Array.isArray(matcher)) {
    return featureCollection;
  }
  const features = Array.isArray(featureCollection?.features) ? featureCollection.features : [];
  return {
    ...featureCollection,
    features: features.filter((feature) => featureMatches(feature, matcher)),
  };
}

export async function resolveLayerSource(layerSpec, props = {}) {
  const featureMatch = resolveLayerFeatureMatch(layerSpec, props);
  const inlineFeatureCollection =
    layerSpec?.featureCollection ||
    layerSpec?.feature_collection ||
    resolveLayerBoundValue(props, layerSpec, ["featureCollection", "feature_collection"]);
  if (inlineFeatureCollection) {
    return filterFeatureCollection(ensureFeatureCollection(inlineFeatureCollection), featureMatch);
  }
  const type = String(layerSpec?.type || "polygon").trim().toLowerCase();
  // `addLayerSpec` 已先调用 `resolveLayerDataPayload`；此处直接消费 layer payload。
  const rows = resolveMetricRows(props);
  const geometryMapping = layerSpec?.geometryMapping || layerSpec?.geometry_mapping || {};
  if (type === "point" && rows.length > 0) {
    return filterFeatureCollection(
      rowsToPointFeatureCollection(rows, geometryMapping, layerSpec),
      featureMatch,
    );
  }
  const url = String(layerSpec?.url || "").trim();
  if (url) {
    return filterFeatureCollection(await fetchGeoJson(url), featureMatch);
  }
  return {
    type: "FeatureCollection",
    features: [],
  };
}

/**
 * 从 rows / valueByCode / geo_map_spec 合并为 code -> number
 */
export function normalizeJoinCode(raw) {
  if (raw == null || raw === "") return "";
  if (typeof raw === "string") return raw.trim();
  if (typeof raw === "number") {
    if (!Number.isSafeInteger(raw)) return "";
    return String(raw);
  }
  return String(raw).trim();
}

export function buildValueByCode(props, joinKey = "code") {
  const key = String(joinKey || "code").trim() || "code";
  const map = {};

  const direct = props.valueByCode || props.value_by_code;
  if (direct && typeof direct === "object" && !Array.isArray(direct)) {
    Object.assign(map, direct);
  }

  const spec = props.geoMapSpec || props.geo_map_spec;
  if (spec?.value_by_code && typeof spec.value_by_code === "object") {
    Object.assign(map, spec.value_by_code);
  }

  const rows = resolveMetricRows(props);
  const valueField = String(props.valueField || props.mapping?.y?.[0]?.field || "value").trim();
  const codeField = String(props.codeField || props.joinKey || key).trim() || key;
  for (const row of rows) {
    if (!row || typeof row !== "object") continue;
    const code = normalizeJoinCode(row[codeField] ?? row.code ?? row.id ?? row.name);
    if (!code) continue;
    const raw = row[valueField] ?? row.value;
    const num = Number(raw);
    if (!Number.isNaN(num)) {
      map[code] = num;
    }
  }
  return map;
}

export function resolveMetricRows(props) {
  const candidates = [props.data, props.value, props.dataset];
  for (const source of candidates) {
    if (!source || typeof source !== "object") continue;
    if (Array.isArray(source.rows)) return source.rows;
    if (Array.isArray(source.value)) return source.value;
    if (source.dataset && Array.isArray(source.dataset.rows)) {
      return source.dataset.rows;
    }
  }
  if (props.dataset && Array.isArray(props.dataset.rows)) {
    return props.dataset.rows;
  }
  return [];
}

/** 从任意 prop 值解析 `metric_ref` 运行时引用（用于 map layer `dataKey`）。 */
export function resolveRuntimeMetricRefFromValue(value) {
  const ref = value?.__mei_runtime_ref;
  if (ref && ref.kind === "metric" && ref.dataset_id && ref.metric_id) {
    return ref;
  }
  return null;
}

/** 收集 `mapSpec.layers[]` 中通过 `dataKey` 绑定的 metric 引用。 */
export function collectMapLayerMetricRefs(layers, props = {}) {
  const refs = [];
  const seen = new Set();
  for (const layer of Array.isArray(layers) ? layers : []) {
    const dataKey = String(layer?.dataKey || layer?.data_key || "").trim();
    if (!dataKey) continue;
    const ref = resolveRuntimeMetricRefFromValue(props?.[dataKey]);
    if (!ref?.metric_id) continue;
    const metricId = String(ref.metric_id).trim();
    if (!metricId || seen.has(metricId)) continue;
    seen.add(metricId);
    refs.push({ dataKey, ref, metricId });
  }
  return refs;
}

export function mapLayersNeedRuntimeMetrics(layers, props = {}) {
  return collectMapLayerMetricRefs(layers, props).length > 0;
}

/** 将 runtime metric 查询结果转为图层 `resolveMetricRows` 可消费的 payload。 */
export function mapLayerMetricPayloadFromResult(metric) {
  if (!metric || typeof metric !== "object") return null;
  const rows = extractMetricResultRows(metric);
  if (rows.length > 0) {
    return { rows, value: rows, dataset: { rows } };
  }
  if (
    metric.shape === "scalar_map" &&
    metric.value &&
    typeof metric.value === "object" &&
    !Array.isArray(metric.value)
  ) {
    return { valueByCode: metric.value, value: metric.value };
  }
  return metric.value != null ? { value: metric.value } : null;
}

export function extractMetricResultRows(metric) {
  if (!metric || typeof metric !== "object") return [];
  if (metric.shape === "dataframe" && Array.isArray(metric.value)) {
    return metric.value;
  }
  if (Array.isArray(metric.value)) {
    return metric.value;
  }
  if (metric.value && typeof metric.value === "object" && Array.isArray(metric.value.rows)) {
    return metric.value.rows;
  }
  return [];
}

/** 将批量 metric 查询结果写回 `dataKey` 字段，供 map 图层消费。 */
export function buildMapLayerMetricPropsPatch(props, layers, metricResults, findMetricFn) {
  const patch = {};
  const metrics = Array.isArray(metricResults?.metrics) ? metricResults.metrics : [];
  for (const { dataKey, ref } of collectMapLayerMetricRefs(layers, props)) {
    const metric = typeof findMetricFn === "function" ? findMetricFn(metrics, ref) : null;
    const payload = mapLayerMetricPayloadFromResult(metric);
    if (payload) {
      patch[dataKey] = payload;
    }
  }
  return patch;
}

export function choroplethRange(valueMap, palette) {
  const values = Object.values(valueMap)
    .map((v) => Number(v))
    .filter((v) => !Number.isNaN(v));
  const min = values.length ? Math.min(...values) : 0;
  const max = values.length ? Math.max(...values) : 1;
  const colors = Array.isArray(palette) && palette.length >= 2
    ? palette
    : ["#14243a", color("chart_2")];
  return { min, max, colors };
}

export function valueToColor(value, min, max, colors) {
  const num = Number(value);
  if (Number.isNaN(num)) {
    return colors[0];
  }
  if (max <= min) {
    return colors[colors.length - 1];
  }
  const t = Math.max(0, Math.min(1, (num - min) / (max - min)));
  return interpolateHex(colors[0], colors[colors.length - 1], t);
}

function isFalseyFlag(value) {
  return value === false || value === 0 || String(value ?? "").trim().toLowerCase() === "false";
}

function isTruthyFlag(value) {
  return value === true || value === 1 || String(value ?? "").trim().toLowerCase() === "true";
}

/** 从图层标题推断弹窗/标注中的数值文案。 */
export function inferLayerMetricLabel(layerSpec = {}) {
  const text = String(layerSpec?.label || layerSpec?.id || "").trim();
  if (/检查次数/.test(text)) return "检查次数";
  if (/处罚次数/.test(text)) return "处罚次数";
  if (/处罚金额/.test(text)) return "处罚金额";
  return String(layerSpec?.valueLabel || layerSpec?.value_label || "数值").trim() || "数值";
}

export function formatMapMetricValue(raw, layerSpec = {}) {
  if (raw == null || raw === "") return "";
  const num = Number(raw);
  if (Number.isNaN(num)) return String(raw);
  if (/金额/.test(String(layerSpec?.label || ""))) {
    if (Math.abs(num) >= 10000) {
      const wan = Math.round((num / 10000) * 100) / 100;
      return `${wan}万`;
    }
    return Number.isInteger(num) ? String(num) : String(Math.round(num * 100) / 100);
  }
  return Number.isInteger(num) ? String(num) : String(Math.round(num * 100) / 100);
}

/**
 * 业务层地图标注（名称 + 指标值）。choropleth 图层默认开启，可用 dataLabels.enabled 关闭。
 */
export function resolveLayerDataLabels(layerSpec = {}, options = {}) {
  const raw = layerSpec?.dataLabels ?? layerSpec?.data_labels ?? null;
  const choroplethOn = options?.choroplethOn === true;
  let enabled = choroplethOn;
  if (raw != null) {
    if (typeof raw === "boolean") {
      enabled = raw;
    } else if (typeof raw === "object") {
      if (isFalseyFlag(raw.enabled)) enabled = false;
      else if (isTruthyFlag(raw.enabled)) enabled = true;
    }
  }
  const style = (raw && typeof raw === "object" ? raw.style : null) || layerSpec?.style || {};
  return {
    enabled,
    labelField: String(
      (raw && typeof raw === "object" ? raw.labelField || raw.label_field : null) || "name",
    ).trim() || "name",
    valueField: String(
      (raw && typeof raw === "object" ? raw.valueField || raw.value_field : null) || "value",
    ).trim() || "value",
    showName:
      raw && typeof raw === "object"
        ? raw.showName !== false && raw.show_name !== false
        : true,
    showValue:
      raw && typeof raw === "object"
        ? raw.showValue !== false && raw.show_value !== false
        : true,
    minZoom: Number((raw && typeof raw === "object" ? raw.minZoom ?? raw.min_zoom : null) ?? 10),
    maxZoom: Number((raw && typeof raw === "object" ? raw.maxZoom ?? raw.max_zoom : null) ?? 22),
    textSize: Number(style.labelTextSize ?? style.label_text_size ?? 11),
    textColor: mapLibrePaintColor(
      style.labelColor || style.label_color || color("text_body"),
      "text_body",
      "#cbd5e1",
    ),
    textHaloColor: mapLibrePaintColor(
      style.labelHaloColor || style.label_halo_color || "#0f172a",
      "text_body",
      "#0f172a",
    ),
    textHaloWidth: Number(style.labelHaloWidth ?? style.label_halo_width ?? 1.2),
    valueLabel: String(
      (raw && typeof raw === "object" ? raw.valueLabel || raw.value_label : null) ||
        inferLayerMetricLabel(layerSpec),
    ).trim(),
  };
}

function buildMapLabelText(name, displayValue, dataLabels) {
  const parts = [];
  if (dataLabels.showName && name) parts.push(name);
  if (dataLabels.showValue && displayValue) parts.push(displayValue);
  return parts.join("\n");
}

/** 为 choropleth / 点位要素写入 value、__mei_value、__mei_label_text 等展示字段。 */
export function enrichFeatureWithLayerMetrics(feature, { joinKey, valueMap, layerSpec, dataLabels }) {
  const properties = { ...(feature?.properties || {}) };
  const resolved = resolveFeatureJoinKey(properties, joinKey);
  let code = normalizeJoinCode(resolved.code);
  if (!code) {
    code = normalizeJoinCode(properties[dataLabels?.labelField] ?? properties.name);
  }
  const rawValue = valueMap?.[code];
  const name = String(properties[dataLabels.labelField] ?? properties.name ?? code ?? "").trim();
  let displayValue = "";
  if (rawValue != null && rawValue !== "" && !Number.isNaN(Number(rawValue))) {
    const num = Number(rawValue);
    properties[dataLabels.valueField] = num;
    properties.value = num;
    displayValue = formatMapMetricValue(num, layerSpec);
    properties.__mei_value = displayValue;
    properties.__mei_metric_label = dataLabels.valueLabel;
  }
  if (name) {
    properties.__mei_name = name;
  }
  const labelText = buildMapLabelText(name, displayValue, dataLabels);
  if (labelText) {
    properties.__mei_label_text = labelText;
  }
  return {
    ...feature,
    properties,
  };
}

export function enrichGeoJsonWithLayerMetrics(geojson, options = {}) {
  const features = (geojson?.features || []).map((feature) =>
    enrichFeatureWithLayerMetrics(feature, options),
  );
  return {
    type: "FeatureCollection",
    features,
  };
}

function interpolateHex(a, b, t) {
  const parse = (hex) => {
    const h = hex.replace("#", "");
    return [
      parseInt(h.slice(0, 2), 16),
      parseInt(h.slice(2, 4), 16),
      parseInt(h.slice(4, 6), 16),
    ];
  };
  const [r1, g1, b1] = parse(a);
  const [r2, g2, b2] = parse(b);
  const r = Math.round(r1 + (r2 - r1) * t);
  const g = Math.round(g1 + (g2 - g1) * t);
  const bl = Math.round(b1 + (b2 - b1) * t);
  return `#${[r, g, bl].map((x) => x.toString(16).padStart(2, "0")).join("")}`;
}

export function dispatchMapSelection(detail) {
  window.dispatchEvent(
    new CustomEvent(MEI_MAP_SELECTION, {
      detail,
      bubbles: true,
    }),
  );
}

/** 从 mei-lang SSR 注入的 meta 读取默认 Martin 地址（可被 .mei 中 mapSpec 覆盖） */
function resolveTilesBaseUrl(raw) {
  const base = String(raw || "").trim();
  if (!base) {
    return "";
  }
  if (base.startsWith("@same-host")) {
    if (typeof window === "undefined") {
      return "";
    }
    const suffix = base.slice("@same-host".length);
    const protocol = window.location.protocol || "http:";
    const hostname = window.location.hostname || "127.0.0.1";
    if (!suffix) {
      return `${protocol}//${hostname}`;
    }
    if (suffix.startsWith(":")) {
      return `${protocol}//${hostname}${suffix}`;
    }
    return `${protocol}//${hostname}/${suffix.replace(/^\/+/, "")}`;
  }
  return base;
}

export function readHostGisTilesDefaults() {
  if (typeof document === "undefined") {
    return {
      tilesUrl: "/gis",
      tilesJsonPath: "/demo-tiles",
    };
  }
  const base =
    document.querySelector('meta[name="mei-tiles-base-url"]')?.getAttribute("content")?.trim() ||
    "";
  const path =
    document.querySelector('meta[name="mei-tiles-json-path"]')?.getAttribute("content")?.trim() ||
    "";
  return {
    tilesUrl: resolveTilesBaseUrl(base) || "/gis",
    tilesJsonPath: path || "/demo-tiles",
  };
}

export function normalizeMapSpec(props) {
  const mapSpec = props.mapSpec || props.map || {};
  const hostDefaults = readHostGisTilesDefaults();
  const basemap = Object.assign(
    {
      tilesUrl: hostDefaults.tilesUrl,
      tilesJsonPath: hostDefaults.tilesJsonPath,
      minZoom: 10,
      maxZoom: 18,
      center: [105.0, 35.0],
      defaultZoom: 11,
    },
    mapSpec.basemap || {},
  );
  const layers = Array.isArray(mapSpec.layers)
    ? mapSpec.layers
    : Array.isArray(props.layers)
      ? props.layers
      : [];
  return { basemap, layers };
}

const DEFAULT_GLYPHS = "/workspace-components/vendor/maplibre/fonts/{fontstack}/{range}.pbf";
const LABEL_FONT = ["Open Sans Regular", "Arial Unicode MS Regular"];
export const MAP_DATA_LABEL_FONT = LABEL_FONT;

/** 底图标注文字：默认简中优先，避免 name:latin / name:en 抢在 name 前面变成拼音 */
export function textFieldForBasemap(basemap = {}) {
  const locale = String(basemap.labelLocale || basemap.label_locale || "zh-Hans")
    .trim()
    .toLowerCase();
  if (locale === "latin" || locale === "en" || locale === "english") {
    return ["coalesce", ["get", "name:latin"], ["get", "name:en"], ["get", "name"]];
  }
  return [
    "coalesce",
    ["get", "name:zh-Hans"],
    ["get", "name:zh"],
    ["get", "name"],
  ];
}

/** 至少有一种可显示的中文/本地名才打标 */
function hasLocalNameFilter() {
  return ["any", ["has", "name:zh-Hans"], ["has", "name:zh"], ["has", "name"]];
}

/** OpenMapTiles transportation / transportation_name 的 class 预设 */
export const ROAD_CLASS_PRESETS = {
  /** 高速、国道、城市快速路/主干（不含次干及以下） */
  major: [
    "motorway",
    "trunk",
    "primary",
    "motorway_construction",
    "trunk_construction",
    "primary_construction",
  ],
  /** major + 次干道 */
  arterial: [
    "motorway",
    "trunk",
    "primary",
    "secondary",
    "motorway_construction",
    "trunk_construction",
    "primary_construction",
    "secondary_construction",
  ],
};

/**
 * 解析 basemap 路网等级过滤。
 * - 未配置 / `"all"`：不过滤（显示 MBTiles 内全部道路）
 * - `"major"` / `"arterial"`：预设
 * - `majorRoadsOnly: true` 等价于 `"major"`
 * - 字符串数组：自定义 class 列表
 * - `"none"`：不绘制路网线
 */
export function resolveBasemapRoadClasses(basemap = {}) {
  const raw =
    basemap.roadClasses ??
    basemap.road_classes ??
    (basemap.majorRoadsOnly === true || basemap.major_roads_only === true ? "major" : null);
  if (raw === false || raw == null || raw === "all") {
    return null;
  }
  if (raw === "none" || raw === "off") {
    return [];
  }
  if (typeof raw === "string") {
    const key = raw.trim().toLowerCase();
    if (ROAD_CLASS_PRESETS[key]) {
      return ROAD_CLASS_PRESETS[key];
    }
    return [raw];
  }
  if (Array.isArray(raw)) {
    return raw.map((c) => String(c).trim()).filter(Boolean);
  }
  return null;
}

/** MapLibre filter：仅保留指定 class；classes 为空数组时隐藏全部道路 */
export function filterByRoadClasses(classes) {
  if (classes == null) {
    return null;
  }
  if (classes.length === 0) {
    return ["==", ["get", "class"], ""];
  }
  return ["in", ["get", "class"], ["literal", classes]];
}

function combineFilters(...parts) {
  const filters = parts.filter(Boolean);
  if (filters.length === 0) {
    return undefined;
  }
  if (filters.length === 1) {
    return filters[0];
  }
  return ["all", ...filters];
}

function basemapValue(basemap, camelKey, snakeKey, fallback) {
  const value = basemap?.[camelKey] ?? basemap?.[snakeKey];
  return value != null && value !== "" ? value : fallback;
}

/** OpenMapTiles 矢量层上的路网/水系/地名/POI 标注（需 Martin + 对应 MBTiles） */
export function basemapLabelLayers(basemap = {}) {
  const textField = textFieldForBasemap(basemap);
  const roadClasses = resolveBasemapRoadClasses(basemap);
  const roadClassFilter = filterByRoadClasses(roadClasses);
  const waterLabelColor = mapLibrePaintColor(
    basemapValue(basemap, "waterLabelColor", "water_label_color", color("text_unit")),
    "text_unit",
    "#94a3b8",
  );
  const waterLabelOpacity = basemapValue(basemap, "waterLabelOpacity", "water_label_opacity", 1);
  const waterLabelHaloColor = basemapValue(basemap, "waterLabelHaloColor", "water_label_halo_color", "#0f172a");
  const waterLabelHaloWidth = basemapValue(basemap, "waterLabelHaloWidth", "water_label_halo_width", 1.2);
  const roadLabelColor = mapLibrePaintColor(
    basemapValue(basemap, "roadLabelColor", "road_label_color", color("text_body")),
    "text_body",
    "#cbd5e1",
  );
  const roadLabelOpacity = basemapValue(basemap, "roadLabelOpacity", "road_label_opacity", 1);
  const roadLabelHaloColor = basemapValue(basemap, "roadLabelHaloColor", "road_label_halo_color", "#0f172a");
  const roadLabelHaloWidth = basemapValue(basemap, "roadLabelHaloWidth", "road_label_halo_width", 1);
  const placeLabelColor = mapLibrePaintColor(
    basemapValue(basemap, "placeLabelColor", "place_label_color", color("text_inverse")),
    "text_inverse",
    "#f8fafc",
  );
  const placeLabelOpacity = basemapValue(basemap, "placeLabelOpacity", "place_label_opacity", 1);
  const placeLabelHaloColor = basemapValue(basemap, "placeLabelHaloColor", "place_label_halo_color", "#0f172a");
  const placeLabelHaloWidth = basemapValue(basemap, "placeLabelHaloWidth", "place_label_halo_width", 1.2);
  const poiLabelColor = basemapValue(basemap, "poiLabelColor", "poi_label_color", "#fde68a");
  const poiLabelOpacity = basemapValue(basemap, "poiLabelOpacity", "poi_label_opacity", 1);
  const poiLabelHaloColor = basemapValue(basemap, "poiLabelHaloColor", "poi_label_halo_color", "#0f172a");
  const poiLabelHaloWidth = basemapValue(basemap, "poiLabelHaloWidth", "poi_label_halo_width", 1);
  return [
    {
      id: "mei-label-water",
      type: "symbol",
      source: "osm",
      "source-layer": "water_name",
      minzoom: 10,
      layout: {
        "text-field": textField,
        "text-font": LABEL_FONT,
        "text-size": 12,
      },
      filter: hasLocalNameFilter(),
      paint: {
        "text-color": waterLabelColor,
        "text-opacity": Number(waterLabelOpacity),
        "text-halo-color": waterLabelHaloColor,
        "text-halo-width": Number(waterLabelHaloWidth),
      },
    },
    {
      id: "mei-label-road",
      type: "symbol",
      source: "osm",
      "source-layer": "transportation_name",
      minzoom: 12,
      filter: combineFilters(hasLocalNameFilter(), roadClassFilter),
      layout: {
        "text-field": textField,
        "text-font": LABEL_FONT,
        "text-size": ["interpolate", ["linear"], ["zoom"], 12, 10, 16, 13],
        "symbol-placement": "line",
        "text-max-angle": 30,
      },
      paint: {
        "text-color": roadLabelColor,
        "text-opacity": Number(roadLabelOpacity),
        "text-halo-color": roadLabelHaloColor,
        "text-halo-width": Number(roadLabelHaloWidth),
      },
    },
    {
      id: "mei-label-place",
      type: "symbol",
      source: "osm",
      "source-layer": "place",
      minzoom: 10,
      filter: hasLocalNameFilter(),
      layout: {
        "text-field": textField,
        "text-font": LABEL_FONT,
        "text-size": ["interpolate", ["linear"], ["zoom"], 10, 11, 14, 14],
      },
      paint: {
        "text-color": placeLabelColor,
        "text-opacity": Number(placeLabelOpacity),
        "text-halo-color": placeLabelHaloColor,
        "text-halo-width": Number(placeLabelHaloWidth),
      },
    },
    {
      id: "mei-label-poi",
      type: "symbol",
      source: "osm",
      "source-layer": "poi",
      minzoom: 14,
      filter: hasLocalNameFilter(),
      layout: {
        "text-field": textField,
        "text-font": LABEL_FONT,
        "text-size": 11,
        "text-offset": [0, 0.6],
      },
      paint: {
        "text-color": poiLabelColor,
        "text-opacity": Number(poiLabelOpacity),
        "text-halo-color": poiLabelHaloColor,
        "text-halo-width": Number(poiLabelHaloWidth),
      },
    },
  ];
}

export function buildBasemapStyle(basemap) {
  const hostDefaults = readHostGisTilesDefaults();
  const tilesUrl = String(basemap.tilesUrl || hostDefaults.tilesUrl).replace(/\/$/, "");
  const tilesJson = basemap.tilesJsonPath || hostDefaults.tilesJsonPath;
  const roadClasses = resolveBasemapRoadClasses(basemap);
  const roadClassFilter = filterByRoadClasses(roadClasses);
  const backgroundColor = basemapValue(basemap, "backgroundColor", "background_color", "#0a1628");
  const waterColor = basemapValue(basemap, "waterColor", "water_color", "#1e3a5f");
  const waterwayColor = basemapValue(basemap, "waterwayColor", "waterway_color", "#2563eb");
  const waterwayOpacity = basemapValue(basemap, "waterwayOpacity", "waterway_opacity", 1);
  const waterwayWidth = basemapValue(basemap, "waterwayWidth", "waterway_width", 1);
  const landuseColor = basemapValue(basemap, "landuseColor", "landuse_color", "#14243a");
  const landuseOpacity = basemapValue(basemap, "landuseOpacity", "landuse_opacity", 0.6);
  const roadColor = basemapValue(basemap, "roadColor", "road_color", color("chart_2"));
  const roadOpacity = basemapValue(basemap, "roadOpacity", "road_opacity", 1);
  const buildingColor = basemapValue(basemap, "buildingColor", "building_color", "#334155");
  const buildingOpacity = basemapValue(basemap, "buildingOpacity", "building_opacity", 0.5);
  const roadsLayer = {
    id: "roads",
    type: "line",
    source: "osm",
    "source-layer": "transportation",
    paint: {
      "line-color": roadColor,
      "line-opacity": Number(roadOpacity),
      "line-width": ["interpolate", ["linear"], ["zoom"], 10, 0.6, 14, 2.2],
    },
  };
  if (roadClassFilter) {
    roadsLayer.filter = roadClassFilter;
  }
  const baseLayers = [
    {
      id: "background",
      type: "background",
      paint: { "background-color": backgroundColor },
    },
    {
      id: "water",
      type: "fill",
      source: "osm",
      "source-layer": "water",
      paint: { "fill-color": waterColor },
    },
    {
      id: "waterway",
      type: "line",
      source: "osm",
      "source-layer": "waterway",
      paint: {
        "line-color": waterwayColor,
        "line-opacity": Number(waterwayOpacity),
        "line-width": Number(waterwayWidth),
      },
    },
    {
      id: "landuse",
      type: "fill",
      source: "osm",
      "source-layer": "landuse",
      paint: {
        "fill-color": landuseColor,
        "fill-opacity": Number(landuseOpacity),
      },
    },
    roadsLayer,
    {
      id: "buildings",
      type: "fill",
      source: "osm",
      "source-layer": "building",
      paint: {
        "fill-color": buildingColor,
        "fill-opacity": Number(buildingOpacity),
      },
    },
  ];
  return {
    version: 8,
    glyphs: String(basemap.glyphs || DEFAULT_GLYPHS),
    sources: {
      osm: {
        type: "vector",
        url: `${tilesUrl}${tilesJson}`,
      },
    },
    layers: baseLayers,
  };
}
