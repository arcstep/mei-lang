  function monthBucketLabel(value) {
    const raw = String(value || "").trim();
    if (!raw) return "";
    const match = raw.match(/^(\d{4})[-/年](\d{1,2})/);
    if (match) {
      return `${match[1]}-${String(match[2]).padStart(2, "0")}`;
    }
    return raw.slice(0, 7);
  }

  function parseCompositionNumber(value) {
    const parsed = Number(String(value ?? "").replace(/,/g, "").trim());
    return Number.isFinite(parsed) ? parsed : 0;
  }

  function compositionWeightField(columns = [], rows = []) {
    const names = Array.isArray(columns) ? columns : [];
    if (names.includes("预警条数")) return "预警条数";
    return rows.some((row) => row && typeof row === "object" && Object.prototype.hasOwnProperty.call(row, "预警条数"))
      ? "预警条数"
      : "";
  }

  function groupRowsByCount(rows, field, columns = []) {
    const grouped = new Map();
    rows.forEach((row) => {
      const key = String(rowFieldValue(row, field, columns) ?? "").trim() || "未标注";
      grouped.set(key, (grouped.get(key) || 0) + 1);
    });
    return Array.from(grouped.entries())
      .map(([label, value]) => ({ label, value }))
      .sort((a, b) => Number(b.value || 0) - Number(a.value || 0));
  }

  function groupRowsByWeightedSum(rows, field, weightField, columns = []) {
    const grouped = new Map();
    rows.forEach((row) => {
      const key = String(rowFieldValue(row, field, columns) ?? "").trim() || "未标注";
      const weight = parseCompositionNumber(rowFieldValue(row, weightField, columns));
      grouped.set(key, (grouped.get(key) || 0) + (weight > 0 ? weight : 1));
    });
    return Array.from(grouped.entries())
      .map(([label, value]) => ({ label, value }))
      .sort((a, b) => Number(b.value || 0) - Number(a.value || 0));
  }

  function normalizeMetricLocalId(metricId) {
    const text = String(metricId || "").trim();
    if (!text) return "";
    const parts = text.split("::");
    if (parts.length >= 2 && parts[parts.length - 1] === "__scalar_rowset__") {
      return parts[parts.length - 2];
    }
    return parts.length >= 2 ? parts[parts.length - 2] : text;
  }

  function resolveCompositionMetricId(config, detail = null) {
    return nonEmptyString(
      config?.tableMetricId,
      config?.runtimeRef?.metricId,
      config?.runtimeRef?.metric_id,
      detail?.table_metric_id,
    );
  }

  function compositionUsesWarningCountSum(config, detail = null) {
    return normalizeMetricLocalId(resolveCompositionMetricId(config, detail)) === "warnings_count";
  }

  function groupRowsForComposition(rows, field, columns = [], config = null, detail = null) {
    if (compositionUsesWarningCountSum(config, detail) && compositionWeightField(columns, rows)) {
      return groupRowsByWeightedSum(rows, field, "预警条数", columns);
    }
    return groupRowsByCount(rows, field, columns);
  }

  function limitCompositionRows(rows, config = null) {
    const topN = positiveInt(config?.top_n, config?.topN, config?.topN);
    if (!Array.isArray(rows) || topN <= 0 || rows.length <= topN) {
      return rows;
    }
    return rows.slice(0, topN);
  }

  function groupRowsByMonth(rows, field, columns = []) {
    const grouped = new Map();
    rows.forEach((row) => {
      const key = monthBucketLabel(rowFieldValue(row, field, columns));
      if (!key) return;
      grouped.set(key, (grouped.get(key) || 0) + 1);
    });
    return Array.from(grouped.entries())
      .map(([month, value]) => ({ month, value }))
      .sort((a, b) => String(a.month).localeCompare(String(b.month)));
  }

  function buildStaticTablePropsFromRows(title, columns, rows) {
    return {
      title: String(title || ""),
      data: {
        title: String(title || ""),
        columns: Array.isArray(columns) ? columns : [],
        rows: Array.isArray(rows) ? rows : [],
      },
    };
  }

