function parseObjectLike(raw, fallback) {
  if (!raw) return fallback;
  if (typeof raw === "string") {
    const text = raw.trim();
    if (!text) return fallback;
    try {
      return JSON.parse(text);
    } catch (_) {
      return fallback;
    }
  }
  if (typeof raw === "object") {
    return raw;
  }
  return fallback;
}

function isTruthyFlag(value) {
  if (value === true || value === 1) return true;
  const text = String(value ?? "").trim().toLowerCase();
  return text === "true" || text === "yes" || text === "1";
}

function isFalseyFlag(value) {
  if (value === false || value === 0) return true;
  const text = String(value ?? "").trim().toLowerCase();
  return text === "false" || text === "no" || text === "0";
}

function normalizeType(value) {
  const text = String(value || "").trim().toLowerCase();
  if (!text) return null;
  if (["number", "numeric", "currency"].includes(text)) return "number";
  if (["percent", "percentage", "pct"].includes(text)) return "percent";
  if (["date"].includes(text)) return "date";
  if (["datetime", "timestamp", "time", "relative_time", "relative-time"].includes(text)) {
    return text === "time" ? "datetime" : text.replace("-", "_");
  }
  if (["text", "string"].includes(text)) return "text";
  return text;
}

function normalizeRule(rule) {
  const op = String(rule?.op || rule?.operator || "").trim().toLowerCase();
  const tone = String(rule?.tone || "").trim().toLowerCase();
  if (!op || !tone) return null;
  return {
    op,
    tone,
    value: rule?.value ?? null,
  };
}

export function normalizeColumnFormats(raw) {
  const parsed = parseObjectLike(raw, {});
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
  const out = {};
  for (const [key, value] of Object.entries(parsed)) {
    const normalizedKey = String(key || "").trim();
    if (!normalizedKey || !value || typeof value !== "object") continue;
    const precision = Number(value.precision);
    const maxChars = Number(value.maxChars ?? value.max_chars);
    const align = String(value.align || "").trim().toLowerCase();
    out[normalizedKey] = {
      type: normalizeType(value.type),
      precision: Number.isFinite(precision) ? Math.max(0, Math.min(precision, 8)) : null,
      percentInput: value.percentInput ?? value.percentMode ?? value.percent_input ?? null,
      maxChars: Number.isFinite(maxChars) ? Math.max(0, Math.floor(maxChars)) : null,
      truncate: isTruthyFlag(value.truncate) ? true : isFalseyFlag(value.truncate) ? false : null,
      compact: value.compact === true || value.compact === "true",
      useGrouping:
        value.useGrouping === false || value.useGrouping === "false"
          ? false
          : value.thousandSeparator === false || value.thousandSeparator === "false"
            ? false
            : true,
      relative: value.relative === true || value.relative === "true",
      tag: value.tag === true || value.tag === "true",
      kind: String(value.kind || "").trim() || null,
      emptyText: value.emptyText != null ? String(value.emptyText) : "",
      align: ["left", "center", "right"].includes(align) ? align : null,
    };
  }
  return out;
}

export function normalizeColumnRules(raw) {
  const parsed = parseObjectLike(raw, {});
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
  const out = {};
  for (const [key, value] of Object.entries(parsed)) {
    const normalizedKey = String(key || "").trim();
    if (!normalizedKey) continue;
    const rules = Array.isArray(value) ? value : Array.isArray(value?.rules) ? value.rules : [];
    out[normalizedKey] = rules.map(normalizeRule).filter(Boolean);
  }
  return out;
}

function metaType(meta) {
  return String(meta?.type_name || meta?.type || "").trim().toLowerCase();
}

export function isDepartmentLikeColumnKey(key) {
  return /(部门|单位|机构|组织)/i.test(String(key || ""));
}

export function isTagLikeColumnKey(key) {
  const name = String(key || "").trim();
  if (!name) return false;
  // 风险/预警等级改走三色块，不再当作普通 tag 胶囊。
  if (name === "风险等级" || name === "预警等级" || name === "级别" || name === "level") return false;
  return /等级|类型|类别|状态|是否|面貌/.test(name);
}

export function isWarningLevelBlocksColumn(descriptor) {
  const kind = String(descriptor?.format?.kind || descriptor?.kind || "")
    .trim()
    .toLowerCase()
    .replace(/-/g, "_");
  if (
    kind === "warning_level_blocks" ||
    kind === "risk_level_blocks" ||
    kind === "warning_level_block" ||
    kind === "alert_level_block"
  ) {
    return true;
  }
  const key = String(descriptor?.key || "").trim();
  return key === "风险等级" || key === "预警等级" || key === "级别" || key === "level";
}

/** 序号列（含「xxx序号」）；展示居中，不当作右对齐数值列。 */
export function isSerialNumberColumnKey(key) {
  const name = String(key || "").trim();
  return Boolean(name) && (name === "序号" || name.endsWith("序号"));
}

/** 业务主键/编号列（预警ID、问题跟踪ID 等）；排除「是否*」布尔列与序号列。 */
export function isIdentifierLikeColumnKey(key) {
  const name = String(key || "").trim();
  if (!name || /是否/.test(name)) return false;
  if (isSerialNumberColumnKey(name)) return false;
  if (/^id$/i.test(name)) return true;
  return /ID$/i.test(name) || /编号$/.test(name) || /编码$/.test(name);
}

export function isLongTextColumnKey(key) {
  if (isDepartmentLikeColumnKey(key)) return false;
  const name = String(key || "").trim();
  // 短标题列（风险事项/预警模型）按内容列，不吃 fr 留白。
  if (/^(风险事项|监督事项|预警模型|事项名称)$/.test(name)) return false;
  return /(notes|note|remark|comment|desc|description|memo|summary|content|备注|说明|摘要|内容|描述|表现|问题|事项|原因|依据|措施|意见|详情)/i.test(
    name,
  );
}

export function foldHeaderKey(key) {
  return String(key || "")
    .replace(/\s+/g, "")
    .replace(/（[^）]*）/g, "");
}

function columnFormatClampsWidth(descriptor) {
  const format = descriptor?.format || {};
  if (isFalseyFlag(format.truncate)) return false;
  if (isDepartmentLikeColumnKey(descriptor?.key)) return false;
  if (isTruthyFlag(format.truncate)) return true;
  return isLongTextColumnKey(descriptor?.key);
}

function normalizeWidthMode(state) {
  const raw = String(state?.width_mode ?? state?.widthMode ?? "").trim().toLowerCase();
  if (raw === "fixed" || state?.width_fixed === true || state?.widthFixed === true) return "fixed";
  if (raw === "max") return "max";
  if (raw === "content" || raw === "fit" || raw === "auto") return "content";
  return "min";
}

export function isCompactDisplayType(type) {
  const text = String(type || "").trim().toLowerCase().replace("-", "_");
  return (
    text === "number" ||
    text === "percent" ||
    text === "date" ||
    text === "datetime" ||
    text === "relative_time" ||
    text === "time"
  );
}

/** 列轨尽量随内容变宽（数字/时间/tag 等）；长文本 truncate 列除外。 */
export function columnPrefersContentWidth(descriptor) {
  if (!descriptor || descriptor.layoutClamp) return false;
  if (descriptor.widthMode === "fixed" && descriptor.layoutFixedWidth) return false;
  if (descriptor.widthMode === "content") return true;
  return columnIsContentSizedSemantic(descriptor);
}

/** 语义上应按内容测宽的列（忽略作者 width_mode=fixed 锁定）。 */
function columnIsContentSizedSemantic(descriptor) {
  if (!descriptor) return false;
  if (isWarningLevelBlocksColumn(descriptor)) return false;
  if (descriptor.tag || isTagLikeColumnKey(descriptor?.key)) return true;
  if (isSerialNumberColumnKey(descriptor?.key)) return true;
  if (isIdentifierLikeColumnKey(descriptor?.key)) return true;
  if (/(部门|单位|主责)/.test(String(descriptor?.key || ""))) return true;
  if (/时间$|日期$/.test(String(descriptor?.key || ""))) return true;
  // 短标题业务名列：按内容测宽，不作长文弹性。
  if (/^(风险事项|监督事项|预警模型)$/.test(String(descriptor?.key || "").trim())) return true;
  if (descriptor.widthMode === "content") return true;
  return isCompactDisplayType(descriptor.type);
}

/** 语义上应吃剩余宽度的长文列（测宽只定下限，不锁死 px）。 */
function columnIsFlexFillSemantic(descriptor) {
  if (!descriptor || columnIsContentSizedSemantic(descriptor)) return false;
  const key = String(descriptor?.key || "");
  if (isAddressLikeColumnKey(key)) return true;
  if (descriptor.layoutClamp || columnFormatClampsWidth(descriptor)) return true;
  if (isLongTextColumnKey(key)) return true;
  return false;
}

function finalizeColumnLayout(entry) {
  const state = entry.state || {};
  const explicitMode = String(entry?.widthMode || "").trim().toLowerCase();
  const mode = explicitMode || normalizeWidthMode(state);
  const width = entry.width;
  let minWidth = entry.layoutMinWidth ?? entry.minWidth;
  let maxWidth = entry.layoutMaxWidth ?? entry.maxWidth;
  let fixedWidth = entry.layoutFixedWidth ?? null;

  if (!fixedWidth && mode === "fixed" && width) {
    fixedWidth = width;
  } else if (mode === "content" && width) {
    minWidth = minWidth ?? width;
  } else if (width) {
    minWidth = minWidth ?? width;
  }
  // 类别/类型胶囊按短枚举抬底：作者若把 fixed 写太窄，至少保证常见 4～6 字标签可读。
  if (
    fixedWidth &&
    (entry.tag === true || isTagLikeColumnKey(entry.key)) &&
    /类型|类别/.test(String(entry.key || ""))
  ) {
    fixedWidth = Math.max(Number(fixedWidth) || 0, 152);
  }
  if (mode === "max" && width && !maxWidth) {
    maxWidth = width;
  }

  const layoutClamp = columnFormatClampsWidth(entry);
  if (layoutClamp && !fixedWidth) {
    maxWidth = maxWidth ?? Math.max(minWidth || 120, 320);
  }

  return {
    ...entry,
    widthMode: mode,
    layoutMinWidth: minWidth,
    layoutMaxWidth: maxWidth,
    layoutFixedWidth: fixedWidth,
    layoutClamp,
  };
}

function guessTypeFromKey(key) {
  const text = String(key || "").trim().toLowerCase();
  if (!text) return "text";
  if (/(^|_)(date|day|日期)($|_)/.test(text)) return "date";
  if (/(^|_)(time|datetime|timestamp|created_at|updated_at|occurred_at|时刻)($|_)/.test(text)) {
    return "datetime";
  }
  if (/(^|_)(date|day|日期)($|_)/.test(text)) return "date";
  if (/时间$/.test(text)) return "date";
  if (/(completion|percent|ratio|rate|比例|进度|完成率)/.test(text)) {
    return "percent";
  }
  if (/(amount|count|score|price|num|qty|total|金额|数量|数值)/.test(text)) {
    return "number";
  }
  return "text";
}

function inferType(key, meta, format) {
  if (format?.type) return format.type;
  // 序号常为 integer/string 混排（如 5-1）；按文本处理，避免右对齐与数值格式化。
  if (isSerialNumberColumnKey(key)) return "text";
  const typeName = metaType(meta);
  if (/(int|float|double|decimal|number|numeric)/.test(typeName)) return "number";
  if (/(datetime|timestamp|time)/.test(typeName)) return "datetime";
  if (/(date)/.test(typeName)) return "date";
  return guessTypeFromKey(key);
}

function defaultAlignForType(type) {
  if (type === "number" || type === "percent") return "right";
  return "left";
}

function defaultAlignForColumn(key, type) {
  if (isSerialNumberColumnKey(key)) return "center";
  return defaultAlignForType(type);
}

function toPlainText(value) {
  if (value == null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  try {
    return JSON.stringify(value);
  } catch (_) {
    return String(value);
  }
}

export function parseDateValue(raw) {
  const text = toPlainText(raw).trim();
  if (!text) return null;
  if (/^\d{4}[-/]\d{1,2}[-/]\d{1,2}(?:[ T]\d{1,2}:\d{1,2}(?::\d{1,2})?)?/.test(text)) {
    const normalized = text.replace(/\//g, "-").replace(" ", "T");
    const date = new Date(normalized);
    return Number.isNaN(date.getTime()) ? null : date;
  }
  if (/^\d{8}$/.test(text)) {
    const year = Number(text.slice(0, 4));
    const month = Number(text.slice(4, 6));
    const day = Number(text.slice(6, 8));
    const date = new Date(Date.UTC(year, month - 1, day));
    return Number.isNaN(date.getTime()) ? null : date;
  }
  const numeric = Number(text);
  if (!Number.isFinite(numeric)) return null;
  if (text.length >= 12 && numeric > 946656000000) {
    const date = new Date(numeric);
    return Number.isNaN(date.getTime()) ? null : date;
  }
  if (text.length === 10 && numeric > 946656000) {
    const date = new Date(numeric * 1000);
    return Number.isNaN(date.getTime()) ? null : date;
  }
  if (numeric > 20000 && numeric < 60000) {
    const dayMs = 24 * 60 * 60 * 1000;
    const millis = Date.UTC(1899, 11, 30) + Math.floor(numeric) * dayMs;
    const date = new Date(millis);
    return Number.isNaN(date.getTime()) ? null : date;
  }
  return null;
}

export function formatRelativeTime(date) {
  const diffMs = date.getTime() - Date.now();
  const abs = Math.abs(diffMs);
  if (abs < 50 * 1000) {
    return "刚刚";
  }
  const steps = [
    ["year", 365 * 24 * 60 * 60 * 1000],
    ["month", 30 * 24 * 60 * 60 * 1000],
    ["week", 7 * 24 * 60 * 60 * 1000],
    ["day", 24 * 60 * 60 * 1000],
    ["hour", 60 * 60 * 1000],
    ["minute", 60 * 1000],
  ];
  const formatter = new Intl.RelativeTimeFormat("zh-CN", { numeric: "auto" });
  for (const [unit, size] of steps) {
    if (abs >= size) {
      return formatter.format(Math.round(diffMs / size), unit);
    }
  }
  const seconds = Math.round(diffMs / 1000);
  if (Math.abs(seconds) < 60) {
    return seconds === 0 ? "刚刚" : formatter.format(seconds, "second");
  }
  return formatter.format(seconds, "second");
}

export function formatAbsoluteDateTime(date, format = {}) {
  const style = String(format.style || "").toLowerCase();
  if (style === "date") {
    return new Intl.DateTimeFormat("zh-CN", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).format(date);
  }
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
}

export function formatPercentValue(raw, format = {}) {
  if (raw == null || raw === "") return "";
  let numeric = typeof raw === "number" ? raw : Number(String(raw).replace(/%/g, "").trim());
  if (!Number.isFinite(numeric)) return toPlainText(raw);
  const input = String(format?.percentInput || "auto").trim().toLowerCase();
  let percentValue = numeric;
  if (input === "ratio") {
    percentValue = numeric * 100;
  } else if (input === "value" || input === "hundred") {
    percentValue = numeric;
  } else if (Math.abs(numeric) <= 1 && !Number.isInteger(numeric)) {
    percentValue = numeric * 100;
  }
  const precision = Number.isFinite(format?.precision) ? format.precision : 0;
  return `${percentValue.toFixed(precision)}%`;
}

function formatNumberValue(raw, format) {
  const numeric = typeof raw === "number" ? raw : Number(raw);
  if (!Number.isFinite(numeric)) return toPlainText(raw);
  const precision = Number.isFinite(format?.precision) ? format.precision : null;
  const formatter = new Intl.NumberFormat("zh-CN", {
    notation: format?.compact ? "compact" : "standard",
    useGrouping: format?.useGrouping !== false,
    minimumFractionDigits: precision ?? 0,
    maximumFractionDigits: precision ?? (Number.isInteger(numeric) ? 0 : 2),
  });
  return formatter.format(numeric);
}

function formatDateOnly(date) {
  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function valueLooksDateOnly(raw) {
  const text = toPlainText(raw).trim();
  if (/^\d{4}[-/]\d{1,2}[-/]\d{1,2}$/.test(text)) return true;
  return /^\d{4}[-/]\d{1,2}[-/]\d{1,2}[ T]\d{1,2}:\d{1,2}(?::\d{1,2})?(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?$/.test(
    text
  );
}

function columnKeyLooksCalendarDate(key) {
  return /时间$|日期$/.test(String(key || "").trim());
}

function formatDateLike(raw, descriptor) {
  const date = parseDateValue(raw);
  if (!date) return toPlainText(raw);
  const format = descriptor?.format || {};
  const type = descriptor?.type || "datetime";
  if (format.relative || type === "relative_time") {
    return formatRelativeTime(date);
  }
  if (type === "date" || (type === "datetime" && valueLooksDateOnly(raw))) {
    return formatDateOnly(date);
  }
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function matchesRule(raw, rule) {
  const text = toPlainText(raw).trim();
  const numeric = Number(raw);
  switch (rule.op) {
    case "lt":
      return Number.isFinite(numeric) && numeric < Number(rule.value);
    case "lte":
      return Number.isFinite(numeric) && numeric <= Number(rule.value);
    case "gt":
      return Number.isFinite(numeric) && numeric > Number(rule.value);
    case "gte":
      return Number.isFinite(numeric) && numeric >= Number(rule.value);
    case "eq":
      return text === toPlainText(rule.value).trim();
    case "includes":
      return text.includes(toPlainText(rule.value).trim());
    case "negative":
      return Number.isFinite(numeric) && numeric < 0;
    case "positive":
      return Number.isFinite(numeric) && numeric > 0;
    case "empty":
      return !text;
    case "not_empty":
      return !!text;
    default:
      return false;
  }
}

export function resolveToneToken(raw, descriptor) {
  const rules = Array.isArray(descriptor?.rules) ? descriptor.rules : [];
  const matched = rules.find((rule) => matchesRule(raw, rule));
  if (matched?.tone) return matched.tone;
  return null;
}

export function formatCellDisplay(raw, descriptor) {
  const format = descriptor?.format || {};
  const emptyText = format.emptyText ?? "";
  if (raw == null || raw === "") {
    return emptyText;
  }
  if (descriptor?.type === "number") {
    return formatNumberValue(raw, format);
  }
  if (descriptor?.type === "percent") {
    return formatPercentValue(raw, format);
  }
  if (
    descriptor?.type === "date" ||
    descriptor?.type === "datetime" ||
    descriptor?.type === "relative_time"
  ) {
    return formatDateLike(raw, descriptor);
  }
  if (
    columnKeyLooksCalendarDate(descriptor?.key) &&
    (typeof raw === "string" || typeof raw === "number") &&
    parseDateValue(raw)
  ) {
    return formatDateLike(raw, { ...descriptor, type: "date" });
  }
  if (typeof raw === "string" && valueLooksDateOnly(raw)) {
    const date = parseDateValue(raw);
    if (date) return formatDateOnly(date);
  }
  return toPlainText(raw);
}

export function formatCellDetail(raw, descriptor) {
  const format = descriptor?.format || {};
  if (raw == null || raw === "") return "";
  const type = descriptor?.type || "text";
  if (type === "relative_time" || format.relative) {
    const date = parseDateValue(raw);
    if (date) return formatAbsoluteDateTime(date, format);
  }
  if (type === "date" || type === "datetime") {
    const date = parseDateValue(raw);
    if (!date) return toPlainText(raw);
    if (
      type === "date" ||
      columnKeyLooksCalendarDate(descriptor?.key) ||
      valueLooksDateOnly(raw)
    ) {
      return formatDateOnly(date);
    }
    return formatAbsoluteDateTime(date, format);
  }
  if (type === "percent") return formatPercentValue(raw, format);
  if (type === "number") return formatNumberValue(raw, format);
  return toPlainText(raw);
}

export function descriptorUsesRelativeTime(descriptor) {
  const type = descriptor?.type || "";
  const format = descriptor?.format || {};
  return type === "relative_time" || format.relative === true;
}

export function descriptorsHaveRelativeTime(descriptors) {
  return (Array.isArray(descriptors) ? descriptors : []).some((entry) => descriptorUsesRelativeTime(entry));
}

export function toRelativeAtIso(raw) {
  const date = parseDateValue(raw);
  if (!date) return String(raw ?? "").trim();
  return date.toISOString();
}

export function formatRelativeTimeForRaw(raw, descriptor = {}) {
  const date = parseDateValue(raw);
  if (!date) {
    const text = toPlainText(raw);
    return { relative: text, absolute: text };
  }
  const format = descriptor?.format || {};
  return {
    relative: formatRelativeTime(date),
    absolute: formatAbsoluteDateTime(date, format),
  };
}

export function formatCellPresentation(raw, descriptor) {
  const display = formatCellDisplay(raw, descriptor);
  const detail = formatCellDetail(raw, descriptor);
  const relative = descriptorUsesRelativeTime(descriptor);
  const showDetail = relative && detail && detail !== display;
  return {
    display,
    detail: showDetail ? detail : detail !== display ? detail : "",
    title: showDetail ? detail : detail !== display ? detail : "",
  };
}

function normalizeEntries(entries) {
  return Array.isArray(entries) ? entries : [];
}

export function resolveColumnDescriptors({
  columns,
  headers,
  columnMeta,
  columnState,
  columnFormats,
  columnRules,
}) {
  const keys = normalizeEntries(columns).map((item) => String(item || "").trim()).filter(Boolean);
  const labels = normalizeEntries(headers).map((item) => String(item || ""));
  const metaByKey = new Map(
    normalizeEntries(columnMeta)
      .map((entry) => {
        const name = String(entry?.name || "").trim();
        return name ? [name, entry] : null;
      })
      .filter(Boolean)
  );
  const stateByKey = new Map(
    normalizeEntries(columnState?.columns).map((entry, index) => {
      const key = String(entry?.key || "").trim();
      if (!key) return null;
      return [
        key,
        {
          ...entry,
          order: Number.isFinite(Number(entry?.order)) ? Math.round(Number(entry.order)) : index,
        },
      ];
    }).filter(Boolean)
  );
  const formats = normalizeColumnFormats(columnFormats);
  const rules = normalizeColumnRules(columnRules);
  const descriptors = keys
    .map((key, index) => {
      const meta = metaByKey.get(key) || {};
      const state = stateByKey.get(key) || {};
      const format = formats[key] || {};
      const type = inferType(key, meta, format);
      return {
        key,
        label: String(labels[index] || key),
        meta,
        state,
        format,
        rules: rules[key] || [],
        type,
        order: Number.isFinite(Number(state.order)) ? Number(state.order) : index,
        hidden: state.hidden === true,
        width: Number.isFinite(Number(state.width)) ? Number(state.width) : null,
        minWidth: Number.isFinite(Number(state.min_width)) ? Number(state.min_width) : null,
        maxWidth: Number.isFinite(Number(state.max_width)) ? Number(state.max_width) : null,
        align:
          String(state.align || format.align || "")
            .trim()
            .toLowerCase() || defaultAlignForColumn(key, type),
        valign: String(state.valign || "").trim().toLowerCase() || "middle",
        headerAlign:
          String(state.header_align || "").trim().toLowerCase() ||
          String(state.align || format.align || "")
            .trim()
            .toLowerCase() ||
          defaultAlignForColumn(key, type),
        headerValign: String(state.header_valign || "").trim().toLowerCase() || "middle",
        wrap: state.wrap === true,
        headerWrap: state.header_wrap === true,
        sortable: meta.sortable !== false,
        filterable: meta.filterable !== false,
        tag: format.tag === true,
      };
    })
    .map(finalizeColumnLayout)
    .filter((entry) => !entry.hidden)
    .sort((left, right) => left.order - right.order);
  return descriptors;
}

export const DEFAULT_CELL_PADDING = "8px 12px";

let textMeasureContext = null;

function parsePaddingTokenPx(token) {
  const value = Number.parseFloat(String(token ?? "").trim());
  return Number.isFinite(value) && value >= 0 ? value : null;
}

function resolveHorizontalPaddingPx(raw, fallback = 24) {
  const parts = String(raw ?? "")
    .trim()
    .split(/\s+/)
    .map(parsePaddingTokenPx)
    .filter((value) => value != null);
  if (parts.length === 0) return fallback;
  if (parts.length === 1) return Math.ceil(parts[0] * 2);
  if (parts.length === 2) return Math.ceil(parts[1] * 2);
  if (parts.length === 3) return Math.ceil(parts[1] * 2);
  return Math.ceil((parts[1] || 0) + (parts[3] || 0));
}

function ensureTextMeasureContext() {
  if (textMeasureContext) return textMeasureContext;
  if (typeof document === "undefined") return null;
  const canvas = document.createElement("canvas");
  textMeasureContext = canvas.getContext("2d");
  return textMeasureContext;
}

/** 估算展示文本宽度（中文≈1，ASCII≈0.55 单位）。 */
export function measureDisplayTextUnits(text) {
  const raw = String(text ?? "");
  let units = 0;
  for (const ch of raw) {
    units += ch.charCodeAt(0) > 255 ? 1 : 0.55;
  }
  return units;
}

export function measureDisplayTextPx(text, options = {}) {
  const raw = String(text ?? "");
  if (!raw) return 0;
  const font = String(options.font || "").trim();
  const charPx = Number(options.charPx) > 0 ? Number(options.charPx) : 7;
  const ctx = font ? ensureTextMeasureContext() : null;
  if (ctx && font) {
    ctx.font = font;
    return Math.ceil(ctx.measureText(raw).width);
  }
  return Math.ceil(measureDisplayTextUnits(raw) * charPx);
}

function displayCharCount(text) {
  return [...String(text ?? "")].length;
}

function columnMeasureSlackPx(descriptor, charPx = 7) {
  let slack = Math.max(6, Math.round((Number(charPx) || 7) * 0.45));
  if (descriptor?.tag || isTagLikeColumnKey(descriptor?.key)) slack += 2;
  if (isDepartmentLikeColumnKey(descriptor?.key)) slack += 2;
  if (descriptor?.type === "date" || descriptor?.type === "datetime" || descriptor?.type === "relative_time") {
    slack += 2;
  }
  return slack;
}

function columnWidthCapForKey(key, descriptor = null) {
  const name = String(key || "").trim();
  const tagLike = descriptor?.tag || isTagLikeColumnKey(name);
  if (!name) return 320;
  if (isIdentifierLikeColumnKey(name)) return 176;
  if (/序号|^id$/i.test(name)) return 88;
  if (/是否/.test(name)) return tagLike ? 136 : 108;
  if (/等级/.test(name)) return tagLike ? 132 : 108;
  if (/类型|类别/.test(name)) return tagLike ? 240 : 220;
  if (/领域/.test(name)) return 140;
  if (/办公地址|住所地址|注册地址|地址$/.test(name)) return 640;
  // 「预警模型」等业务名列按中等宽；仅模型ID/裸「模型」保持紧凑。
  if (/模型ID|模板ID|^(模型|模板)$/.test(name)) return 132;
  if (/模型|模板/.test(name)) return 240;
  if (/政策文件|模型依据|规则|依据|文件|描述|事项|表现|情况|数据/.test(name)) {
    return 280;
  }
  return 220;
}

function columnWidthFloorForKey(key) {
  const name = String(key || "").trim();
  if (isIdentifierLikeColumnKey(name)) return 112;
  if (/序号|^id$/i.test(name)) return 52;
  if (/时间$|日期$/.test(name)) return 128;
  if (/等级|类型|类别/.test(name)) return 64;
  return 56;
}

function isCompactWidthKey(key) {
  const name = String(key || "").trim();
  if (!name || isIdentifierLikeColumnKey(name)) return false;
  return /等级|是否|日期|时间/i.test(name);
}

function inferMinVisibleWidthPx(descriptor, charPx, padPx, minChars = 10, font = "", slackPx = 0) {
  const chars = Math.max(0, Number(minChars) || 0);
  if (chars <= 0) return 0;
  if (!descriptor) return 0;
  if (isCompactDisplayType(descriptor.type)) return 0;
  if (isCompactWidthKey(descriptor.key)) return 0;
  return Math.ceil(measureDisplayTextPx("测".repeat(chars), { font, charPx }) + padPx + slackPx + 10);
}

function tagChipExtraWidthPx(descriptor) {
  if (!(descriptor?.tag || isTagLikeColumnKey(descriptor?.key))) return 0;
  return 28;
}

function rowFieldValue(row, key) {
  if (!row || typeof row !== "object") return null;
  if (Object.prototype.hasOwnProperty.call(row, key)) return row[key];
  return null;
}

function previewTextForWidth(raw, descriptor) {
  const format = descriptor?.format || {};
  const display = formatCellDisplay(raw, descriptor);
  const maxChars = Number(format.maxChars ?? format.max_chars);
  if (Number.isFinite(maxChars) && maxChars > 0 && display.length > maxChars) {
    return `${display.slice(0, maxChars)}…`;
  }
  return display;
}

function columnHasManualWidth(descriptor) {
  return Number.isFinite(Number(descriptor?.width)) && Number(descriptor.width) > 0;
}

function isAddressLikeColumnKey(key) {
  const name = String(key || "").trim();
  return /办公地址|住所地址|注册地址|地址$/.test(name);
}

/** 需占满剩余宽度的弹性列（不可落入全 px 显式模板）。 */
function columnPrefersFlexGrow(descriptor) {
  if (!descriptor) return false;
  if (columnIsFlexFillSemantic(descriptor)) return true;
  if (Number(descriptor?.layoutFixedWidth) > 0) return false;
  // 测宽后长文列已清掉 width，仅留 layoutMinWidth；勿再被 columnHasManualWidth 挡掉。
  if (columnHasManualWidth(descriptor) && Number(descriptor?.layoutFixedWidth) > 0) return false;
  const key = String(descriptor?.key || "");
  if (isAddressLikeColumnKey(key)) return true;
  const mode = String(descriptor?.widthMode || descriptor?.state?.width_mode || descriptor?.state?.widthMode || "")
    .trim()
    .toLowerCase();
  if (mode === "content" || mode === "min") return true;
  if (Number(descriptor?.layoutMinWidth) > 0 && !Number(descriptor?.layoutFixedWidth)) return true;
  return false;
}

function inferIdentifierColumnLayout(descriptor, sample, options = {}) {
  const key = String(descriptor?.key || "");
  const charPx = Number(options.charPx) > 0 ? Number(options.charPx) : 7;
  // ID 列留白敏感：用更紧的水平 padding，避免 YJ2025001 被测成 160+。
  const defaultPadPx = resolveHorizontalPaddingPx(DEFAULT_CELL_PADDING, 20);
  const padPx = Math.min(
    24,
    Number(options.cellPaddingPx) > 0 ? Number(options.cellPaddingPx) : defaultPadPx,
  );
  const bodyFont = String(options.font || "").trim();
  const labelFont = String(options.labelFont || bodyFont).trim();
  const label = String(descriptor?.label || key).trim();
  let maxWidthPx = measureDisplayTextPx(label, { font: labelFont, charPx });
  for (const row of sample) {
    const preview = previewTextForWidth(rowFieldValue(row, key), descriptor);
    const text = String(preview ?? "");
    maxWidthPx = Math.max(maxWidthPx, measureDisplayTextPx(text, { font: bodyFont, charPx }));
  }
  const floor = Math.max(
    columnWidthFloorForKey(key),
    Math.ceil(measureDisplayTextPx(label, { font: labelFont, charPx }) + 12),
  );
  const cap = Math.min(152, columnWidthCapForKey(key, descriptor));
  const slackPx = Math.max(4, columnMeasureSlackPx(descriptor, charPx) - 2);
  const width = Math.ceil(Math.min(cap, Math.max(floor, maxWidthPx + padPx + slackPx)));
  return finalizeColumnLayout({
    ...descriptor,
    width: null,
    widthMode: "fixed",
    layoutFixedWidth: width,
    layoutMinWidth: width,
    layoutMaxWidth: width,
    layoutClamp: false,
    format: { ...(descriptor.format || {}), truncate: false },
  });
}

function inferWarningLevelBlocksLayout(descriptor, authorWidth = 0) {
  const kind = String(descriptor?.format?.kind || descriptor?.kind || "")
    .trim()
    .toLowerCase()
    .replace(/-/g, "_");
  // 三色块（风险等级）需要约 3×方块+间隙；单色块（预警等级）更窄。
  const defaultWidth =
    kind === "risk_level_blocks" || kind === "warning_level_blocks" ? 180 : 88;
  const width = authorWidth > 0 ? authorWidth : defaultWidth;
  return finalizeColumnLayout({
    ...descriptor,
    width: null,
    widthMode: "fixed",
    layoutFixedWidth: width,
    layoutMinWidth: width,
    layoutMaxWidth: width,
    layoutClamp: false,
  });
}

/**
 * 用前 N 行样本推断列宽（表头+单元格），写入 layoutFixedWidth。
 * - 作者 `width_mode=fixed` + width：精确尊重（难测列可手工指定）
 * - 标签/日期/ID/数字等：按内容测宽
 * - 描述/依据等长文：只定 layoutMinWidth，留给 fr 吃剩余宽度
 */
export function inferColumnWidthsFromSample(rows, descriptors, options = {}) {
  const sampleLimit = Math.max(1, Number(options.sampleLimit) || 100);
  const charPx = Number(options.charPx) > 0 ? Number(options.charPx) : 7;
  const defaultPadPx = resolveHorizontalPaddingPx(DEFAULT_CELL_PADDING, 24);
  const padPx = Number(options.cellPaddingPx) > 0 ? Number(options.cellPaddingPx) : defaultPadPx;
  const minVisibleChars = Math.max(0, Number(options.minVisibleChars) || 0);
  const bodyFont = String(options.font || "").trim();
  const labelFont = String(options.labelFont || bodyFont).trim();
  const sample = (Array.isArray(rows) ? rows : []).slice(0, sampleLimit);

  return (Array.isArray(descriptors) ? descriptors : []).map((descriptor) => {
    const key = String(descriptor?.key || "");
    const authorWidth =
      columnHasManualWidth(descriptor) && Number.isFinite(Number(descriptor.width))
        ? Math.round(Number(descriptor.width))
        : 0;
    const explicitFixedMode =
      String(descriptor?.widthMode || descriptor?.state?.width_mode || descriptor?.state?.widthMode || "")
        .trim()
        .toLowerCase() === "fixed";

    // 等级色块：文本测宽无效，走专用默认宽；有作者 fixed 则完全尊重。
    if (isWarningLevelBlocksColumn(descriptor)) {
      if (authorWidth > 0 && explicitFixedMode) {
        return inferWarningLevelBlocksLayout(descriptor, authorWidth);
      }
      return inferWarningLevelBlocksLayout(descriptor, authorWidth);
    }

    // 作者显式 fixed + width → 精确锁定（含 ID/标签等难测或需控留白的列）。
    if (authorWidth > 0 && explicitFixedMode) {
      if (columnIsFlexFillSemantic(descriptor)) {
        return finalizeColumnLayout({
          ...descriptor,
          width: null,
          widthMode: "min",
          layoutFixedWidth: null,
          layoutMinWidth: authorWidth,
          layoutMaxWidth: null,
          layoutClamp: false,
        });
      }
      return finalizeColumnLayout({
        ...descriptor,
        widthMode: "fixed",
        layoutFixedWidth: authorWidth,
        layoutMinWidth: authorWidth,
        layoutMaxWidth: authorWidth,
      });
    }

    if (isIdentifierLikeColumnKey(key)) {
      return inferIdentifierColumnLayout(descriptor, sample, options);
    }

    if (isAddressLikeColumnKey(key) && !explicitFixedMode) {
      return finalizeColumnLayout({
        ...descriptor,
        width: null,
        widthMode: "content",
        layoutFixedWidth: null,
        layoutMinWidth: Math.max(280, columnWidthFloorForKey(key), authorWidth),
        layoutMaxWidth: null,
        layoutClamp: false,
      });
    }

    let maxWidthPx = measureDisplayTextPx(descriptor?.label || key, { font: labelFont, charPx });
    let maxContentChars = 0;
    for (const row of sample) {
      const raw = rowFieldValue(row, key);
      const preview = previewTextForWidth(raw, descriptor);
      maxContentChars = Math.max(maxContentChars, displayCharCount(preview));
      maxWidthPx = Math.max(
        maxWidthPx,
        measureDisplayTextPx(preview, { font: bodyFont, charPx }),
      );
    }

    const floor = Math.max(columnWidthFloorForKey(key), authorWidth);
    const cap = columnWidthCapForKey(key, descriptor);
    const slackPx = columnMeasureSlackPx(descriptor, charPx);
    const minVisibleWidth =
      maxContentChars > minVisibleChars
        ? inferMinVisibleWidthPx(descriptor, charPx, padPx, minVisibleChars, bodyFont, slackPx)
        : 0;
    const measured = Math.ceil(
      Math.min(
        cap,
        Math.max(
          floor,
          minVisibleWidth,
          maxWidthPx + padPx + tagChipExtraWidthPx(descriptor) + slackPx,
        ),
      ),
    );

    // 长文列：测宽结果作下限，轨道用 fr 吃满剩余空间。
    if (columnIsFlexFillSemantic(descriptor)) {
      return finalizeColumnLayout({
        ...descriptor,
        width: null,
        widthMode: "min",
        layoutFixedWidth: null,
        layoutMinWidth: measured,
        layoutMaxWidth: null,
        layoutClamp: false,
      });
    }

    return finalizeColumnLayout({
      ...descriptor,
      width: null,
      widthMode: "fixed",
      layoutFixedWidth: measured,
      layoutMinWidth: measured,
      layoutMaxWidth: measured,
      layoutClamp: false,
    });
  });
}

/** 全表统一 px 轨（表头/各行共用，避免 max-content 逐行错位）。 */
export function buildExplicitColumnTemplate(descriptors) {
  const list = Array.isArray(descriptors) ? descriptors : [];
  if (list.length === 0) return "";
  const tracks = list.map((descriptor) => {
    const fixed = Number(descriptor?.layoutFixedWidth);
    if (Number.isFinite(fixed) && fixed > 0) {
      return `${Math.round(fixed)}px`;
    }
    if (columnPrefersFlexGrow(descriptor)) {
      return "";
    }
    const min = Number(descriptor?.layoutMinWidth ?? descriptor?.minWidth);
    if (Number.isFinite(min) && min > 0) {
      return `${Math.round(min)}px`;
    }
    return "";
  });
  if (tracks.some((track) => !track)) return "";
  return tracks.join(" ");
}

export function columnLayoutWeights(descriptors, fallbackMin = 96) {
  const weights = (Array.isArray(descriptors) ? descriptors : []).map((descriptor) => {
    const key = String(descriptor?.key || "");
    if (isAddressLikeColumnKey(key)) {
      return Math.max(columnMinWidthPx(descriptor, fallbackMin), 720);
    }
    return columnMinWidthPx(descriptor, fallbackMin);
  });
  const total = weights.reduce((sum, value) => sum + value, 0) || 1;
  return weights.map((value) => (value / total) * 100);
}

/** 单列最小轨宽（px）；供 `<table>` 横向滚动与 `<col>` 定宽使用。 */
export function columnMinWidthPx(descriptor, fallbackMin = 96) {
  const floor = Math.max(48, Number(fallbackMin) || 96);
  const key = String(descriptor?.key || "");
  const fixed = Number(descriptor?.layoutFixedWidth);
  if (Number.isFinite(fixed) && fixed > 0) {
    return Math.round(fixed);
  }
  const min = Number(descriptor?.layoutMinWidth ?? descriptor?.minWidth);
  if (Number.isFinite(min) && min > 0) {
    return Math.round(min);
  }
  const max = Number(descriptor?.layoutMaxWidth ?? descriptor?.maxWidth);
  if (Number.isFinite(max) && max > 0) {
    return Math.round(Math.min(max, floor * 2));
  }
  const label = String(descriptor?.label || key).trim();
  const labelFloor = Math.min(180, Math.max(floor, label.length * 14 + 28));
  if (isIdentifierLikeColumnKey(key)) {
    return Math.max(labelFloor, 112);
  }
  if (isCompactWidthKey(key)) {
    return Math.max(columnWidthFloorForKey(key), 48);
  }
  if (isDepartmentLikeColumnKey(key)) {
    return Math.max(labelFloor, 180);
  }
  if (/描述|事项|表现|问题分类|问题描述|存在的问题/.test(key)) {
    return Math.max(labelFloor, 140);
  }
  if (/单位|机构/.test(key)) {
    return Math.max(labelFloor, 120);
  }
  if (isAddressLikeColumnKey(key)) {
    return Math.max(labelFloor, 280);
  }
  return labelFloor;
}

export function sumDescriptorColumnWidths(descriptors, fallbackMin = 96) {
  return (Array.isArray(descriptors) ? descriptors : []).reduce(
    (acc, descriptor) => acc + columnMinWidthPx(descriptor, fallbackMin),
    0
  );
}

export function resolveDatasetTableColumnMinWidth(props) {
  const raw = Number(props?.columnMinWidth ?? props?.column_min_width);
  if (Number.isFinite(raw) && raw > 0) {
    return Math.floor(raw);
  }
  return 96;
}

export function inlineStyleForColPixelWidth(descriptor, fallbackMin = 96) {
  const px = columnMinWidthPx(descriptor, fallbackMin);
  return `box-sizing:border-box;min-width:${px}px;width:${px}px`;
}

export function inlineStyleForColWidth(descriptor, widthPercent) {
  const parts = ["box-sizing:border-box", "min-width:0"];
  const fixed = descriptor?.layoutFixedWidth;
  const min = descriptor?.layoutMinWidth ?? descriptor?.minWidth;
  const max = descriptor?.layoutMaxWidth ?? descriptor?.maxWidth;
  if (Number.isFinite(widthPercent) && widthPercent > 0) {
    parts.push(`width:${widthPercent.toFixed(4)}%`);
  } else if (fixed) {
    parts.push(`width:${fixed}px`);
  } else if (min) {
    parts.push(`min-width:${min}px`);
  }
  if (max) parts.push(`max-width:${max}px`);
  return parts.join(";");
}

export function inlineStyleForColumn(descriptor, target = "cell", options = {}) {
  const parts = ["box-sizing:border-box"];
  const respectColWidth = options?.respectColWidth === true;
  if (respectColWidth) {
    const px = columnMinWidthPx(descriptor, options?.fallbackMin ?? 96);
    parts.push(`min-width:${px}px`, `width:${px}px`, `max-width:${px}px`);
  } else {
    parts.push("min-width:0");
  }
  const align = target === "header" ? descriptor?.headerAlign : descriptor?.align;
  const valign = target === "header" ? descriptor?.headerValign : descriptor?.valign;
  const wrap = target === "header" ? descriptor?.headerWrap : descriptor?.wrap;
  const fixed = descriptor?.layoutFixedWidth;
  const max = descriptor?.layoutMaxWidth ?? descriptor?.maxWidth;
  if (fixed) {
    parts.push(`min-width:${fixed}px`, `width:${fixed}px`, `max-width:${fixed}px`);
  } else if (max && descriptor?.layoutClamp) {
    parts.push(`max-width:${max}px`);
  }
  if (align) {
    parts.push(`text-align:${align}`);
    // .td-cell 是 flex；仅 text-align 无法居中/右齐子节点。
    if (target === "cell" || target === "header") {
      if (align === "center") parts.push("justify-content:center");
      else if (align === "right" || align === "end") parts.push("justify-content:flex-end");
      else if (align === "left" || align === "start") parts.push("justify-content:flex-start");
    }
  }
  if (valign) parts.push(`vertical-align:${valign === "middle" ? "middle" : valign}`);
  if (wrap) {
    parts.push("white-space:normal");
    parts.push("overflow-wrap:anywhere");
  } else {
    parts.push("white-space:nowrap");
  }
  return parts.join(";");
}

function columnGridTrack(descriptor, floor, { shrinkFit = false, weightFr = 1 } = {}) {
  const min = Number(descriptor?.layoutMinWidth ?? descriptor?.minWidth);
  const minPx = Number.isFinite(min) && min > 0 ? Math.max(40, min) : 0;
  const max = Number(descriptor?.layoutMaxWidth ?? descriptor?.maxWidth);
  const fixed = Number(descriptor?.layoutFixedWidth);
  const fr = Math.max(0.2, Number(weightFr) || 1);

  if (Number.isFinite(fixed) && fixed > 0) {
    if (isIdentifierLikeColumnKey(descriptor?.key) || descriptor?.widthMode === "fixed") {
      return `minmax(${fixed}px, ${fixed}px)`;
    }
    return shrinkFit ? `minmax(0, ${fixed}px)` : `minmax(${fixed}px, ${fixed}px)`;
  }
  if (Number.isFinite(max) && max > 0 && descriptor?.layoutClamp) {
    const base = Math.max(minPx, shrinkFit ? 0 : floor, 80);
    return `minmax(${base}px, ${max}px)`;
  }
  const base = Math.max(minPx, shrinkFit ? 0 : floor, 48);
  return `minmax(${base}px, ${fr.toFixed(3)}fr)`;
}

export function buildColumnTemplate(descriptors, defaultMinWidth = 120, options = {}) {
  const floor = Math.max(60, Number(defaultMinWidth) || 120);
  const list = Array.isArray(descriptors) ? descriptors : [];
  if (list.length === 0) return "";
  const shrinkFit = options?.shrinkFit === true;
  const weights = columnLayoutWeights(list, floor);
  return list
    .map((descriptor, index) => {
      const fr = Math.max(0.2, (weights[index] / 100) * list.length);
      return columnGridTrack(descriptor, floor, { shrinkFit, weightFr: fr });
    })
    .join(" ");
}
