# UI link / popup projection (not data-layer DSL).

_BOARD_TEMPLATE_SCENE_FILES = {
    "metric_board_default": "templates/cockpit/drilldown/metric-explain-board.mei",
}

def explain_metric_ref(id):
    """Reference an explain_metric id on the root metric's metric.explain.metrics[]."""
    if id == None or str(id).strip() == "":
        fail("explain_metric_ref requires non-empty id")
    return {
        "__ref": "explain_metric",
        "id": id,
    }

def board_link(scene_file, scene_id = None, projection = "overlay", entry_tab = None, focus = None, title = None, slots = None):
    """Link a home entry to a named secondary scene; projection only controls overlay vs route."""
    if scene_file == None or str(scene_file).strip() == "":
        fail("board_link requires scene_file")
    tab = entry_tab if entry_tab != None else focus
    return _without_empty({
        "__kind": "board_link",
        "mode": "board_link",
        "scene_file": str(scene_file).strip(),
        "scene_id": scene_id,
        "projection": projection,
        "entry_tab": tab,
        "focus": tab,
        "title": title,
        "slots": slots,
    })

def link(scene_file, scene_id = None, projection = "overlay", entry_tab = None, focus = None, title = None, slots = None):
    """Alias of board_link for metric-card / chart entry links."""
    return board_link(
        scene_file = scene_file,
        scene_id = scene_id,
        projection = projection,
        entry_tab = entry_tab,
        focus = focus,
        title = title,
        slots = slots,
    )

def popup_panel(template, focus = None, slots = None, title = None, projection = "overlay"):
    """Deprecated sugar: known templates lower to board_link; unknown templates stay inline overlay."""
    if template == None or str(template).strip() == "":
        fail("popup_panel requires template")
    resolved_template = str(template).strip()
    if resolved_template == "metric_default":
        resolved_template = "metric_board_default"
    scene_file = _BOARD_TEMPLATE_SCENE_FILES.get(resolved_template)
    if scene_file != None:
        payload = board_link(
            scene_file = scene_file,
            scene_id = "metric_explain_board",
            projection = projection,
            entry_tab = focus,
            title = title,
            slots = slots,
        )
        payload["legacy_template"] = resolved_template
        return payload
    return _without_empty({
        "__kind": "popup_panel",
        "mode": "popup_panel",
        "template": resolved_template,
        "focus": focus,
        "slots": slots,
        "title": title,
        "projection": projection,
    })
