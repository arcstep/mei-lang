# Projection assembly layer: board assembly, drilldown slots, explicit view descriptors.

def _metric_ref_id(metric):
    if metric == None or type(metric) != "dict":
        fail("projection assembly requires metric_ref(...)")
    if metric.get("__ref") != "metric":
        fail("projection assembly metric must be metric_ref(...)")
    mid = metric.get("id")
    if mid == None or str(mid).strip() == "":
        fail("metric_ref requires non-empty id")
    return str(mid).strip()

def explain_ref(id):
    """Reference an explain block on the board context metric's explain scope."""
    if id == None or str(id).strip() == "":
        fail("explain_ref requires non-empty id")
    return {
        "__ref": "explain_block",
        "id": str(id).strip(),
    }

def build_view(kind, source, chart_kind = None, mapping = None, label = None, columns = None, fields = None, top_n = None, topN = None):
    """Explicit view descriptor: data source + how to render (not inferred from explain alone)."""
    if kind == None or str(kind).strip() == "":
        fail("build_view requires kind=chart|table|metric_card|summary")
    if source == None:
        fail("build_view requires source=explain_ref(...) or metric_ref(...)")
    resolved_kind = str(kind).strip()
    if resolved_kind == "chart" and (chart_kind == None or str(chart_kind).strip() == ""):
        fail("build_view(kind=chart) requires chart_kind")
    payload = {
        "__kind": "board_view",
        "kind": resolved_kind,
        "source": source,
    }
    if chart_kind != None and str(chart_kind).strip() != "":
        payload["chart_kind"] = str(chart_kind).strip()
    if mapping != None:
        payload["mapping"] = mapping
    if label != None and str(label).strip() != "":
        payload["label"] = str(label).strip()
    resolved_columns = columns if columns != None else fields
    if resolved_columns != None:
        payload["columns"] = resolved_columns
    resolved_top_n = top_n if top_n != None else topN
    if resolved_top_n != None:
        payload["top_n"] = resolved_top_n
    return _without_empty(payload)

def filter_field(key, label = None, column = None, control = "multi_select"):
    """Explicit analytics filter field (V1: rowset-backed options at runtime)."""
    if key == None or str(key).strip() == "":
        fail("filter_field requires key")
    resolved_column = column if column != None and str(column).strip() != "" else str(key).strip()
    resolved_label = label if label != None and str(label).strip() != "" else resolved_column
    resolved_control = control if control != None and str(control).strip() != "" else "multi_select"
    return _without_empty({
        "key": str(key).strip(),
        "label": str(resolved_label).strip(),
        "column": str(resolved_column).strip(),
        "control": str(resolved_control).strip(),
    })

def build_board_assembly(scene, context, charts = None, detail = None, filters = None, include_hero = False):
    """Build a board instance independent of link/route/popup.

    scene: target board shell (scene_ref).
    context: root metric_ref for explain-first lineage (V1).
    charts: ordered list of build_view(kind=chart, ...) descriptors (analytics: 1..3).
    detail: build_view(kind=table, ...) or omitted when shell allows default.
    filters: e.g. {"rowset_dataset_id": "warning_list", "fields": [filter_field(...), ...]}.
    """
    if scene == None or type(scene) != "dict" or scene.get("__ref") != "scene":
        fail("build_board_assembly requires scene=scene_ref(...)")
    _metric_ref_id(context)
    payload = {
        "__kind": "board_assembly",
        "scene": scene,
        "context": context,
        "include_hero": include_hero,
    }
    if charts != None:
        payload["charts"] = charts
    if detail != None:
        payload["detail"] = detail
    if filters != None:
        payload["filters"] = filters
    return _without_empty(payload)

def slot(metric, as_component = None, label = None):
    """Explicit projection slot; compiler lowers to projection_slots entry."""
    if metric == None:
        fail("slot requires metric=metric_ref(...)")
    return _without_empty({
        "__kind": "projection_slot",
        "metric": metric,
        "as": as_component,
        "label": label,
    })

def build_from_metric(metric, as_component = None, label = None):
    """Single slot from a metric (scalar card or scoped dataframe by shape)."""
    _metric_ref_id(metric)
    return slot(metric = metric, as_component = as_component, label = label)

def build_from_explain(metric):
    """Slot list from metric explain scope; lowered at compile time."""
    return _without_empty({
        "__kind": "projection_slot_list",
        "source": metric,
        "include_hero": False,
    })

def build_drilldown_tabs(metric, include_hero = True, default_slot = None):
    """Hero + explain slots; default_tab index is 0-based. Legacy generic drilldown."""
    payload = {
        "__kind": "projection_slot_list",
        "source": metric,
        "include_hero": include_hero,
    }
    if default_slot != None:
        payload["default_slot"] = default_slot
    return _without_empty(payload)

def build_analytics_drilldown(metric, charts, detail = None, include_hero = False, rowset_dataset_id = None):
    """Legacy analytics assembly via link.tabs. Prefer build_board_assembly + link(board=...)."""
    if charts == None or type(charts) != "list":
        fail("build_analytics_drilldown requires charts=[...]")
    payload = {
        "__kind": "analytics_projection_slot_list",
        "source": metric,
        "charts": charts,
        "include_hero": include_hero,
    }
    if detail != None:
        payload["detail"] = detail
    if rowset_dataset_id != None and str(rowset_dataset_id).strip() != "":
        payload["rowset_dataset_id"] = str(rowset_dataset_id).strip()
    return _without_empty(payload)

def build_analytics_drilldown_tabs(metric, include_hero = False, rowset_dataset_id = None):
    """Legacy sugar: infer all explain blocks into analytics layout."""
    payload = {
        "__kind": "analytics_projection_slot_list",
        "source": metric,
        "include_hero": include_hero,
    }
    if rowset_dataset_id != None and str(rowset_dataset_id).strip() != "":
        payload["rowset_dataset_id"] = str(rowset_dataset_id).strip()
    return _without_empty(payload)

def warning_list_filter_fields():
    """Shared filter_field list for alert_tracking / warning_list analytics boards."""
    return [
        filter_field(key = "supervisionDomain", label = "监督领域", column = "监督领域", control = "multi_select"),
        filter_field(key = "supervisionCategory", label = "监督类别", column = "监督类别", control = "multi_select"),
        filter_field(key = "warningId", label = "预警ID", column = "预警ID", control = "text"),
        filter_field(key = "agency", label = "主责单位", column = "主责单位", control = "multi_select"),
        filter_field(key = "category", label = "问题分类名称", column = "问题分类名称", control = "multi_select"),
        filter_field(key = "warningType", label = "预警类型", column = "预警类型", control = "multi_select"),
        filter_field(key = "warningLevel", label = "预警等级", column = "预警等级", control = "multi_select"),
        filter_field(key = "warningTime", label = "预警时间", column = "预警时间", control = "month_multi_select"),
        filter_field(key = "trackingId", label = "问题跟踪ID", column = "问题跟踪ID", control = "text"),
        filter_field(key = "handlingDept", label = "承办部门", column = "承办部门", control = "multi_select"),
        filter_field(key = "assignTime", label = "分办时间", column = "分办时间", control = "month_multi_select"),
        filter_field(key = "closeTime", label = "办结时间", column = "办结时间", control = "month_multi_select"),
        filter_field(key = "verified", label = "是否查实", column = "是否查实", control = "multi_select"),
        filter_field(key = "transferredToClue", label = "是否转问题线索", column = "是否转问题线索", control = "multi_select"),
    ]
