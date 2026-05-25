# 指标卡 DSL：
# - 单卡：metric_card(template|layout, source, map?, patch?)
# - 宽卡/组合：panel(layout, blocks=[metric_card(...), ...])
# - 固定语义槽：label / value / unit / desc
# - source 直接接 metric_ref(...) 或静态对象

def _metric_text(content, area, role, font = None, align = None, line_height = None, variant = None, vertical_align = None):
    props = {"content": content, "metric_role": role}
    if font != None:
        props["font"] = font
    if align != None:
        props["align"] = align
    if line_height != None:
        props["line_height"] = line_height
    if variant != None and str(variant).strip() != "":
        props["metric_variant"] = str(variant).strip()
    if vertical_align != None and str(vertical_align).strip() != "":
        props["metric_v_align"] = str(vertical_align).strip()
    return component(
        "mei.text",
        area = area,
        props = _without_empty(props),
    )

def label(content, area = None, font = None, align = None, line_height = None, variant = None, vertical_align = None):
    return _metric_text(content, area, "label", font, align, line_height, variant, vertical_align)

def value(content, area = None, font = None, align = None, line_height = None, variant = None, vertical_align = None):
    return _metric_text(content, area, "value", font, align, line_height, variant, vertical_align)

def unit(content, area = None, font = None, align = None, line_height = None, variant = None, vertical_align = None):
    return _metric_text(content, area, "unit", font, align, line_height, variant, vertical_align)

def desc(content, area = None, font = None, align = None, line_height = None, variant = None, vertical_align = None):
    return _metric_text(content, area, "desc", font, align, line_height, variant, vertical_align)

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

def _metric_inline_align_mode(template = None, inline_align = None):
    raw = str(inline_align).strip().lower() if inline_align != None else ""
    raw = raw.replace("-", "_")
    if raw in ["between", "justify", "justified", "split", "space_between"]:
        return "between"
    if raw in ["compact", "compact_center", "center", "centre", "centered", "centred"]:
        return "compact"
    tpl = _metric_template_name(template)
    if tpl == "row":
        return "compact"
    return "compact"

def _metric_ratio_track(value = None, fallback = 1):
    raw = str(value).strip() if value != None else str(fallback)
    if raw == "":
        raw = str(fallback)
    return raw + "fr"

def _metric_title_content_tracks(title_ratio = None, content_ratio = None):
    return [
        _metric_ratio_track(title_ratio, 1),
        _metric_ratio_track(content_ratio, 1),
    ]

def layout_metric_stack(density = None, title_ratio = None, content_ratio = None):
    density = density if density != None else "normal"
    return grid(
        rows = _metric_title_content_tracks(title_ratio, content_ratio),
        columns = ["auto", "auto"],
        areas = [["label", "label"], ["value", "unit"]],
        gap = _metric_default_gap("stack", density),
        align = "stretch",
        justify = "center",
    )

def layout_metric_row(density = None, inline_align = None, slot_align = None):
    density = density if density != None else "normal"
    mode = _metric_inline_align_mode("row", inline_align)
    columns = ["auto", "auto", "auto"] if mode == "compact" else ["1fr", "auto", "auto"]
    justify = "center" if mode == "compact" else "stretch"
    row_align = "end"
    if slot_align != None and str(slot_align).strip() != "":
        row_align = str(slot_align).strip().lower()
    return grid(
        rows = ["1fr"],
        columns = columns,
        areas = [["label", "value", "unit"]],
        gap = _metric_default_gap("row", density),
        align = row_align,
        justify = justify,
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

def layout_metric_stack_desc(density = None, title_ratio = None, content_ratio = None):
    density = density if density != None else "normal"
    return grid(
        rows = _metric_title_content_tracks(title_ratio, content_ratio) + ["auto"],
        columns = ["auto", "auto"],
        areas = [["label", "label"], ["value", "unit"], ["desc", "desc"]],
        gap = _metric_default_gap("stack_desc", density),
        align = "stretch",
        justify = "center",
    )

def _metric_shell_props(bg = None, width_px = None, height_px = None, extra = None, template = None, inline_align = None, title_ratio = None, content_ratio = None):
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
        "__mei_metric_inline_align": _metric_inline_align_mode(tpl, inline_align),
        "__mei_metric_title_ratio": str(title_ratio).strip() if title_ratio != None else "1",
        "__mei_metric_content_ratio": str(content_ratio).strip() if content_ratio != None else "1",
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

# panel(base=...) 克隆时只写入显式覆写字段，避免用默认壳冲掉模板 background/width。
# stamp_template_meta=False：保留模板上的 __mei_metric_template / inline_align（勿用默认 template=stack 盖掉 stack_desc）。
def _metric_shell_overlay(bg = None, width_px = None, height_px = None, extra = None, template = None, inline_align = None, title_ratio = None, content_ratio = None, stamp_template_meta = True):
    density = _metric_density(height_px, template)
    tpl = _metric_template_name(template)
    props = {
        "__mei_metric_density": density,
        "__mei_metric_title_ratio": str(title_ratio).strip() if title_ratio != None else "1",
        "__mei_metric_content_ratio": str(content_ratio).strip() if content_ratio != None else "1",
    }
    if stamp_template_meta:
        props["__mei_metric_template"] = tpl
        props["__mei_metric_inline_align"] = _metric_inline_align_mode(tpl, inline_align)
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
    if extra != None and _is_dict(extra):
        for k, v in extra.items():
            props[k] = v
    return _without_empty(props)

def _metric_template_name(template):
    raw = str(template).strip().lower() if template != None else "stack"
    if raw == "stack-desc":
        return "stack_desc"
    return raw

def _metric_layout_from_template(template, height_px = None, inline_align = None, title_ratio = None, content_ratio = None):
    tpl = _metric_template_name(template)
    density = _metric_density(height_px, tpl)
    if tpl == "row":
        return layout_metric_row(density, inline_align)
    if tpl == "column":
        return layout_metric_column(density)
    if tpl == "stack_desc":
        return layout_metric_stack_desc(density, title_ratio, content_ratio)
    return layout_metric_stack(density, title_ratio, content_ratio)

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

def _metric_slot_align(template, role, inline_align = None):
    tpl = _metric_template_name(template)
    if tpl == "column":
        return "center"
    mode = _metric_inline_align_mode(tpl, inline_align)
    if tpl == "row" and mode == "between" and (role == "label" or role == "desc"):
        return "left"
    if tpl == "row" and mode == "between" and (role == "value" or role == "unit"):
        return "right"
    return "center"

def _metric_slot_vertical_align(template, role):
    tpl = _metric_template_name(template)
    if tpl == "row":
        return "end"
    if tpl == "column":
        return "center"
    if role == "value" or role == "unit":
        return "end"
    return "center"

def _metric_slot_vertical_align_key(role):
    return "__mei_metric_" + str(role).strip() + "_v_align"

def _metric_slot_vertical_align_for_props(shell_props, template, role):
    if _is_dict(shell_props):
        raw = shell_props.get(_metric_slot_vertical_align_key(role))
        if raw != None and str(raw).strip() != "":
            return str(raw).strip()
    return _metric_slot_vertical_align(template, role)

def _metric_stamp_slot_vertical_align(shell_props, label_vertical_align = None, value_vertical_align = None, unit_vertical_align = None, desc_vertical_align = None):
    if not _is_dict(shell_props):
        shell_props = {}
    pairs = [
        ("label", label_vertical_align),
        ("value", value_vertical_align),
        ("unit", unit_vertical_align),
        ("desc", desc_vertical_align),
    ]
    for role, raw in pairs:
        if raw != None and str(raw).strip() != "":
            shell_props[_metric_slot_vertical_align_key(role)] = str(raw).strip()
    return shell_props

def _metric_desc_mode(shell_props):
    if not _is_dict(shell_props):
        return ""
    raw = shell_props.get("metric_desc_mode")
    if raw == None:
        raw = shell_props.get("__mei_metric_desc_mode")
    return str(raw).strip().lower() if raw != None else ""

def _metric_desc_shell(shell_props):
    if not _is_dict(shell_props):
        return None
    shell = shell_props.get("metric_desc_shell")
    return shell if _is_dict(shell) else None

def _metric_desc_progress_block(value, shell_props = None, vertical_align = None):
    props = {
        "value": value if value != None else "",
        "metric_role": "desc",
        "align": "center",
    }
    shell = _metric_desc_shell(shell_props)
    if shell != None:
        props["progress_shell"] = shell
    if vertical_align != None and str(vertical_align).strip() != "":
        props["metric_v_align"] = str(vertical_align).strip()
    return component(
        "cockpit.metric-progress",
        area = "desc",
        props = _without_empty(props),
    )

def _metric_literal_blocks(values, template = None, variant = None, inline_align = None, shell_props = None, defer_slot_vertical_align = False):
    tpl = _metric_template_name(template)
    def _slot_v_align(role):
        if defer_slot_vertical_align:
            return None
        return _metric_slot_vertical_align_for_props(shell_props, tpl, role)
    blocks = [
        label(
            values.get("label") if values.get("label") != None else "",
            area = "label",
            align = _metric_slot_align(tpl, "label", inline_align),
            variant = variant,
            vertical_align = _slot_v_align("label"),
        ),
        value(
            values.get("value") if values.get("value") != None else "",
            area = "value",
            align = _metric_slot_align(tpl, "value", inline_align),
            variant = variant,
            vertical_align = _slot_v_align("value"),
        ),
        unit(
            values.get("unit") if values.get("unit") != None else "",
            area = "unit",
            align = _metric_slot_align(tpl, "unit", inline_align),
            variant = variant,
            vertical_align = _slot_v_align("unit"),
        ),
    ]
    if values.get("desc") != None:
        if _metric_desc_mode(shell_props) == "progress":
            blocks.append(
                _metric_desc_progress_block(
                    values.get("desc"),
                    shell_props = shell_props,
                    vertical_align = _slot_v_align("desc"),
                ),
            )
        else:
            blocks.append(
                desc(
                    values.get("desc"),
                    area = "desc",
                    align = _metric_slot_align(tpl, "desc", inline_align),
                    variant = variant,
                    vertical_align = _slot_v_align("desc"),
                ),
            )
    return blocks

def _metric_runtime_slot_block(source, role, area, template = None, variant = None, inline_align = None, shell_props = None, map = None, patch = None, defer_slot_vertical_align = False):
    tpl = _metric_template_name(template)
    props = {
        "content": source,
        "metric_role": role,
        "align": _metric_slot_align(tpl, role, inline_align),
    }
    if variant != None and str(variant).strip() != "":
        props["metric_variant"] = str(variant).strip()
    if not defer_slot_vertical_align:
        v_align = _metric_slot_vertical_align_for_props(shell_props, tpl, role)
        if v_align != None and str(v_align).strip() != "":
            props["metric_v_align"] = str(v_align).strip()
    if map != None and _is_dict(map):
        props["metric_map"] = map
    if patch != None and _is_dict(patch):
        props["metric_patch"] = patch
    return component(
        "mei.text",
        area = area,
        props = _without_empty(props),
    )

def _metric_runtime_blocks(source, template = None, variant = None, inline_align = None, shell_props = None, map = None, patch = None, defer_slot_vertical_align = False):
    tpl = _metric_template_name(template)
    blocks = [
        _metric_runtime_slot_block(
            source,
            "label",
            "label",
            template = tpl,
            variant = variant,
            inline_align = inline_align,
            shell_props = shell_props,
            map = map,
            patch = patch,
            defer_slot_vertical_align = defer_slot_vertical_align,
        ),
        _metric_runtime_slot_block(
            source,
            "value",
            "value",
            template = tpl,
            variant = variant,
            inline_align = inline_align,
            shell_props = shell_props,
            map = map,
            patch = patch,
            defer_slot_vertical_align = defer_slot_vertical_align,
        ),
        _metric_runtime_slot_block(
            source,
            "unit",
            "unit",
            template = tpl,
            variant = variant,
            inline_align = inline_align,
            shell_props = shell_props,
            map = map,
            patch = patch,
            defer_slot_vertical_align = defer_slot_vertical_align,
        ),
    ]
    wants_desc = tpl == "stack_desc"
    if not wants_desc and _is_dict(map) and map.get("desc") != None:
        wants_desc = True
    if not wants_desc and _is_dict(patch) and patch.get("desc") != None:
        wants_desc = True
    if wants_desc:
        blocks.append(
            _metric_runtime_slot_block(
                source,
                "desc",
                "desc",
                template = tpl,
                variant = variant,
                inline_align = inline_align,
                shell_props = shell_props,
                map = map,
                patch = patch,
                defer_slot_vertical_align = defer_slot_vertical_align,
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

def _metric_runtime_tile_props(source, template, layout, density, variant = None, map = None, patch = None, extra = None, inline_align = None, title_ratio = None, content_ratio = None):
    props = _metric_component_extra(extra)
    props["template"] = _metric_template_name(template)
    props["metric_density"] = density
    props["metric_inline_align"] = _metric_inline_align_mode(template, inline_align)
    props["metric_title_ratio"] = str(title_ratio).strip() if title_ratio != None else "1"
    props["metric_content_ratio"] = str(content_ratio).strip() if content_ratio != None else "1"
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
    if _is_dict(extra):
        for role in ["label", "value", "unit", "desc"]:
            key = _metric_slot_vertical_align_key(role)
            if extra.get(key) != None:
                props[key] = extra.get(key)
    return props

def _metric_is_metric_ref(source):
    return _is_dict(source) and source.get("__ref") == "metric"

# Pure reference value: metric_card(base=metric_card_ref(...)) clones an external panel template.
# Equivalent to panel_ref(...); metric_card(...) lowers to panel(...).
def metric_card_ref(id = None, scene_file = None, scene_id = None):
    if id == None or str(id).strip() == "":
        fail("metric_card_ref requires `id` (target metric panel template id in scene_file)")
    return panel_ref(id = id, scene_file = scene_file, scene_id = scene_id)

def metric_card(
    id = None,
    area = None,
    base = None,
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
    inline_align = None,
    title_ratio = None,
    content_ratio = None,
    scale = None,
    label_vertical_align = None,
    value_vertical_align = None,
    unit_vertical_align = None,
    desc_vertical_align = None,
):
    effective_props = _clone_props(props)
    if scale != None and str(scale).strip() != "":
        effective_props["scale"] = scale
    has_source_content = (
        source != None
        or label_text != None
        or value_text != None
        or unit_text != None
        or desc_text != None
        or blocks != None
    )
    has_shell_override = (
        bg != None
        or width_px != None
        or height_px != None
        or props != None
        or body_props != None
    )
    has_layout_override = layout != None
    if base != None and not has_source_content and not has_shell_override and not has_layout_override:
        return panel(id = id, area = area, base = base)
    density = _metric_density(height_px, template)
    # base= 克隆模板时只用 overlay，避免 _metric_shell_props 的 background: transparent / width:100% 冲掉模板壳。
    card_props = (
        _metric_shell_overlay(
            bg,
            width_px,
            height_px,
            effective_props,
            template,
            inline_align,
            title_ratio,
            content_ratio,
            stamp_template_meta = False,
        )
        if base != None
        else _metric_shell_props(bg, width_px, height_px, effective_props, template, inline_align, title_ratio, content_ratio)
    )
    card_props = _metric_stamp_slot_vertical_align(
        card_props,
        label_vertical_align,
        value_vertical_align,
        unit_vertical_align,
        desc_vertical_align,
    )
    card_layout = layout
    card_blocks = blocks
    # base= 克隆模板时 layout 由模板 panel 提供；勿用默认 template=stack 生成布局覆盖 stack_desc/desc 区。
    if card_layout == None and base == None:
        card_layout = _metric_layout_from_template(template, height_px, inline_align, title_ratio, content_ratio)
    effective_layout = card_layout
    if base != None and not has_layout_override:
        effective_layout = None
    if effective_layout != None:
        _metric_validate_layout(effective_layout)
    if base != None and _is_dict(effective_props):
        for key in ["metric_desc_mode", "__mei_metric_desc_mode", "metric_desc_shell"]:
            if effective_props.get(key) != None:
                card_props[key] = effective_props.get(key)
    if source == None:
        source = _metric_legacy_source(label_text, value_text, unit_text, desc_text)
    if card_blocks == None:
        if source == None:
            source = {}
        if _metric_is_metric_ref(source):
            card_blocks = _metric_runtime_blocks(
                source,
                template,
                variant,
                inline_align,
                card_props,
                map,
                patch,
                base != None,
            )
        elif _is_dict(source):
            card_blocks = _metric_literal_blocks(
                _metric_static_slots(source, map, patch),
                template,
                variant,
                inline_align,
                card_props,
                base != None,
            )
        else:
            fail("metric_card(source=...) expects metric_ref(...) or a static object")
    if base != None:
        if has_shell_override:
            return panel(
                id = id,
                area = area,
                base = base,
                show_heading = False,
                chrome = "bare",
                variant = "container",
                props = card_props,
                body_props = body_props,
                layout = effective_layout,
                blocks = card_blocks,
            )
        return panel(
            id = id,
            area = area,
            base = base,
            props = card_props,
            body_props = body_props,
            layout = effective_layout,
            blocks = card_blocks,
        )
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

