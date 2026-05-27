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

def _scene_ref_with_entry(scene = None, scene_file = None, scene_id = None, entry = None, entry_tab = None, focus = None):
    resolved_entry = entry if entry != None else (entry_tab if entry_tab != None else focus)
    if scene != None and (scene_file != None or scene_id != None):
        fail("board_link: use either scene=scene_ref(...) or scene_file/scene_id, not both")
    if scene == None:
        if scene_file == None or str(scene_file).strip() == "":
            fail("board_link requires scene=scene_ref(...) or scene_file")
        return scene_ref(scene_file = str(scene_file).strip(), scene_id = scene_id, entry = resolved_entry)
    if type(scene) != "dict" or scene.get("__ref") != "scene":
        fail("board_link scene must be scene_ref(...)")
    if resolved_entry == None or str(resolved_entry).strip() == "":
        return scene
    merged = dict(scene)
    merged["entry"] = str(resolved_entry).strip()
    return merged

def board_link(scene = None, scene_file = None, scene_id = None, projection = "overlay", type = None, entry = None, entry_tab = None, focus = None, title = None, entry_overrides = None, bindings = None, slots = None):
    """Link a home entry to a named secondary scene; projection only controls overlay vs route."""
    scene_value = _scene_ref_with_entry(
        scene = scene,
        scene_file = scene_file,
        scene_id = scene_id,
        entry = entry,
        entry_tab = entry_tab,
        focus = focus,
    )
    resolved_entry = scene_value.get("entry")
    overrides = bindings if bindings != None else (entry_overrides if entry_overrides != None else slots)
    return _without_empty({
        "__kind": "board_link",
        "mode": "board_link",
        "type": type if type != None else "popup",
        "scene": scene_value,
        "projection": projection,
        "entry": resolved_entry,
        # Legacy aliases kept for one migration cycle.
        "entry_tab": resolved_entry,
        "focus": resolved_entry,
        "entry_overrides": overrides,
        "bindings": overrides,
        "slots": overrides,
        "title": title,
    })

def link(scene = None, scene_file = None, scene_id = None, projection = "overlay", type = None, entry = None, entry_tab = None, focus = None, title = None, entry_overrides = None, bindings = None, slots = None):
    """Alias of board_link for metric-card / chart entry links."""
    return board_link(
        scene = scene,
        scene_file = scene_file,
        scene_id = scene_id,
        projection = projection,
        type = type,
        entry = entry,
        entry_tab = entry_tab,
        focus = focus,
        title = title,
        entry_overrides = entry_overrides,
        bindings = bindings,
        slots = slots,
    )

def popup_panel(template, focus = None, entry = None, slots = None, entry_overrides = None, bindings = None, title = None, projection = "overlay"):
    """Deprecated sugar: known templates lower to board_link; unknown templates stay inline overlay."""
    if template == None or str(template).strip() == "":
        fail("popup_panel requires template")
    resolved_template = str(template).strip()
    if resolved_template == "metric_default":
        resolved_template = "metric_board_default"
    scene_file = _BOARD_TEMPLATE_SCENE_FILES.get(resolved_template)
    resolved_entry = entry if entry != None else focus
    overrides = bindings if bindings != None else (entry_overrides if entry_overrides != None else slots)
    if scene_file != None:
        payload = board_link(
            scene = scene_ref(
                scene_file = scene_file,
                scene_id = "metric_explain_board",
                entry = resolved_entry,
            ),
            projection = projection,
            title = title,
            entry_overrides = overrides,
        )
        payload["legacy_template"] = resolved_template
        return payload
    return _without_empty({
        "__kind": "popup_panel",
        "mode": "popup_panel",
        "template": resolved_template,
        "entry": resolved_entry,
        "focus": resolved_entry,
        "entry_overrides": overrides,
        "slots": overrides,
        "title": title,
        "projection": projection,
    })
