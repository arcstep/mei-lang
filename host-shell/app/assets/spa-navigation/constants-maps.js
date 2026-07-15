  const DRILLDOWN_SCENE_BY_FILE = {
    "templates/cockpit/drilldown/metric-explain-board.mei": "metric_explain_board",
    "templates/cockpit/drilldown/generic-drilldown-board.mei": "generic_drilldown_board",
  };
  const BOARD_TEMPLATE_SCENE_FILES = {
    metric_board_default: "templates/cockpit/drilldown/metric-explain-board.mei",
  };
  const SCENE_LOCAL_NAV_BY_FILE = {
    "templates/cockpit/drilldown/metric-explain-board.mei": {
      sceneId: "metric_explain_board",
      kind: "metric_explain_board",
      defaultEntry: "definition",
      items: [
        { id: "hero", role: "hero", label: "概览" },
        { id: "definition", role: "explain", label: "口径" },
        { id: "composition", role: "explain", label: "构成" },
        { id: "trend", role: "explain", label: "趋势" },
        { id: "numerator_denominator", role: "explain", label: "分子分母" },
        { id: "detail", role: "table", label: "明细" },
      ],
    },
  };
  const SCENE_KIND_ORDER_FALLBACK = ["definition", "composition", "trend", "numerator_denominator", "detail"];
  const SCENE_PROJECTION_CONTEXT_KEY = "mei.scene_projection_context";

