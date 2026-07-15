/**
 * 表格底部分页区共用的条数总计文案。
 */
export function resolveTableRowCount(total) {
  const n = Number(total);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : 0;
}

export function formatTableRowCountLabel(total, locale = "zh-CN") {
  const n = resolveTableRowCount(total);
  return `共 ${n.toLocaleString(locale)} 条`;
}
