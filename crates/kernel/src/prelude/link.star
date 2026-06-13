# UI link / scene projection opening (not data-layer DSL).

def _link_is_dict(value):
    return type(value) == "dict"

def _scene_ref_with_entry(scene = None, scene_file = None, scene_id = None, entry = None, entry_tab = None, focus = None):
    resolved_entry = entry if entry != None else (entry_tab if entry_tab != None else focus)
    if scene != None and (scene_file != None or scene_id != None):
        fail("link: use either scene=scene_ref(...) or scene_file/scene_id, not both")
    if scene == None:
        if scene_file == None or str(scene_file).strip() == "":
            fail("link requires scene=scene_ref(...) or scene_file")
        return scene_ref(scene_file = str(scene_file).strip(), scene_id = scene_id, entry = resolved_entry)
    if type(scene) != "dict" or scene.get("__ref") != "scene":
        fail("link scene must be scene_ref(...)")
    if resolved_entry == None or str(resolved_entry).strip() == "":
        return scene
    merged = dict(scene)
    merged["entry"] = str(resolved_entry).strip()
    return merged

def board_link(scene = None, scene_file = None, scene_id = None, projection = "overlay", type = None, entry = None, entry_tab = None, focus = None, title = None, overlay_size = None, entry_overrides = None, bindings = None, slots = None, params = None, default_slot = None):
    """Compatibility alias for link(scene=..., params=..., projection=...)."""
    return link(
        scene = scene,
        scene_file = scene_file,
        scene_id = scene_id,
        projection = projection,
        type = type,
        entry = entry,
        entry_tab = entry_tab,
        focus = focus,
        title = title,
        overlay_size = overlay_size,
        entry_overrides = entry_overrides,
        bindings = bindings,
        slots = slots,
        params = params,
        default_slot = default_slot,
    )

def link(scene = None, scene_file = None, scene_id = None, projection = "overlay", type = None, entry = None, entry_tab = None, focus = None, title = None, overlay_size = None, entry_overrides = None, bindings = None, slots = None, params = None, default_slot = None):
    """Primary scene-first link entry: scene + params + projection."""
    scene_value = _scene_ref_with_entry(
        scene = scene,
        scene_file = scene_file,
        scene_id = scene_id,
        entry = entry,
        entry_tab = entry_tab,
        focus = focus,
    )
    resolved_entry = None
    if scene_value != None and _link_is_dict(scene_value):
        resolved_entry = scene_value.get("entry")
    if resolved_entry == None:
        resolved_entry = entry if entry != None else (entry_tab if entry_tab != None else focus)
    overrides = bindings if bindings != None else (entry_overrides if entry_overrides != None else slots)
    return _without_empty({
        "__kind": "board_link",
        "mode": "board_link",
        "type": type if type != None else "popup",
        "scene": scene_value,
        "projection": projection,
        "entry": resolved_entry,
        "entry_tab": resolved_entry,
        "focus": resolved_entry,
        "entry_overrides": overrides,
        "bindings": overrides,
        "slots": overrides,
        "params": params if params != None else {},
        "title": title,
        "overlay_size": overlay_size,
        "default_slot": default_slot,
    })

def popup_panel(template, focus = None, entry = None, slots = None, entry_overrides = None, bindings = None, title = None, projection = "overlay"):
    """Removed: use link(scene=scene_ref(...), params=..., projection=...)."""
    fail("popup_panel(...) removed; use link(scene=scene_ref(...), params=..., projection=...)")
