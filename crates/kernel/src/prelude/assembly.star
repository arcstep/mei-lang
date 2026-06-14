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

def _shell_layout(columns = None, rows = None, areas = None, gap = None, padding = None):
    return _without_empty({
        "columns": columns,
        "rows": rows,
        "areas": areas,
        "gap": gap,
        "padding": padding,
    })

def _shell_zone(id, role, area = None, parent = None, accepts = None, source = None, required = None, max = None, layout = None, selection_source = None):
    if id == None or str(id).strip() == "":
        fail("shell zone requires id")
    if role == None or str(role).strip() == "":
        fail("shell zone requires role")
    return _without_empty({
        "id": str(id).strip(),
        "role": str(role).strip(),
        "area": area,
        "parent": parent,
        "accepts": accepts,
        "source": source,
        "required": required,
        "max": max,
        "layout": layout,
        "selection_source": selection_source,
    })

def _shell_contract(layout_mode, zones, overlay_size = None, layout = None):
    if layout_mode == None or str(layout_mode).strip() == "":
        fail("shell contract requires layout_mode")
    if zones == None or type(zones) != "list" or len(zones) == 0:
        fail("shell contract requires zones=[...]")
    return _without_empty({
        "__kind": "scene_shell_contract",
        "layout_mode": str(layout_mode).strip(),
        "overlay_size": overlay_size,
        "layout": layout,
        "zones": zones,
    })

_GENERIC_DRILLDOWN_SCENE_SHELL = _shell_contract(
    "generic_tabs",
    zones = [
        _shell_zone("tabs", "tab_bar"),
        _shell_zone("content", "tab_content"),
    ],
)

_ANALYTICS_DRILLDOWN_SCENE_SHELL = _shell_contract(
    "analytics",
    overlay_size = "large",
    layout = _shell_layout(
        columns = ["minmax(180px, 1fr)", "minmax(0, 5fr)"],
        rows = ["minmax(0, 1fr)"],
        areas = [["filter", "main"]],
        gap = "12px",
        padding = "12px",
    ),
    zones = [
        _shell_zone("filter", "filter", area = "filter", source = "filter_schema"),
        _shell_zone(
            "main",
            "container",
            area = "main",
            layout = _shell_layout(
                columns = ["1fr"],
                rows = ["auto", "minmax(0, 1fr)"],
                areas = [["chart"], ["detail"]],
                gap = "12px",
            ),
        ),
        _shell_zone("chart", "slots", area = "chart", parent = "main", accepts = ["chart"], max = 3),
        _shell_zone("detail", "slots", area = "detail", parent = "main", accepts = ["data_table"], required = True),
    ],
)

_LIST_PREVIEW_DRILLDOWN_SCENE_SHELL = _shell_contract(
    "list_preview",
    overlay_size = "large",
    layout = _shell_layout(
        columns = ["minmax(180px, 1fr)", "minmax(0, 2.2fr)", "minmax(220px, 1.1fr)"],
        rows = ["minmax(0, 1fr)"],
        areas = [["filter", "list", "preview"]],
        gap = "12px",
        padding = "12px",
    ),
    zones = [
        _shell_zone("filter", "filter", area = "filter", source = "filter_schema"),
        _shell_zone("list", "slots", area = "list", accepts = ["data_table"], required = True),
        _shell_zone("preview", "row_preview", area = "preview", accepts = ["summary"], selection_source = "list"),
    ],
)

def _builtin_scene_shell_contract(scene):
    if scene == None or type(scene) != "dict" or scene.get("__ref") != "scene":
        return None
    scene_id = str(scene.get("scene_id") or "").strip()
    scene_file = str(scene.get("scene_file") or "").strip()
    if scene_id == "analytics_drilldown_board" or "analytics-drilldown-board" in scene_file:
        return _ANALYTICS_DRILLDOWN_SCENE_SHELL
    if scene_id == "list_preview_drilldown_board" or "list-preview-drilldown-board" in scene_file:
        return _LIST_PREVIEW_DRILLDOWN_SCENE_SHELL
    if scene_id == "generic_drilldown_board" or "generic-drilldown-board" in scene_file:
        return _GENERIC_DRILLDOWN_SCENE_SHELL
    return None

def build_view(kind, source, chart_kind = None, mapping = None, label = None, columns = None, fields = None, top_n = None, topN = None, column_state = None, page_size = None, pageSize = None):
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
    if column_state != None:
        payload["column_state"] = column_state
    resolved_page_size = page_size if page_size != None else pageSize
    if resolved_page_size != None:
        payload["page_size"] = resolved_page_size
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

def build_board_assembly(scene, context, charts = None, detail = None, filters = None, include_hero = False, preview = None, shell_contract = None):
    """Build a board instance independent of link/route/popup.

    scene: target board shell (scene_ref).
    context: root metric_ref for explain-first lineage (V1).
    charts: ordered list of build_view(kind=chart, ...) descriptors (analytics: 1..3).
    detail: build_view(kind=table, ...) or omitted when shell allows default.
    preview: build_view(kind=summary, ...) for list_preview shell right pane (optional).
    shell_contract: internal shell override; omitted uses built-in scene shell registry for known drilldown scenes.
    filters: e.g. {"rowset_dataset_id": "warning_list", "fields": [filter_field(...), ...]}.
    """
    if scene == None or type(scene) != "dict" or scene.get("__ref") != "scene":
        fail("build_board_assembly requires scene=scene_ref(...)")
    _metric_ref_id(context)
    resolved_shell = shell_contract if shell_contract != None else _builtin_scene_shell_contract(scene)
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
    if preview != None:
        payload["preview"] = preview
    if resolved_shell != None:
        payload["shell_contract"] = resolved_shell
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
    """Slot list from metric explain scope; lowered at compile time via scene.bindings."""
    fail("build_from_explain(...) removed; declare scene.bindings and use link(scene=..., params=...)")

def build_drilldown_tabs(metric, include_hero = True, default_slot = None, rowset_dataset_id = None):
    fail("build_drilldown_tabs(...) removed; use scene.params + scene.bindings + link(scene=..., params=...)")

def generic_drilldown_link(scene, metric, include_hero = True, default_slot = None, rowset_dataset_id = None, title = None):
    """Scene-first generic drilldown popup."""
    if scene == None or type(scene) != "dict" or scene.get("__ref") != "scene":
        fail("generic_drilldown_link requires scene=scene_ref(...)")
    params = {"metric": metric}
    if rowset_dataset_id != None and str(rowset_dataset_id).strip() != "":
        params["rowset_dataset_id"] = str(rowset_dataset_id).strip()
    return link(
        type = "popup",
        projection = "overlay",
        scene = scene,
        title = title,
        params = params,
        default_slot = default_slot,
    )

def build_analytics_drilldown(metric, charts, detail = None, include_hero = False, rowset_dataset_id = None):
    fail("build_analytics_drilldown(...) removed; declare scene.bindings and use link(scene=..., params=...)")

def build_analytics_drilldown_tabs(metric, include_hero = False, rowset_dataset_id = None):
    fail("build_analytics_drilldown_tabs(...) removed; declare scene.bindings and use link(scene=..., params=...)")

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
