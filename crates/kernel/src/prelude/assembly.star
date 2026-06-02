# Projection assembly layer: derive drilldown slots from metric / explain scope.

def _metric_ref_id(metric):
    if metric == None or type(metric) != "dict":
        fail("projection assembly requires metric_ref(...)")
    if metric.get("__ref") != "metric":
        fail("projection assembly metric must be metric_ref(...)")
    mid = metric.get("id")
    if mid == None or str(mid).strip() == "":
        fail("metric_ref requires non-empty id")
    return str(mid).strip()

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
    """Slot list from metric explain scope; lowered at compile time."""
    return _without_empty({
        "__kind": "projection_slot_list",
        "source": metric,
        "include_hero": False,
    })

def build_drilldown_tabs(metric, include_hero = True, default_slot = None):
    """Hero + explain slots; default_tab index is 0-based."""
    payload = {
        "__kind": "projection_slot_list",
        "source": metric,
        "include_hero": include_hero,
    }
    if default_slot != None:
        payload["default_slot"] = default_slot
    return _without_empty(payload)
