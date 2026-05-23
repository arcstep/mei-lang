# 指标卡 DSL：
# - 单卡：metric_card(template|layout, source, map?, patch?)
# - 宽卡/组合：panel(layout, blocks=[metric_card(...), ...])
# - 固定语义槽：label / value / unit / desc
# - source 直接接 metric_ref(...) 或静态对象

def _metric_text(content, area, role, font = None, align = None, line_height = None, variant = None):
    props = {"content": content, "metric_role": role}
    if font != None:
        props["font"] = font
    if align != None:
        props["align"] = align
    if line_height != None:
        props["line_height"] = line_height
    if variant != None and str(variant).strip() != "":
        props["metric_variant"] = str(variant).strip()
    return component(
        "mei.text",
        area = area,
        props = _without_empty(props),
    )

def label(content, area = None, font = None, align = None, line_height = None, variant = None):
    return _metric_text(content, area, "label", font, align, line_height, variant)

def value(content, area = None, font = None, align = None, line_height = None, variant = None):
    return _metric_text(content, area, "value", font, align, line_height, variant)

def unit(content, area = None, font = None, align = None, line_height = None, variant = None):
    return _metric_text(content, area, "unit", font, align, line_height, variant)

def desc(content, area = None, font = None, align = None, line_height = None, variant = None):
    return _metric_text(content, area, "desc", font, align, line_height, variant)

def _metric_density(height_px = None, template = None):
    if height_px == None:
        return "normal"
    h = height_px
    tpl = _metric_template_name(template)
    if h <= 84:
        return "compact"
    if tpl == "row" and h >= 120:
        return "roomy"
    if h >= 132:
        return "roomy"
    return "normal"

def _metric_default_gap(template, density):
    tpl = _metric_template_name(template)
    if tpl == "row":
        if density == "compact":
            return "3px"
        return "4px"
    if tpl == "column":
        if density == "compact":
            return "3px"
        return "4px"
    if tpl == "stack_desc":
        if density == "compact":
            return "2px 2px"
        if density == "roomy":
            return "4px 3px"
        return "3px 2px"
    if density == "compact":
        return "2px 2px"
    if density == "roomy":
        return "4px 3px"
    return "3px 2px"

def _metric_default_padding(template, density):
    tpl = _metric_template_name(template)
    if tpl == "row":
        return "0 4px"
    if density == "compact":
        return "4px 3px"
    if density == "roomy":
        return "8px 5px"
    return "6px 4px"

def layout_metric_stack(density = None):
    density = density if density != None else "normal"
    return grid(
        rows = ["auto", "auto"],
        columns = ["1fr", "auto"],
        areas = [["label", "label"], ["value", "unit"]],
        gap = _metric_default_gap("stack", density),
        align = "center",
        justify = "stretch",
    )

def layout_metric_row(density = None):
    density = density if density != None else "normal"
    return grid(
        rows = ["1fr"],
        columns = ["1fr", "auto", "auto"],
        areas = [["label", "value", "unit"]],
        gap = _metric_default_gap("row", density),
        align = "center",
        justify = "stretch",
    )

def layout_metric_column(density = None):
    density = density if density != None else "normal"
    return grid(
        rows = ["auto", "auto", "auto"],
        columns = ["1fr"],
        areas = [["label"], ["value"], ["unit"]],
        gap = _metric_default_gap("column", density),
        align = "center",
        justify = "stretch",
    )

def layout_metric_stack_desc(density = None):
    density = density if density != None else "normal"
    return grid(
        rows = ["auto", "auto", "auto"],
        columns = ["1fr", "auto"],
        areas = [["label", "label"], ["value", "unit"], ["desc", "desc"]],
        gap = _metric_default_gap("stack_desc", density),
        align = "center",
        justify = "stretch",
    )

def _metric_shell_props(bg = None, width_px = None, height_px = None, extra = None, template = None):
    density = _metric_density(height_px, template)
    tpl = _metric_template_name(template)
    props = {
        "chrome": "bare",
        "padding": _metric_default_padding(tpl, density),
        "width": "100%",
        "box_sizing": "border-box",
        "overflow": "hidden",
        "__mei_metric_card": True,
        "__mei_metric_density": density,
        "__mei_metric_template": tpl,
    }
    if height_px != None:
        props["height"] = str(height_px) + "px"
    if width_px != None:
        props["width"] = str(width_px) + "px"
    if bg != None and str(bg).strip() != "":
        props["background"] = {
            "image": "url(" + str(bg) + ")",
            "size": "100% 100%",
            "position": "center",
            "repeat": "no-repeat",
        }
    else:
        props["background"] = "transparent"
    if extra != None and _is_dict(extra):
        for k, v in extra.items():
            props[k] = v
    return props

def _metric_template_name(template):
    raw = str(template).strip().lower() if template != None else "stack"
    if raw == "stack-desc":
        return "stack_desc"
    return raw

def _metric_layout_from_template(template, height_px = None):
    tpl = _metric_template_name(template)
    density = _metric_density(height_px, tpl)
    if tpl == "row":
        return layout_metric_row(density)
    if tpl == "column":
        return layout_metric_column(density)
    if tpl == "stack_desc":
        return layout_metric_stack_desc(density)
    return layout_metric_stack(density)

def _metric_allowed_slots():
    return {
        "label": True,
        "value": True,
        "unit": True,
        "desc": True,
    }

def _metric_validate_layout(layout):
    if layout == None:
        return
    areas = layout.get("areas")
    if areas == None:
        fail("metric_card(layout=...) requires named grid areas: label/value/unit[/desc]")
    allowed = _metric_allowed_slots()
    for row in areas:
        if type(row) != "list":
            continue
        for cell in row:
            area = str(cell).strip()
            if area == "" or area == ".":
                continue
            if allowed.get(area) != True:
                fail("metric_card layout area `" + area + "` is invalid; expected label/value/unit/desc")

def _metric_legacy_source(label_text = None, value_text = None, unit_text = None, desc_text = None):
    if label_text == None and value_text == None and unit_text == None and desc_text == None:
        return None
    return _without_empty({
        "label": label_text,
        "value": value_text,
        "unit": unit_text,
        "desc": desc_text,
    })

def _metric_patch_slots(values, patch):
    out = {}
    for key, value in values.items():
        out[key] = value
    if _is_dict(patch):
        for key in ["label", "value", "unit", "desc"]:
            if patch.get(key) != None:
                out[key] = patch.get(key)
    return out

def _metric_slot_key(slot, map = None):
    if _is_dict(map):
        key = map.get(slot)
        if key != None and str(key).strip() != "":
            return str(key).strip()
    return slot

def _metric_static_slots(source, map = None, patch = None):
    if not _is_dict(source):
        return None
    values = {
        "label": source.get(_metric_slot_key("label", map)),
        "value": source.get(_metric_slot_key("value", map)),
        "unit": source.get(_metric_slot_key("unit", map)),
        "desc": source.get(_metric_slot_key("desc", map)),
    }
    for slot in ["label", "value", "unit", "desc"]:
        if values.get(slot) == None and source.get(slot) != None:
            values[slot] = source.get(slot)
    return _metric_patch_slots(values, patch)

def _metric_slot_align(template, role):
    tpl = _metric_template_name(template)
    if tpl == "column":
        return "center"
    if role == "label" or role == "desc":
        return "left"
    if role == "value" or role == "unit":
        return "right"
    return None

def _metric_literal_blocks(values, template = None, variant = None):
    tpl = _metric_template_name(template)
    blocks = [
        label(
            values.get("label") if values.get("label") != None else "",
            area = "label",
            align = _metric_slot_align(tpl, "label"),
            variant = variant,
        ),
        value(
            values.get("value") if values.get("value") != None else "",
            area = "value",
            align = _metric_slot_align(tpl, "value"),
            variant = variant,
        ),
        unit(
            values.get("unit") if values.get("unit") != None else "",
            area = "unit",
            align = _metric_slot_align(tpl, "unit"),
            variant = variant,
        ),
    ]
    if values.get("desc") != None:
        blocks.append(
            desc(
                values.get("desc"),
                area = "desc",
                align = _metric_slot_align(tpl, "desc"),
                variant = variant,
            ),
        )
    return blocks

def _metric_component_extra(extra):
    out = {}
    if _is_dict(extra):
        for k, v in extra.items():
            if k in [
                "background",
                "border",
                "radius",
                "padding",
                "width",
                "height",
                "min_height",
                "max_height",
                "margin",
                "box_sizing",
                "overflow",
                "chrome",
            ]:
                continue
            out[k] = v
    return out

def _metric_runtime_tile_props(source, template, layout, density, variant = None, map = None, patch = None, extra = None):
    props = _metric_component_extra(extra)
    props["template"] = _metric_template_name(template)
    props["metric_density"] = density
    props["width"] = "100%"
    props["height"] = "100%"
    if variant != None and str(variant).strip() != "":
        props["metric_variant"] = str(variant).strip()
    if layout != None:
        props["metric_layout"] = layout
    if map != None and _is_dict(map):
        props["metric_map"] = map
    if patch != None and _is_dict(patch):
        props["metric_patch"] = patch
    if _metric_is_metric_ref(source):
        props["value"] = source
    return props

def _metric_is_metric_ref(source):
    return _is_dict(source) and source.get("__ref") == "metric"

def metric_card(
    id = None,
    area = None,
    bg = None,
    label_text = None,
    value_text = None,
    unit_text = None,
    desc_text = None,
    source = None,
    map = None,
    patch = None,
    template = "stack",
    width_px = None,
    height_px = None,
    props = None,
    body_props = None,
    blocks = None,
    layout = None,
    variant = None,
):
    density = _metric_density(height_px, template)
    card_props = _metric_shell_props(bg, width_px, height_px, props, template)
    card_layout = layout
    card_blocks = blocks
    if card_layout == None:
        card_layout = _metric_layout_from_template(template, height_px)
    _metric_validate_layout(card_layout)
    if source == None:
        source = _metric_legacy_source(label_text, value_text, unit_text, desc_text)
    if card_blocks == None:
        if source == None:
            source = {}
        if _metric_is_metric_ref(source):
            card_blocks = [
                component(
                    "cockpit.qunfu-metric-tile",
                    area = "auto",
                    props = _metric_runtime_tile_props(
                        source,
                        template,
                        card_layout,
                        density,
                        variant,
                        map,
                        patch,
                        props,
                    ),
                ),
            ]
            card_layout = None
        elif _is_dict(source):
            card_blocks = _metric_literal_blocks(
                _metric_static_slots(source, map, patch),
                template,
                variant,
            )
        else:
            fail("metric_card(source=...) expects metric_ref(...) or a static object")
    return panel(
        id = id,
        area = area,
        show_heading = False,
        chrome = "bare",
        variant = "container",
        props = card_props,
        body_props = body_props,
        layout = card_layout,
        blocks = card_blocks,
    )

