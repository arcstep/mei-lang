(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.spaNavigationMounted) return;
  boot.spaNavigationMounted = true;

  const RELOAD_APP_SCRIPTS = new Set([
    "/app-assets/frame-stage.js",
    "/app-assets/statusbar.js",
    "/app-assets/manage-tabs.js",
    "/app-assets/manage-diagnostics.js",
    "/app-assets/workspace-splitters.js",
    "/app-assets/source-tree-controls.js",
    "/app-assets/source-highlight.js",
    "/app-assets/agent-panel-utils.js",
    "/app-assets/agent-panel-routing.js",
    "/app-assets/agent-panel-access-float.js",
    "/app-assets/agent-panel-source.js",
    "/app-assets/agent-panel-session.js",
    "/app-assets/agent-panel-context.js",
    "/app-assets/agent-panel-chrome.js",
    "/app-assets/agent-panel-messages-model.js",
    "/app-assets/agent-panel-messages.js",
    "/app-assets/agent-panel-layout.js",
    "/app-assets/agent-panel-delta-debug.js",
    "/app-assets/agent-panel-bindings.js",
    "/app-assets/agent-panel.js",
  ]);
  const RELOAD_BUNDLE_SCRIPTS = new Set([
    "/app-bundles/manage.js",
    "/app-bundles/access.js",
  ]);
  const SPA_NAV_SCRIPT = "/app-assets/spa-navigation.js";
  const LOADING_DELAY_MS = 140;
  const LOADING_MIN_VISIBLE_MS = 180;
  const SCRIPT_LOAD_TIMEOUT_MS = 15000;
  const SPA_FETCH_TIMEOUT_MS = 120000;
  const METRIC_DRILLDOWN_EVENT = "mei:metric-drilldown";
  const ANALYSIS_OPEN_EVENT = "mei:analysis-open";
  const POPUP_OPEN_EVENT = "mei:popup-open";
  const DRILLDOWN_OVERLAY_ROOT_ID = "mei-access-drilldown-overlay";
  const DRILLDOWN_CONTEXT_BANNER_ID = "mei-drilldown-context-banner";
  const DRILLDOWN_SCENE_BY_FILE = {
    "scenes/4_监督和问题办理/监督事项.mei": "supervision_matters",
    "scenes/4_监督和问题办理/预警模型.mei": "warning_models",
    "scenes/4_监督和问题办理/预警清单.mei": "warning_list",
    "scenes/4_监督和问题办理/问题处理结果清单.mei": "issue_result_list",
  };
  const DRILLDOWN_DATASET_BY_SCENE = {
    enforcement_units: "enforcement_units",
    enforcement_officers: "enforcement_officers",
    enforcement_matters: "enforcement_matters",
    key_enterprises: "key_enterprises",
    enforcement_parks: "enforcement_parks",
    enterprise_whitelist: "enterprise_whitelist",
    administrative_inspection: "administrative_inspection_dashboard_ds",
    enterprise_complaints: "enterprise_complaints",
    ai_recognition_warnings: "ai_recognition_warnings",
    body_cameras: "body_cameras",
    penalty_dashboard: "penalty_result_dashboard_ds",
    admin_reconsideration_register: "admin_reconsideration_register",
    supervision_matters: "supervision_matters",
    warning_models: "warning_models",
    warning_list: "warning_list",
    issue_result_list: "issue_result_list",
  };
  const ENFORCEMENT_UNIT_COLUMNS = [
    "序号",
    "类别",
    "执法单位",
    "办公地址",
    "GCJ02经度",
    "GCJ02纬度",
    "GCJ02经度度分秒",
    "GCJ02纬度度分秒",
    "BD09经度",
    "BD09纬度",
    "BD09经度度分秒",
    "BD09纬度度分秒",
    "CGCS2000经度",
    "CGCS2000纬度",
    "CGCS2000经度度分秒",
    "CGCS2000纬度度分秒",
  ];
  const ENFORCEMENT_OFFICER_COLUMNS = [
    "序号",
    "所属部门",
    "姓名",
    "性别",
    "出生日期",
    "民族",
    "政治面貌",
    "最高学历",
    "执法性质",
    "执法证号",
    "人员编制",
    "职级",
    "证件生效日期",
    "证件终止日期",
    "备注",
  ];
  const ENFORCEMENT_MATTER_COLUMNS = [
    "序号",
    "事项编码",
    "事项名称",
    "事项类型",
    "业务领域",
    "监管层级",
    "监管对象",
    "编制部门",
    "是否编制清单",
    "法定依据",
    "检查方式",
    "是否涉企事项",
  ];
  const KEY_ENTERPRISE_COLUMNS = [
    "所属街道",
    "所属园区",
    "组团分区",
    "企业名称",
    "社会信用代码",
    "基本情况",
    "行业代码",
    "员工人数",
    "企业经营用地",
    "占地面积平方米",
    "建筑面积平方米",
    "注册企业",
    "企业规模",
    "注册地址",
    "法定代表人",
    "注册资本万元",
    "成立日期",
    "经营范围",
    "企业经纬度坐标",
    "企业联系人及电话",
    "上市情况",
    "升规入统时间",
  ];
  const PARK_COLUMNS = ["id", "name", "townId", "townName", "address"];
  const PARK_HEADERS = ["园区ID", "园区名称", "所属街镇ID", "所属街镇", "地址"];
  const WHITELIST_COLUMNS = ["序号", "企业名称", "统一社会信用代码"];
  const INSPECTION_COLUMNS = [
    "序号",
    "检查日期",
    "检查机构",
    "检查人员1",
    "检查人员2",
    "检查对象名称",
    "检查对象编码",
    "任务名称",
    "检查结果",
    "后续处置",
    "备注",
  ];
  const COMPLAINT_COLUMNS = ["id", "source", "complaintTime", "title", "content", "agency", "status"];
  const COMPLAINT_HEADERS = ["流水号", "反映来源", "反映时间", "反映事项", "反映内容", "办结机构", "办结状态"];
  const AI_WARNING_COLUMNS = [
    "id",
    "inspectionDate",
    "inspectionAgency",
    "subjectName",
    "inspectionResult",
    "aiRecognitionResult",
    "actionLabel",
    "recognizedAt",
    "behaviorType",
    "confidence",
  ];
  const AI_WARNING_HEADERS = [
    "记录ID",
    "检查日期",
    "检查机构",
    "检查对象名称",
    "检查结果",
    "AI识别结果",
    "行为标签",
    "识别时间",
    "行为类型",
    "置信度",
  ];
  const BODY_CAMERA_COLUMNS = ["序号", "设备号", "所属单位", "使用人", "可回放时长", "备注"];
  const PENALTY_COLUMNS = [
    "序号",
    "执法类型",
    "处罚决定书文号",
    "办案单位",
    "主办人",
    "协办人",
    "当事人",
    "案件来源",
    "案件领域",
    "处罚事项",
    "立案日期",
    "做出处罚日期",
    "执行日期",
    "拟处罚金额",
    "罚款金额",
    "是否柔性执法事项",
    "备注",
  ];
  const RECONSIDERATION_COLUMNS = [
    "案号",
    "收到日期",
    "申请人",
    "被申请人",
    "第三人",
    "复议事项",
    "行政类别",
    "复议请求事项",
    "复议决定",
    "结案日期",
    "是否化解",
    "备注",
  ];
  const TREND_COLUMNS = ["month", "value"];
  const STATUS_COUNT_COLUMNS = ["是否查实", "value"];
  const MECHANISM_COUNT_COLUMNS = ["健全机制", "value"];
  const WARNING_COLUMNS = [
    "预警ID",
    "主责单位",
    "问题分类名称",
    "问题描述",
    "预警类型",
    "预警等级",
    "预警时间",
    "问题跟踪ID",
    "承办部门",
    "分办时间",
    "办结时间",
    "是否查实",
    "是否转问题线索",
    "核查情况",
    "处理结果",
  ];
  const WARNING_PENDING_COLUMNS = [
    "预警ID",
    "主责单位",
    "问题分类名称",
    "问题描述",
    "预警等级",
    "预警时间",
    "承办部门",
    "问题跟踪ID",
    "分办时间",
    "办结时间",
    "是否查实",
    "是否转问题线索",
  ];
  const ISSUE_COLUMNS = [
    "处理结果ID",
    "问题跟踪ID",
    "预警ID",
    "主责单位",
    "问题分类名称",
    "问题描述",
    "承办部门",
    "分办时间",
    "办结时间",
    "是否查实",
    "是否转问题线索",
    "是否立案",
    "处理处分",
    "挽回资金",
    "健全机制",
  ];
  const MATTERS_COLUMNS = ["序号", "监督类别", "监督事项", "预警等级", "存在的问题", "表现形式"];
  const MODEL_COLUMNS = [
    "序号",
    "监督领域",
    "监督类别",
    "监督模型",
    "模型ID",
    "政策文件",
    "模型依据",
    "预警类型",
    "模型类别",
    "监督规则",
    "监督数据",
    "预警等级",
  ];
  const DETAIL_TABLE_DEFAULTS = {
    pageSize: 9,
    cellPreviewMaxChars: 16,
    columnMinWidth: 180,
  };
  const EXPLAIN_TABLE_DEFAULTS = {
    pageSize: 10,
    cellPreviewMaxChars: 18,
    columnMinWidth: 180,
  };
  const DRILLDOWN_METRIC_CONTEXT = {
    enforcement_units_count: {
      sceneId: "enforcement_units",
      title: "执法单位明细",
      columns: ENFORCEMENT_UNIT_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    enforcement_personnel_count: {
      sceneId: "enforcement_officers",
      title: "执法人员明细",
      columns: ENFORCEMENT_OFFICER_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    key_enterprises_count: {
      sceneId: "key_enterprises",
      title: "重点企业明细",
      columns: KEY_ENTERPRISE_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    park_count: {
      sceneId: "enforcement_parks",
      ...DETAIL_TABLE_DEFAULTS,
    },
    whitelist_enterprises_count: {
      sceneId: "enterprise_whitelist",
      ...DETAIL_TABLE_DEFAULTS,
    },
    inspections_today_count: {
      sceneId: "administrative_inspection",
      ...DETAIL_TABLE_DEFAULTS,
    },
    inspections_week_count: {
      sceneId: "administrative_inspection",
      ...DETAIL_TABLE_DEFAULTS,
    },
    enterprise_complaints_count: {
      sceneId: "enterprise_complaints",
      ...DETAIL_TABLE_DEFAULTS,
    },
    inspections_no_violation_count: {
      sceneId: "administrative_inspection",
      ...DETAIL_TABLE_DEFAULTS,
    },
    ai_recognition_warnings_count: {
      sceneId: "ai_recognition_warnings",
      ...DETAIL_TABLE_DEFAULTS,
    },
    records_devices_count: {
      sceneId: "body_cameras",
      ...DETAIL_TABLE_DEFAULTS,
    },
    records_devices_playback_hours_total: {
      sceneId: "body_cameras",
      ...DETAIL_TABLE_DEFAULTS,
    },
    penalties_total_count: {
      sceneId: "penalty_dashboard",
      ...DETAIL_TABLE_DEFAULTS,
    },
    penalties_today_count: {
      sceneId: "penalty_dashboard",
      ...DETAIL_TABLE_DEFAULTS,
    },
    penalties_week_count: {
      sceneId: "penalty_dashboard",
      ...DETAIL_TABLE_DEFAULTS,
    },
    administrative_reconsiderations_count: {
      sceneId: "admin_reconsideration_register",
      ...DETAIL_TABLE_DEFAULTS,
    },
    penalty_revenue_growth_rate: {
      sceneId: "penalty_dashboard",
      ...EXPLAIN_TABLE_DEFAULTS,
    },
    warnings_verification_rate: {
      sceneId: "warning_list",
      ...EXPLAIN_TABLE_DEFAULTS,
    },
    effectiveness_verified_rectification_rate: {
      sceneId: "issue_result_list",
      ...EXPLAIN_TABLE_DEFAULTS,
      columnMinWidth: 220,
    },
    supervision_items_count: {
      sceneId: "supervision_matters",
      ...EXPLAIN_TABLE_DEFAULTS,
    },
    supervision_models_count: {
      sceneId: "warning_models",
      ...EXPLAIN_TABLE_DEFAULTS,
      columnMinWidth: 190,
    },
    warnings_count: {
      sceneId: "warning_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    warnings_pending_count: {
      sceneId: "warning_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_in_progress_count: {
      sceneId: "issue_result_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_completed_count: {
      sceneId: "issue_result_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_issue_verification_rate: {
      sceneId: "issue_result_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_transfer_clue_count: {
      sceneId: "issue_result_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_filing_count: {
      sceneId: "issue_result_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_party_gov_sanction_count: {
      sceneId: "issue_result_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_handled_person_times: {
      sceneId: "issue_result_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_recovered_funds_total: {
      sceneId: "issue_result_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_mechanism_item_count: {
      sceneId: "issue_result_list",
      ...DETAIL_TABLE_DEFAULTS,
    },
  };
  let currentNavigationId = 0;
  let spaNavigationInFlight = 0;
  let loadingTimer = null;
  let loadingVisibleAt = 0;
  let drilldownContextRetryTimer = null;

  function isAccessRoute(pathname = window.location.pathname) {
    return String(pathname || "").startsWith("/apps/access/");
  }

  function normalizeDrilldownScenePath(raw) {
    return String(raw || "")
      .trim()
      .replace(/\\/g, "/")
      .replace(/^\.?\/*/, "");
  }

  function nonEmptyString(...values) {
    for (const value of values) {
      const text = String(value || "").trim();
      if (text) return text;
    }
    return "";
  }

  function cloneArray(value) {
    return Array.isArray(value) ? value.slice() : [];
  }

  function positiveInt(...values) {
    for (const value of values) {
      const parsed = Number(value);
      if (Number.isFinite(parsed) && parsed > 0) {
        return Math.floor(parsed);
      }
    }
    return 0;
  }

  function boolValue(...values) {
    for (const value of values) {
      if (typeof value === "boolean") return value;
    }
    return undefined;
  }

  function runtimeDrilldownConfig(detail) {
    const value = detail?.analysis_contract || detail?.drilldown;
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      return {};
    }
    return value;
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

  function normalizeExplainMetrics(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) return {};
    const normalized = {};
    Object.entries(value).forEach(([key, entry]) => {
      const id = normalizeTabId(key);
      if (!id || !entry || typeof entry !== "object" || Array.isArray(entry)) return;
      normalized[id] = {
        id,
        kind: normalizeTabId(entry.kind || id),
        label: nonEmptyString(entry.label, key),
        by: nonEmptyString(entry.by),
        dateField: nonEmptyString(entry.date_field, entry.dateField),
        grain: nonEmptyString(entry.grain),
        datasetId: nonEmptyString(entry.dataset, entry.dataset_id, entry.datasetId),
        sceneId: nonEmptyString(entry.scene_id, entry.sceneId),
        scenePath: nonEmptyString(entry.scene_file, entry.sceneFile),
        fields: cloneArray(entry.fields),
        metric: entry.metric && typeof entry.metric === "object" ? entry.metric : null,
        headers: cloneArray(entry.headers),
        mapping: entry.mapping && typeof entry.mapping === "object" ? entry.mapping : null,
        chartKind: nonEmptyString(entry.chart_kind, entry.chartKind),
      };
    });
    return normalized;
  }

  function explainMetricForTab(config, tabId) {
    const key = normalizeTabId(tabId);
    return config?.explainMetrics?.[key] || null;
  }

  function explainMetricKind(config, tabId) {
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

  function resolveDrilldownTabs({ detail, runtime, mapped, explainKind, hasDetail }) {
    const explainMetrics = normalizeExplainMetrics(detail?.explain_metrics);
    const explicitExplainTabs = Object.keys(explainMetrics);
    const popupMetricTabs = Object.keys(
      normalizeTabMetricOverrides(detail?.popup?.metrics),
    );
    if (explicitExplainTabs.length || popupMetricTabs.length) {
      return Array.from(new Set([...explicitExplainTabs, ...popupMetricTabs]));
    }
    const explicit = runtimeTabIds(
      detail?.analysis_tabs,
      detail?.drilldown_tabs,
      runtime?.tabs,
      runtime?.analysis_tabs,
      mapped?.tabs,
    );
    const defaults = defaultDrilldownTabs(explainKind, { hasDetail });
    if (!explicit.length) return defaults;
    const normalizedExplicit = Array.from(new Set(explicit.map((tab) => normalizeTabId(tab)).filter(Boolean)));
    const basicTabs = new Set(["definition", "detail", "numerator_denominator"]);
    const hasOnlyBasicTabs = normalizedExplicit.every((tab) => basicTabs.has(tab));
    if (!hasOnlyBasicTabs) return normalizedExplicit;
    const merged = normalizedExplicit.filter((tab) => tab !== "detail");
    defaults.forEach((tab) => {
      const normalized = normalizeTabId(tab);
      if (!normalized || normalized === "detail" || merged.includes(normalized)) return;
      merged.push(normalized);
    });
    if (normalizedExplicit.includes("detail") || defaults.includes("detail")) {
      merged.push("detail");
    }
    return merged;
  }

  function normalizeTabMetricOverrides(...values) {
    const raw = values.find(
      (value) =>
        value &&
        typeof value === "object" &&
        !Array.isArray(value) &&
        Object.keys(value).length > 0,
    );
    if (!raw) return {};
    const normalized = {};
    Object.entries(raw).forEach(([key, entry]) => {
      const tabId = normalizeTabId(key);
      if (!tabId) return;
      if (typeof entry === "string") {
        const metricId = String(entry || "").trim();
        if (!metricId) return;
        normalized[tabId] = { tableMetricId: metricId };
        return;
      }
      if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
        return;
      }
      if (entry.__ref === "metric") {
        const metricId = nonEmptyString(entry.id);
        if (!metricId) return;
        normalized[tabId] = {
          runtimeRef: {
            kind: "metric",
            metricId,
            datasetId: nonEmptyString(entry.from_dataset, entry.fromDataset),
            sceneId: nonEmptyString(entry.scene_id, entry.sceneId),
            scenePath: nonEmptyString(entry.scene_file, entry.sceneFile),
          },
        };
        return;
      }
      if (entry.__ref === "dataset") {
        const datasetId = nonEmptyString(entry.id);
        if (!datasetId) return;
        normalized[tabId] = {
          runtimeRef: {
            kind: "data",
            datasetId,
            sceneId: nonEmptyString(entry.scene_id, entry.sceneId),
            scenePath: nonEmptyString(entry.scene_file, entry.sceneFile, entry.path),
          },
        };
        return;
      }
      let columns = cloneArray(entry.columns);
      if (!columns.length) columns = cloneArray(entry.detail_fields);
      if (!columns.length) columns = cloneArray(entry.detailFields);
      const override = {
        title: nonEmptyString(entry.title),
        note: nonEmptyString(entry.note),
        tableMetricId: nonEmptyString(
          entry.table_metric_id,
          entry.tableMetricId,
          entry.metric_id,
          entry.metricId,
        ),
        datasetId: nonEmptyString(entry.dataset_id, entry.datasetId),
        columns,
        headers: cloneArray(entry.headers),
        layoutPreset: nonEmptyString(entry.layout_preset, entry.layoutPreset),
        chartKind: nonEmptyString(entry.chart_kind, entry.chartKind, entry.chart),
        mapping: entry.mapping && typeof entry.mapping === "object" ? entry.mapping : null,
      };
      if (
        !override.title &&
        !override.note &&
        !override.tableMetricId &&
        !override.datasetId &&
        !override.columns.length &&
        !override.headers.length &&
        !override.layoutPreset &&
        !override.chartKind &&
        !override.mapping
      ) {
        return;
      }
      normalized[tabId] = override;
    });
    return normalized;
  }

  function resolveDrilldownTabConfig(config, tabId) {
    const tabMetrics = config?.tabMetrics || {};
    const normalizedTab = explainMetricKind(config, tabId);
    const explainMetric = explainMetricForTab(config, normalizedTab);
    const override = tabMetrics[normalizedTab];
    if (!override && !explainMetric) return config;
    const explainDatasetId = nonEmptyString(explainMetric?.datasetId);
    const explainMetricRef =
      explainMetric?.metric && typeof explainMetric.metric === "object" ? explainMetric.metric : null;
    const overrideDatasetId = nonEmptyString(override.datasetId);
    const overrideTableMetricId = nonEmptyString(override.tableMetricId);
    const suppressDetailMetricFallback = Boolean(overrideDatasetId && !overrideTableMetricId);
    const merged = {
      ...config,
      title: nonEmptyString(override?.title, explainMetric?.label, config.title),
      note: nonEmptyString(override?.note, config.note),
      tableMetricId:
        overrideTableMetricId ||
        nonEmptyString(explainMetricRef?.id) ||
        (overrideDatasetId ? "" : nonEmptyString(config.tableMetricId)),
      datasetId:
        overrideDatasetId ||
        nonEmptyString(explainMetricRef?.from_dataset, explainDatasetId, config.datasetId),
      suppressDetailMetricFallback,
      layoutPreset: nonEmptyString(override?.layoutPreset, config.layoutPreset),
      chartKind: nonEmptyString(override?.chartKind, explainMetric?.chartKind, config.chartKind),
      mapping:
        override?.mapping && typeof override.mapping === "object"
          ? override.mapping
          : explainMetric?.mapping && typeof explainMetric.mapping === "object"
            ? explainMetric.mapping
          : config.mapping && typeof config.mapping === "object"
            ? config.mapping
            : null,
      runtimeRef:
        override?.runtimeRef && typeof override.runtimeRef === "object"
          ? override.runtimeRef
          : explainMetricRef && typeof explainMetricRef === "object"
            ? {
                kind: "metric",
                metricId: nonEmptyString(explainMetricRef.id),
                datasetId: nonEmptyString(explainMetricRef.from_dataset),
                sceneId: nonEmptyString(explainMetricRef.scene_id),
                scenePath: nonEmptyString(explainMetricRef.scene_file),
              }
          : explainMetric?.sceneId || explainMetric?.scenePath
            ? {
                kind: "data",
                sceneId: nonEmptyString(explainMetric.sceneId),
                scenePath: nonEmptyString(explainMetric.scenePath),
                datasetId: explainDatasetId,
              }
          : config.runtimeRef && typeof config.runtimeRef === "object"
            ? config.runtimeRef
            : null,
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
      compositionBy: explainMetric?.by ? [explainMetric.by] : config.compositionBy,
      trendField: nonEmptyString(explainMetric?.dateField, config.trendField),
      trendGrain: nonEmptyString(explainMetric?.grain, config.trendGrain),
    };
    return merged;
  }

  function drilldownTabLabel(tabId) {
    const id = normalizeTabId(tabId);
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
      return "未配置构成数据块，当前展示推荐维度；可通过 drilldown.tab_metrics.composition 指定 table_metric_id。";
    }
    if (normalized === "trend") {
      return "未配置趋势数据块，当前展示推荐维度；可通过 drilldown.tab_metrics.trend 指定 table_metric_id。";
    }
    if (normalized === "attribution") {
      return "未配置归因数据块，当前展示推荐维度；可通过 drilldown.tab_metrics.attribution 指定 table_metric_id。";
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
    if (titleEl) titleEl.textContent = String(config?.title || "");
    if (noteEl) {
      const note = String(config?.note || "").trim();
      noteEl.textContent = note;
      noteEl.toggleAttribute("hidden", !note);
    }
  }

  function resolveAccessAppBasePath(pathname = window.location.pathname) {
    const raw = String(pathname || "");
    const prefix = "/apps/access/";
    if (!raw.startsWith(prefix)) return "";
    const tail = raw.slice(prefix.length);
    const marker = tail.indexOf("/scene/");
    const app = marker >= 0 ? tail.slice(0, marker) : tail;
    const trimmed = String(app || "").trim();
    return trimmed ? `${prefix}${trimmed}` : "";
  }

  function resolveDrilldownSceneId(detail, mapped = {}, runtime = {}) {
    const runtimeTargetSceneId = nonEmptyString(
      detail?.drilldown_target_scene_id,
      detail?.drilldown_scene_id,
      runtime?.target_scene_id,
      runtime?.targetSceneId,
      runtime?.scene_id,
      runtime?.sceneId,
    );
    if (runtimeTargetSceneId) return runtimeTargetSceneId;
    const mappedSceneId = nonEmptyString(mapped?.sceneId);
    if (mappedSceneId) return mappedSceneId;
    const runtimeScene = normalizeDrilldownScenePath(
      nonEmptyString(
        detail?.drilldown_scene,
        runtime?.scene_file,
        runtime?.sceneFile,
        runtime?.scene_path,
        runtime?.scenePath,
        runtime?.scene,
      ),
    );
    if (!runtimeScene) return "";
    return DRILLDOWN_SCENE_BY_FILE[runtimeScene] || runtimeScene;
  }

  function resolveDrilldownConfig(detail) {
    const metricId = String(detail?.metric_id || "").trim();
    const mapped = DRILLDOWN_METRIC_CONTEXT[metricId] || {};
    const runtime = runtimeDrilldownConfig(detail);
    const popup =
      detail?.popup && typeof detail.popup === "object" && !Array.isArray(detail.popup) ? detail.popup : {};
    const analysisLink =
      detail?.analysis_link && typeof detail.analysis_link === "object" ? detail.analysis_link : {};
    const sceneId = nonEmptyString(
      detail?.scene_id,
      popup?.scene_id,
      popup?.sceneId,
      resolveDrilldownSceneId(detail, mapped, runtime),
    );
    const runtimeEnabled = boolValue(detail?.analysis_enabled, detail?.drilldown_enabled, runtime?.enabled);
    const explainKind = nonEmptyString(
      detail?.analysis_kind,
      detail?.explain_kind,
      runtime?.kind,
      runtime?.explain_kind,
    );
    const explainMetrics = normalizeExplainMetrics(detail?.explain_metrics);
    let detailFields = cloneArray(detail?.explain_detail_fields);
    if (!detailFields.length) detailFields = cloneArray(detail?.drilldown_detail_fields);
    if (!detailFields.length) detailFields = cloneArray(runtime?.detail_fields);
    if (!detailFields.length) detailFields = cloneArray(runtime?.detailFields);
    if (!detailFields.length) detailFields = cloneArray(mapped?.detailFields);
    let columns = cloneArray(detail?.drilldown_columns);
    if (!columns.length) columns = cloneArray(runtime?.columns);
    if (!columns.length) columns = cloneArray(runtime?.detail_fields);
    if (!columns.length) columns = cloneArray(runtime?.detailFields);
    if (!columns.length) columns = cloneArray(detailFields);
    if (!columns.length) columns = cloneArray(mapped?.columns);
    let headers = cloneArray(detail?.drilldown_headers);
    if (!headers.length) headers = cloneArray(runtime?.headers);
    if (!headers.length) headers = cloneArray(mapped?.headers);
    let basisRefs = cloneArray(detail?.explain_basis_refs);
    if (!basisRefs.length) basisRefs = cloneArray(detail?.drilldown_basis_refs);
    if (!basisRefs.length) basisRefs = cloneArray(runtime?.basis_refs);
    if (!basisRefs.length) basisRefs = cloneArray(runtime?.basisRefs);
    let recommendedDimensions = cloneArray(detail?.explain_recommended_dimensions);
    if (!recommendedDimensions.length) recommendedDimensions = cloneArray(detail?.drilldown_recommended_dimensions);
    if (!recommendedDimensions.length) recommendedDimensions = cloneArray(runtime?.recommended_dimensions);
    if (!recommendedDimensions.length) recommendedDimensions = cloneArray(runtime?.recommendedDimensions);
    const ratioNumerator = nonEmptyString(
      detail?.drilldown_ratio_numerator,
      runtime?.ratio_numerator,
      runtime?.ratioNumerator,
    );
    const ratioDenominator = nonEmptyString(
      detail?.drilldown_ratio_denominator,
      runtime?.ratio_denominator,
      runtime?.ratioDenominator,
    );
    const ratioFormula = nonEmptyString(
      detail?.drilldown_ratio_formula,
      runtime?.ratio_formula,
      runtime?.ratioFormula,
    );
    const tableMetricId = nonEmptyString(
      detail?.drilldown_table_metric_id,
      detail?.drilldown_table_metric,
      runtime?.table_metric_id,
      runtime?.tableMetricId,
      mapped?.tableMetricId,
    );
    const datasetId = nonEmptyString(
      detail?.explain_detail_dataset,
      detail?.drilldown_dataset_id,
      runtime?.dataset_id,
      runtime?.datasetId,
      mapped?.datasetId,
      detail?.dataset_id,
    );
    const layoutPreset = nonEmptyString(
      detail?.drilldown_layout_preset,
      runtime?.layout_preset,
      runtime?.layoutPreset,
      mapped?.layoutPreset,
    );
    const tabMetrics = normalizeTabMetricOverrides(
      popup?.metrics,
      detail?.analysis_tab_metrics,
      detail?.drilldown_tab_metrics,
      runtime?.analysis_tab_metrics,
      runtime?.tab_metrics,
      runtime?.tabMetrics,
      mapped?.tabMetrics,
    );
    const hasDetail = Boolean(
      tableMetricId ||
        columns.length ||
        detailFields.length ||
        nonEmptyString(detail?.drilldown_dataset_id, runtime?.dataset_id, runtime?.datasetId, mapped?.datasetId),
    );
    const tabs = resolveDrilldownTabs({
      detail,
      runtime,
      mapped,
      explainKind,
      hasDetail,
    });
    const ratioNote = buildRatioExplainNote({
      numerator: ratioNumerator,
      denominator: ratioDenominator,
      formula: ratioFormula,
    });
    return {
      enabled: popup?.mode === "popup" || (runtimeEnabled !== false && Boolean(sceneId)),
      sceneId,
      title: nonEmptyString(
        detail?.explain_title,
        detail?.drilldown_title,
        runtime?.title,
        mapped?.title,
        detail?.label,
        metricId,
        "指标明细",
      ),
      note: nonEmptyString(
        detail?.explain_note,
        detail?.analysis_note,
        detail?.drilldown_note,
        runtime?.note,
        mapped?.note,
        ratioNote,
      ),
      tableMetricId,
      datasetId,
      columns,
      headers,
      detailFields,
      basisRefs,
      recommendedDimensions,
      ratioParts: {
        numerator: ratioNumerator,
        denominator: ratioDenominator,
        formula: ratioFormula,
      },
      compositionBy: cloneArray(detail?.explain_composition_by).length
        ? cloneArray(detail?.explain_composition_by)
        : cloneArray(recommendedDimensions),
      trendField: nonEmptyString(detail?.explain_trend_field),
      trendGrain: nonEmptyString(detail?.explain_trend_grain, "month"),
      layoutPreset,
      explainKind,
      explainMetrics,
      tabs,
      tabMetrics,
      link: {
        mode: nonEmptyString(analysisLink.mode),
        template: nonEmptyString(analysisLink.template),
        entry: nonEmptyString(analysisLink.entry),
        defaultFocus: nonEmptyString(analysisLink.default_focus, analysisLink.defaultFocus),
      },
      popup: {
        mode: nonEmptyString(popup?.mode, "popup"),
        template: nonEmptyString(popup?.template),
        focus: nonEmptyString(popup?.focus),
      },
      chartKind: nonEmptyString(runtime?.chart_kind, runtime?.chartKind),
      mapping: runtime?.mapping && typeof runtime.mapping === "object" ? runtime.mapping : null,
      pageSize:
        positiveInt(detail?.drilldown_page_size, runtime?.page_size, runtime?.pageSize, mapped?.pageSize, 8) || 8,
      cellPreviewMaxChars:
        positiveInt(
          detail?.drilldown_cell_preview_max_chars,
          runtime?.cell_preview_max_chars,
          runtime?.cellPreviewMaxChars,
          mapped?.cellPreviewMaxChars,
        ) > 0
          ? positiveInt(
              detail?.drilldown_cell_preview_max_chars,
              runtime?.cell_preview_max_chars,
              runtime?.cellPreviewMaxChars,
              mapped?.cellPreviewMaxChars,
            )
          : 0,
      columnMinWidth:
        positiveInt(
          detail?.drilldown_column_min_width,
          runtime?.column_min_width,
          runtime?.columnMinWidth,
          mapped?.columnMinWidth,
        ) > 0
          ? positiveInt(
              detail?.drilldown_column_min_width,
              runtime?.column_min_width,
              runtime?.columnMinWidth,
              mapped?.columnMinWidth,
            )
          : 0,
    };
  }

  function resolveAccessAppPath(pathname = window.location.pathname) {
    const raw = String(pathname || "");
    const prefix = "/apps/access/";
    if (!raw.startsWith(prefix)) return "";
    const tail = raw.slice(prefix.length);
    const marker = tail.indexOf("/scene/");
    const app = marker >= 0 ? tail.slice(0, marker) : tail;
    return String(app || "").trim();
  }

  async function fetchPopupDatasetRows(detail, datasetId) {
    const appPath = resolveAccessAppPath();
    const sceneId = nonEmptyString(detail?.scene_id);
    if (!appPath || !sceneId || !datasetId) return null;
    const response = await fetch(`/api/datasets/query/${appPath}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        scene_id: sceneId,
        target: nonEmptyString(detail?.scene_path),
        dataset_id: datasetId,
        page: 1,
        page_size: 100000,
        full: true,
      }),
    });
    if (!response.ok) {
      throw new Error(await response.text());
    }
    const payload = await response.json();
    return {
      rows: Array.isArray(payload?.rows) ? payload.rows : [],
      columns: Array.isArray(payload?.columns) ? payload.columns : [],
    };
  }

  function monthBucketLabel(value) {
    const raw = String(value || "").trim();
    if (!raw) return "";
    const match = raw.match(/^(\d{4})[-/年](\d{1,2})/);
    if (match) {
      return `${match[1]}-${String(match[2]).padStart(2, "0")}`;
    }
    return raw.slice(0, 7);
  }

  function groupRowsByCount(rows, field) {
    const grouped = new Map();
    rows.forEach((row) => {
      const key = String(row?.[field] ?? "").trim() || "未标注";
      grouped.set(key, (grouped.get(key) || 0) + 1);
    });
    return Array.from(grouped.entries())
      .map(([label, value]) => ({ label, value }))
      .sort((a, b) => Number(b.value || 0) - Number(a.value || 0));
  }

  function groupRowsByMonth(rows, field) {
    const grouped = new Map();
    rows.forEach((row) => {
      const key = monthBucketLabel(row?.[field]);
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

  function buildStaticChartModel(title, tabId, rows, mapping = null) {
    const normalized = normalizeTabId(tabId);
    const data = {
      columns: Array.isArray(rows) && rows.length > 0 ? Object.keys(rows[0]) : [],
      rows: Array.isArray(rows) ? rows : [],
    };
    const defaultMapping =
      normalized === "trend"
        ? { x: "month", y: "value" }
        : { x: "label", y: "value" };
    return {
      title: String(title || ""),
      data,
      mapping: mapping && typeof mapping === "object" ? mapping : defaultMapping,
    };
  }

  function buildDrilldownTableProps(detail, config) {
    const runtimeRefConfig = config?.runtimeRef && typeof config.runtimeRef === "object" ? config.runtimeRef : {};
    const sceneId = nonEmptyString(runtimeRefConfig.sceneId, config?.sceneId, detail?.scene_id);
    if (!sceneId) return null;
    const appPath = resolveAccessAppPath();
    if (!appPath) return null;
    const datasetId =
      nonEmptyString(
        runtimeRefConfig.datasetId,
        detail?.drilldown_dataset_id,
        config?.datasetId,
        DRILLDOWN_DATASET_BY_SCENE[sceneId],
        detail?.dataset_id,
      ) || sceneId;
    const metricId = config?.suppressDetailMetricFallback
      ? nonEmptyString(runtimeRefConfig.metricId, config?.tableMetricId)
      : nonEmptyString(runtimeRefConfig.metricId, config?.tableMetricId, detail?.drilldown_table_metric_id);
    const runtimeRef = metricId
      ? {
          kind: "metric",
          scene_id: sceneId,
          dataset_id: datasetId,
          metric_id: metricId,
        }
      : {
          kind: "data",
          scene_id: sceneId,
          dataset_id: datasetId,
        };
    return {
      columns: Array.isArray(config?.columns) ? config.columns : [],
      headers: Array.isArray(config?.headers) && config.headers.length > 0 ? config.headers : undefined,
      layoutPreset: config?.layoutPreset || "default",
      embedded: true,
      pageSize: Number(config?.pageSize) > 0 ? Number(config.pageSize) : 8,
      cellPreviewMaxChars:
        Number(config?.cellPreviewMaxChars) > 0 ? Number(config.cellPreviewMaxChars) : undefined,
      columnMinWidth:
        Number(config?.columnMinWidth) > 0 ? Number(config.columnMinWidth) : undefined,
      pagination: true,
      paginationMode: "client",
      dataset: {
        shape: metricId ? "dataframe" : "table",
        __mei_runtime_ref: runtimeRef,
      },
      _mei: {
        dataset_query_api: `/api/datasets/query/${appPath}`,
        metric_query_api: `/api/datasets/metrics/${appPath}`,
        active_scene_id: sceneId,
        active_target_file: nonEmptyString(runtimeRefConfig.scenePath, detail?.scene_path),
      },
    };
  }

  function drilldownChartTag(chartKind, tabId) {
    const explicit = String(chartKind || "").trim().toLowerCase();
    const fallback = normalizeTabId(tabId) === "trend" ? "line" : "bar";
    const kind = explicit || fallback;
    const supported = new Set([
      "line",
      "area",
      "trend",
      "column",
      "bar",
      "scatter",
      "pie",
      "donut",
      "rose",
      "radar",
      "ranking",
      "boxplot",
    ]);
    if (!supported.has(kind)) return "";
    return `mei-chart-${kind}`;
  }

  function buildDrilldownChartProps(detail, config, tabId) {
    const tableProps = buildDrilldownTableProps(detail, config);
    if (!tableProps) return null;
    const chartTag = drilldownChartTag(config?.chartKind, tabId);
    if (!chartTag) return null;
    const columns = Array.isArray(config?.columns) ? config.columns : [];
    const xField = columns[0] || "label";
    const yField = columns[1] || "value";
    const mapping =
      config?.mapping && typeof config.mapping === "object"
        ? config.mapping
        : {
            x: xField,
            y: yField,
          };
    return {
      chartTag,
      props: {
        title: String(config?.title || ""),
        data: tableProps.dataset,
        _mei: tableProps._mei,
        mapping,
        chartHeight: 300,
      },
    };
  }

  function mountDrilldownChart(root, detail, config, tabId) {
    const host = root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const chart = buildDrilldownChartProps(detail, config, tabId);
    if (!chart) return false;
    host.replaceChildren();
    const node = document.createElement(chart.chartTag);
    node.dataset.props = JSON.stringify(chart.props);
    host.appendChild(node);
    return true;
  }

  function mountDrilldownTable(root, detail, config) {
    const host = root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const props = buildDrilldownTableProps(detail, config);
    if (!props) {
      return false;
    }
    host.replaceChildren();
    const table = document.createElement("mei-cockpit-qunfu-data-table");
    table.dataset.props = JSON.stringify(props);
    host.appendChild(table);
    return true;
  }

  async function mountDerivedDrilldownContent(root, detail, config, tabId) {
    const host = root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const datasetId = nonEmptyString(config?.datasetId, detail?.dataset_id);
    if (!datasetId) return false;
    const dataset = await fetchPopupDatasetRows(detail, datasetId);
    const rows = Array.isArray(dataset?.rows) ? dataset.rows : [];
    if (explainMetricKind(config, tabId) === "composition") {
      const dimension = nonEmptyString(
        Array.isArray(config?.compositionBy) ? config.compositionBy[0] : "",
        Array.isArray(config?.recommendedDimensions) ? config.recommendedDimensions[0] : "",
      );
      if (!dimension) return false;
      const grouped = groupRowsByCount(rows, dimension);
      if (!grouped.length) return false;
      host.replaceChildren();
      const node = document.createElement("mei-chart-bar");
      node.dataset.props = JSON.stringify(
        buildStaticChartModel(config?.title || `${dimension}构成`, tabId, grouped, {
          x: "label",
          y: "value",
        }),
      );
      host.appendChild(node);
      return true;
    }
    if (explainMetricKind(config, tabId) === "trend") {
      const trendField = nonEmptyString(config?.trendField);
      if (!trendField) return false;
      const grouped = groupRowsByMonth(rows, trendField);
      if (!grouped.length) return false;
      host.replaceChildren();
      const node = document.createElement("mei-chart-line");
      node.dataset.props = JSON.stringify(
        buildStaticChartModel(config?.title || "趋势", tabId, grouped, {
          x: "month",
          y: "value",
        }),
      );
      host.appendChild(node);
      return true;
    }
    return false;
  }

  function renderDrilldownContent(root, detail, config, tabId) {
    const activeConfig = resolveDrilldownTabConfig(config, tabId);
    applyDrilldownOverlayMeta(root, activeConfig);
    const host = root.querySelector('[data-drilldown-table-host="true"]');
    if (!(host instanceof HTMLElement)) {
      return false;
    }
    const normalizedTab = normalizeTabId(tabId);
    const explainMetric = explainMetricForTab(config, tabId);
    const hasCustomMetricSource = Boolean(
      config?.tabMetrics?.[normalizedTab] || (explainMetric?.metric && typeof explainMetric.metric === "object"),
    );
    if (
      isDrilldownSummaryTab(tabId, config) ||
      (isDrilldownAnalysisTab(tabId, config) && !hasCustomMetricSource)
    ) {
      if (isDrilldownAnalysisTab(tabId, config) && !hasCustomMetricSource) {
        setDrilldownOverlayStatus(root, "loading");
        mountDerivedDrilldownContent(root, detail, activeConfig, tabId)
          .then((mounted) => {
            if (mounted) {
              setDrilldownOverlayStatus(root, "ready");
              return;
            }
            const summaryConfig = {
              ...activeConfig,
              note: nonEmptyString(activeConfig.note, unconfiguredTabNote(tabId)),
            };
            host.replaceChildren(createDrilldownSummaryNode(summaryConfig, tabId));
            setDrilldownOverlayStatus(root, "ready");
          })
          .catch(() => {
            const summaryConfig = {
              ...activeConfig,
              note: nonEmptyString(activeConfig.note, unconfiguredTabNote(tabId)),
            };
            host.replaceChildren(createDrilldownSummaryNode(summaryConfig, tabId));
            setDrilldownOverlayStatus(root, "ready");
          });
        return true;
      }
      host.replaceChildren(createDrilldownSummaryNode(activeConfig, tabId));
      setDrilldownOverlayStatus(root, "ready");
      return true;
    }
    if (isDrilldownAnalysisTab(tabId, config)) {
      setDrilldownOverlayStatus(root, "loading");
      if (mountDrilldownChart(root, detail, activeConfig, tabId)) {
        setDrilldownOverlayStatus(root, "ready");
        return true;
      }
    }
    setDrilldownOverlayStatus(root, "loading");
    if (!mountDrilldownTable(root, detail, activeConfig)) {
      setDrilldownOverlayStatus(root, "error");
      return false;
    }
    setDrilldownOverlayStatus(root, "ready");
    return true;
  }

  function renderDrilldownTabs(root, detail, config) {
    const tabsHost = root.querySelector('[data-drilldown-tabs="true"]');
    if (!(tabsHost instanceof HTMLElement)) {
      return defaultActiveDrilldownTab(config?.tabs || []);
    }
    const normalizedTabs = Array.from(
      new Set(
        (Array.isArray(config?.tabs) ? config.tabs : [])
          .map((tab) => normalizeTabId(tab))
          .filter(Boolean),
      ),
    );
    const tabs = normalizedTabs.length
      ? normalizedTabs
      : [defaultActiveDrilldownTab(defaultDrilldownTabs(config?.explainKind, { hasDetail: true }))];
    const preferredTab = normalizeTabId(
      nonEmptyString(config?.popup?.focus, config?.link?.defaultFocus),
    );
    const activeTab = preferredTab && tabs.includes(preferredTab) ? preferredTab : defaultActiveDrilldownTab(tabs);
    tabsHost.replaceChildren();
    tabsHost.toggleAttribute("hidden", tabs.length <= 1);
    tabs.forEach((tab) => {
      const explainMetric = explainMetricForTab(config, tab);
      const button = document.createElement("button");
      button.type = "button";
      button.className = "access-drilldown-tab-button";
      button.dataset.drilldownTab = tab;
      button.setAttribute("role", "tab");
      button.setAttribute("aria-selected", tab === activeTab ? "true" : "false");
      button.textContent = nonEmptyString(explainMetric?.label, drilldownTabLabel(explainMetric?.kind || tab));
      button.addEventListener("click", () => {
        if (button.getAttribute("aria-selected") === "true") return;
        tabsHost
          .querySelectorAll(".access-drilldown-tab-button")
          .forEach((node) => node.setAttribute("aria-selected", node === button ? "true" : "false"));
        renderDrilldownContent(root, detail, config, tab);
      });
      tabsHost.appendChild(button);
    });
    return activeTab;
  }

  function ensureDrilldownOverlayRoot() {
    let root = document.getElementById(DRILLDOWN_OVERLAY_ROOT_ID);
    if (root) return root;
    root = document.createElement("div");
    root.id = DRILLDOWN_OVERLAY_ROOT_ID;
    root.className = "access-drilldown-overlay";
    root.setAttribute("hidden", "hidden");
    root.innerHTML =
      '<div class="access-drilldown-overlay-backdrop" data-drilldown-close="mask"></div>' +
      '<section class="access-drilldown-overlay-panel" role="dialog" aria-modal="true" aria-label="指标下钻明细">' +
      '<header class="access-drilldown-overlay-head">' +
      '<div class="access-drilldown-overlay-head-meta">' +
      '<div class="access-drilldown-overlay-title" data-drilldown-title="true"></div>' +
      '<div class="access-drilldown-overlay-note" data-drilldown-note="true" hidden></div>' +
      "</div>" +
      '<button type="button" class="access-drilldown-overlay-close" data-drilldown-close="button" aria-label="关闭">×</button>' +
      "</header>" +
      '<div class="access-drilldown-overlay-tabs" data-drilldown-tabs="true" hidden></div>' +
      '<div class="access-drilldown-overlay-body">' +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="loading">正在加载明细表...</div>' +
      '<div class="access-drilldown-overlay-status" data-drilldown-status="error" hidden>明细表加载失败，请稍后重试。</div>' +
      '<div class="access-drilldown-table-shell" data-drilldown-status="ready" hidden>' +
      '<div class="access-drilldown-table-host" data-drilldown-table-host="true"></div>' +
      "</div>" +
      "</div>" +
      "</section>";
    root.addEventListener("click", (event) => {
      const target = event.target;
      if (!(target instanceof HTMLElement)) return;
      if (!target.dataset.drilldownClose) return;
      closeDrilldownOverlay();
    });
    document.body.appendChild(root);
    return root;
  }

  function setDrilldownOverlayStatus(root, status) {
    root
      .querySelectorAll("[data-drilldown-status]")
      .forEach((node) => node.toggleAttribute("hidden", node.dataset.drilldownStatus !== status));
  }

  function closeDrilldownOverlay() {
    const root = document.getElementById(DRILLDOWN_OVERLAY_ROOT_ID);
    if (!root) return;
    root.setAttribute("hidden", "hidden");
    root.classList.remove("is-open");
    const host = root.querySelector('[data-drilldown-table-host="true"]');
    if (host instanceof HTMLElement) {
      host.replaceChildren();
    }
    document.body.classList.remove("access-drilldown-open");
  }

  function openDrilldownOverlay(detail) {
    const config = resolveDrilldownConfig(detail);
    if (!config.enabled || !config.sceneId) return;
    const root = ensureDrilldownOverlayRoot();
    applyDrilldownOverlayMeta(root, config);
    const activeTab = renderDrilldownTabs(root, detail, config);
    if (!renderDrilldownContent(root, detail, config, activeTab)) {
      root.removeAttribute("hidden");
      root.classList.add("is-open");
      document.body.classList.add("access-drilldown-open");
      return;
    }
    root.removeAttribute("hidden");
    root.classList.add("is-open");
    document.body.classList.add("access-drilldown-open");
  }

  function installDrilldownOverlayHost() {
    if (window.self !== window.top) return;
    if (!isAccessRoute()) return;
    if (boot.metricDrilldownHostMounted) return;
    boot.metricDrilldownHostMounted = true;
    const openByEvent = (event) => {
      if (!isAccessRoute()) return;
      const detail = event?.detail || {};
      const config = resolveDrilldownConfig(detail);
      if (!config.enabled || !config.sceneId) return;
      openDrilldownOverlay(detail);
    };
    document.addEventListener(METRIC_DRILLDOWN_EVENT, openByEvent);
    document.addEventListener(ANALYSIS_OPEN_EVENT, openByEvent);
    document.addEventListener(POPUP_OPEN_EVENT, openByEvent);
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        closeDrilldownOverlay();
      }
    });
  }

  function patchDrilldownTableByMetric(tableMetricId) {
    const metricId = String(tableMetricId || "").trim();
    if (!metricId) return true;
    const table = document.querySelector("mei-dataset-table");
    if (!(table instanceof HTMLElement)) return false;
    let props = {};
    try {
      props = JSON.parse(table.dataset.props || "{}");
    } catch (_) {
      props = {};
    }
    if (!props || typeof props !== "object") return true;
    const currentData = props.data && typeof props.data === "object" ? props.data : {};
    const runtimeRef =
      (currentData && currentData.__mei_runtime_ref) ||
      (props.dataset && props.dataset.__mei_runtime_ref) ||
      {};
    const datasetId = String(
      runtimeRef.dataset_id ||
        currentData.from_dataset ||
        currentData.id ||
        props?.dataset?.id ||
        "",
    ).trim();
    if (!datasetId) return true;
    props.data = { __ref: "metric", id: metricId, from_dataset: datasetId };
    table.dataset.props = JSON.stringify(props);
    const remount = document.createElement("mei-dataset-table");
    remount.dataset.props = table.dataset.props;
    table.replaceWith(remount);
    window.dispatchEvent(new Event("meilang:preview-updated"));
    return true;
  }

  function renderDrilldownContextBanner(title, note) {
    const header = String(title || "").trim();
    const body = String(note || "").trim();
    if (!header && !body) return;
    let banner = document.getElementById(DRILLDOWN_CONTEXT_BANNER_ID);
    if (!banner) {
      banner = document.createElement("div");
      banner.id = DRILLDOWN_CONTEXT_BANNER_ID;
      banner.className = "access-drilldown-context-banner";
      document.body.appendChild(banner);
    }
    banner.innerHTML =
      `<div class="access-drilldown-context-title">${header || "指标口径"}</div>` +
      (body ? `<div class="access-drilldown-context-note">${body}</div>` : "");
  }

  function clearDrilldownContextBanner() {
    const banner = document.getElementById(DRILLDOWN_CONTEXT_BANNER_ID);
    if (banner) {
      banner.remove();
    }
  }

  function applyDrilldownContextFromQuery() {
    clearTimeout(drilldownContextRetryTimer);
    if (!isAccessRoute()) {
      clearDrilldownContextBanner();
      return;
    }
    let parsed = null;
    try {
      parsed = new URL(window.location.href);
    } catch (_) {
      return;
    }
    const metricId = String(parsed.searchParams.get("drill_metric") || "").trim();
    if (!metricId) {
      clearDrilldownContextBanner();
      return;
    }
    const context = DRILLDOWN_METRIC_CONTEXT[metricId] || {};
    const title = parsed.searchParams.get("drill_title") || context.title || "";
    const note = parsed.searchParams.get("drill_note") || context.note || "";
    renderDrilldownContextBanner(title, note);
    const tableMetricId =
      String(parsed.searchParams.get("drill_table_metric") || context.tableMetricId || "").trim();
    if (!tableMetricId) return;
    let attempts = 0;
    const retry = () => {
      attempts += 1;
      if (patchDrilldownTableByMetric(tableMetricId) || attempts >= 24) {
        return;
      }
      drilldownContextRetryTimer = window.setTimeout(retry, 90);
    };
    retry();
  }

  function currentMainPane() {
    return document.querySelector("#workspace-root main.main");
  }

  function clearManageWorkspaceLoadingState() {
    const currentMain = currentMainPane();
    if (!currentMain) return;
    currentMain.removeAttribute("aria-busy");
    const overlay = currentMain.querySelector('[data-mei-manage-nav-loading="true"]');
    if (overlay) overlay.remove();
  }

  function navigationTargetLabel(url) {
    try {
      const parsed = new URL(url, window.location.href);
      const file = String(parsed.searchParams.get("file") || "").trim();
      if (file) return file;
      const scene = String(parsed.searchParams.get("scene") || "").trim();
      if (scene) return `scene:${scene}`;
    } catch (_) {}
    return "目标预览";
  }

  function showManageWorkspaceLoadingState(url) {
    const currentUrl = new URL(window.location.href);
    const nextUrl = new URL(url, window.location.href);
    const isSameManageRoute =
      currentUrl.pathname === nextUrl.pathname &&
      currentUrl.pathname.startsWith("/apps/manage/");
    if (!isSameManageRoute) {
      clearManageWorkspaceLoadingState();
      return;
    }
    const currentMain = currentMainPane();
    if (!currentMain) return;
    currentMain.setAttribute("aria-busy", "true");
    let overlay = currentMain.querySelector('[data-mei-manage-nav-loading="true"]');
    if (!overlay) {
      overlay = document.createElement("div");
      overlay.setAttribute("data-mei-manage-nav-loading", "true");
      overlay.style.cssText = [
        "position:absolute",
        "inset:0",
        "z-index:40",
        "display:grid",
        "place-items:center",
        "padding:24px",
        "background:linear-gradient(180deg, rgba(8,15,30,.42), rgba(8,15,30,.70))",
        "backdrop-filter:blur(2px)",
        "pointer-events:none",
      ].join(";");
      const card = document.createElement("div");
      card.style.cssText = [
        "display:grid",
        "gap:8px",
        "min-width:220px",
        "padding:16px 18px",
        "border-radius:14px",
        "border:1px solid rgba(96,165,250,.35)",
        "background:rgba(15,23,42,.88)",
        "box-shadow:0 12px 40px rgba(2,6,23,.28)",
        "color:#e2e8f0",
        "text-align:center",
      ].join(";");
      const title = document.createElement("strong");
      title.textContent = "正在切换预览";
      title.style.cssText = "font-size:14px;font-weight:700;color:#f8fafc;";
      const detail = document.createElement("span");
      detail.setAttribute("data-mei-manage-nav-target", "true");
      detail.style.cssText =
        "font-size:12px;line-height:1.5;color:#93c5fd;font-family:ui-monospace,SFMono-Regular,monospace;";
      const hint = document.createElement("span");
      hint.textContent = "旧画面将被替换，请稍候...";
      hint.style.cssText = "font-size:11px;line-height:1.5;color:#94a3b8;";
      card.appendChild(title);
      card.appendChild(detail);
      card.appendChild(hint);
      overlay.appendChild(card);
      if (getComputedStyle(currentMain).position === "static") {
        currentMain.style.position = "relative";
      }
      currentMain.appendChild(overlay);
    }
    const detail = overlay.querySelector('[data-mei-manage-nav-target="true"]');
    if (detail) {
      detail.textContent = navigationTargetLabel(url);
    }
  }

  function createLoadingOverlay() {
    if (document.getElementById("mei-spa-loading")) return;
    const overlay = document.createElement("div");
    overlay.id = "mei-spa-loading";
    overlay.className = "spa-loading-overlay";
    overlay.innerHTML =
      '<div class="spa-loading-inner">' +
      '<img class="spa-loading-icon" src="/app-assets/favicon.svg" alt="loading"/>' +
      '<span class="spa-loading-text">加载中...</span>' +
      "</div>";
    document.body.appendChild(overlay);
  }

  function clearLoadingTimer() {
    if (loadingTimer) {
      clearTimeout(loadingTimer);
      loadingTimer = null;
    }
  }

  function showLoading() {
    clearLoadingTimer();
    loadingTimer = setTimeout(() => {
      createLoadingOverlay();
      const overlay = document.getElementById("mei-spa-loading");
      if (!overlay) return;
      overlay.classList.add("is-visible");
      loadingVisibleAt = Date.now();
      loadingTimer = null;
    }, LOADING_DELAY_MS);
  }

  function hideLoading() {
    clearLoadingTimer();
    const overlay = document.getElementById("mei-spa-loading");
    if (!overlay) return;
    if (!overlay.classList.contains("is-visible")) {
      return;
    }
    const elapsed = Date.now() - loadingVisibleAt;
    const finish = () => {
      overlay.classList.remove("is-visible");
    };
    if (elapsed < LOADING_MIN_VISIBLE_MS) {
      setTimeout(finish, LOADING_MIN_VISIBLE_MS - elapsed);
    } else {
      finish();
    }
  }

  function forceHideLoading() {
    clearLoadingTimer();
    const overlay = document.getElementById("mei-spa-loading");
    if (overlay) {
      overlay.classList.remove("is-visible");
    }
  }

  function finishNavigationUi(navigationId) {
    clearManageWorkspaceLoadingState();
    if (navigationId !== currentNavigationId && spaNavigationInFlight > 0) {
      return;
    }
    forceHideLoading();
    clearManageWorkspaceLoadingState();
  }

  function isManageSamePathNavigation(currentUrl, nextUrl) {
    return (
      currentUrl.pathname === nextUrl.pathname &&
      currentUrl.pathname.startsWith("/apps/manage/")
    );
  }

  function shouldReloadHostBundle(path, currentUrl, nextUrl) {
    if (path === "/app-bundles/manage.js") {
      const cur = currentUrl.pathname.startsWith("/apps/manage/");
      const next = nextUrl.pathname.startsWith("/apps/manage/");
      return cur !== next;
    }
    if (path === "/app-bundles/access.js") {
      const cur = currentUrl.pathname.startsWith("/apps/access/");
      const next = nextUrl.pathname.startsWith("/apps/access/");
      return cur !== next;
    }
    return false;
  }

  function syncManageTabFromUrl(url) {
    try {
      const tab = new URL(url, window.location.href).searchParams.get("tab");
      if (typeof boot.switchManageTab === "function") {
        boot.switchManageTab(tab || "preview", { updateUrl: false, emit: true });
      }
    } catch (_) {}
  }

  function sameOrigin(url) {
    try {
      const parsed = new URL(url, window.location.href);
      return parsed.origin === window.location.origin;
    } catch (_) {
      return false;
    }
  }

  function shouldHandleUrl(url) {
    if (!sameOrigin(url)) return false;
    const parsed = new URL(url, window.location.href);
    return parsed.pathname.startsWith("/apps/");
  }

  function isSameLocation(url) {
    try {
      const next = new URL(url, window.location.href);
      const current = new URL(window.location.href);
      return (
        next.pathname === current.pathname &&
        next.search === current.search &&
        next.hash === current.hash
      );
    } catch (_) {
      return false;
    }
  }

  function resolveClickTarget(event) {
    const path = event.composedPath ? event.composedPath() : [];
    for (const item of path) {
      if (item instanceof HTMLAnchorElement && item.href) {
        return {
          url: item.href,
          target: item.getAttribute("target") || "",
          download: item.hasAttribute("download"),
        };
      }
      if (
        item instanceof HTMLElement &&
        item.tagName === "SL-BUTTON" &&
        item.hasAttribute("href")
      ) {
        const rawHref = item.getAttribute("href") || "";
        let absolute = rawHref;
        try {
          absolute = new URL(rawHref, window.location.href).href;
        } catch (_) {}
        return {
          url: absolute,
          target: item.getAttribute("target") || "",
          download: item.hasAttribute("download"),
        };
      }
    }
    return null;
  }

  /** 仅管理视图 Tab 走客户端切换；顶栏、资源树与其它 /apps/ 链路由全局 SPA 拦截。 */
  function shouldBypassSpaClick(event) {
    const path = event.composedPath ? event.composedPath() : [];
    for (const item of path) {
      if (
        item instanceof HTMLElement &&
        item.matches &&
        item.matches("a.manage-view-tab[data-manage-tab]")
      ) {
        return true;
      }
    }
    return false;
  }

  function normalizePath(rawUrl) {
    try {
      const parsed = new URL(rawUrl, window.location.href);
      return parsed.pathname;
    } catch (_) {
      return "";
    }
  }

  function collectBodyScripts(doc) {
    return Array.from(doc.body.querySelectorAll("script[src]"))
      .map((script) => script.getAttribute("src") || "")
      .map((src) => src.trim())
      .filter(Boolean);
  }

  function tagExistingBodyScripts() {
    Array.from(document.body.querySelectorAll("script[src]")).forEach((script) => {
      const src = script.getAttribute("src");
      if (!src) return;
      const path = normalizePath(src);
      if (!path || path === SPA_NAV_SCRIPT) return;
      if (path.startsWith("/workspace-components/")) {
        script.setAttribute("data-mei-persistent-script", path);
        return;
      }
      if (path.startsWith("/app-assets/")) {
        if (RELOAD_APP_SCRIPTS.has(path)) {
          script.setAttribute("data-mei-reload-script", path);
        } else {
          script.setAttribute("data-mei-persistent-script", path);
        }
        return;
      }
      if (path.startsWith("/app-bundles/")) {
        if (RELOAD_BUNDLE_SCRIPTS.has(path)) {
          script.setAttribute("data-mei-reload-script", path);
        } else {
          script.setAttribute("data-mei-persistent-script", path);
        }
      }
    });
  }

  function disposeRuntimeHooks(options) {
    const opts = options || {};
    const names = [
      "disposeAgentPanel",
      "disposeStatusBar",
      "disposeManageTabs",
      "disposeWorkspaceSplitters",
      "disposeFrameStage",
      "disposeSourceTreeControls",
      "disposeSourceHighlight",
    ];
    names.forEach((name) => {
      if (opts.preserveAgentPanel && name === "disposeAgentPanel") return;
      if (opts.preserveStatusBar && name === "disposeStatusBar") return;
      if (opts.preserveManageTabs && name === "disposeManageTabs") return;
      if (opts.preserveWorkspaceSplitters && name === "disposeWorkspaceSplitters") return;
      if (opts.preserveFrameStage && name === "disposeFrameStage") return;
      if (opts.preserveSourceTreeControls && name === "disposeSourceTreeControls") return;
      if (opts.preserveSourceHighlight && name === "disposeSourceHighlight") return;
      const hook = boot[name];
      if (typeof hook === "function") {
        try {
          hook();
        } catch (_) {}
        boot[name] = null;
      }
    });
  }

  function loadScript(rawSrc, options) {
    const opts = options || {};
    const absolute = new URL(rawSrc, window.location.href).toString();
    if (opts.persistentKey) {
      const found = document.querySelector(
        'script[data-mei-persistent-script="' + opts.persistentKey + '"]',
      );
      if (found) return Promise.resolve();
    }
    if (opts.reloadKey) {
      document
        .querySelectorAll('script[data-mei-reload-script="' + opts.reloadKey + '"]')
        .forEach((node) => node.remove());
    }
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (fn) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        fn();
      };
      const timer = setTimeout(() => {
        if (opts.softFail) {
          console.warn("[spa-navigation] script load timeout", rawSrc);
          finish(resolve);
          return;
        }
        finish(() => reject(new Error("script load timeout: " + rawSrc)));
      }, SCRIPT_LOAD_TIMEOUT_MS);
      const script = document.createElement("script");
      if (opts.module) script.type = "module";
      script.src = absolute;
      script.async = false;
      if (opts.persistentKey) {
        script.setAttribute("data-mei-persistent-script", opts.persistentKey);
      }
      if (opts.reloadKey) {
        script.setAttribute("data-mei-reload-script", opts.reloadKey);
      }
      script.onload = () => finish(resolve);
      script.onerror = () => {
        if (opts.softFail) {
          console.warn("[spa-navigation] script load skipped", rawSrc);
          finish(resolve);
          return;
        }
        finish(() => reject(new Error("failed to load script: " + rawSrc)));
      };
      document.body.appendChild(script);
    });
  }

  function pulseManagePreview(detail) {
    dispatchManageContextChange(detail);
    window.dispatchEvent(new Event("meilang:preview-updated"));
    requestAnimationFrame(() => {
      window.dispatchEvent(new Event("meilang:preview-updated"));
      if (typeof boot.scheduleFrameViewportRelayout === "function") {
        try {
          boot.scheduleFrameViewportRelayout();
        } catch (_) {}
      }
    });
  }

  function publishManagePreviewFromDoc(doc) {
    const panelRoot =
      document.querySelector("#meilang-author-panel") ||
      (doc && doc.querySelector("#meilang-author-panel"));
    pulseManagePreview(extractManagePanelContext(panelRoot));
  }

  function replaceShellFromDoc(doc, url, replaceHistory) {
    const currentShell = document.querySelector(".shell");
    const nextShell = doc.querySelector(".shell");
    if (!currentShell || !nextShell) return false;
    currentShell.className = nextShell.className;
    currentShell.replaceChildren(
      ...Array.from(nextShell.childNodes).map((node) => node.cloneNode(true)),
    );
    if (replaceHistory) {
      window.history.replaceState({}, "", url);
    } else {
      window.history.pushState({}, "", url);
    }
    return true;
  }

  async function syncMissingWorkspaceModulesOnly(doc, navigationId) {
    const scripts = collectBodyScripts(doc).filter((src) => {
      const path = normalizePath(src);
      return path.startsWith("/workspace-components/");
    });
    for (const src of scripts) {
      if (navigationId !== currentNavigationId) return false;
      const path = normalizePath(src);
      if (
        document.querySelector(
          'script[data-mei-persistent-script="' + path + '"]',
        )
      ) {
        continue;
      }
      await loadScript(src, { module: true, persistentKey: path, softFail: true });
    }
    return true;
  }

  async function ensureHostBundlesFromDoc(doc, navigationId, currentUrl, nextUrl) {
    for (const src of collectBodyScripts(doc)) {
      if (navigationId !== currentNavigationId) return false;
      const path = normalizePath(src);
      if (path !== "/app-bundles/manage.js" && path !== "/app-bundles/access.js") {
        continue;
      }
      const alreadyLoaded =
        document.querySelector('script[data-mei-persistent-script="' + path + '"]') ||
        document.querySelector('script[data-mei-reload-script="' + path + '"]');
      if (alreadyLoaded) {
        if (currentUrl && nextUrl && shouldReloadHostBundle(path, currentUrl, nextUrl)) {
          await loadScript(path + "?spa=" + Date.now(), {
            reloadKey: path,
            softFail: true,
          });
        }
        continue;
      }
      await loadScript(src, { persistentKey: path, softFail: true });
    }
    return true;
  }

  async function syncScriptsFromDocument(doc, navigationId, options) {
    const opts = options || {};
    const currentUrl = opts.currentUrl;
    const nextUrl = opts.nextUrl;
    const scripts = collectBodyScripts(doc);
    for (const src of scripts) {
      if (navigationId !== currentNavigationId) return false;
      const path = normalizePath(src);
      if (!path) continue;
      if (path === SPA_NAV_SCRIPT) continue;
      if (
        path === "/app-bundles/manage.js" ||
        path === "/app-bundles/access.js"
      ) {
        if (
          currentUrl &&
          nextUrl &&
          !shouldReloadHostBundle(path, currentUrl, nextUrl)
        ) {
          await loadScript(src, { persistentKey: path });
          continue;
        }
      }
      if (
        opts.preserveManageWorkspace &&
        path === "/app-bundles/manage.js"
      ) {
        continue;
      }
      if (
        opts.preserveAgentPanel &&
        path.startsWith("/app-assets/") &&
        path.includes("agent-panel")
      ) {
        continue;
      }
      if (opts.preserveStatusBar && path === "/app-assets/statusbar.js") {
        continue;
      }
      if (
        opts.preserveWorkspaceSplitters &&
        path === "/app-assets/workspace-splitters.js"
      ) {
        continue;
      }
      if (
        opts.preserveSourceTreeControls &&
        path === "/app-assets/source-tree-controls.js"
      ) {
        continue;
      }
      if (path.startsWith("/workspace-components/")) {
        await loadScript(src, { module: true, persistentKey: path, softFail: true });
        continue;
      }
      if (path.startsWith("/app-assets/")) {
        if (RELOAD_APP_SCRIPTS.has(path)) {
          const withBuster = path + "?spa=" + Date.now();
          await loadScript(withBuster, { reloadKey: path, softFail: true });
          continue;
        }
        await loadScript(src, { persistentKey: path, softFail: true });
        continue;
      }
      if (path.startsWith("/app-bundles/")) {
        if (RELOAD_BUNDLE_SCRIPTS.has(path)) {
          const withBuster = path + "?spa=" + Date.now();
          await loadScript(withBuster, { reloadKey: path, softFail: true });
          continue;
        }
        await loadScript(src, { persistentKey: path, softFail: true });
        continue;
      }
    }
    return true;
  }

  function cloneNodeOrNull(node) {
    return node ? node.cloneNode(true) : null;
  }

  function extractManagePanelContext(root) {
    if (!root) return null;
    return {
      app: String(root.dataset.app || ""),
      scene: String(root.dataset.scene || ""),
      file: String(root.dataset.file || root.dataset.target || ""),
      sceneTarget: String(root.dataset.sceneTarget || ""),
      mode: String(root.dataset.mode || ""),
      sourceViews: String(root.dataset.sourceViews || ""),
      viewTab: String(root.dataset.viewTab || ""),
    };
  }

  function dispatchManageContextChange(detail) {
    if (!detail) return;
    document.dispatchEvent(
      new CustomEvent("mei:manage-context-change", {
        detail,
      }),
    );
  }

  function normalizeNavHref(rawHref) {
    try {
      const url = new URL(rawHref, window.location.href);
      url.searchParams.delete("tab");
      return url.pathname + "?" + url.searchParams.toString();
    } catch (_) {
      return String(rawHref || "");
    }
  }

  function syncSidebarLinkState(currentSidebar, nextSidebar) {
    if (!currentSidebar || !nextSidebar) return;
    currentSidebar.className = nextSidebar.className;
    const currentLinks = Array.from(currentSidebar.querySelectorAll("a.tree-link"));
    const nextLinks = Array.from(nextSidebar.querySelectorAll("a.tree-link"));
    const nextByKey = new Map();
    nextLinks.forEach((link) => {
      nextByKey.set(normalizeNavHref(link.getAttribute("href") || ""), link);
    });
    currentLinks.forEach((link) => {
      const key = normalizeNavHref(link.getAttribute("href") || "");
      const next = nextByKey.get(key);
      if (!next) return;
      link.className = next.className;
      link.setAttribute("href", next.getAttribute("href") || "");
      if (next.hasAttribute("title")) {
        link.setAttribute("title", next.getAttribute("title") || "");
      } else {
        link.removeAttribute("title");
      }
      Array.from(link.attributes)
        .filter((attr) => attr.name.startsWith("data-"))
        .forEach((attr) => link.removeAttribute(attr.name));
      Array.from(next.attributes)
        .filter((attr) => attr.name.startsWith("data-"))
        .forEach((attr) => link.setAttribute(attr.name, attr.value));
      link.innerHTML = next.innerHTML;
    });
    const currentDetails = Array.from(
      currentSidebar.querySelectorAll(".tree-li-branch > details"),
    );
    const nextDetails = Array.from(
      nextSidebar.querySelectorAll(".tree-li-branch > details"),
    );
    currentDetails.forEach((detail, index) => {
      if (index >= nextDetails.length) return;
      const wasOpen = detail.open;
      const serverWantsOpen = nextDetails[index].open;
      // 服务端仅按「选中路径」展开祖先；合并保留用户已展开的其它分支，避免换文件整树收起。
      detail.open = serverWantsOpen || wasOpen;
    });
  }

  function syncStatusbarContent(currentStatusbar, nextStatusbar) {
    if (!currentStatusbar || !nextStatusbar) return;
    currentStatusbar.className = nextStatusbar.className;
    const currentLayout = currentStatusbar.querySelector(".statusbar-layout");
    const nextLayout = nextStatusbar.querySelector(".statusbar-layout");
    if (!currentLayout || !nextLayout) return;
    currentLayout.className = nextLayout.className;
    const currentTracks = Array.from(currentLayout.children);
    const nextTracks = Array.from(nextLayout.children);
    currentTracks.forEach((track, index) => {
      if (index >= nextTracks.length) return;
      track.className = nextTracks[index].className;
      track.replaceChildren(
        ...Array.from(nextTracks[index].childNodes).map((node) => node.cloneNode(true)),
      );
    });
  }

  function syncElementAttributes(currentEl, nextEl, options) {
    if (!currentEl || !nextEl) return;
    const opts = options || {};
    const preserve = new Set(opts.preserve || []);
    Array.from(currentEl.attributes).forEach((attr) => {
      if (preserve.has(attr.name)) return;
      currentEl.removeAttribute(attr.name);
    });
    Array.from(nextEl.attributes).forEach((attr) => {
      if (preserve.has(attr.name) && currentEl.hasAttribute(attr.name)) return;
      currentEl.setAttribute(attr.name, attr.value);
    });
  }

  /** 同一 manage 路径下换 file/scene/tab 只换工作区，避免整页重载 manage bundle。 */
  function shouldPreserveManageWorkspace(currentUrl, nextUrl) {
    return (
      currentUrl.pathname === nextUrl.pathname &&
      currentUrl.pathname.startsWith("/apps/manage/")
    );
  }

  /** 同路径 SPA 只替换 #workspace-root 时，顶栏仍在壳外，需从下一页文档同步 href（访问 / 演示 / 应用切换）。 */
  function syncManageTopbarFromDoc(doc) {
    try {
      const currentHeader = document.querySelector("header.topbar-shell");
      const nextHeader = doc.querySelector("header.topbar-shell");
      if (!currentHeader || !nextHeader) return;

      const currentGroup = currentHeader.querySelector("sl-button-group.mode-tab-group");
      const nextGroup = nextHeader.querySelector("sl-button-group.mode-tab-group");
      if (currentGroup && nextGroup) {
        const curBtns = currentGroup.querySelectorAll("sl-button[href]");
        const nextBtns = nextGroup.querySelectorAll("sl-button[href]");
        const n = Math.min(curBtns.length, nextBtns.length);
        for (let i = 0; i < n; i++) {
          const nh = nextBtns[i].getAttribute("href");
          if (nh) curBtns[i].setAttribute("href", nh);
        }
      }

      const curLaunch = currentHeader.querySelector("sl-button.topbar-launch-btn");
      const nextLaunch = nextHeader.querySelector("sl-button.topbar-launch-btn");
      if (curLaunch && nextLaunch) {
        const nh = nextLaunch.getAttribute("href");
        if (nh) curLaunch.setAttribute("href", nh);
      }

      const curTabs = currentHeader.querySelectorAll("nav.app-tabs a[href]");
      const nextTabs = nextHeader.querySelectorAll("nav.app-tabs a[href]");
      const m = Math.min(curTabs.length, nextTabs.length);
      for (let j = 0; j < m; j++) {
        const h = nextTabs[j].getAttribute("href");
        if (h) curTabs[j].setAttribute("href", h);
      }

      const curBread = currentHeader.querySelector(".app-current-path");
      const nextBread = nextHeader.querySelector(".app-current-path");
      if (curBread && nextBread) {
        curBread.replaceWith(nextBread.cloneNode(true));
      }
    } catch (err) {
      console.warn("[spa-navigation] sync topbar skipped", err);
    }
  }

  function swapManageWorkspace(doc, url, replaceHistory) {
    const currentShell = document.querySelector(".shell");
    const nextShell = doc.querySelector(".shell");
    const currentWorkspace = document.getElementById("workspace-root");
    const nextWorkspace = doc.getElementById("workspace-root");
    const currentLeftSidebar =
      currentWorkspace && currentWorkspace.querySelector("aside.sidebar.left");
    const nextLeftSidebar =
      nextWorkspace && nextWorkspace.querySelector("aside.sidebar.left");
    const currentMain = currentWorkspace && currentWorkspace.querySelector("main.main");
    const nextMain = nextWorkspace && nextWorkspace.querySelector("main.main");
    const currentRightSidebar =
      currentWorkspace && currentWorkspace.querySelector("aside.sidebar.right");
    const nextRightSidebar =
      nextWorkspace && nextWorkspace.querySelector("aside.sidebar.right");
    const currentStatusbar = document.querySelector(".statusbar");
    const nextStatusbar = doc.querySelector(".statusbar");
    const nextPanelRoot =
      nextRightSidebar && nextRightSidebar.querySelector("#meilang-author-panel");
    const nextPanelContext = extractManagePanelContext(nextPanelRoot);

    if (
      !currentShell ||
      !nextShell ||
      !currentWorkspace ||
      !currentLeftSidebar ||
      !nextLeftSidebar ||
      !currentMain ||
      !nextMain ||
      !currentRightSidebar ||
      !nextRightSidebar
    ) {
      return false;
    }

    currentShell.className = nextShell.className;
    syncElementAttributes(currentWorkspace, nextWorkspace, { preserve: ["id"] });
    syncSidebarLinkState(currentLeftSidebar, nextLeftSidebar);
    currentRightSidebar.className = nextRightSidebar.className;
    const preparedMain = cloneNodeOrNull(nextMain);
    if (!preparedMain) return false;
    preparedMain.classList.add("spa-fragment-enter");
    currentMain.replaceWith(preparedMain);
    if (currentStatusbar && nextStatusbar) {
      syncStatusbarContent(currentStatusbar, nextStatusbar);
    }

    syncManageTopbarFromDoc(doc);

    if (replaceHistory) {
      window.history.replaceState({}, "", url);
    } else {
      window.history.pushState({}, "", url);
    }
    return true;
  }

  function runPostSpaWork(doc, url, navigationId, currentUrl, nextUrl) {
    void (async () => {
      try {
        if (navigationId !== currentNavigationId) return;
        if (!preserveManageWorkspaceFromUrls(currentUrl, nextUrl)) {
          const bundlesReady = await ensureHostBundlesFromDoc(
            doc,
            navigationId,
            currentUrl,
            nextUrl,
          );
          if (!bundlesReady || navigationId !== currentNavigationId) return;
        }
        if (navigationId !== currentNavigationId) return;
        await syncMissingWorkspaceModulesOnly(doc, navigationId);
        if (navigationId !== currentNavigationId) return;
        if (nextUrl.pathname.startsWith("/apps/manage/")) {
          if (typeof boot.installManageTabs === "function") {
            boot.installManageTabs();
          }
          if (typeof boot.mountSourceTreeControls === "function") {
            boot.mountSourceTreeControls();
          }
          syncManageTabFromUrl(url);
          pulseManagePreview(extractManagePanelContext(document.querySelector("#meilang-author-panel")));
        }
        installDrilldownOverlayHost();
        applyDrilldownContextFromQuery();
      } catch (err) {
        console.warn("[spa-navigation] post-spa work failed", err);
      }
    })();
  }

  function preserveManageWorkspaceFromUrls(currentUrl, nextUrl) {
    return shouldPreserveManageWorkspace(currentUrl, nextUrl);
  }

  async function loadAndSwap(url, replaceHistory, navigationId) {
    const fetchController = new AbortController();
    const fetchTimer = setTimeout(() => fetchController.abort(), SPA_FETCH_TIMEOUT_MS);
    let response;
    try {
      response = await fetch(url, {
        credentials: "same-origin",
        headers: { "x-mei-spa-nav": "1" },
        signal: fetchController.signal,
      });
    } finally {
      clearTimeout(fetchTimer);
    }
    if (!response.ok) throw new Error("navigation failed: " + response.status);
    const html = await response.text();
    if (navigationId !== currentNavigationId) return false;
    const doc = new DOMParser().parseFromString(html, "text/html");
    const nextShell = doc.querySelector(".shell");
    const currentShell = document.querySelector(".shell");
    if (!nextShell || !currentShell) {
      const err = new Error("spa shell missing in response");
      err.meiSpaHardNav = true;
      throw err;
    }
    const currentUrl = new URL(window.location.href);
    const nextUrl = new URL(url, window.location.href);
    const preserveManageWorkspace = shouldPreserveManageWorkspace(currentUrl, nextUrl);
    disposeRuntimeHooks({
      preserveAgentPanel: preserveManageWorkspace,
      preserveStatusBar: preserveManageWorkspace,
      preserveManageTabs: preserveManageWorkspace,
      preserveWorkspaceSplitters: preserveManageWorkspace,
      preserveFrameStage: preserveManageWorkspace,
      preserveSourceTreeControls: preserveManageWorkspace,
      preserveSourceHighlight: preserveManageWorkspace,
    });
    if (preserveManageWorkspace && typeof window.__meiClearRuntimePerfDiagnostics === "function") {
      try {
        window.__meiClearRuntimePerfDiagnostics("SPA 换文件");
      } catch (_) {}
    }
    document.title = doc.title || document.title;
    if (document.body.className !== doc.body.className) {
      document.body.className = doc.body.className;
    }
    if (preserveManageWorkspace) {
      const swapped = swapManageWorkspace(doc, url, replaceHistory);
      if (!swapped) {
        replaceShellFromDoc(doc, url, replaceHistory);
      }
    } else {
      replaceShellFromDoc(doc, url, replaceHistory);
    }
    if (navigationId !== currentNavigationId) return false;
    publishManagePreviewFromDoc(doc);
    runPostSpaWork(doc, url, navigationId, currentUrl, nextUrl);
    return true;
  }

  async function navigateInternal(url, replaceHistory) {
    currentNavigationId += 1;
    const navigationId = currentNavigationId;
    spaNavigationInFlight += 1;
    boot._spaInFlight = spaNavigationInFlight;
    closeDrilldownOverlay();
    let currentUrl = null;
    let nextUrl = null;
    try {
      currentUrl = new URL(window.location.href);
      nextUrl = new URL(url, window.location.href);
    } catch (_) {}
    const manageSamePath =
      currentUrl && nextUrl && isManageSamePathNavigation(currentUrl, nextUrl);
    if (manageSamePath) {
      showManageWorkspaceLoadingState(url);
    } else {
      showManageWorkspaceLoadingState(url);
      showLoading();
    }
    try {
      const completed = await loadAndSwap(url, replaceHistory, navigationId);
      if (!completed && navigationId === currentNavigationId) {
        console.warn("[spa-navigation] navigation superseded", url);
      }
    } catch (error) {
      console.error("[spa-navigation] navigation failed", error);
      if (error && error.name === "AbortError") {
        console.warn("[spa-navigation] fetch timeout", url);
        return;
      }
      if (error && error.meiSpaHardNav) {
        window.location.assign(url);
        return;
      }
    } finally {
      spaNavigationInFlight = Math.max(0, spaNavigationInFlight - 1);
      boot._spaInFlight = spaNavigationInFlight;
      finishNavigationUi(navigationId);
    }
  }

  boot.navigateSpa = function (url, replaceHistory) {
    return navigateInternal(url, !!replaceHistory);
  };

  tagExistingBodyScripts();
  installDrilldownOverlayHost();
  applyDrilldownContextFromQuery();

  document.addEventListener(
    "click",
    (event) => {
      if (event.defaultPrevented) return;
      if (event.button !== 0) return;
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      if (shouldBypassSpaClick(event)) return;
      const target = resolveClickTarget(event);
      if (!target) return;
      if (target.download) return;
      if (target.target && target.target !== "_self") return;
      if (!shouldHandleUrl(target.url)) return;
      if (isSameLocation(target.url)) {
        event.preventDefault();
        return;
      }
      event.preventDefault();
      void navigateInternal(target.url, false);
    },
    true,
  );

  window.addEventListener("popstate", () => {
    closeDrilldownOverlay();
    if (shouldHandleUrl(window.location.href)) {
      void navigateInternal(window.location.href, true);
    }
  });
})();
