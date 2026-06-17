const ENUM_MAX = 48;
const ENUM_AVG_LEN = 28;
const NUMERIC_RATIO = 0.75;
const DATE_RATIO = 0.65;

const DATE_FIELD_RE = /时间$|日期$|年月/;
const ID_FIELD_RE = /ID$|编号$|编码$/;
const MEASURE_COLUMN_RE = /条数$|金额$|人数$|^序号$|^value$/i;

export function sampleColumnValues(rows, column) {
  const values = [];
  for (const row of rows || []) {
    if (!row || typeof row !== "object") continue;
    const text = String(row[column] ?? "").trim();
    if (text) values.push(text);
  }
  return values;
}

function uniqueNonEmpty(values) {
  return Array.from(new Set(values.map((value) => String(value || "").trim()).filter(Boolean)));
}

function averageLength(values) {
  if (!values.length) return 0;
  return values.reduce((sum, value) => sum + [...String(value)].length, 0) / values.length;
}

function isNumericLike(text) {
  const normalized = String(text || "").trim().replace(/,/g, "");
  if (!normalized) return false;
  return /^-?\d+(?:\.\d+)?$/.test(normalized);
}

function isDateLike(text) {
  const trimmed = String(text || "").trim();
  if (!trimmed) return false;
  if (/^\d{4}-\d{2}/.test(trimmed)) return true;
  if (/^\d{4}\/\d{2}/.test(trimmed)) return true;
  return !Number.isNaN(Date.parse(trimmed));
}

function normalizeControlHint(hint) {
  const control = String(hint?.control || hint?.type || "").trim().toLowerCase();
  if (control === "month_multi_select" || control === "date_range" || control === "date") {
    return control === "date_range" ? "date_range" : "month_multi_select";
  }
  if (control === "multi_select") return "multi_select";
  if (control === "text") return "text";
  return "";
}

function isMeasureColumn(name) {
  return MEASURE_COLUMN_RE.test(String(name || "").trim());
}

function buildDateProfile(values) {
  const months = uniqueNonEmpty(values.map(extractYearMonth).filter(Boolean)).sort((a, b) =>
    a.localeCompare(b, "zh-CN"),
  );
  return {
    kind: "date",
    operators: ["month_in", "month_range"],
    options: months,
  };
}

function buildEnumProfile(distinct) {
  return {
    kind: "enum",
    operators: ["in"],
    options: distinct.sort((a, b) => a.localeCompare(b, "zh-CN")),
  };
}

export function extractYearMonth(text) {
  const trimmed = String(text || "").trim();
  if (/^\d{4}-\d{2}/.test(trimmed)) {
    return trimmed.slice(0, 7);
  }
  const parsed = Date.parse(trimmed);
  if (!Number.isNaN(parsed)) {
    const date = new Date(parsed);
    const month = String(date.getMonth() + 1).padStart(2, "0");
    return `${date.getFullYear()}-${month}`;
  }
  return "";
}

export function inferColumnProfile(column, rows = [], hint = null) {
  const name = String(column || "").trim();
  const values = sampleColumnValues(rows, name);
  const distinct = uniqueNonEmpty(values);
  const controlHint = normalizeControlHint(hint);

  if (!name) {
    return { kind: "text", operators: ["contains"], options: [] };
  }

  if (controlHint === "month_multi_select" || (controlHint !== "text" && DATE_FIELD_RE.test(name))) {
    return buildDateProfile(values);
  }
  if (controlHint === "date_range") {
    const profile = buildDateProfile(values);
    return {
      ...profile,
      operators: ["month_range"],
    };
  }
  if (controlHint === "multi_select") {
    if (distinct.length > 0) {
      return buildEnumProfile(distinct);
    }
    return { kind: "enum", operators: ["in"], options: [] };
  }
  if (controlHint === "text") {
    return { kind: "text", operators: ["contains", "eq"], options: [] };
  }

  if (ID_FIELD_RE.test(name)) {
    return { kind: "text", operators: ["contains", "eq"], options: [] };
  }
  if (isMeasureColumn(name)) {
    return {
      kind: "number",
      operators: ["eq", "gt", "gte", "lt", "lte"],
      options: [],
    };
  }

  const numericHits = values.filter(isNumericLike).length;
  const dateHits = values.filter(isDateLike).length;
  const ratio = values.length > 0 ? numericHits / values.length : 0;
  const dateRatio = values.length > 0 ? dateHits / values.length : 0;

  if (DATE_FIELD_RE.test(name) || dateRatio >= DATE_RATIO) {
    return buildDateProfile(values);
  }
  if (ratio >= NUMERIC_RATIO) {
    return {
      kind: "number",
      operators: ["eq", "gt", "gte", "lt", "lte"],
      options: [],
    };
  }
  if (
    distinct.length > 0 &&
    distinct.length <= ENUM_MAX &&
    averageLength(distinct) <= ENUM_AVG_LEN
  ) {
    return buildEnumProfile(distinct);
  }
  return {
    kind: "text",
    operators: ["contains", "eq"],
    options: [],
  };
}

export function buildColumnProfiles(catalog, rows) {
  const profiles = new Map();
  for (const entry of catalog || []) {
    const column =
      typeof entry === "string"
        ? String(entry || "").trim()
        : String(entry?.column || entry?.key || "").trim();
    if (!column) continue;
    const hint = typeof entry === "object" && entry ? entry : null;
    profiles.set(column, inferColumnProfile(column, rows, hint));
  }
  return profiles;
}

export function defaultOperatorForProfile(profile, hint = null) {
  const hinted = String(hint?.operator || hint?.default_operator || hint?.defaultOperator || "").trim();
  if (hinted) return hinted;
  if (!profile) return "contains";
  if (profile.kind === "enum") return "in";
  if (profile.kind === "number") return "eq";
  if (profile.kind === "date") return "month_in";
  return "contains";
}

export function operatorOptionsForProfile(profile) {
  const labels = {
    in: "属于",
    contains: "包含",
    eq: "等于",
    gt: "大于",
    gte: "大于等于",
    lt: "小于",
    lte: "小于等于",
    month_in: "月份属于",
    month_range: "月份范围",
  };
  const operators = Array.isArray(profile?.operators) ? profile.operators : ["contains"];
  return operators.map((id) => ({ id, label: labels[id] || id }));
}
