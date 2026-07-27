import { getQueryState, mergeFilters } from "../runtime-query.js";

export function sameFilters(left, right) {
  const leftKeys = Object.keys(left || {});
  const rightKeys = Object.keys(right || {});
  if (leftKeys.length !== rightKeys.length) return false;
  return leftKeys.every((key) => String(left[key] || "") === String(right[key] || ""));
}

export function sharedFiltersForProps(props, queryStateId) {
  const id = String(queryStateId || "").trim();
  if (!id) return {};
  return getQueryState(id).filters || {};
}

export function activeTableFilters(props, queryStateId, localFilters = {}) {
  const defaultFilters =
    props?.default_filters && typeof props.default_filters === "object" && !Array.isArray(props.default_filters)
      ? props.default_filters
      : props?.defaultFilters && typeof props.defaultFilters === "object" && !Array.isArray(props.defaultFilters)
        ? props.defaultFilters
        : {};
  const scopeFilters =
    props?.scope_filters && typeof props.scope_filters === "object" && !Array.isArray(props.scope_filters)
      ? props.scope_filters
      : props?.scopeFilters && typeof props.scopeFilters === "object" && !Array.isArray(props.scopeFilters)
        ? props.scopeFilters
        : {};
  const identityFilters =
    props?.drilldown_filters && typeof props.drilldown_filters === "object" && !Array.isArray(props.drilldown_filters)
      ? props.drilldown_filters
      : props?.drilldownFilters && typeof props.drilldownFilters === "object" && !Array.isArray(props.drilldownFilters)
        ? props.drilldownFilters
        : {};
  const id = String(queryStateId || "").trim();
  // 024005：QS 绑定时 seed 不盖面板；scope / identity 始终 AND（后写覆盖同维）。
  const base = id ? sharedFiltersForProps(props, id) : defaultFilters;
  return mergeFilters(base, scopeFilters, identityFilters, localFilters);
}

export function normalizeSort(sort) {
  if (!Array.isArray(sort)) return [];
  return sort
    .map((item) => ({
      field: String(item?.field || "").trim(),
      direction: String(item?.direction || "asc").trim().toLowerCase() || "asc",
    }))
    .filter((item) => item.field);
}

export function sameSort(left, right) {
  const lhs = normalizeSort(left);
  const rhs = normalizeSort(right);
  if (lhs.length !== rhs.length) return false;
  return lhs.every(
    (item, index) => item.field === rhs[index]?.field && item.direction === rhs[index]?.direction
  );
}

function collectColumnKeys(props) {
  const keys = [];
  const push = (value) => {
    const name = String(value || "").trim();
    if (name) keys.push(name);
  };
  (Array.isArray(props?.columns) ? props.columns : []).forEach(push);
  (Array.isArray(props?.dataset?.columns) ? props.dataset.columns : []).forEach(push);
  const stateColumns = props?.column_state?.columns || props?.columnState?.columns;
  (Array.isArray(stateColumns) ? stateColumns : []).forEach((entry) => push(entry?.key));
  return keys;
}

/** 有序号列且未指定排序时，默认按序号升序。 */
export function inferDefaultSortFromColumns(props) {
  const serial = collectColumnKeys(props).find(
    (name) => name === "序号" || name.endsWith("序号")
  );
  return serial ? [{ field: serial, direction: "asc" }] : [];
}

export function resolveSortConfig(props, fallback = []) {
  const raw = props?.sort ?? props?.defaultSort ?? props?.default_sort;
  if (Array.isArray(raw)) {
    const normalized = normalizeSort(raw);
    if (normalized.length > 0) return normalized;
    // 显式空数组：仍回落到序号默认序（用户通过表头三次点击清空后走 localSort=[]，不经此路径）
  } else if (typeof raw === "string") {
    const text = raw.trim();
    if (text) {
      try {
        const parsed = JSON.parse(text);
        if (Array.isArray(parsed)) {
          const normalized = normalizeSort(parsed);
          if (normalized.length > 0) return normalized;
        }
      } catch (_) {
        /* fall through */
      }
    }
  } else {
    const fromFallback = normalizeSort(fallback);
    if (fromFallback.length > 0) return fromFallback;
  }
  const inferred = inferDefaultSortFromColumns(props);
  if (inferred.length > 0) return inferred;
  return normalizeSort(Array.isArray(raw) ? [] : fallback);
}

export function sharedSortForProps(props, queryStateId) {
  const id = String(queryStateId || "").trim();
  if (!id) return [];
  return normalizeSort(getQueryState(id).sort || []);
}

export function activeTableSort(props, queryStateId, localSort = []) {
  const shared = sharedSortForProps(props, queryStateId);
  if (shared.length > 0) return shared;
  return normalizeSort(localSort);
}

export function cycleSingleColumnSort(currentSort, field) {
  const normalizedField = String(field || "").trim();
  if (!normalizedField) return [];
  const current = normalizeSort(currentSort);
  const active = current.find((item) => item.field === normalizedField);
  if (!active) {
    return [{ field: normalizedField, direction: "asc" }];
  }
  if (active.direction === "asc") {
    return [{ field: normalizedField, direction: "desc" }];
  }
  return [];
}

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

function normalizePixelValue(value) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric) || numeric <= 0) return null;
  return Math.round(numeric);
}

function normalizeAlign(value) {
  const text = String(value || "").trim().toLowerCase();
  return ["left", "center", "right", "justify"].includes(text) ? text : null;
}

function normalizeVerticalAlign(value) {
  const text = String(value || "").trim().toLowerCase();
  return ["top", "middle", "bottom"].includes(text) ? text : null;
}

function normalizeBoolOption(value) {
  if (value === true || value === false) return value;
  if (typeof value === "string") {
    const text = value.trim().toLowerCase();
    if (text === "true") return true;
    if (text === "false") return false;
  }
  return null;
}

function normalizeColumnStateEntry(entry, index) {
  const key = String(entry?.key || entry?.field || entry?.name || "").trim();
  if (!key) return null;
  const rawHidden = entry?.hidden;
  const hidden =
    rawHidden === true || rawHidden === "true" || rawHidden === 1 || rawHidden === "1";
  const rawOrder = Number(entry?.order ?? index);
  const order = Number.isFinite(rawOrder) ? Math.round(rawOrder) : index;
  return {
    key,
    hidden,
    order,
    width: normalizePixelValue(entry?.width),
    min_width: normalizePixelValue(entry?.min_width ?? entry?.minWidth),
    max_width: normalizePixelValue(entry?.max_width ?? entry?.maxWidth),
    align: normalizeAlign(entry?.align),
    valign: normalizeVerticalAlign(entry?.valign ?? entry?.vertical_align ?? entry?.verticalAlign),
    header_align: normalizeAlign(entry?.header_align ?? entry?.headerAlign ?? entry?.align),
    header_valign: normalizeVerticalAlign(
      entry?.header_valign ?? entry?.headerValign ?? entry?.valign ?? entry?.vertical_align
    ),
    wrap: normalizeBoolOption(entry?.wrap),
    header_wrap: normalizeBoolOption(entry?.header_wrap ?? entry?.headerWrap ?? entry?.wrap),
  };
}

export function normalizeColumnState(raw) {
  const parsed = parseObjectLike(raw, null);
  const columns = Array.isArray(parsed?.columns)
    ? parsed.columns
    : Array.isArray(parsed)
      ? parsed
      : [];
  const normalized = columns
    .map((entry, index) => normalizeColumnStateEntry(entry, index))
    .filter(Boolean);
  return { columns: normalized };
}

export function sameColumnState(left, right) {
  const lhs = normalizeColumnState(left).columns;
  const rhs = normalizeColumnState(right).columns;
  if (lhs.length !== rhs.length) return false;
  return lhs.every((entry, index) => JSON.stringify(entry) === JSON.stringify(rhs[index] || null));
}

export function resolveColumnStateConfig(props, fallback = null) {
  const raw = props?.columnState ?? props?.column_state;
  if (!raw) return normalizeColumnState(fallback);
  return normalizeColumnState(raw);
}

export function sharedColumnStateForProps(props, queryStateId) {
  const id = String(queryStateId || "").trim();
  if (!id) return { columns: [] };
  const state = getQueryState(id);
  return normalizeColumnState(state.column_state || state.columnState || null);
}

export function activeTableColumnState(props, queryStateId, localColumnState = null) {
  const shared = sharedColumnStateForProps(props, queryStateId);
  if (shared.columns.length > 0) return shared;
  return normalizeColumnState(localColumnState);
}

export function ensureColumnStateForKeys(columnState, keys) {
  const normalized = normalizeColumnState(columnState);
  const byKey = new Map(normalized.columns.map((entry) => [entry.key, entry]));
  const next = [];
  (Array.isArray(keys) ? keys : []).forEach((key, index) => {
    const normalizedKey = String(key || "").trim();
    if (!normalizedKey) return;
    const existing = byKey.get(normalizedKey);
    next.push(
      existing
        ? { ...existing, order: Number.isFinite(existing.order) ? existing.order : index }
        : { key: normalizedKey, hidden: false, order: index }
    );
  });
  normalized.columns.forEach((entry) => {
    if (!next.find((item) => item.key === entry.key)) {
      next.push(entry);
    }
  });
  next.sort((left, right) => (left.order ?? 0) - (right.order ?? 0));
  return { columns: next.map((entry, index) => ({ ...entry, order: index })) };
}

export function withColumnVisibility(columnState, key, visible) {
  const normalizedKey = String(key || "").trim();
  const next = ensureColumnStateForKeys(columnState, [normalizedKey, ...normalizeColumnState(columnState).columns.map((entry) => entry.key)]);
  return {
    columns: next.columns.map((entry) =>
      entry.key === normalizedKey ? { ...entry, hidden: !visible } : entry
    ),
  };
}

export function withColumnOrder(columnState, orderedKeys) {
  const keys = Array.isArray(orderedKeys) ? orderedKeys.map((item) => String(item || "").trim()).filter(Boolean) : [];
  const next = ensureColumnStateForKeys(columnState, keys);
  const orderByKey = new Map(keys.map((key, index) => [key, index]));
  return {
    columns: next.columns
      .map((entry, index) => ({
        ...entry,
        order: orderByKey.has(entry.key) ? orderByKey.get(entry.key) : keys.length + index,
      }))
      .sort((left, right) => (left.order ?? 0) - (right.order ?? 0))
      .map((entry, index) => ({ ...entry, order: index })),
  };
}
