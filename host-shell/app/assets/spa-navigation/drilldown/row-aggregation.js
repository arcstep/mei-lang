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

  function groupRowsByCount(rows, field, columns = [], options = {}) {
    const delimiter = String(options?.delimiter || "").trim();
    const dropEmpty = options?.dropEmpty !== false;
    const grouped = new Map();
    rows.forEach((row) => {
      const raw = String(rowFieldValue(row, field, columns) ?? "").trim();
      const keys = delimiter
        ? raw
            .split(delimiter)
            .map((part) => part.trim())
            .filter(Boolean)
        : [raw || "未标注"];
      if (!keys.length) {
        if (!dropEmpty) grouped.set(raw || "未标注", (grouped.get(raw || "未标注") || 0) + 1);
        return;
      }
      keys.forEach((key) => {
        grouped.set(key, (grouped.get(key) || 0) + 1);
      });
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
    const parts = text.split("::").map((part) => String(part || "").trim()).filter(Boolean);
    if (parts.length >= 2 && parts[parts.length - 1] === "__scalar_rowset__") {
      return parts[parts.length - 2];
    }
    // scenes/Foo.mei::metric_id（及带 explain 后缀）应取 capsule 后的 metric 本地名，而非 scene 路径。
    if (parts.length >= 2 && /\.mei$/i.test(parts[0])) {
      return parts[1];
    }
    return parts.length >= 2 ? parts[parts.length - 2] : text;
  }

  function resolveCardMetricRowsetId(metricId) {
    const text = String(metricId || "").trim();
    if (!text) return "";
    if (text.endsWith("::__scalar_rowset__")) return text;
    return `${text}::__scalar_rowset__`;
  }

  function isScalarRowsetMetricId(metricId) {
    const text = String(metricId || "").trim();
    return text.endsWith("::__scalar_rowset__");
  }

  /** scene-qualified 指标 id 在 `.mei::` 之后是否还带 explain 派生后缀（如 composition_by_agency）。 */
  function sceneQualifiedMetricHasExplainSuffix(metricId) {
    const text = String(metricId || "").trim();
    const marker = ".mei::";
    const sceneIdx = text.indexOf(marker);
    if (sceneIdx <= 0) return false;
    return text.slice(sceneIdx + marker.length).includes("::");
  }

  /** explain 派生的 composition/trend dataframe（服务端已聚合），不是明细 rowset。 */
  function isDedicatedExplainMetricId(metricId, { supportRole = "" } = {}) {
    const text = String(metricId || "").trim();
    if (!text || isScalarRowsetMetricId(text)) return false;
    // `metric::detail` 不是服务端 dataframe；应回退为 `::__scalar_rowset__`。
    if (text.endsWith("::detail")) return false;
    if (sceneQualifiedMetricHasExplainSuffix(text)) return true;
    if (text.includes("::")) return !text.includes(".mei::");
    const role = String(supportRole || "").trim().toLowerCase();
    return role === "composition" || role === "trend" || role === "attribution";
  }

  function resolveDrilldownDetailTableMetricId(config, detail = null) {
    const popupMetricId = resolveDrilldownTableMetricId(detail, config);
    const slotMetricIds = [
      config?.tableMetricId,
      config?.detailSlot?.metricId,
      detail?.table_metric_id,
      config?.runtimeRef?.metricId,
      config?.runtimeRef?.metric_id,
    ]
      .map((value) => String(value || "").trim())
      .filter(Boolean);
    const dedicatedSlotMetricId = slotMetricIds.find(
      (metricId) =>
        isScalarRowsetMetricId(metricId) ||
        isDedicatedExplainMetricId(metricId, { supportRole: config?.supportRole }),
    );
    const raw = nonEmptyString(dedicatedSlotMetricId, popupMetricId, ...slotMetricIds);
    if (!raw) return "";
    if (isScalarRowsetMetricId(raw)) return raw;
    if (isDedicatedExplainMetricId(raw, { supportRole: config?.supportRole })) return raw;
    return resolveCardMetricRowsetId(raw);
  }

  function resolveDrilldownFetchPageSize(config, { previewRow = false, clientAggregate = false } = {}) {
    if (clientAggregate) return 100000;
    // previewRow is used for identity drilldown (object focus). Scalar rowset SQL may not
    // apply filter.warningId; fetch a window and pick the matching row client-side.
    if (previewRow) return 500;
    const dedicated = isDedicatedExplainMetricId(resolveCompositionMetricId(config), {
      supportRole: config?.supportRole,
    });
    if (dedicated) {
      const topN = positiveInt(config?.top_n, config?.topN, config?.topN);
      return topN > 0 ? Math.max(topN, 16) : 64;
    }
    const tablePage = positiveInt(config?.pageSize, config?.page_size);
    if (tablePage > 0) return tablePage;
    return 20;
  }

  function resolveCompositionMetricId(config, detail = null) {
    return nonEmptyString(
      config?.tableMetricId,
      config?.runtimeRef?.metricId,
      config?.runtimeRef?.metric_id,
      detail?.table_metric_id,
    );
  }

  /** 从父 metric + explain block id 推导服务端 composition dataframe（如 inspections_total_count::composition_by_agency）。 */
  function resolveCompositionScopedMetricId(parentMetricId, explainBlockId) {
    const parent = String(parentMetricId || "").trim();
    const blockId = String(explainBlockId || "").trim();
    if (!parent || !blockId) return "";
    if (blockId === "composition" || blockId === "metric" || blockId === "detail" || blockId === "trend") {
      return "";
    }
    const scoped = `${parent}::${blockId}`;
    if (isScalarRowsetMetricId(scoped)) return "";
    return scoped;
  }

  function resolveCompositionDelimiter(field, config = null, detail = null) {
    const explicit = nonEmptyString(
      config?.delimiter,
      config?.compositionDelimiter,
      config?.composition_delimiter,
      detail?.delimiter,
    );
    if (explicit) return explicit;
    // 风险等级多标签（蓝/黄/红）：客户端重聚合时按「/」拆分做 membership 计数
    if (String(field || "").trim() === "风险等级") return "/";
    return "";
  }

  function groupRowsForComposition(rows, field, columns = [], config = null, detail = null) {
    const valueField = resolveCompositionValueField(config, detail);
    if (compositionUsesWeightedSum(config, detail, columns, rows) && valueField) {
      return groupRowsByWeightedSum(rows, field, valueField, columns);
    }
    return groupRowsByCount(rows, field, columns, {
      delimiter: resolveCompositionDelimiter(field, config, detail),
      dropEmpty: true,
    });
  }

  function limitCompositionRows(rows, config = null) {
    const topN = positiveInt(config?.top_n, config?.topN, config?.topN);
    if (!Array.isArray(rows) || topN <= 0 || rows.length <= topN) {
      return rows;
    }
    return rows.slice(0, topN);
  }

  function resolveCompositionYDisplayName(config, detail = null, yField = "value") {
    const mappingY = config?.mapping?.y;
    if (Array.isArray(mappingY)) {
      for (const item of mappingY) {
        if (!item || typeof item !== "object") continue;
        if (String(item.field || "").trim() !== yField) continue;
        const mappedName = nonEmptyString(item.name);
        if (mappedName && mappedName !== yField) return mappedName;
      }
    }
    const valueField = resolveCompositionValueField(config, detail);
    if (valueField && valueField !== yField) return valueField;
    const contract =
      detail?.analysis_contract && typeof detail.analysis_contract === "object"
        ? detail.analysis_contract
        : null;
    const title = nonEmptyString(detail?.label, contract?.title, config?.title);
    const unit = nonEmptyString(detail?.unit, contract?.unit, config?.unit, config?.metricUnit);
    if (title && unit) return `${title}（${unit}）`;
    if (unit) return unit;
    return yField;
  }

  function buildDefaultCompositionMapping(config, detail = null, xField, yField = "value") {
    const yDisplayName = resolveCompositionYDisplayName(config, detail, yField);
    const defaults = {
      x: [{ field: xField, name: xField }],
      y: [{ field: yField, name: yDisplayName }],
      label: [{ field: xField, name: xField }],
    };
    const override = config?.mapping;
    if (!override || typeof override !== "object" || Array.isArray(override)) {
      return defaults;
    }
    const merged = {
      x: Array.isArray(override.x) && override.x.length ? override.x : defaults.x,
      y: Array.isArray(override.y) && override.y.length ? override.y : defaults.y,
      label: Array.isArray(override.label) && override.label.length ? override.label : defaults.label,
    };
    if (Array.isArray(override.group) && override.group.length) {
      merged.group = override.group;
    }
    return merged;
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
