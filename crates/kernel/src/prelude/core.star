exports = []

def _without_empty(values):
    result = {}
    for key, value in values.items():
        if value != None and value != False:
            result[key] = value
    return result

def _clean(values):
    return _without_empty(values)

def _declare(value):
    exports.append(value)
    return value

def _expr(source):
    return {"__expr": "source", "source": source}

def _expr_source(value):
    if type(value) == "dict" and value.get("__expr") == "source":
        return value["source"]
    return value

def _metric_source(metric):
    value = _expr_source(metric.get("value"))
    values = metric.get("values")
    source = _without_empty({
        "label": metric.get("label"),
        "unit": metric.get("unit"),
        "shape": metric.get("shape"),
        "schema": metric.get("schema"),
    })
    if type(values) == "dict":
        scalar_values = {}
        for entry_key, entry_value in values.items():
            scalar_values[entry_key] = _expr_source(entry_value)
        source["values"] = scalar_values
    if value != None:
        if type(value) == "list":
            source["list"] = value
        elif type(value) == "dict" and value.get("__expr") == "series":
            source["series"] = value["series"]
        elif type(value) == "dict" and value.get("__expr") == "list":
            source["list"] = value["list"]
            if value.get("requires") != None:
                source["requires"] = value["requires"]
        else:
            source["value"] = value
    if metric.get("where") != None:
        source["where"] = _expr_source(metric["where"])
    if metric.get("drilldown") != None:
        source["drilldown_dataset"] = metric["drilldown"]
    return source

def _data_product(shape, id = None, key = None, label = None, value = None, values = None, unit = None, schema = None, drilldown = None):
    product_id = id if id != None else key
    return _without_empty({
        "__kind": "data_product",
        "key": product_id,
        "shape": shape,
        "label": label,
        "value": value,
        "values": values,
        "unit": unit,
        "schema": schema,
        "drilldown": drilldown,
    })

def app(id, title = None, default_scene = None, scene = None, entries = None):
    return _declare(_clean({
        "kind": "app",
        "id": id,
        "title": title,
        "default_scene": default_scene,
        "scene": scene,
        "entries": entries if entries != None else [],
    }))

def entry(scene = None, frame = None, id = None, title = None):
    return _clean({
        "id": id,
        "scene": scene,
        "frame": frame,
        "title": title,
    })

def scene_file_ref(path, id = None):
    return _clean({
        "kind": "scene_file_ref",
        "path": path,
        "id": id,
    })

def world_file_ref(path, id = None):
    return _clean({
        "kind": "world_file_ref",
        "path": path,
        "id": id,
    })

def frame_file_ref(path, id = None):
    return _clean({
        "kind": "frame_file_ref",
        "path": path,
        "id": id,
    })

def app_add_scene(scene = None, id = None, profile = None, theme = None, summary = None, goal = None, state = None):
    if scene != None:
        return _declare({
            "kind": "app_scene_ref",
            "scene": scene,
        })
    return scene_decl(
        id = id,
        profile = profile,
        theme = theme,
        summary = summary,
        goal = goal,
        state = state,
    )

def scene_decl(id = None, world = None, flow = None, frame = None, profile = None, theme = None, summary = None, goal = None, state = None):
    return _declare(_clean({
        "kind": "scene",
        "id": id,
        "world": world,
        "flow": flow,
        "frame": frame,
        "profile": profile,
        "theme": theme,
        "summary": summary,
        "goal": goal,
        "state": state if state != None else {},
    }))

def scene(id = None, world = None, flow = None, frame = None, profile = None, theme = None, summary = None, goal = None, state = None):
    return scene_decl(
        id = id,
        world = world,
        flow = flow,
        frame = frame,
        profile = profile,
        theme = theme,
        summary = summary,
        goal = goal,
        state = state,
    )

def world(id = None, topology = None, resources = None, entities = None):
    return _declare(_clean({
        "kind": "world",
        "id": id,
        "topology": topology,
        "resources": resources if resources != None else [],
        "entities": entities if entities != None else [],
    }))

def world_add_resource(item):
    return _declare({
        "kind": "world_add_resource",
        "resource": item,
    })

def world_add_entity(item):
    return _declare({
        "kind": "world_add_entity",
        "entity": item,
    })

def world_set_topology(rows, cols, cells = None):
    return _declare(_clean({
        "kind": "world_set_topology",
        "topology": {
            "rows": rows,
            "cols": cols,
            "cells": cells if cells != None else [],
        },
    }))

def cell(id, row = None, col = None, surface_kind = None, flammable = None, walkable = None, occupiable = None, capacity = None, hazard_state = None, tags = None):
    return _clean({
        "id": id,
        "row": row,
        "col": col,
        "surface_kind": surface_kind,
        "flammable": flammable,
        "walkable": walkable,
        "occupiable": occupiable,
        "capacity": capacity,
        "hazard_state": hazard_state,
        "tags": tags if tags != None else [],
    })

def resource(id, kind, title = None, source = None, content = None):
    return _clean({
        "id": id,
        "kind": kind,
        "title": title,
        "source": source,
        "content": content,
    })

def entity(id, kind, label = None, spawns = None, status = None, flags = None):
    return _clean({
        "id": id,
        "kind": kind,
        "label": label,
        "spawns": spawns if spawns != None else [],
        "status": status,
        "flags": flags if flags != None else {},
    })

def start(mode = None, action_label = None):
    return _clean({
        "mode": mode,
        "action_label": action_label,
    })

def has(value):
    return {
        "type": "has",
        "value": value,
    }

def grant(value):
    return {
        "type": "grant",
        "value": value,
    }

def set_status(target, value):
    return {
        "type": "set_status",
        "target": target,
        "value": value,
    }

def set_flag(target, value):
    return {
        "type": "set_flag",
        "target": target,
        "value": value,
    }

def finish(target, value = None):
    return _clean({
        "type": "finish",
        "target": target,
        "value": value,
    })

def effects(items):
    return {
        "type": "effects",
        "effects": items,
    }

def click(target, require = None, effect = None):
    return _clean({
        "target": target,
        "require": require,
        "effect": effect,
    })

def rule_timer(seconds, on_timeout):
    return {
        "seconds": seconds,
        "on_timeout": on_timeout,
    }

def subject_timer(subject_ref, timer_type, delay_seconds, on_timeout, id = None, interval_seconds = None, repeat = False, cancel_when = None):
    return _clean({
        "id": id,
        "subject_ref": subject_ref,
        "type": timer_type,
        "delay_seconds": delay_seconds,
        "interval_seconds": interval_seconds,
        "repeat": repeat,
        "on_timeout": on_timeout,
        "cancel_when": cancel_when,
    })

def rule_outcome(success = None, fail = None):
    return _clean({
        "success": success,
        "fail": fail,
    })

def flow(id = None, start = None, interactions = None, timer = None, subject_timers = None, outcome = None):
    return _declare(_clean({
        "kind": "flow",
        "id": id,
        "start": start,
        "interactions": interactions if interactions != None else [],
        "timer": timer,
        "subject_timers": subject_timers if subject_timers != None else [],
        "outcome": outcome,
    }))

def world_ref(id):
    return {
        "__ref": "world",
        "id": id,
    }

def scene_ref(id):
    return {
        "__ref": "scene",
        "id": id,
    }
