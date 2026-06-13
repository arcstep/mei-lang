  function runtimeDrilldownConfig(detail) {
    const value =
      detail?.analysis_contract &&
      typeof detail.analysis_contract === "object" &&
      !Array.isArray(detail.analysis_contract)
        ? detail.analysis_contract
        : detail?.__mei_runtime_ref?.analysis_contract &&
            typeof detail.__mei_runtime_ref.analysis_contract === "object" &&
            !Array.isArray(detail.__mei_runtime_ref.analysis_contract)
          ? detail.__mei_runtime_ref.analysis_contract
          : null;
    if (!value) {
      return {};
    }
    return value;
  }

  function disabledDrilldownConfig(errorCode, errorMessage) {
    return {
      enabled: false,
      errorCode: String(errorCode || "").trim(),
      errorMessage: String(errorMessage || "").trim(),
      sceneId: "",
      hostSceneId: "",
      boardSceneId: "",
      popup: {},
    };
  }

  function runtimeTabIds(...values) {
    const raw = values.find((value) => Array.isArray(value));
    if (!raw) return [];
    return Array.from(
      new Set(
        raw
      .map((entry) => {
        if (entry && typeof entry === "object") {
          return nonEmptyString(entry.id, entry.tab, entry.key, entry.name);
        }
        return String(entry || "").trim();
      })
          .map((entry) => normalizeTabId(entry))
          .filter(Boolean),
      ),
    );
  }

  function normalizeTabId(value) {
    const raw = String(value || "").trim().toLowerCase();
    if (!raw) return "";
    if (["口径", "definition", "def", "metric_definition", "metric-definition"].includes(raw)) {
      return "definition";
    }
    if (["构成", "composition", "breakdown", "group"].includes(raw)) {
      return "composition";
    }
    if (["趋势", "trend", "timeseries", "time_series", "time-series"].includes(raw)) {
      return "trend";
    }
    if (
      ["分子分母", "ratio", "numerator_denominator", "numerator-denominator", "numerator"].includes(
        raw,
      )
    ) {
      return "numerator_denominator";
    }
    if (["归因", "attribution", "reason"].includes(raw)) {
      return "attribution";
    }
    if (["明细", "detail", "details"].includes(raw)) {
      return "detail";
    }
    return raw.replaceAll(/[\s-]+/g, "_");
  }

  function normalizeExplainMetrics(...values) {
    const byId = {};
    const order = [];
    const pushEntry = (entry, fallbackId = "", fallbackLabel = "") => {
      if (!entry || typeof entry !== "object" || Array.isArray(entry)) return;
      const id = normalizeTabId(entry.id || fallbackId || "");
      const kind = normalizeTabId(entry.kind || id);
      if (!id || !kind || kind === "note") return;
      if (byId[id]) return;
      byId[id] = {
        id,
        kind,
        label: nonEmptyString(entry.label, fallbackLabel, fallbackId),
        by: nonEmptyString(entry.by),
        dateField: nonEmptyString(entry.date_field, entry.dateField),
        grain: nonEmptyString(entry.grain),
        fields: cloneArray(entry.fields),
        headers: cloneArray(entry.headers),
        mapping: entry.mapping && typeof entry.mapping === "object" ? entry.mapping : null,
        chartKind: nonEmptyString(entry.chart_kind, entry.chartKind),
        source: entry.source && typeof entry.source === "object" && !Array.isArray(entry.source) ? entry.source : null,
        tableMetricId: nonEmptyString(entry.table_metric_id, entry.tableMetricId, entry.metric_id, entry.metricId),
        datasetId: nonEmptyString(entry.dataset_id, entry.datasetId),
        topN: positiveInt(entry.top_n, entry.topN),
        numerator: nonEmptyString(entry.numerator),
        denominator: nonEmptyString(entry.denominator),
        formula: nonEmptyString(entry.formula),
      };
      order.push(id);
    };
    values.forEach((value) => {
      if (Array.isArray(value)) {
        value.forEach((entry) => pushEntry(entry));
        return;
      }
      if (value && typeof value === "object") {
        Object.entries(value).forEach(([key, entry]) => pushEntry(entry, key, key));
      }
    });
    return { byId, order };
  }

  function explainMetricForTab(config, tabId) {
    const key = normalizeTabId(tabId);
    if (!key) return null;
    const byId = config?.explainMetrics;
    if (!byId || typeof byId !== "object") return null;
    if (byId[key]) return byId[key];
    const genericKinds = new Set([
      "definition",
      "composition",
      "trend",
      "numerator_denominator",
      "attribution",
      "detail",
    ]);
    if (!genericKinds.has(key)) return null;
    const order = Array.isArray(config?.explainMetricOrder) ? config.explainMetricOrder : Object.keys(byId);
    for (const candidateId of order) {
      const entry = byId[normalizeTabId(candidateId)];
      if (!entry) continue;
      if (normalizeTabId(entry.kind || entry.id) === key) return entry;
    }
    return null;
  }

  function compositionFieldsFromOverride(override) {
    if (!override || typeof override !== "object") return [];
    return cloneArray(override.compositionBy).length
      ? cloneArray(override.compositionBy)
      : cloneArray(override.composition_by);
  }

  function compositionFieldForTab(config, tabId, override = null) {
    const exactTab = normalizeTabId(tabId);
    const slot = config?.slotByTab?.[exactTab];
    if (slot) {
      const fromSlot = compositionFieldsFromOverride(slot)[0];
      if (fromSlot) return fromSlot;
    }
    const explainMetric =
      explainMetricForTab(config, exactTab) || explainMetricForTab(config, explainMetricKind(config, tabId));
    const fromExplain = nonEmptyString(explainMetric?.by);
    if (fromExplain) return fromExplain;
    const fromOverride = nonEmptyString(compositionFieldsFromOverride(override)[0]);
    if (fromOverride) return fromOverride;
    const fromConfig = nonEmptyString(
      Array.isArray(config?.compositionBy) ? config.compositionBy[0] : "",
      Array.isArray(config?.recommendedDimensions) ? config.recommendedDimensions[0] : "",
    );
    return fromConfig;
  }

  function rowFieldValue(row, field, columns = []) {
    const name = String(field || "").trim();
    if (!name || !row || typeof row !== "object") return "";
    if (Object.prototype.hasOwnProperty.call(row, name)) {
      return row[name];
    }
    const trimmed = name.trim();
    for (const [key, value] of Object.entries(row)) {
      if (String(key).trim() === trimmed) return value;
    }
    const column = columns.find((entry) => String(entry || "").trim() === trimmed);
    if (column && Object.prototype.hasOwnProperty.call(row, column)) {
      return row[column];
    }
    return "";
  }

  function explainMetricKind(config, tabId) {
    const slot = config?.slotByTab?.[normalizeTabId(tabId)];
    if (slot?.supportRole) return normalizeTabId(slot.supportRole);
    const explainMetric = explainMetricForTab(config, tabId);
    if (explainMetric?.kind) return normalizeTabId(explainMetric.kind);
    return normalizeTabId(tabId);
  }

  function defaultDrilldownTabs(explainKind, options = {}) {
    const kind = String(explainKind || "").trim().toLowerCase();
    const hasDetail = options.hasDetail !== false;
    const aggregateKinds = new Set(["count", "sum", "avg", "average", "median"]);
    const ratioKinds = new Set(["ratio", "percent", "yoy", "mom"]);
    const trendKinds = new Set(["trend", "dataframe", "timeseries", "series"]);
    const breakdownKinds = new Set(["ranking", "breakdown", "group", "group_by", "groupby"]);

    if (aggregateKinds.has(kind)) {
      return hasDetail
        ? ["definition", "composition", "trend", "detail"]
        : ["definition", "composition", "trend"];
    }
    if (ratioKinds.has(kind)) {
      return hasDetail
        ? ["definition", "numerator_denominator", "trend", "detail"]
        : ["definition", "numerator_denominator", "trend"];
    }
    if (trendKinds.has(kind)) {
      return hasDetail
        ? ["definition", "trend", "composition", "detail"]
        : ["definition", "trend", "composition"];
    }
    if (breakdownKinds.has(kind)) {
      return hasDetail
        ? ["definition", "composition", "attribution", "detail"]
        : ["definition", "composition", "attribution"];
    }
    return hasDetail ? ["definition", "detail"] : ["definition"];
  }

  function buildRatioExplainNote({ numerator = "", denominator = "", formula = "" } = {}) {
    const n = String(numerator || "").trim();
    const d = String(denominator || "").trim();
    const f = String(formula || "").trim();
    if (f) return `分子分母口径：${f}`;
    if (n && d) return `分子分母口径：${n} / ${d}`;
    return "";
  }

  function panelPopupSlotSources(popup) {
    if (!popup || typeof popup !== "object") return null;
    if (popup.entry_overrides && typeof popup.entry_overrides === "object" && !Array.isArray(popup.entry_overrides)) {
      return popup.entry_overrides;
    }
    if (popup.entryOverrides && typeof popup.entryOverrides === "object" && !Array.isArray(popup.entryOverrides)) {
      return popup.entryOverrides;
    }
    if (popup.slots && typeof popup.slots === "object" && !Array.isArray(popup.slots)) {
      return popup.slots;
    }
    if (popup.metrics && typeof popup.metrics === "object" && !Array.isArray(popup.metrics)) {
      return popup.metrics;
    }
    return null;
  }

  function resolveDrilldownTabs({ detail, runtime, explainKind, hasDetail, localNav }) {
    const resolvedLocalNav =
      normalizeSceneLocalNav(localNav) ||
      resolveSceneLocalNav(
        nonEmptyString(detail?.board_scene_file, detail?.scene_path, detail?.popup?.scene_file),
        detail?.scene_local_nav_by_target,
      );
    const explainMetrics = normalizeExplainMetrics(
      detail?.analysis_contract?.blocks,
      detail?.analysis_contract?.explain_metrics,
      detail?.analysis_contract?.explainMetrics,
      detail?.explain_metrics,
      runtime?.blocks,
      runtime?.explain_metrics,
      runtime?.explainMetrics,
    );
    const explicitExplainTabs = explainMetrics.order.filter((tab) => normalizeTabId(tab) !== "hero");
    const popup = detail?.popup && typeof detail.popup === "object" ? detail.popup : {};
    const popupFocus = normalizeTabId(popup?.entry || popup?.entry_tab || popup?.focus);
    const overrideTabs = Object.keys(
      normalizeTabMetricOverrides(
        popup?.entry_overrides,
        popup?.entryOverrides,
        panelPopupSlotSources(popup),
        popup?.metrics,
      ),
    );
    let merged = explicitExplainTabs.length
      ? explicitExplainTabs
      : defaultDrilldownTabs(explainKind, { hasDetail });
    overrideTabs.forEach((tab) => {
      const normalized = normalizeTabId(tab);
      if (!normalized || merged.includes(normalized)) return;
      merged.push(normalized);
    });
    if (popupFocus && !merged.includes(popupFocus)) {
      merged.push(popupFocus);
    }
    merged = Array.from(new Set(merged)).filter((tab) => tab !== "hero");
    if (merged.length) {
      const kindOrder =
        Array.isArray(resolvedLocalNav?.kindOrder) && resolvedLocalNav.kindOrder.length
          ? resolvedLocalNav.kindOrder
          : SCENE_KIND_ORDER_FALLBACK;
      const kindRank = new Map(kindOrder.map((kind, index) => [normalizeTabId(kind), index]));
      merged.sort((left, right) => {
        const leftKind = normalizeTabId(explainMetrics.byId[left]?.kind || left);
        const rightKind = normalizeTabId(explainMetrics.byId[right]?.kind || right);
        const leftRank = kindRank.has(leftKind) ? kindRank.get(leftKind) : Number.MAX_SAFE_INTEGER;
        const rightRank = kindRank.has(rightKind) ? kindRank.get(rightKind) : Number.MAX_SAFE_INTEGER;
        if (leftRank !== rightRank) return leftRank - rightRank;
        return left.localeCompare(right);
      });
      return merged;
    }
    const explicit = runtimeTabIds(
      detail?.analysis_contract?.tabs,
      runtime?.tabs,
      detail?.analysis_tabs,
      runtime?.analysis_tabs,
    );
    const defaults = defaultDrilldownTabs(explainKind, { hasDetail });
    if (!explicit.length) return defaults;
    const normalizedExplicit = Array.from(new Set(explicit.map((tab) => normalizeTabId(tab)).filter(Boolean)));
    const basicTabs = new Set(["definition", "detail", "numerator_denominator"]);
    const hasOnlyBasicTabs = normalizedExplicit.every((tab) => basicTabs.has(tab));
    if (!hasOnlyBasicTabs) return normalizedExplicit;
    const mergedTabs = normalizedExplicit.filter((tab) => tab !== "detail");
    defaults.forEach((tab) => {
      const normalized = normalizeTabId(tab);
      if (!normalized || normalized === "detail" || mergedTabs.includes(normalized)) return;
      mergedTabs.push(normalized);
    });
    if (normalizedExplicit.includes("detail") || defaults.includes("detail")) {
      mergedTabs.push("detail");
    }
    return mergedTabs;
  }

