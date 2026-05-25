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
  const METRIC_DRILLDOWN_EVENT = "mei:metric-drilldown";
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
    enforcement_items_count: {
      sceneId: "enforcement_matters",
      title: "执法事项明细",
      columns: ENFORCEMENT_MATTER_COLUMNS,
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
      title: "园区明细",
      columns: PARK_COLUMNS,
      headers: PARK_HEADERS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    whitelist_enterprises_count: {
      sceneId: "enterprise_whitelist",
      title: "白名单企业明细",
      columns: WHITELIST_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    inspections_total_count: {
      sceneId: "administrative_inspection",
      title: "行政检查总数明细",
      columns: INSPECTION_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    inspections_today_count: {
      sceneId: "administrative_inspection",
      title: "今日行政检查明细",
      note: "统计口径：检查日期落在最近 1 天。",
      tableMetricId: "inspections_today_detail_table",
      columns: INSPECTION_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    inspections_week_count: {
      sceneId: "administrative_inspection",
      title: "近7日行政检查明细",
      note: "统计口径：检查日期落在最近 7 天。",
      tableMetricId: "inspections_week_detail_table",
      columns: INSPECTION_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    enterprise_complaints_count: {
      sceneId: "enterprise_complaints",
      title: "涉企投诉明细",
      columns: COMPLAINT_COLUMNS,
      headers: COMPLAINT_HEADERS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    inspections_no_violation_count: {
      sceneId: "administrative_inspection",
      title: "无违规检查明细",
      note: "统计口径：筛选检查结果为“无违规项”的记录。",
      tableMetricId: "inspections_no_violation_detail_table",
      columns: INSPECTION_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    ai_recognition_warnings_count: {
      sceneId: "ai_recognition_warnings",
      title: "AI识别记录明细",
      columns: AI_WARNING_COLUMNS,
      headers: AI_WARNING_HEADERS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    records_devices_count: {
      sceneId: "body_cameras",
      title: "执法记录仪明细",
      columns: BODY_CAMERA_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    records_devices_playback_hours_total: {
      sceneId: "body_cameras",
      title: "可回放时长计算依据",
      note: "统计口径：汇总“可回放时长”字段。",
      columns: BODY_CAMERA_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    penalties_total_count: {
      sceneId: "penalty_dashboard",
      title: "行政处罚总数明细",
      columns: PENALTY_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    penalties_today_count: {
      sceneId: "penalty_dashboard",
      title: "今日行政处罚明细",
      note: "统计口径：做出处罚日期落在最近 1 天。",
      tableMetricId: "penalties_today_detail_table",
      columns: PENALTY_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    penalties_week_count: {
      sceneId: "penalty_dashboard",
      title: "近7日行政处罚明细",
      note: "统计口径：做出处罚日期落在最近 7 天。",
      tableMetricId: "penalties_week_detail_table",
      columns: PENALTY_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    administrative_reconsiderations_count: {
      sceneId: "admin_reconsideration_register",
      title: "行政复议明细",
      columns: RECONSIDERATION_COLUMNS,
      ...DETAIL_TABLE_DEFAULTS,
    },
    inspection_frequency_reduction_rate: {
      sceneId: "administrative_inspection",
      title: "检查频次降低率口径表",
      note: "统计口径：按最近 6 个月月度检查次数分组，当前值基于最近月份相对上月变化。",
      tableMetricId: "inspections_6m_count_trend",
      columns: TREND_COLUMNS,
      headers: ["月份", "检查次数"],
      ...EXPLAIN_TABLE_DEFAULTS,
    },
    penalty_revenue_growth_rate: {
      sceneId: "penalty_dashboard",
      title: "罚没收入增长率口径表",
      note: "统计口径：按最近 6 个月月度罚没金额汇总，当前值基于该分组统计计算。",
      tableMetricId: "penalties_6m_amount_trend",
      columns: TREND_COLUMNS,
      headers: ["月份", "罚没收入"],
      ...EXPLAIN_TABLE_DEFAULTS,
    },
    warnings_verification_rate: {
      sceneId: "warning_list",
      title: "预警查实率口径表",
      note: "统计口径：按预警ID去重后，按“是否查实”分组统计；「是」「查实」「已查实」计为已查实。",
      tableMetricId: "warnings_verification_breakdown_table",
      columns: STATUS_COUNT_COLUMNS,
      headers: ["查实情况（空值=未核查；是/查实/已查实=已查实）", "预警数"],
      ...EXPLAIN_TABLE_DEFAULTS,
    },
    effectiveness_verified_rectification_rate: {
      sceneId: "issue_result_list",
      title: "查实预警整改率口径表",
      note: "统计口径：按问题跟踪ID去重并筛选已查实问题，再按“健全机制”分组统计。",
      tableMetricId: "effectiveness_verified_rectification_breakdown_table",
      columns: MECHANISM_COUNT_COLUMNS,
      headers: ["健全机制（空值=未整改）", "已查实问题数"],
      ...EXPLAIN_TABLE_DEFAULTS,
      columnMinWidth: 220,
    },
    supervision_items_count: {
      sceneId: "supervision_matters",
      title: "监督事项明细",
      columns: MATTERS_COLUMNS,
      layoutPreset: "drilldown_matters",
      ...EXPLAIN_TABLE_DEFAULTS,
    },
    supervision_models_count: {
      sceneId: "warning_models",
      title: "预警模型明细",
      columns: MODEL_COLUMNS,
      layoutPreset: "drilldown_models",
      ...EXPLAIN_TABLE_DEFAULTS,
      columnMinWidth: 190,
    },
    warnings_count: {
      sceneId: "warning_list",
      title: "预警清单",
      columns: WARNING_COLUMNS,
      layoutPreset: "drilldown_warnings",
      ...DETAIL_TABLE_DEFAULTS,
    },
    warnings_pending_count: {
      sceneId: "warning_list",
      title: "待办预警清单",
      tableMetricId: "warnings_pending_detail_table",
      note: "统计口径：预警ID 非空且承办部门为空。",
      columns: WARNING_PENDING_COLUMNS,
      layoutPreset: "drilldown_warnings",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_in_progress_count: {
      sceneId: "issue_result_list",
      title: "问题处理结果（在办）",
      tableMetricId: "issue_results_in_progress_table",
      note: "统计口径：按问题跟踪ID 去重，处理结果ID 为空。",
      columns: ISSUE_COLUMNS,
      layoutPreset: "drilldown_issues",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_completed_count: {
      sceneId: "issue_result_list",
      title: "问题处理结果（已办）",
      tableMetricId: "issue_results_completed_table",
      note: "统计口径：按问题跟踪ID 去重，处理结果ID 非空。",
      columns: ISSUE_COLUMNS,
      layoutPreset: "drilldown_issues",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_issue_verification_rate: {
      sceneId: "issue_result_list",
      title: "问题处理结果（查实率）",
      tableMetricId: "issue_results_handled_table",
      note: "统计口径：按问题跟踪ID 去重后，统计“是否查实=是”占比。",
      columns: ISSUE_COLUMNS,
      layoutPreset: "drilldown_issues",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_transfer_clue_count: {
      sceneId: "issue_result_list",
      title: "问题处理结果（转问题线索）",
      tableMetricId: "issue_results_transfer_clue_table",
      note: "统计口径：先筛“是否转问题线索=是”，再按问题跟踪ID 去重（避免首行覆盖后续“是”）。",
      columns: ISSUE_COLUMNS,
      layoutPreset: "drilldown_issues",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_filing_count: {
      sceneId: "issue_result_list",
      title: "问题处理结果（立案数）",
      tableMetricId: "issue_results_filing_table",
      note: "统计口径：按处理结果ID 去重后，筛选“是否立案=是”。",
      columns: ISSUE_COLUMNS,
      layoutPreset: "drilldown_issues",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_party_gov_sanction_count: {
      sceneId: "issue_result_list",
      title: "问题处理结果（党纪政务处分）",
      tableMetricId: "issue_results_sanction_table",
      note: "统计口径：先筛“处理处分”含第二/三/四种等关键词，再按处理结果ID 去重计数。",
      columns: ISSUE_COLUMNS,
      layoutPreset: "drilldown_issues",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_handled_person_times: {
      sceneId: "issue_result_list",
      title: "问题处理结果（处理人数）",
      tableMetricId: "issue_results_handled_table",
      note: "统计口径：按处理结果ID 去重后统计处理记录。",
      columns: ISSUE_COLUMNS,
      layoutPreset: "drilldown_issues",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_recovered_funds_total: {
      sceneId: "issue_result_list",
      title: "问题处理结果（挽回资金）",
      tableMetricId: "issue_results_handled_table",
      note: "统计口径：按处理结果ID 去重后汇总“挽回资金”。",
      columns: ISSUE_COLUMNS,
      layoutPreset: "drilldown_issues",
      ...DETAIL_TABLE_DEFAULTS,
    },
    effectiveness_mechanism_item_count: {
      sceneId: "issue_result_list",
      title: "问题处理结果（健全机制）",
      tableMetricId: "issue_results_handled_table",
      note: "统计口径：按“健全机制”字段拆分并去重计数。",
      columns: ISSUE_COLUMNS,
      layoutPreset: "drilldown_issues",
      ...DETAIL_TABLE_DEFAULTS,
    },
  };
  let currentNavigationId = 0;
  let activeController = null;
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

  function resolveDrilldownSceneId(detail) {
    const metricId = String(detail?.metric_id || "").trim();
    const fromMetric = DRILLDOWN_METRIC_CONTEXT[metricId]?.sceneId;
    if (fromMetric) return fromMetric;
    const runtimeScene = normalizeDrilldownScenePath(detail?.drilldown_scene);
    if (!runtimeScene) return "";
    return DRILLDOWN_SCENE_BY_FILE[runtimeScene] || "";
  }

  function resolveDrilldownConfig(detail) {
    const metricId = String(detail?.metric_id || "").trim();
    const mapped = DRILLDOWN_METRIC_CONTEXT[metricId] || {};
    return {
      sceneId: resolveDrilldownSceneId(detail),
      title:
        String(mapped.title || detail?.label || metricId || "指标明细").trim() ||
        "指标明细",
      note: String(mapped.note || "").trim(),
      tableMetricId: String(mapped.tableMetricId || "").trim(),
      columns: Array.isArray(mapped.columns) ? mapped.columns.slice() : [],
      headers: Array.isArray(mapped.headers) ? mapped.headers.slice() : [],
      layoutPreset: String(mapped.layoutPreset || "").trim(),
      pageSize: Number(mapped.pageSize) > 0 ? Math.floor(Number(mapped.pageSize)) : 8,
      cellPreviewMaxChars:
        Number(mapped.cellPreviewMaxChars) > 0
          ? Math.floor(Number(mapped.cellPreviewMaxChars))
          : 0,
      columnMinWidth:
        Number(mapped.columnMinWidth) > 0
          ? Math.floor(Number(mapped.columnMinWidth))
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

  function buildDrilldownTableProps(detail, config) {
    const sceneId = String(config?.sceneId || "").trim();
    if (!sceneId) return null;
    const appPath = resolveAccessAppPath();
    if (!appPath) return null;
    const datasetId =
      DRILLDOWN_DATASET_BY_SCENE[sceneId] || DRILLDOWN_SCENE_BY_FILE[sceneId] || sceneId;
    const metricId = String(config?.tableMetricId || "").trim();
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
      },
    };
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
    if (!config.sceneId) return;
    const root = ensureDrilldownOverlayRoot();
    const titleEl = root.querySelector('[data-drilldown-title="true"]');
    const noteEl = root.querySelector('[data-drilldown-note="true"]');
    if (titleEl) titleEl.textContent = config.title;
    if (noteEl) {
      noteEl.textContent = config.note;
      noteEl.toggleAttribute("hidden", !config.note);
    }
    setDrilldownOverlayStatus(root, "loading");
    if (!mountDrilldownTable(root, detail, config)) {
      setDrilldownOverlayStatus(root, "error");
      root.removeAttribute("hidden");
      root.classList.add("is-open");
      document.body.classList.add("access-drilldown-open");
      return;
    }
    setDrilldownOverlayStatus(root, "ready");
    root.removeAttribute("hidden");
    root.classList.add("is-open");
    document.body.classList.add("access-drilldown-open");
  }

  function installDrilldownOverlayHost() {
    if (window.self !== window.top) return;
    if (!isAccessRoute()) return;
    if (boot.metricDrilldownHostMounted) return;
    boot.metricDrilldownHostMounted = true;
    document.addEventListener(METRIC_DRILLDOWN_EVENT, (event) => {
      if (!isAccessRoute()) return;
      const detail = event?.detail || {};
      if (!resolveDrilldownSceneId(detail)) return;
      openDrilldownOverlay(detail);
    });
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
    if (!overlay || !overlay.classList.contains("is-visible")) return;
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
        return {
          url: item.getAttribute("href"),
          target: item.getAttribute("target") || "",
          download: item.hasAttribute("download"),
        };
      }
    }
    return null;
  }

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
      if (
        item instanceof HTMLElement &&
        item.matches &&
        item.matches(".sidebar.left a.tree-link[href]")
      ) {
        return true;
      }
      if (
        item instanceof HTMLElement &&
        item.closest &&
        item.closest("header.topbar-shell")
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

  function routePreserveKey(url) {
    try {
      const parsed = new URL(url, window.location.href);
      const file = String(parsed.searchParams.get("file") || "").trim();
      const scene = String(parsed.searchParams.get("scene") || "").trim();
      return `${file}::${scene}`;
    } catch (_) {
      return "::";
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
      script.onload = () => resolve();
      script.onerror = () => reject(new Error("failed to load script: " + rawSrc));
      document.body.appendChild(script);
    });
  }

  async function syncScriptsFromDocument(doc, navigationId, options) {
    const opts = options || {};
    const scripts = collectBodyScripts(doc);
    for (const src of scripts) {
      if (navigationId !== currentNavigationId) return;
      const path = normalizePath(src);
      if (!path) continue;
      if (path === SPA_NAV_SCRIPT) continue;
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
        await loadScript(src, { module: true, persistentKey: path });
        continue;
      }
      if (path.startsWith("/app-assets/")) {
        if (RELOAD_APP_SCRIPTS.has(path)) {
          const withBuster = path + "?spa=" + Date.now();
          await loadScript(withBuster, { reloadKey: path });
          continue;
        }
        await loadScript(src, { persistentKey: path });
        continue;
      }
      if (path.startsWith("/app-bundles/")) {
        if (RELOAD_BUNDLE_SCRIPTS.has(path)) {
          const withBuster = path + "?spa=" + Date.now();
          await loadScript(withBuster, { reloadKey: path });
          continue;
        }
        await loadScript(src, { persistentKey: path });
        continue;
      }
    }
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

  function shouldPreserveManageWorkspace(currentUrl, nextUrl) {
    return (
      currentUrl.pathname === nextUrl.pathname &&
      currentUrl.pathname.startsWith("/apps/manage/") &&
      routePreserveKey(currentUrl) === routePreserveKey(nextUrl)
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
    dispatchManageContextChange(nextPanelContext);
    window.dispatchEvent(new Event("meilang:preview-updated"));
    return true;
  }

  async function loadAndSwap(url, replaceHistory, navigationId, controller) {
    const response = await fetch(url, {
      credentials: "same-origin",
      headers: { "x-mei-spa-nav": "1" },
      signal: controller.signal,
    });
    if (!response.ok) throw new Error("navigation failed: " + response.status);
    const html = await response.text();
    if (navigationId !== currentNavigationId) return;
    const doc = new DOMParser().parseFromString(html, "text/html");
    const nextShell = doc.querySelector(".shell");
    const currentShell = document.querySelector(".shell");
    if (!nextShell || !currentShell) {
      window.location.assign(url);
      return;
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
        currentShell.className = nextShell.className;
        const nextNodes = Array.from(nextShell.childNodes).map((node) =>
          node.cloneNode(true),
        );
        currentShell.replaceChildren(...nextNodes);
        if (replaceHistory) {
          window.history.replaceState({}, "", url);
        } else {
          window.history.pushState({}, "", url);
        }
      }
    } else {
      currentShell.className = nextShell.className;
      const nextNodes = Array.from(nextShell.childNodes).map((node) =>
        node.cloneNode(true),
      );
      currentShell.replaceChildren(...nextNodes);
      if (replaceHistory) {
        window.history.replaceState({}, "", url);
      } else {
        window.history.pushState({}, "", url);
      }
    }
    await syncScriptsFromDocument(doc, navigationId, {
      preserveManageWorkspace,
      preserveAgentPanel: preserveManageWorkspace,
      preserveStatusBar: preserveManageWorkspace,
      preserveManageTabs: preserveManageWorkspace,
      preserveWorkspaceSplitters: preserveManageWorkspace,
      preserveSourceTreeControls: preserveManageWorkspace,
    });
    installDrilldownOverlayHost();
    applyDrilldownContextFromQuery();
  }

  async function navigate(url, replaceHistory) {
    currentNavigationId += 1;
    const navigationId = currentNavigationId;
    closeDrilldownOverlay();
    if (activeController) {
      try {
        activeController.abort();
      } catch (_) {}
    }
    activeController = new AbortController();
    showManageWorkspaceLoadingState(url);
    showLoading();
    try {
      await loadAndSwap(url, replaceHistory, navigationId, activeController);
    } catch (error) {
      if (error && error.name === "AbortError") return;
      console.error("[spa-navigation] fallback to hard reload", error);
      window.location.assign(url);
    } finally {
      clearManageWorkspaceLoadingState();
      if (navigationId === currentNavigationId) {
        hideLoading();
      }
    }
  }

  boot.navigateSpa = function (url, replaceHistory) {
    return navigate(url, !!replaceHistory);
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
      void navigate(target.url, false);
    },
    true,
  );

  window.addEventListener("popstate", () => {
    closeDrilldownOverlay();
    if (shouldHandleUrl(window.location.href)) {
      void navigate(window.location.href, true);
    }
  });
})();
