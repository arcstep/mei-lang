/**
 * Development selective eval / warmup scope (0535).
 * Reads window.__mei.dev_eval injected by host-shell.
 *
 * 双集合：
 * - warmupScopes：允许预热的 scope 前缀（仅服务端使用；客户端用于诊断展示）
 * - evalScopes：允许客户端动态求值（bind / eval-pack）的 scope 前缀
 * 向后兼容：旧 `scopes` 字段回退为 evalScopes。
 */
(function initDevEvalScope(global) {
  "use strict";

  const boot = (global.__meiLangBoot = global.__meiLangBoot || {});

  function readConfig() {
    const raw = global.__mei?.dev_eval;
    if (!raw || typeof raw !== "object") {
      return {
        profile: "full",
        warmupScopes: [],
        evalScopes: [],
        fill: "placeholder",
        runtimePlan: null,
        appId: "",
      };
    }
    const profile = String(raw.profile || "full").trim().toLowerCase() || "full";
    const normalize = (list) =>
      Array.isArray(list)
        ? list
            .map((value) => String(value || "").trim().replace(/^\/+|\/+$/g, ""))
            .filter(Boolean)
        : [];
    let evalScopes = normalize(raw.evalScopes);
    if (!evalScopes.length) {
      evalScopes = normalize(raw.scopes);
    }
    return {
      profile,
      warmupScopes: normalize(raw.warmupScopes),
      evalScopes,
      fill: String(raw.fill || "placeholder").trim() || "placeholder",
      runtimePlan:
        raw.runtimePlan && typeof raw.runtimePlan === "object" ? raw.runtimePlan : null,
      appId: String(raw.appId || global.__mei?.view_revision_envelope?.app_id || ""),
    };
  }

  function normalizeScope(value) {
    return String(value || "")
      .trim()
      .replace(/^\/+|\/+$/g, "")
      .replace(/^(content:|scope:)/i, "");
  }

  function scopeMatches(previewScope, prefixes) {
    const scope = normalizeScope(previewScope);
    if (!scope) return false;
    if (scope === "scene:default") {
      return prefixes.some((prefix) => prefix === "scene:default" || prefix === "*");
    }
    return prefixes.some((prefix) => {
      const needle = normalizeScope(prefix);
      if (!needle || needle === "*") return true;
      return scope === needle || scope.startsWith(`${needle}/`);
    });
  }

  function runtimePlanApp(config) {
    const plan = config.runtimePlan;
    if (!plan) return null;
    return plan.apps?.[config.appId] && typeof plan.apps[config.appId] === "object"
      ? plan.apps[config.appId]
      : plan.apps?.["*"];
  }

  function runtimeMode(previewScope) {
    const config = readConfig();
    const plan = config.runtimePlan;
    if (!plan) return null;
    const app = runtimePlanApp(config);
    const scope = normalizeScope(previewScope);
    let selected = {
      specificity: 0,
      mode: String(plan.defaultMode || "hot").toLowerCase(),
    };
    for (const target of Array.isArray(app?.targets) ? app.targets : []) {
      const prefix = normalizeScope(target?.scope);
      if (
        prefix === "*" ||
        scope === prefix ||
        (prefix && scope.startsWith(`${prefix}/`))
      ) {
        if (prefix.length >= selected.specificity) {
          selected = {
            specificity: prefix.length,
            mode: String(target?.mode || selected.mode).toLowerCase(),
          };
        }
      }
    }
    return selected.mode;
  }

  function runtimeMetricMode(metricId, previewScope) {
    const config = readConfig();
    const app = runtimePlanApp(config);
    const id = String(metricId || "").trim();
    const override = id ? String(app?.metricOverrides?.[id] || "").toLowerCase() : "";
    if (override === "hot" || override === "lazy" || override === "frozen") {
      return override;
    }
    return runtimeMode(previewScope);
  }

  function allowsMetric(metricId, previewScope) {
    const mode = runtimeMetricMode(metricId, previewScope);
    if (mode === "hot") return true;
    if (mode === "frozen") return false;
    if (mode === "lazy") {
      // A lazy metric is materialized only when the user explicitly enters a
      // lazy target (for example a secondary stage/drilldown), never merely
      // because it shares a hot section with another metric.
      return runtimeMode(previewScope) === "lazy";
    }
    return allowsEvalScope(previewScope);
  }

  function allowsEvalScope(previewScope) {
    const config = readConfig();
    const mode = runtimeMode(previewScope);
    if (mode) return mode !== "frozen";
    if (config.profile === "full") return true;
    if (config.profile === "static" || config.profile === "off") return false;
    if (config.profile === "scoped") {
      if (!config.evalScopes.length) return false;
      return scopeMatches(previewScope, config.evalScopes);
    }
    return true;
  }

  function allowsWarmupScope(previewScope) {
    const config = readConfig();
    const mode = runtimeMode(previewScope);
    if (mode) return mode === "hot";
    if (config.profile === "full") return true;
    if (config.profile === "static" || config.profile === "off") return false;
    if (config.profile === "scoped") {
      if (!config.warmupScopes.length) return false;
      return scopeMatches(previewScope, config.warmupScopes);
    }
    return true;
  }

  function shouldFetchEvalPack(scopeKey) {
    const config = readConfig();
    const mode = runtimeMode(scopeKey || "");
    if (mode) return mode !== "frozen";
    if (config.profile === "static" || config.profile === "off") return false;
    if (config.profile === "full") return true;
    return allowsEvalScope(scopeKey || "");
  }

  // pretty-panels `prototype/static-layout.fixture.json` — frozen 布局调试默认真源；
  // 应用可在 SSR 注入 `window.__mei.static_layout_fixture` 覆盖。
  const DEFAULT_STATIC_LAYOUT_FIXTURE = [
    { metric_id: "inspection_frequency_reduction_rate", label: "检查频次降低率", value: 53.8, unit: "%" },
    { metric_id: "penalty_revenue_growth_rate", label: "罚没收入增长率", value: -52.4, unit: "%" },
    { metric_id: "warnings_verification_rate", label: "预警查实率", value: 81.8, unit: "%" },
    { metric_id: "effectiveness_verified_rectification_rate", label: "查实预警整改率", value: 0.0, unit: "%" },
    { metric_id: "enforcement_units_count", label: "执法单位", value: 42, unit: "个" },
    { metric_id: "enforcement_personnel_count", label: "执法人员", value: 1000, unit: "人" },
    { metric_id: "enforcement_items_count", label: "执法事项", value: 1000, unit: "项" },
    { metric_id: "enforcement_objects_count", label: "执法对象", value: 16.4, unit: "万" },
    { metric_id: "key_enterprises_count", label: "重点企业", value: 1000, unit: "家" },
    { metric_id: "enforcement_parks_count", label: "园区", value: 3, unit: "个" },
    { metric_id: "whitelist_enterprises_count", label: "白名单", value: 18, unit: "家" },
    { metric_id: "supervision_items_count", label: "监督事项", value: 23, unit: "项" },
    { metric_id: "supervision_models_count", label: "预警模型", value: 20, unit: "个" },
    { metric_id: "warnings_count", label: "预警总数", value: 14, unit: "件" },
    { metric_id: "warnings_pending_count", label: "待办", value: 4, unit: "件" },
    { metric_id: "effectiveness_in_progress_count", label: "在办", value: 10, unit: "件" },
    { metric_id: "effectiveness_completed_count", label: "已办", value: 86, unit: "件" },
    { metric_id: "effectiveness_issue_verification_rate", label: "查实率", value: 81.8, unit: "%" },
    { metric_id: "inspection_total_count", label: "总数", value: 42053, unit: "次" },
    { metric_id: "inspection_no_violation_count", label: "无违规", value: 33994, unit: "次" },
    { metric_id: "penalty_total_count", label: "总数", value: 8718, unit: "件" },
    { metric_id: "ai_enforcement_recognition_count", label: "AI执法识别", value: 916, unit: "次" },
  ];

  const STATIC_DEMO_ROWS = {
    park_inspection_total_by_park: [
      { 园区名称: "西部科学城", value: 18620 },
      { 园区名称: "联东U谷", value: 12480 },
      { 园区名称: "凤凰山工业园", value: 9953 },
    ],
    inspections_no_violation_by_park: [
      { 园区名称: "西部科学城", value: 16204 },
      { 园区名称: "联东U谷", value: 11026 },
      { 园区名称: "凤凰山工业园", value: 8801 },
    ],
    park_penalty_amount_by_park: [
      { 园区名称: "西部科学城", value: 2860000 },
      { 园区名称: "联东U谷", value: 1945000 },
      { 园区名称: "凤凰山工业园", value: 1320000 },
    ],
    penalties_top_party_year_amount_bars: [
      { 当事人: "甲公司", year: "2024", value: 128 },
      { 当事人: "乙公司", year: "2024", value: 96 },
      { 当事人: "丙公司", year: "2025", value: 84 },
    ],
    penalties_top_matter_year_ranking: [
      { 处罚事项: "未按规定公示信息", 处罚次数_2025: 42 },
      { 处罚事项: "安全生产违法", 处罚次数_2025: 31 },
      { 处罚事项: "占道经营", 处罚次数_2025: 18 },
    ],
  };

  function readStaticLayoutFixture() {
    const injected = global.__mei?.static_layout_fixture;
    if (Array.isArray(injected) && injected.length) {
      return injected;
    }
    return DEFAULT_STATIC_LAYOUT_FIXTURE;
  }

  function scalarFromStaticFixture(metricId) {
    const id = String(metricId || "").trim();
    if (!id) return null;
    const entry = readStaticLayoutFixture().find(
      (item) => String(item?.metric_id || "").trim() === id,
    );
    if (!entry) return null;
    const unit = String(entry.unit || "").trim();
    const value = entry.value;
    const text = unit ? `${value}${unit}` : String(value ?? "--");
    return {
      content: text,
      text,
      value: entry.value,
      label: entry.label || text,
      unit,
    };
  }

  function staticDatasetRowsForMetric(metricId) {
    const id = String(metricId || "").trim();
    if (!id) return null;
    const rows = STATIC_DEMO_ROWS[id];
    return Array.isArray(rows) ? rows.map((row) => ({ ...row })) : null;
  }

  function metricIdFromMount(mount) {
    const rawProps = mount?.props && typeof mount.props === "object" ? mount.props : {};
    const content =
      rawProps.content && typeof rawProps.content === "object" && !Array.isArray(rawProps.content)
        ? rawProps.content
        : null;
    return String(
      mount?.metric_id ||
        rawProps.metric_id ||
        rawProps.metricId ||
        content?.id ||
        content?.metric_id ||
        "",
    ).trim();
  }

  function placeholderPropsForMount(mount) {
    const metricId = metricIdFromMount(mount);
    if (metricId) {
      const scalar = scalarFromStaticFixture(metricId);
      if (scalar) {
        const rawProps = mount?.props && typeof mount.props === "object" ? mount.props : {};
        const role = String(rawProps.metric_role || rawProps.metricRole || "").trim();
        return {
          ...(role ? { metric_role: role } : {}),
          ...scalar,
          "data-mei-dev-eval-placeholder": "1",
        };
      }
    }
    const kind = String(mount?.kind || mount?.component || mount?.use_key || "").toLowerCase();
    if (kind.includes("chart") || kind.includes("echarts")) {
      return {
        option: {
          animation: false,
          xAxis: { type: "category", data: ["A", "B", "C"] },
          yAxis: { type: "value" },
          series: [{ type: "bar", data: [3, 5, 2] }],
        },
        "data-mei-dev-eval-placeholder": "1",
      };
    }
    if (kind.includes("map") || kind.includes("maplibre")) {
      return { "data-mei-dev-eval-placeholder": "1" };
    }
    return {
      content: "--",
      text: "--",
      value: "--",
      label: "--",
      "data-mei-dev-eval-placeholder": "1",
    };
  }

  function scopeFromProps(props) {
    return normalizeScope(
      props?._mei?.preview_scope ||
        props?._mei?.previewScope ||
        props?.preview_scope ||
        props?.previewScope ||
        "",
    );
  }

  function scopeFromElement(element) {
    if (!(element instanceof Element)) return "";
    const scoped = element.closest("[data-preview-scope], [data-mei-preview-scope]");
    return normalizeScope(
      scoped?.getAttribute("data-preview-scope") ||
        scoped?.getAttribute("data-mei-preview-scope") ||
        "",
    );
  }

  function metricIdsFromProps(props) {
    const content = props?.content && typeof props.content === "object" ? props.content : null;
    return [
      props?.metric_id,
      props?.metricId,
      props?.__mei_runtime_ref?.metric_id,
      content?.__mei_runtime_ref?.metric_id,
      content?.metric_id,
      content?.__ref === "metric" ? content?.id : "",
    ]
      .map((value) => String(value || "").trim())
      .filter((value, index, values) => value && values.indexOf(value) === index);
  }

  function allowsRuntimeQuery(props, element) {
    const config = readConfig();
    const scope = scopeFromProps(props) || scopeFromElement(element);
    const metricIds = metricIdsFromProps(props);
    if (config.runtimePlan && metricIds.length) {
      return metricIds.every((metricId) => allowsMetric(metricId, scope));
    }
    if (config.profile === "full") return true;
    return allowsEvalScope(scope);
  }

  boot.devEvalReadConfig = readConfig;
  boot.devEvalAllowsPreviewScope = allowsEvalScope; // 向后兼容别名
  boot.devEvalAllowsEvalScope = allowsEvalScope;
  boot.devEvalRuntimeMetricMode = runtimeMetricMode;
  boot.devEvalAllowsMetric = allowsMetric;
  boot.devEvalAllowsWarmupScope = allowsWarmupScope;
  boot.devEvalShouldFetchEvalPack = shouldFetchEvalPack;
  boot.devEvalPlaceholderProps = placeholderPropsForMount;
  boot.devEvalScalarFromFixture = scalarFromStaticFixture;
  boot.devEvalStaticDatasetRows = staticDatasetRowsForMetric;
  boot.devEvalScopeFromProps = scopeFromProps;
  boot.devEvalScopeFromElement = scopeFromElement;
  boot.devEvalAllowsRuntimeQuery = allowsRuntimeQuery;
})(typeof window !== "undefined" ? window : globalThis);
