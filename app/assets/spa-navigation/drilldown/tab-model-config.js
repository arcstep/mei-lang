  function readGlobalSceneDrilldownContext() {
    if (typeof window === "undefined") return null;
    const cached = window.__meiSceneDrilldownContext;
    if (cached && typeof cached === "object" && !Array.isArray(cached)) {
      return cached;
    }
    const script = document.getElementById("mei-scene-drilldown-context");
    const raw = String(script?.textContent || "").trim();
    if (!raw) return null;
    try {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        window.__meiSceneDrilldownContext = parsed;
        return parsed;
      }
    } catch (_) {
      /* ignore */
    }
    return null;
  }

  function sceneDrilldownContextMap(detail, key) {
    const local = detail?.[key];
    if (local && typeof local === "object" && !Array.isArray(local)) {
      return local;
    }
    const global = readGlobalSceneDrilldownContext();
    const value = global?.[key];
    return value && typeof value === "object" && !Array.isArray(value) ? value : null;
  }

  function sceneDrilldownAssemblyById(detail) {
    return sceneDrilldownContextMap(detail, "scene_projection_assembly_by_id");
  }

  function sceneProjectionAssembly(sceneId, assemblyById) {
    const normalizedSceneId = nonEmptyString(sceneId);
    if (!normalizedSceneId) return null;
    if (
      !assemblyById ||
      typeof assemblyById !== "object" ||
      Array.isArray(assemblyById) ||
      !assemblyById[normalizedSceneId] ||
      typeof assemblyById[normalizedSceneId] !== "object" ||
      Array.isArray(assemblyById[normalizedSceneId])
    ) {
      return null;
    }
    return assemblyById[normalizedSceneId];
  }

  function sceneBindingDefaults(sceneId, bindingsById, examplesById, assemblyById = null) {
    const normalizedSceneId = nonEmptyString(sceneId);
    if (!normalizedSceneId) return {};
    const assembly = sceneProjectionAssembly(normalizedSceneId, assemblyById);
    if (assembly) {
      const direct = normalizeTabMetricOverrides(assembly.bindings);
      if (Object.keys(direct).length) return direct;
      const rawExamples = assembly.examples;
      const example = Array.isArray(rawExamples)
        ? rawExamples.find((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
        : rawExamples && typeof rawExamples === "object"
          ? rawExamples
          : null;
      const bindings =
        example && typeof example === "object" && !Array.isArray(example) ? example.bindings : null;
      const normalized = normalizeTabMetricOverrides(bindings);
      if (Object.keys(normalized).length) return normalized;
    }
    if (
      bindingsById &&
      typeof bindingsById === "object" &&
      !Array.isArray(bindingsById) &&
      bindingsById[normalizedSceneId]
    ) {
      const direct = normalizeTabMetricOverrides(bindingsById[normalizedSceneId]);
      if (Object.keys(direct).length) return direct;
    }
    if (
      examplesById &&
      typeof examplesById === "object" &&
      !Array.isArray(examplesById) &&
      examplesById[normalizedSceneId]
    ) {
      const rawExamples = examplesById[normalizedSceneId];
      const example = Array.isArray(rawExamples)
        ? rawExamples.find((entry) => entry && typeof entry === "object" && !Array.isArray(entry))
        : rawExamples && typeof rawExamples === "object"
          ? rawExamples
          : null;
      const bindings =
        example && typeof example === "object" && !Array.isArray(example) ? example.bindings : null;
      const normalized = normalizeTabMetricOverrides(bindings);
      if (Object.keys(normalized).length) return normalized;
    }
    return {};
  }

  function resolveDrilldownTabConfig(config, tabId) {
    const tabMetrics = config?.tabMetrics || {};
    const exactTab = normalizeTabId(tabId);
    const kindTab = explainMetricKind(config, tabId);
    const override = tabMetrics[exactTab] || tabMetrics[kindTab];
    const explainMetricTab = normalizeTabId(
      nonEmptyString(override?.explainMetricId, exactTab, kindTab)
    );
    const explainMetric =
      explainMetricForTab(config, explainMetricTab) ||
      explainMetricForTab(config, exactTab) ||
      explainMetricForTab(config, kindTab);
    if (!override && !explainMetric) return config;
    const overrideDatasetId = nonEmptyString(override?.datasetId);
    const overrideTableMetricId = nonEmptyString(override?.tableMetricId);
    const suppressDetailMetricFallback = Boolean(overrideDatasetId && !overrideTableMetricId);
    const merged = {
      ...config,
      title: nonEmptyString(override?.title, override?.label, explainMetric?.label, config.title),
      note: nonEmptyString(override?.note, config.note),
      tableMetricId:
        overrideTableMetricId ||
        (overrideDatasetId ? "" : nonEmptyString(explainMetric?.tableMetricId, config.tableMetricId)),
      datasetId: overrideDatasetId || nonEmptyString(override?.runtimeRef?.datasetId, explainMetric?.datasetId, config.datasetId),
      suppressDetailMetricFallback,
      layoutPreset: nonEmptyString(override?.layoutPreset, config.layoutPreset),
      chartKind: nonEmptyString(override?.chartKind, explainMetric?.chartKind, config.chartKind),
      topN: positiveInt(
        override?.top_n,
        override?.topN,
        explainMetric?.topN,
        config?.top_n,
        config?.topN,
      ),
      mapping:
        override?.mapping && typeof override.mapping === "object"
          ? override.mapping
          : explainMetric?.mapping && typeof explainMetric.mapping === "object"
            ? explainMetric.mapping
          : config.mapping && typeof config.mapping === "object"
            ? config.mapping
            : null,
      runtimeRef: (() => {
        const base =
          override?.runtimeRef && typeof override.runtimeRef === "object"
            ? { ...override.runtimeRef }
            : explainMetric?.source && typeof explainMetric.source === "object"
              ? {
                  kind:
                    nonEmptyString(explainMetric.source.kind) === "dataset_ref"
                      ? "data"
                      : nonEmptyString(explainMetric.source.kind) === "metric_ref"
                        ? "metric"
                        : nonEmptyString(explainMetric.source.kind),
                  metricId: nonEmptyString(explainMetric.source.metric_id, explainMetric.source.metricId),
                  datasetId: nonEmptyString(explainMetric.source.dataset_id, explainMetric.source.datasetId),
                  sceneId: nonEmptyString(explainMetric.source.scene_id, explainMetric.source.sceneId),
                  scenePath: nonEmptyString(explainMetric.source.scene_file, explainMetric.source.sceneFile),
                }
            : config.runtimeRef && typeof config.runtimeRef === "object"
              ? { ...config.runtimeRef }
              : null;
        if (!base) return null;
        if (!nonEmptyString(base.sceneId)) {
          base.sceneId = nonEmptyString(config.hostSceneId, config.sceneId);
        }
        if (!nonEmptyString(base.scenePath)) {
          base.scenePath = nonEmptyString(
            config.hostSceneFile,
            resolveMetricOwnerScenePath(
              config?.slotByTab ? Object.values(config.slotByTab) : [],
              null,
            ),
          );
        }
        return base;
      })(),
      columns: cloneArray(override?.columns).length
        ? cloneArray(override.columns)
        : cloneArray(explainMetric?.fields).length
          ? cloneArray(explainMetric.fields)
          : cloneArray(config.columns),
      headers: cloneArray(override?.headers).length
        ? cloneArray(override.headers)
        : cloneArray(explainMetric?.headers).length
          ? cloneArray(explainMetric.headers)
          : cloneArray(config.headers),
      compositionBy: (() => {
        const fromExplain = compositionFieldForTab(config, tabId, override);
        if (fromExplain) return [fromExplain];
        const fromOverride = compositionFieldsFromOverride(override);
        if (fromOverride.length) return fromOverride;
        return cloneArray(config.compositionBy);
      })(),
      trendField: nonEmptyString(
        explainMetric?.dateField,
        override?.trendField,
        config.trendField,
      ),
      trendGrain: nonEmptyString(explainMetric?.grain, override?.trendGrain, config.trendGrain),
    };
    return merged;
  }

  function hasTabMetricDataSource(override) {
    if (!override || typeof override !== "object") return false;
    if (
      nonEmptyString(override.explainMetricId) &&
      !nonEmptyString(override.tableMetricId, override.datasetId) &&
      !(override.runtimeRef && typeof override.runtimeRef === "object") &&
      !cloneArray(override.columns).length &&
      !compositionFieldsFromOverride(override).length &&
      !nonEmptyString(override.trendField) &&
      Number(override.topN) <= 0
    ) {
      return false;
    }
    return Boolean(
      (override.runtimeRef && typeof override.runtimeRef === "object") ||
        nonEmptyString(override.tableMetricId, override.datasetId) ||
        cloneArray(override.columns).length ||
        cloneArray(override.headers).length ||
        Number(override.topN) > 0 ||
        nonEmptyString(override.layoutPreset, override.chartKind) ||
        (override.mapping && typeof override.mapping === "object")
    );
  }

  function drilldownTabLabel(tabId, config = null) {
    const id = normalizeTabId(tabId);
    const metricId = String(
      config?.tableMetricId ||
        config?.runtimeRef?.metric_id ||
        config?.runtimeRef?.metricId ||
        "",
    ).trim();
    if (id === "trend" && metricId.includes("year_month_matrix")) {
      return "汇总";
    }
    const labels = {
      definition: "口径",
      composition: "构成",
      trend: "趋势",
      numerator_denominator: "分子分母",
      attribution: "归因",
      detail: "明细",
    };
    return labels[id] || id || "明细";
  }

  function defaultActiveDrilldownTab(tabs = []) {
    const normalized = Array.isArray(tabs) ? tabs.map((tab) => normalizeTabId(tab)).filter(Boolean) : [];
    if (!normalized.length) return "detail";
    for (const preferred of ["detail", "trend", "composition", "numerator_denominator", "definition"]) {
      if (normalized.includes(preferred)) {
        return preferred;
      }
    }
    return normalized[0];
  }

  function isDrilldownSummaryTab(tabId, config = null) {
    const normalized = explainMetricKind(config, tabId);
    return normalized === "definition" || normalized === "numerator_denominator";
  }

  function isDrilldownAnalysisTab(tabId, config = null) {
    const normalized = explainMetricKind(config, tabId);
    return normalized === "composition" || normalized === "trend" || normalized === "attribution";
  }

  function unconfiguredTabNote(tabId) {
    const normalized = normalizeTabId(tabId);
    if (normalized === "composition") {
      return "未配置构成数据块，当前展示推荐维度；可通过 popup.metrics.composition 指定正式 metric。";
    }
    if (normalized === "trend") {
      return "未配置趋势数据块，当前展示推荐维度；可通过 popup.metrics.trend 指定正式 metric。";
    }
    if (normalized === "attribution") {
      return "未配置归因数据块，当前展示推荐维度；可通过 popup.metrics.attribution 指定正式 metric。";
    }
    return "";
  }

  function createDrilldownSummaryNode(config, tabId) {
    const panel = document.createElement("div");
    panel.className = "access-drilldown-summary";
    const normalizedTab = explainMetricKind(config, tabId);
    const rows = [];

    if (config.explainKind) {
      rows.push(["指标类型", config.explainKind]);
    }

    if (normalizedTab === "numerator_denominator") {
      if (config.ratioParts?.numerator) {
        rows.push(["分子", config.ratioParts.numerator]);
      }
      if (config.ratioParts?.denominator) {
        rows.push(["分母", config.ratioParts.denominator]);
      }
      if (config.ratioParts?.formula) {
        rows.push(["公式", config.ratioParts.formula]);
      }
      if (!rows.length && config.note) {
        rows.push(["说明", config.note]);
      }
    } else {
      if (config.note) {
        rows.push(["说明", config.note]);
      }
      if (Array.isArray(config.basisRefs) && config.basisRefs.length) {
        rows.push(["口径依据", config.basisRefs.join(" / ")]);
      }
      if (Array.isArray(config.recommendedDimensions) && config.recommendedDimensions.length) {
        rows.push(["推荐维度", config.recommendedDimensions.join(" / ")]);
      }
      if (Array.isArray(config.detailFields) && config.detailFields.length) {
        rows.push(["明细字段", config.detailFields.join(" / ")]);
      }
    }

    if (!rows.length) {
      const empty = document.createElement("div");
      empty.className = "access-drilldown-summary-empty";
      empty.textContent = "暂无可展示的解释信息";
      panel.appendChild(empty);
      return panel;
    }

    rows.forEach(([label, value]) => {
      const row = document.createElement("div");
      row.className = "access-drilldown-summary-row";
      const labelEl = document.createElement("div");
      labelEl.className = "access-drilldown-summary-label";
      labelEl.textContent = String(label || "");
      const valueEl = document.createElement("div");
      valueEl.className = "access-drilldown-summary-value";
      valueEl.textContent = String(value || "");
      row.append(labelEl, valueEl);
      panel.appendChild(row);
    });
    return panel;
  }

  function applyDrilldownOverlayMeta(root, config) {
    const titleEl = root.querySelector('[data-drilldown-title="true"]');
    const noteEl = root.querySelector('[data-drilldown-note="true"]');
    const panelEl = root.querySelector(".access-drilldown-overlay-panel");
    const heroEl = root.querySelector('[data-drilldown-hero="true"]');
    const headMetaEl = root.querySelector(".access-drilldown-overlay-head-meta");
    const structuredLayout = root.querySelector('[data-drilldown-structured-layout="true"]');
    if (titleEl) titleEl.textContent = String(config?.title || "");
    if (noteEl) {
      const note = String(config?.note || "").trim();
      noteEl.textContent = note;
      noteEl.toggleAttribute("hidden", !note);
    }
    const boardMode = Boolean(
      config?.boardLink || (config?.panelPopup && config?.panelTemplate),
    );
    const structuredBoardMode = Boolean(config?.structuredBoard);
    if (panelEl) {
      panelEl.classList.toggle("access-drilldown-overlay-panel--board", boardMode);
      panelEl.dataset.drilldownPanelTemplate = boardMode ? String(config.panelTemplate) : "";
      panelEl.dataset.drilldownLayoutMode = String(config?.sceneShell?.layoutMode || "");
    }
    const genericBody = root.querySelector('[data-drilldown-body-mode="generic"]');
    const structuredBody = root.querySelector('[data-drilldown-body-mode="structured"]');
    if (genericBody instanceof HTMLElement) {
      genericBody.toggleAttribute("hidden", structuredBoardMode);
    }
    if (structuredBody instanceof HTMLElement) {
      structuredBody.toggleAttribute("hidden", !structuredBoardMode);
    }
    const tabsHost = root.querySelector('[data-drilldown-tabs="true"]');
    if (tabsHost instanceof HTMLElement) {
      tabsHost.toggleAttribute("hidden", structuredBoardMode);
    }
    if (structuredLayout instanceof HTMLElement) {
      structuredLayout.dataset.shellLayoutMode = String(config?.sceneShell?.layoutMode || "");
    }
    applyDrilldownOverlaySize(root, config);
    if (headMetaEl) {
      headMetaEl.toggleAttribute("hidden", boardMode);
    }
    if (heroEl) {
      heroEl.toggleAttribute("hidden", !boardMode);
      if (boardMode) {
        const heroTitle = heroEl.querySelector('[data-drilldown-hero-title="true"]');
        const heroNote = heroEl.querySelector('[data-drilldown-hero-note="true"]');
        if (heroTitle) heroTitle.textContent = String(config?.title || "");
        if (heroNote) {
          // 口径说明留在「口径」tab；明细 tab 不再重复展示 metric_explain.note 副标题。
          heroNote.textContent = "";
          heroNote.toggleAttribute("hidden", true);
        }
      }
    }
  }

