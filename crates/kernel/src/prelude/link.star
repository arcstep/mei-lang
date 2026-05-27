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

def _scene_ref_with_entry(scene = None, scene_file = None, scene_id = None, entry_tab = None, focus = None):
    tab = entry_tab if entry_tab != None else focus
    if scene != None and (scene_file != None or scene_id != None):
        fail("board_link: use either scene=scene_ref(...) or scene_file/scene_id, not both")
    if scene == None:
        if scene_file == None or str(scene_file).strip() == "":
            fail("board_link requires scene=scene_ref(...) or scene_file")
        return scene_ref(scene_file = str(scene_file).strip(), scene_id = scene_id, entry_tab = tab)
    if type(scene) != "dict" or scene.get("__ref") != "scene":
        fail("board_link scene must be scene_ref(...)")
    if tab == None or str(tab).strip() == "":
        return scene
    merged = dict(scene)
    merged["entry_tab"] = str(tab).strip()
    merged["entry"] = str(tab).strip()
    return merged

def board_link(scene = None, scene_file = None, scene_id = None, projection = "overlay", entry_tab = None, focus = None, title = None, slots = None):
    """Link a home entry to a named secondary scene; projection only controls overlay vs route."""
    scene_value = _scene_ref_with_entry(
        scene = scene,
        scene_file = scene_file,
        scene_id = scene_id,
        entry_tab = entry_tab,
        focus = focus,
    )
    tab = scene_value.get("entry_tab")
    return _without_empty({
        "__kind": "board_link",
        "mode": "board_link",
        "scene": scene_value,
        "scene_file": scene_value.get("scene_file"),
        "scene_id": scene_value.get("scene_id"),
        "projection": projection,
        "entry_tab": tab,
        "focus": tab,
        "title": title,
        "slots": slots,
    })

def link(scene = None, scene_file = None, scene_id = None, projection = "overlay", entry_tab = None, focus = None, title = None, slots = None):
    """Alias of board_link for metric-card / chart entry links."""
    return board_link(
        scene = scene,
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
            scene = scene_ref(
                scene_file = scene_file,
                scene_id = "metric_explain_board",
                entry_tab = focus,
            ),
            projection = projection,
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
