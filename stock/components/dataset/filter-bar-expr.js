import { defaultOperatorForProfile, extractYearMonth } from "./filter-bar-infer.js";

function splitEncodedList(raw) {
  return String(raw || "")
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean);
}

function stripNegate(encoded) {
  const text = String(encoded || "").trim();
  if (text.startsWith("not:")) {
    return { negate: true, body: text.slice(4) };
  }
  return { negate: false, body: text };
}

export function createEmptyFilterRow(nextRowId, options = {}) {
  const status = options.status === "active" ? "active" : "draft";
  return {
    id: nextRowId(),
    status,
    column: "",
    operator: "contains",
    negate: false,
    value: "",
    values: [],
    rangeStart: "",
    rangeEnd: "",
  };
}

export function encodeFilterRow(row, profile) {
  const column = String(row?.column || "").trim();
  if (!column) return "";
  const operator = String(row?.operator || defaultOperatorForProfile(profile)).trim();
  const negate = Boolean(row?.negate);
  let body = "";
  if (operator === "in") {
    const values = Array.isArray(row?.values) ? row.values.filter(Boolean) : [];
    if (!values.length) return "";
    body = `in:${values.join(",")}`;
  } else if (operator === "contains_any") {
    const values = Array.isArray(row?.values) ? row.values.filter(Boolean) : [];
    if (!values.length) {
      const single = String(row?.value || "").trim();
      if (!single) return "";
      body = `contains_any:${single}`;
    } else {
      body = `contains_any:${values.join(",")}`;
    }
  } else if (operator === "month_in") {
    const values = Array.isArray(row?.values) ? row.values.filter(Boolean) : [];
    if (!values.length) return "";
    body = `m:${values.join(",")}`;
  } else if (operator === "month_range") {
    const start = String(row?.rangeStart || "").trim();
    const end = String(row?.rangeEnd || "").trim();
    if (!start || !end) return "";
    body = `mrange:${start}..${end}`;
  } else if (operator === "date_range") {
    const start = String(row?.rangeStart || "").trim();
    const end = String(row?.rangeEnd || "").trim();
    if (!start || !end) return "";
    body = `drange:${start}..${end}`;
  } else if (operator === "contains") {
    const value = String(row?.value || "").trim();
    if (!value) return "";
    // 必须带 contains: 前缀：SQL 路径对裸值走等值匹配，关键字过滤会失效。
    body = `contains:${value}`;
  } else if (["eq", "gt", "gte", "lt", "lte"].includes(operator)) {
    const value = String(row?.value ?? "").trim();
    if (!value) return "";
    body = `${operator}:${value}`;
  } else {
    const value = String(row?.value || "").trim();
    if (!value) return "";
    body = value;
  }
  return negate ? `not:${body}` : body;
}

export function decodeFilterRow(encoded, column, profile) {
  const row = createEmptyFilterRow(() => "row-decode");
  row.column = String(column || "").trim();
  const { negate, body } = stripNegate(encoded);
  row.negate = negate;
  if (!body) {
    row.operator = defaultOperatorForProfile(profile);
    return row;
  }
  if (body.startsWith("in:")) {
    row.operator = "in";
    row.values = splitEncodedList(body.slice(3));
    return row;
  }
  if (body.startsWith("contains_any:")) {
    row.operator = "contains_any";
    row.values = splitEncodedList(body.slice("contains_any:".length));
    return row;
  }
  if (body.startsWith("mrange:")) {
    const [start, end] = body.slice(7).split("..");
    row.operator = "month_range";
    row.rangeStart = String(start || "").trim();
    row.rangeEnd = String(end || "").trim();
    return row;
  }
  if (body.startsWith("drange:")) {
    const [start, end] = body.slice(7).split("..");
    row.operator = "date_range";
    row.rangeStart = String(start || "").trim();
    row.rangeEnd = String(end || "").trim();
    return row;
  }
  if (body.startsWith("m:")) {
    row.operator = "month_in";
    row.values = splitEncodedList(body.slice(2));
    return row;
  }
  if (body.startsWith("contains:")) {
    row.operator = "contains";
    row.value = body.slice(9);
    return row;
  }
  for (const operator of ["gte", "lte", "gt", "lt", "eq"]) {
    if (body.startsWith(`${operator}:`)) {
      row.operator = operator;
      row.value = body.slice(operator.length + 1);
      return row;
    }
  }
  if (profile?.kind === "number") {
    row.operator = "eq";
    row.value = body;
    return row;
  }
  if (profile?.kind === "date") {
    const month = extractYearMonth(body);
    row.operator = month ? "month_in" : "contains";
    if (month) row.values = [month];
    else row.value = body;
    return row;
  }
  row.operator = profile?.kind === "enum" ? "in" : "contains";
  if (row.operator === "in") {
    row.values = splitEncodedList(body.startsWith("in:") ? body.slice(3) : body);
  } else {
    row.value = body;
  }
  return row;
}

function fieldQueryKey(field) {
  return String(field?.key || field?.field || field?.column || "").trim();
}

function filterStateKey(field) {
  // Prefer authored filter key (modelType / supervisionCategory) so query_state
  // aligns with dataset filters `… in filter.<key>` and filter_intents dimensions.
  // Fall back to physical column when key is absent.
  return fieldQueryKey(field) || String(field?.column || "").trim();
}

function findCatalogFieldByStateKey(catalog, stateKey) {
  const needle = String(stateKey || "").trim();
  if (!needle) return null;
  return (
    (catalog || []).find((field) => {
      const queryKey = fieldQueryKey(field);
      const column = String(field?.column || "").trim();
      return queryKey === needle || column === needle || filterStateKey(field) === needle;
    }) || null
  );
}

export function filtersToRows(filters, catalog, profiles, nextRowId) {
  const entries = Object.entries(filters || {}).filter(([, raw]) => String(raw ?? "").trim());
  if (!entries.length) {
    return null;
  }
  return entries.map(([stateKey, raw]) => {
    const field = findCatalogFieldByStateKey(catalog, stateKey);
    // 行绑定优先数据列名，保证与 SQL/明细列一致（勿回写成 matter 这类逻辑 key）。
    const rowColumn = field
      ? String(field?.column || fieldQueryKey(field)).trim()
      : stateKey;
    const profile =
      profiles?.get(rowColumn) ||
      profiles?.get(fieldQueryKey(field)) ||
      profiles?.get(stateKey) ||
      null;
    const row = decodeFilterRow(String(raw ?? ""), rowColumn, profile);
    row.id = nextRowId();
    row.status = "active";
    return row;
  });
}

export function schemaToRows(schemaFields, filters, profiles, nextRowId) {
  return (schemaFields || [])
    .map((field) => {
      const column = String(field?.column || field?.key || "").trim();
      if (!column) return null;
      const fieldKey = String(field?.key || column).trim();
      const profile = profiles?.get(column) || null;
      // Prefer filter_schema field key (e.g. dateRange) over column name for query_state round-trip.
      const raw = String(filters?.[fieldKey] ?? filters?.[column] ?? "").trim();
      const row = raw
        ? decodeFilterRow(raw, column, profile)
        : createEmptyFilterRow(() => "schema-empty");
      row.id = nextRowId();
      row.column = column;
      row.label = String(field?.label || column).trim();
      row.fieldKey = fieldKey;
      const hinted = String(field?.operator || field?.default_operator || field?.defaultOperator || "").trim();
      if (hinted) {
        row.operator = hinted;
      } else if (!raw) {
        row.operator = defaultOperatorForProfile(profile, field);
      }
      return row;
    })
    .filter(Boolean);
}
