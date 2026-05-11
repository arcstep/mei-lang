def _sim_without_empty(values):
    result = {}
    for key, value in values.items():
        if value != None and value != False:
            result[key] = value
    return result

def scene(id, kind = "discrete_scene", summary = None, goal = None, start_label = None):
    return _declare({"scene": _sim_without_empty({
        "id": id,
        "kind": kind,
        "summary": summary,
        "goal": goal,
        "start_label": start_label,
    })})

def space(kind, slots = None, fire_spawns = None, extinguisher_spawns = None):
    return _declare({"space": _sim_without_empty({
        "kind": kind,
        "slots": slots,
        "fire_spawns": fire_spawns,
        "extinguisher_spawns": extinguisher_spawns,
    })})

def grid(rows = None, cols = None, columns = None, areas = None, gap = None, padding = None, align = None, justify = None, slots = None, fire_spawns = None, extinguisher_spawns = None):
    if columns != None or areas != None or type(rows) == "list":
        return _without_empty({
            "type": "grid",
            "columns": columns,
            "rows": rows,
            "areas": areas,
            "gap": gap,
            "padding": padding,
            "align": align,
            "justify": justify,
        })
    return _sim_without_empty({
        "rows": rows,
        "cols": cols,
        "slots": slots,
        "fire_spawns": fire_spawns,
        "extinguisher_spawns": extinguisher_spawns,
    })

def entity(id, kind, label = None, spawns = None, status = None, flags = None):
    return _sim_without_empty({
        "id": id,
        "kind": kind,
        "label": label,
        "spawns": spawns,
        "status": status,
        "flags": flags,
    })

def world(grid, entities = [], seed_mode = "shuffle"):
    return _declare({"world": _sim_without_empty({
        "grid": grid,
        "entities": entities,
        "seed_mode": seed_mode,
    })})

def click(target, effect = None, require = None):
    return _sim_without_empty({
        "kind": "click",
        "target": target,
        "effect": effect,
        "require": require,
    })

def has(item):
    return {
        "kind": "has",
        "item": item,
    }

def grant(item):
    return {
        "kind": "grant",
        "item": item,
    }

def own(item):
    return grant(item)

def extinguish(target):
    return set_status(target, "out")

def set_status(entity, value):
    return {
        "kind": "set_status",
        "entity": entity,
        "value": value,
    }

def set_flag(key, value):
    return {
        "kind": "set_flag",
        "key": key,
        "value": value,
    }

def finish(result, reason = None):
    return _sim_without_empty({
        "kind": "finish",
        "result": result,
        "reason": reason,
    })

def fail(reason):
    return finish("fail", reason)

def effects(items):
    return items

def start(mode = "manual", action_label = "开始演练"):
    return _sim_without_empty({
        "mode": mode,
        "action_label": action_label,
    })

def interaction(event = None, intent = None, target = None, pass_ = None, rules = None):
    if rules != None or (event == None and intent == None and target == None):
        return _declare({"interaction": {
            "rules": rules if rules != None else [],
        }})
    return _sim_without_empty({
        "event": event,
        "intent": intent,
        "target": target,
        "pass": pass_,
    })

def timer(seconds, on_timeout):
    return _declare({"timer": {
        "seconds": seconds,
        "on_timeout": on_timeout,
    }})

def outcome(success, fail):
    return _declare({"outcome": {
        "success": success,
        "fail": fail,
    }})

def rule_timer(seconds, on_timeout, counter = "countdown"):
    return _sim_without_empty({
        "counter": counter,
        "seconds": seconds,
        "on_timeout": on_timeout,
    })

def rule_outcome(success, fail):
    return _sim_without_empty({
        "success": success,
        "fail": fail,
    })

def rules(start = None, interactions = [], timer = None, outcome = None):
    return _declare({"rules": _sim_without_empty({
        "start": start,
        "interactions": interactions,
        "timer": timer,
        "outcome": outcome,
    })})

def present(show):
    return _declare({"present": {
        "show": show,
    }})

def view_layout(columns, rows = None, gap = None):
    return _sim_without_empty({
        "columns": columns,
        "rows": rows,
        "gap": gap,
    })

def view_section(id, title = None, slot = None, show = []):
    return _sim_without_empty({
        "id": id,
        "title": title,
        "slot": slot,
        "show": show,
    })

def view(layout = None, sections = [], show = None):
    return _declare({"view": _sim_without_empty({
        "layout": layout,
        "sections": sections,
        "show": show,
    })})
