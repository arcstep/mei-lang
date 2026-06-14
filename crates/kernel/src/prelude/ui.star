def _is_dict(value):
    return type(value) == "dict"

def _is_list(value):
    return type(value) == "list"

def _data_ref_value(data):
    if data == None:
        return None
    if _is_dict(data):
        if data.get("__ref") == "data":
            return data.get("id")
        return data.get("ref")
    return None

def _metric_data_items(data):
    items = []
    if data == None:
        return items
    if _is_dict(data):
        if data.get("metric") != None:
            items.append(data)
        if data.get("__ref") == "metric":
            items.append({
                "metric": data.get("id"),
                "from": data.get("from_dataset"),
            })
        return items
    if _is_list(data):
        for item in data:
            if _is_dict(item) and item.get("metric") != None:
                items.append(item)
            if _is_dict(item) and item.get("__ref") == "metric":
                items.append({
                    "metric": item.get("id"),
                    "from": item.get("from_dataset"),
                })
    return items

def _with_metric_data_props(data, props):
    metric_items = _metric_data_items(data)
    if len(metric_items) == 0:
        return props if props != None else {}
    out = {}
    if props != None:
        for k, v in props.items():
            out[k] = v
    if len(metric_items) == 1 and out.get("value") == None:
        out["value"] = {"metric": metric_items[0].get("metric"), "from_dataset": metric_items[0].get("from")}
        return out
    if out.get("metrics") == None:
        out["metrics"] = [item.get("metric") for item in metric_items]
    return out

def _metric_data_ref(data):
    if data == None:
        return None
    if _is_dict(data) and data.get("__ref") == "data":
        return data.get("id")
    if _is_dict(data) and data.get("__kind") == "analysis_expr" and data.get("type") == "rows":
        return data.get("dataset")
    metric_items = _metric_data_items(data)
    if len(metric_items) == 0:
        return None
    from_set = {}
    for item in metric_items:
        from_dataset = item.get("from")
        if from_dataset != None:
            from_set[from_dataset] = True
    keys = from_set.keys()
    if len(keys) == 1:
        return keys[0]
    return None

def _node(kind, id = None, title = None, area = None, layout = None, blocks = [], data = None, props = None, component = None, placement = None, interactions = [], lifecycle = None, constraints = None, data_plan = None):
    node_id = id if id != None else area
    return _without_empty({
        "id": node_id,
        "title": title,
        "block_kind": kind,
        "area": area if area != None else node_id,
        "layout": layout,
        "blocks": blocks,
        "data_ref": _data_ref_value(data),
        "data": data_plan,
        "props": props,
        "component": component,
        "placement": placement,
        "interactions": interactions,
        "lifecycle": lifecycle,
        "constraints": constraints,
    })

def grid(rows = None, cols = None, columns = None, areas = None, gap = None, padding = None, align = None, justify = None, cells = None):
    if rows != None and cols != None and columns == None and areas == None:
        return _clean({
            "rows": rows,
            "cols": cols,
            "cells": cells if cells != None else [],
        })
    return _without_empty({
        "type": "grid",
        "rows": rows,
        "cols": cols,
        "columns": columns,
        "areas": areas,
        "gap": gap,
        "padding": padding,
        "align": align,
        "justify": justify,
        "cells": cells,
    })

def flex(direction, wrap = None, gap = None, padding = None, align = None, justify = None):
    return _without_empty({
        "type": "flex",
        "direction": direction,
        "wrap": wrap,
        "gap": gap,
        "padding": padding,
        "align": align,
        "justify": justify,
    })

def _frame_node(id = None, title = None, layout = None, blocks = None, profile = None, props = None, panels = None, base = None):
    payload = {
        "kind": "frame",
        "id": id,
        "title": title,
        "layout": layout,
        "profile": profile,
        "props": props if props != None else {},
    }
    if base != None:
        payload["base"] = base
    if blocks != None:
        payload["blocks"] = blocks
    if panels != None:
        payload["panels"] = panels
    return _clean(payload)

def frame(id = None, title = None, layout = None, blocks = None, profile = None, props = None, panels = None, base = None):
    return _declare(_frame_node(
        id = id,
        title = title,
        layout = layout,
        blocks = blocks,
        profile = profile,
        props = props,
        panels = panels,
        base = base,
    ))

def frame_export(id, title = None, layout = None, blocks = None, profile = None, props = None, panels = None, base = None):
    export_id = str(id).strip() if id != None else ""
    if export_id == "":
        fail("frame_export(...) requires `id`")
    return _declare({
        "kind": "frame_export",
        "id": export_id,
        "frame": _frame_node(
            id = export_id,
            title = title,
            layout = layout,
            blocks = blocks,
            profile = profile,
            props = props,
            panels = panels,
            base = base,
        ),
    })

def frame_set_layout(layout):
    return _declare({
        "kind": "frame_set_layout",
        "layout": layout,
    })

def _merge_dict(base, overlay):
    out = _clone_props(base)
    if _is_dict(overlay):
        for k, v in overlay.items():
            out[k] = v
    return out

def theme(id, frame = None, panel = None, panel_bare = None, panel_head = None, panel_body = None, heading = None, font = None, metric_label = None, metric_value = None, metric_unit = None, metric_desc = None, metric_sub_label = None, metric_sub_value = None, metric_sub_unit = None, tokens = None, shared = None, components = None):
    resolved_panel_head = _merge_dict(panel_head, heading)
    return _declare(_clean({
        "kind": "theme",
        "id": id,
        "frame": frame if frame != None else {},
        "panel": panel if panel != None else {},
        "panel_bare": panel_bare if panel_bare != None else {},
        "panel_head": resolved_panel_head,
        "panel_body": panel_body if panel_body != None else {},
        "heading": heading if heading != None else {},
        "font": font if font != None else {},
        "metric_label": metric_label if metric_label != None else {},
        "metric_value": metric_value if metric_value != None else {},
        "metric_unit": metric_unit if metric_unit != None else {},
        "metric_desc": metric_desc if metric_desc != None else {},
        "metric_sub_label": metric_sub_label if metric_sub_label != None else {},
        "metric_sub_value": metric_sub_value if metric_sub_value != None else {},
        "metric_sub_unit": metric_sub_unit if metric_sub_unit != None else {},
        "tokens": tokens if tokens != None else {},
        "shared": shared if shared != None else {},
        "components": components if components != None else {},
    }))

def _clone_props(value):
    out = {}
    if _is_dict(value):
        for k, v in value.items():
            out[k] = v
    return out

def _title_is_block_shape(value):
    if not _is_dict(value):
        return False
    if value.get("kind") == "block":
        return True
    if value.get("use_key") != None:
        return True
    if value.get("use") != None:
        return True
    return False

def _merge_head_props(head_props = None, heading = None, heading_variant = None):
    merged = _clone_props(head_props)
    if _is_dict(heading):
        merged = _merge_dict(merged, heading)
    if heading_variant != None:
        merged["variant"] = heading_variant
    return merged

def panel_slot(kind = None, role = None, accepts = None, required = None, max = None, source = None, active = None, selection_from = None):
    resolved_kind = kind if kind != None else role
    return _without_empty({
        "__kind": "panel_slot",
        "kind": resolved_kind,
        "accepts": accepts,
        "required": required,
        "max": max,
        "source": source,
        "active": active,
        "selection_from": selection_from,
    })

def _panel_node(id = None, title = None, subtitle = None, area = None, layout = None, blocks = None, data = None, props = None, slot = None, head_props = None, body_props = None, data_plan = None, variant = None, chrome = None, show_heading = None, heading = None, heading_variant = None, title_background = None, title_decor = None, title_height = None, title_align = None, layout_policy = None, layout_gap = None, layout_padding = None, layout_columns = None, base = None, scale = None):
    if base != None:
        panel_id = id if id != None and str(id).strip() != "" else ""
    else:
        panel_id = id if id != None else area
    panel_props = _clone_props(props)
    panel_head_props = _merge_head_props(head_props, heading, heading_variant)
    panel_body_props = _clone_props(body_props)
    title_label = None
    head_slot = None
    if title != None:
        if type(title) == "string":
            title_label = title
        elif _title_is_block_shape(title):
            head_slot = title
        else:
            title_label = str(title)
    if subtitle != None:
        panel_props["subtitle"] = subtitle
    if chrome != None:
        panel_props["chrome"] = chrome
    if show_heading != None:
        panel_props["show_heading"] = show_heading
    if layout_policy != None and str(layout_policy).strip() != "":
        panel_props["__mei_layout_policy"] = str(layout_policy).strip()
    if layout_gap != None and str(layout_gap).strip() != "":
        panel_props["__mei_layout_gap"] = str(layout_gap).strip()
    if layout_padding != None and str(layout_padding).strip() != "":
        panel_props["__mei_layout_padding"] = str(layout_padding).strip()
    if scale != None and str(scale).strip() != "":
        panel_props["scale"] = scale
    if type(layout_columns) == "list" and len(layout_columns) > 0:
        panel_props["__mei_layout_columns"] = layout_columns
    if title_background != None:
        panel_head_props["background"] = title_background
    if title_decor != None:
        panel_head_props["carets"] = title_decor
    if title_height != None:
        panel_head_props["height"] = title_height
    if title_align != None:
        panel_head_props["align"] = title_align
    # slot is a first-class panel field; do not mirror into props.projection_*.
    variant_key = variant
    if type(variant_key) == "string":
        variant_norm = variant_key.strip().lower()
        if variant_norm == "container" or variant_norm == "bare":
            if panel_props.get("chrome") == None:
                panel_props["chrome"] = "bare"
    payload = {
        "kind": "panel",
        "id": panel_id,
        "title": title_label,
        "head": head_slot,
        "area": area,
        "layout": layout,
        "data_ref": _data_ref_value(data),
        "props": panel_props,
        "slot": slot if slot != None else {},
        "head_props": panel_head_props,
        "body_props": panel_body_props,
        "data": data_plan,
    }
    if base != None:
        payload["base"] = base
    if blocks != None:
        payload["blocks"] = blocks
    return _clean(payload)

def panel(id = None, title = None, subtitle = None, area = None, layout = None, blocks = None, data = None, props = None, slot = None, head_props = None, body_props = None, data_plan = None, variant = None, chrome = None, show_heading = None, heading = None, heading_variant = None, title_background = None, title_decor = None, title_height = None, title_align = None, layout_policy = None, layout_gap = None, layout_padding = None, layout_columns = None, base = None, scale = None):
    return _panel_node(
        id = id,
        title = title,
        subtitle = subtitle,
        area = area,
        layout = layout,
        blocks = blocks,
        data = data,
        props = props,
        slot = slot,
        head_props = head_props,
        body_props = body_props,
        data_plan = data_plan,
        variant = variant,
        chrome = chrome,
        show_heading = show_heading,
        heading = heading,
        heading_variant = heading_variant,
        title_background = title_background,
        title_decor = title_decor,
        title_height = title_height,
        title_align = title_align,
        layout_policy = layout_policy,
        layout_gap = layout_gap,
        layout_padding = layout_padding,
        layout_columns = layout_columns,
        base = base,
        scale = scale,
    )

def panel_decl(id = None, title = None, subtitle = None, area = None, layout = None, blocks = None, data = None, props = None, slot = None, head_props = None, body_props = None, data_plan = None, variant = None, chrome = None, show_heading = None, heading = None, heading_variant = None, title_background = None, title_decor = None, title_height = None, title_align = None, layout_policy = None, layout_gap = None, layout_padding = None, layout_columns = None, base = None, scale = None):
    return _declare(_panel_node(
        id = id,
        title = title,
        subtitle = subtitle,
        area = area,
        layout = layout,
        blocks = blocks,
        data = data,
        props = props,
        slot = slot,
        head_props = head_props,
        body_props = body_props,
        data_plan = data_plan,
        variant = variant,
        chrome = chrome,
        show_heading = show_heading,
        heading = heading,
        heading_variant = heading_variant,
        title_background = title_background,
        title_decor = title_decor,
        title_height = title_height,
        title_align = title_align,
        layout_policy = layout_policy,
        layout_gap = layout_gap,
        layout_padding = layout_padding,
        layout_columns = layout_columns,
        base = base,
        scale = scale,
    ))

def panel_export(id, title = None, subtitle = None, area = None, layout = None, blocks = None, data = None, props = None, slot = None, head_props = None, body_props = None, data_plan = None, variant = None, chrome = None, show_heading = None, heading = None, heading_variant = None, title_background = None, title_decor = None, title_height = None, title_align = None, layout_policy = None, layout_gap = None, layout_padding = None, layout_columns = None, base = None, scale = None):
    export_id = str(id).strip() if id != None else ""
    if export_id == "":
        fail("panel_export(...) requires `id`")
    return _declare({
        "kind": "panel_export",
        "id": export_id,
        "panel": _panel_node(
            id = export_id,
            title = title,
            subtitle = subtitle,
            area = area,
            layout = layout,
            blocks = blocks,
            data = data,
            props = props,
            slot = slot,
            head_props = head_props,
            body_props = body_props,
            data_plan = data_plan,
            variant = variant,
            chrome = chrome,
            show_heading = show_heading,
            heading = heading,
            heading_variant = heading_variant,
            title_background = title_background,
            title_decor = title_decor,
            title_height = title_height,
            title_align = title_align,
            layout_policy = layout_policy,
            layout_gap = layout_gap,
            layout_padding = layout_padding,
            layout_columns = layout_columns,
            base = base,
            scale = scale,
        ),
    })

def box(id = None, title = None, area = None, layout = None, blocks = [], data = None, props = None, data_plan = None, variant = None, chrome = None, show_heading = None, heading = None, heading_variant = None, title_background = None, title_decor = None, title_height = None, title_align = None, layout_policy = None, layout_gap = None, layout_padding = None, layout_columns = None, scale = None):
    return panel(
        id = id,
        title = title,
        area = area,
        layout = layout,
        blocks = blocks,
        data = data,
        props = props,
        data_plan = data_plan,
        variant = variant,
        chrome = chrome,
        show_heading = show_heading,
        heading = heading,
        heading_variant = heading_variant,
        title_background = title_background,
        title_decor = title_decor,
        title_height = title_height,
        title_align = title_align,
        layout_policy = layout_policy,
        layout_gap = layout_gap,
        layout_padding = layout_padding,
        layout_columns = layout_columns,
        scale = scale,
    )

def box_decl(id = None, title = None, area = None, layout = None, blocks = [], data = None, props = None, data_plan = None, variant = None, chrome = None, show_heading = None, heading = None, heading_variant = None, title_background = None, title_decor = None, title_height = None, title_align = None, layout_policy = None, layout_gap = None, layout_padding = None, layout_columns = None, scale = None):
    return panel_decl(
        id = id,
        title = title,
        area = area,
        layout = layout,
        blocks = blocks,
        data = data,
        props = props,
        data_plan = data_plan,
        variant = variant,
        chrome = chrome,
        show_heading = show_heading,
        heading = heading,
        heading_variant = heading_variant,
        title_background = title_background,
        title_decor = title_decor,
        title_height = title_height,
        title_align = title_align,
        layout_policy = layout_policy,
        layout_gap = layout_gap,
        layout_padding = layout_padding,
        layout_columns = layout_columns,
        scale = scale,
    )

def component(use = None, id = None, title = None, area = None, pack = "cockpit-default", data = None, props = None, mapping = None, layout = None, blocks = None, interactions = [], placement = None, lifecycle = None, constraints = None, data_plan = None, base = None):
    if use == None and base == None:
        fail("component(...) requires `use` or `base=component_ref(...)`")
    resolved_props = _with_metric_data_props(data, props)
    if data != None and resolved_props.get("data") == None and _is_dict(data):
        if data.get("__ref") == "world":
            resolved_props["data"] = data
        if data.get("__ref") == "data":
            resolved_props["data"] = data
        if data.get("__kind") == "analysis_expr" and data.get("type") == "rows":
            resolved_props["data"] = data
    if mapping != None:
        resolved_props["mapping"] = mapping
    component_ref = _without_empty({
        "use": use,
        "pack": pack,
        "data_ref": _metric_data_ref(data),
        "props": resolved_props,
        "mapping": mapping,
    })
    payload = {
        "kind": "block",
        "use_key": use,
        "id": id,
        "title": title,
        "area": area,
        "layout": layout,
        "props": resolved_props,
        "component": component_ref,
        "placement": placement,
        "interactions": interactions,
        "lifecycle": lifecycle,
        "constraints": constraints,
        "data": data_plan,
    }
    if blocks != None:
        payload["blocks"] = blocks
    if base != None:
        payload["base"] = base
    return _clean(payload)

def component_export(id, use = None, title = None, area = None, pack = "cockpit-default", data = None, props = None, mapping = None, layout = None, blocks = None, interactions = [], placement = None, lifecycle = None, constraints = None, data_plan = None, base = None):
    export_id = str(id).strip() if id != None else ""
    if export_id == "":
        fail("component_export(...) requires `id`")
    return _declare({
        "kind": "component_export",
        "id": export_id,
        "block": component(
            use = use,
            id = export_id,
            title = title,
            area = area,
            pack = pack,
            data = data,
            props = props,
            mapping = mapping,
            layout = layout,
            blocks = blocks,
            interactions = interactions,
            placement = placement,
            lifecycle = lifecycle,
            constraints = constraints,
            data_plan = data_plan,
            base = base,
        ),
    })

def component_ref(use = None, id = None, pack = "cockpit-default", scene_file = None, scene_id = None, data = None, props = None, mapping = None):
    _ = (data, props, mapping, pack)
    return _without_empty({
        "__ref": "component",
        "id": id,
        "use": use,
        "scene_id": scene_id,
        "scene_file": scene_file,
    })

def metric_block(id, title, component, metrics):
    return _without_empty({
        "id": id,
        "title": title,
        "block_kind": "metric_block",
        "component": component,
        "metrics": metrics,
    })

def chart_block(id, title, component, series):
    return _without_empty({
        "id": id,
        "title": title,
        "block_kind": "chart_block",
        "component": component,
        "series": series,
    })

def placement(**kwargs):
    return kwargs

def lifecycle(if_parent_missing = "render_standalone", if_data_missing = "placeholder", if_reference_missing = "placeholder"):
    return {
        "if_parent_missing": if_parent_missing,
        "if_data_missing": if_data_missing,
        "if_reference_missing": if_reference_missing,
    }

def target_frame(key):
    return {"kind": "frame", "key": key}

def interaction(event, intent, target, pass_ = None):
    return _without_empty({
        "event": event,
        "intent": intent,
        "target": target,
        "pass": pass_,
    })
