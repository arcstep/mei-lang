/**
 * 将宿主 table query 响应归一化为组件可消费状态。
 */
export function applyTableQueryResult(state, result, { pagingMode = "server" } = {}) {
  if (!state || !result) return state;
  const next = { ...state };
  next.loading = false;
  next.error = "";
  next.page = Number(result.page) || next.page || 1;
  next.pageSize = Number(result.page_size) || next.pageSize;
  next.total = Number.isFinite(result.total) ? result.total : next.rows?.length || 0;
  next.hasMore = !!result.has_more;
  next.rows = Array.isArray(result.rows) ? result.rows : [];
  if (Array.isArray(result.columns) && result.columns.length > 0) {
    next.columns = result.columns;
  }
  if (Array.isArray(result.column_meta) && result.column_meta.length > 0) {
    next.columnMeta = result.column_meta;
  }
  if (result.summary && typeof result.summary === "object") {
    next.summary = result.summary;
  }
  if (result.query_state_echo) {
    next.queryStateEcho = result.query_state_echo;
    if (result.query_state_echo.column_state) {
      next.columnStateEcho = result.query_state_echo.column_state;
    }
  }
  next.perf = result.perf && typeof result.perf === "object" ? result.perf : null;
  if (pagingMode === "client" && Array.isArray(next.allRows)) {
    next.total = next.allRows.length;
  }
  return next;
}

export function headersFromColumnMeta(columnMeta, columns) {
  if (!Array.isArray(columnMeta) || columnMeta.length === 0) {
    return Array.isArray(columns) ? columns.slice() : [];
  }
  return columnMeta.map((col) => String(col?.name || "").trim()).filter(Boolean);
}
