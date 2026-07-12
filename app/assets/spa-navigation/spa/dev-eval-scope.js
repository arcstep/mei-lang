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

  function placeholderPropsForMount(mount) {
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
  boot.devEvalScopeFromProps = scopeFromProps;
  boot.devEvalScopeFromElement = scopeFromElement;
  boot.devEvalAllowsRuntimeQuery = allowsRuntimeQuery;
})(typeof window !== "undefined" ? window : globalThis);
