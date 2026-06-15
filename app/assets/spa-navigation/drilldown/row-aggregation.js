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

  function resolveCompositionValueField(config, detail = null) {
    return nonEmptyString(
      config?.valueField,
      config?.value_field,
      config?.compositionValueField,
      config?.composition_value_field,
      config?.weightField,
      config?.weight_field,
      detail?.value_field,
      detail?.valueField,
    );
  }

  function resolveCompositionAgg(config, detail = null) {
    return nonEmptyString(
      config?.compositionAgg,
      config?.composition_agg,
      config?.agg,
      detail?.agg,
    ).toLowerCase();
  }

  function compositionUsesWeightedSum(config, detail = null, columns = [], rows = []) {
    const valueField = resolveCompositionValueField(config, detail);
    if (!valueField) return false;
    const agg = resolveCompositionAgg(config, detail);
    if (agg === "count") return false;
    if (agg === "sum" || agg === "weighted_sum" || agg === "weighted") return true;
    if (agg) return agg !== "count";
    const hasNumericWeight = rows.some((row) => {
      if (!row || typeof row !== "object") return false;
      const raw = rowFieldValue(row, valueField, columns);
      return parseCompositionNumber(raw) > 0;
    });
    return hasNumericWeight;
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

  function resolveCardMetricRowsetId(metricId) {
    const text = String(metricId || "").trim();
    if (!text) return "";
    if (text.endsWith("::__scalar_rowset__")) return text;
    return `${text}::__scalar_rowset__`;
  }

  function resolveCompositionMetricId(config, detail = null) {
    return nonEmptyString(
      config?.tableMetricId,
      config?.runtimeRef?.metricId,
      config?.runtimeRef?.metric_id,
      detail?.table_metric_id,
    );
  }

  function groupRowsForComposition(rows, field, columns = [], config = null, detail = null) {
    const valueField = resolveCompositionValueField(config, detail);
    if (compositionUsesWeightedSum(config, detail, columns, rows) && valueField) {
      return groupRowsByWeightedSum(rows, field, valueField, columns);
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
