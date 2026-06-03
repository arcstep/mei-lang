def xlsx(path, sheet = None, header_row = 1, preview_rows = None, page_size = None, max_page_size = None):
    return _without_empty({
        "__source": "xlsx",
        "path": path,
        "sheet": sheet,
        "header_row": header_row,
        "preview_rows": preview_rows,
        "page_size": page_size,
        "max_page_size": max_page_size,
    })

def json(path, preview_rows = None, page_size = None, max_page_size = None):
    return _without_empty({
        "__source": "json",
        "path": path,
        "preview_rows": preview_rows,
        "page_size": page_size,
        "max_page_size": max_page_size,
    })

def geojson(path, preview_rows = None, page_size = None, max_page_size = None):
    return _without_empty({
        "__source": "geojson",
        "path": path,
        "preview_rows": preview_rows,
        "page_size": page_size,
        "max_page_size": max_page_size,
    })

def csv(path, header_row = 1, preview_rows = None, page_size = None, max_page_size = None):
    return _without_empty({
        "__source": "csv",
        "path": path,
        "header_row": header_row,
        "preview_rows": preview_rows,
        "page_size": page_size,
        "max_page_size": max_page_size,
    })

def db(connection, table = None, query = None, preview_rows = None, page_size = None, max_page_size = None):
    return _without_empty({
        "__source": "db",
        "connection": connection,
        "table": table,
        "query": query,
        "preview_rows": preview_rows,
        "page_size": page_size,
        "max_page_size": max_page_size,
    })

def column(name, type, source = None, optional = False, unit = None):
    return _without_empty({
        "name": name,
        "type": type,
        "source": source,
        "optional": optional,
        "unit": unit,
    })

def _resolve_resource_id(kind, id = None, key = None):
    dataset_id = id if id != None else key
    if dataset_id == None:
        fail(kind + " requires explicit `id` or `key` in world-only mode")
    if dataset_id == "__source_path__":
        fail(kind + " does not allow legacy id `__source_path__`; use a stable id")
    if type(dataset_id) == "string" and dataset_id.endswith(".mei"):
        fail(kind + " does not allow `.mei` path aliases as resource id; use a stable id")
    return dataset_id

def dataset_resource(id = None, key = None, title = None, desc = None, purpose = None, source = None, schema = None, columns = None, metrics = [], filters = {}, binding = None):
    dataset_id = _resolve_resource_id("dataset_resource", id = id, key = key)
    dataset_schema = schema if schema != None else columns
    if dataset_schema == None:
        dataset_schema = []

    normalize = {}
    normalized_columns = []
    for item in dataset_schema:
        if item.get("source") != None and item.get("name") != None:
            normalize[item["source"]] = item["name"]
        column_item = {}
        for column_key, column_value in item.items():
            if column_key != "source":
                column_item[column_key] = column_value
        normalized_columns.append(column_item)

    dataset_node = {
        "key": dataset_id,
        "kind": "dataframe",
        "columns": normalized_columns,
    }
    if binding != None:
        dataset_node["binding"] = binding
    if len(normalize) > 0:
        dataset_node["normalize"] = normalize

    metric_map = {}
    for item in metrics:
        metric_map[item["key"]] = _metric_source(item)

    source_node = {}
    if source != None:
        source_node = _without_empty({
            "kind": source.get("__source", "xlsx"),
            "path": source.get("path"),
            "sheet": source.get("sheet"),
            "header_row": source.get("header_row"),
            "preview_rows": source.get("preview_rows"),
            "page_size": source.get("page_size"),
            "max_page_size": source.get("max_page_size"),
            "table": source.get("table"),
            "query": source.get("query"),
            "connection": source.get("connection"),
        })

    return resource(
        id = dataset_id,
        kind = "dataset",
        title = title,
        purpose = desc if desc != None else purpose,
        source = source_node,
        dataset = dataset_node,
        metrics = metric_map,
        filters = filters,
        binding = binding,
    )

def world_add_dataset(id = None, key = None, title = None, desc = None, purpose = None, source = None, schema = None, columns = None, metrics = [], filters = {}, binding = None):
    return world_add_resource(dataset_resource(
        id = id,
        key = key,
        title = title,
        desc = desc,
        purpose = purpose,
        source = source,
        schema = schema,
        columns = columns,
        metrics = metrics,
        filters = filters,
        binding = binding,
    ))

def dataset(id = None, key = None, title = None, desc = None, purpose = None, source = None, schema = None, columns = None, metrics = [], filters = {}, binding = None):
    legacy_id = id if id != None else key
    if legacy_id == None:
        legacy_id = "__forbidden_world_only__"
    return _declare({
        "schema_version": "0.1.0",
        "data_ref": "dataset." + legacy_id,
        "purpose": desc if desc != None else purpose,
        "title": title,
        "source": {},
        "dataset": {
            "key": legacy_id,
            "kind": "dataframe",
            "columns": [],
        },
        "metrics": {},
        "filters": filters,
        "binding": binding,
        "__forbidden_world_only__": "dataset",
    })

def dataset_ref(id, scene_file = None, scene_id = None, path = None):
    return _without_empty({
        "__ref": "dataset",
        "id": id,
        "scene_id": scene_id,
        "scene_file": scene_file if scene_file != None else path,
    })

def data_ref(path):
    if type(path) == "dict" and path.get("__kind") == "analysis_expr":
        return path
    return _analysis("rows", dataset = path)

def metric_ref(id, from_dataset = None, scene_file = None, scene_id = None):
    return _without_empty({
        "__ref": "metric",
        "id": id,
        "from_dataset": from_dataset,
        "scene_id": scene_id,
        "scene_file": scene_file,
    })

def _explain_item(
    kind,
    id = None,
    label = None,
    note = None,
    content = None,
    format = None,
    basis_refs = None,
    recommended_dimensions = None,
    numerator = None,
    denominator = None,
    formula = None,
    source = None,
    by = None,
    date_field = None,
    grain = None,
    fields = None,
    headers = None,
    mapping = None,
    chart_kind = None,
):
    return _without_empty({
        "__kind": "explain_item",
        "id": id if id != None else kind,
        "kind": kind,
        "label": label,
        "note": note,
        "content": content,
        "format": format,
        "basis_refs": basis_refs,
        "recommended_dimensions": recommended_dimensions,
        "numerator": numerator,
        "denominator": denominator,
        "formula": formula,
        "source": source,
        "by": by,
        "date_field": date_field,
        "grain": grain,
        "fields": fields,
        "headers": headers,
        "mapping": mapping,
        "chart_kind": chart_kind,
    })

def text(content):
    if content == None or str(content).strip() == "":
        fail("text requires non-empty content")
    return {
        "format": "text",
        "content": str(content).strip(),
    }

def md(content):
    if content == None or str(content).strip() == "":
        fail("md requires non-empty content")
    return {
        "format": "md",
        "content": str(content).strip(),
    }

def note(content, id = "note", label = None):
    if content == None or str(content).strip() == "":
        fail("note requires non-empty content")
    return _explain_item(
        "note",
        id = id,
        label = label,
        note = str(content).strip(),
        content = str(content).strip(),
        format = "text",
    )

def definition(id = "definition", label = None, note = None, basis_refs = None, recommended_dimensions = None):
    return _explain_item(
        "definition",
        id = id,
        label = label,
        note = note,
        basis_refs = basis_refs,
        recommended_dimensions = recommended_dimensions,
    )

def numerator_denominator(id = "numerator_denominator", label = None, numerator = None, denominator = None, formula = None):
    return _explain_item(
        "numerator_denominator",
        id = id,
        label = label,
        numerator = numerator,
        denominator = denominator,
        formula = formula,
    )

def detail(id = "detail", label = None, source = None, fields = None, headers = None):
    return _explain_item(
        "detail",
        id = id,
        label = label,
        source = source,
        fields = fields,
        headers = headers,
    )

def composition(id = "composition", label = None, by = None, source = None):
    return _explain_item(
        "composition",
        id = id,
        label = label,
        by = by,
        source = source,
    )

def analysis(kind, title = None, note = None, columns = None, headers = None, mapping = None, chart_kind = None):
    return _without_empty({
        "__kind": "metric_analysis",
        "kind": kind,
        "title": title,
        "note": note,
        "columns": columns,
        "headers": headers,
        "mapping": mapping,
        "chart_kind": chart_kind,
    })

def analysis_link(entry = None, mode = None, template = None, default_focus = None):
    return _without_empty({
        "entry": entry,
        "mode": mode,
        "template": template,
        "default_focus": default_focus,
    })

def _computed_metric_source(metric):
    value = _expr_source(metric.get("value"))
    values = metric.get("values")
    source = _without_empty({
        "label": metric.get("label"),
        "unit": metric.get("unit"),
        "shape": metric.get("shape"),
        "schema": metric.get("schema"),
        "value": value,
        "dataset": metric.get("dataset"),
        "fallback": metric.get("fallback"),
        "drilldown_dataset": metric.get("drilldown"),
        "explain": metric.get("explain"),
        "analyses": metric.get("analyses"),
        "binding": metric.get("binding"),
    })
    if type(values) == "dict":
        scalar_values = {}
        for entry_key, entry_value in values.items():
            scalar_values[entry_key] = _expr_source(entry_value)
        source["values"] = scalar_values
    if metric.get("transforms") != None:
        source["transforms"] = metric.get("transforms")
    if metric.get("op") != None:
        source["op"] = metric.get("op")
    return source

def metric_pack_resource(id, desc = None, purpose = None, metrics = []):
    pack_id = _resolve_resource_id("metric_pack_resource", id = id)
    metric_map = {}
    for item in metrics:
        metric_map[item["key"]] = _computed_metric_source(item)
    return resource(
        id = pack_id,
        kind = "metric_pack",
        title = desc if desc != None else purpose,
        purpose = desc if desc != None else purpose,
        metrics = metric_map,
    )

def world_add_metric_pack(id, desc = None, purpose = None, metrics = []):
    return world_add_resource(metric_pack_resource(
        id = id,
        desc = desc,
        purpose = purpose,
        metrics = metrics,
    ))

def metric_pack(id, desc = None, purpose = None, metrics = []):
    return _declare({
        "schema_version": "0.1.0",
        "metric_pack": {
            "id": id,
            "purpose": desc if desc != None else purpose,
        },
        "metrics": {},
        "__forbidden_world_only__": "metric_pack",
    })

def dataset_view_resource(id = None, title = None, desc = None, purpose = None, sources = [], rowset = None, schema = None, columns = None, metrics = [], filters = {}, binding = None):
    dataset_id = _resolve_resource_id("dataset_view_resource", id = id)
    dataset_schema = schema if schema != None else columns
    if dataset_schema == None:
        dataset_schema = []
    metric_map = {}
    for item in metrics:
        metric_map[item["key"]] = _computed_metric_source(item)
    return resource(
        id = dataset_id,
        kind = "dataset_view",
        title = title,
        purpose = desc if desc != None else purpose,
        dataset = {
            "key": dataset_id,
            "kind": "dataset_view",
            "sources": sources,
            "columns": dataset_schema,
            "rowset": rowset,
            "binding": binding,
        },
        metrics = metric_map,
        filters = filters,
        binding = binding,
    )

def world_add_dataset_view(id = None, title = None, desc = None, purpose = None, sources = [], rowset = None, schema = None, columns = None, metrics = [], filters = {}, binding = None):
    return world_add_resource(dataset_view_resource(
        id = id,
        title = title,
        desc = desc,
        purpose = purpose,
        sources = sources,
        rowset = rowset,
        schema = schema,
        columns = columns,
        metrics = metrics,
        filters = filters,
        binding = binding,
    ))

def dataset_view(id = None, title = None, desc = None, purpose = None, sources = [], rowset = None, schema = None, columns = None, metrics = [], filters = {}, binding = None):
    legacy_id = id if id != None else "__forbidden_world_only__"
    return _declare({
        "kind": "dataset_view",
        "id": legacy_id,
        "title": title,
        "rowset": rowset,
        "schema": schema if schema != None else (columns if columns != None else []),
        "metrics": [],
        "binding": binding,
        "__forbidden_world_only__": "dataset_view",
    })

def computed_metric(id = None, key = None, label = None, unit = None, dataset = None, transforms = [], op = None, fallback = None, drilldown = None, explain = None, analyses = None, binding = None):
    metric_id = id if id != None else key
    return _without_empty({
        "__kind": "computed_metric",
        "key": metric_id,
        "label": label,
        "unit": unit,
        "dataset": dataset,
        "transforms": transforms,
        "op": op,
        "fallback": fallback,
        "drilldown": drilldown,
        "explain": explain,
        "analyses": analyses,
        "binding": binding,
    })

def _analysis(type, **kwargs):
    result = {
        "__kind": "analysis_expr",
        "type": type,
    }
    for key, value in kwargs.items():
        if value != None:
            result[key] = value
    return result

def _is_analysis(value):
    return type(value) == "dict" and value.get("__kind") == "analysis_expr"

def col(name):
    return _analysis("col", field = name)

def lit(value):
    return _analysis("lit", value = value)

def where(rowset, predicate):
    return _analysis("where", rowset = rowset, predicate = predicate)

def filter_rows(rowset, predicate):
    return where(rowset, predicate)

def first_by(rowset, field):
    return _analysis("first_by", rowset = rowset, field = field)

def distinct_by(rowset, fields):
    return _analysis("distinct_by", rowset = rowset, fields = fields)

def sort_by(rowset, field, order = "asc"):
    return _analysis("sort_by", rowset = rowset, field = field, order = order)

def select(rowset, fields):
    return _analysis("select", rowset = rowset, fields = fields)

def rename(rowset, mapping):
    return _analysis("rename", rowset = rowset, mapping = mapping)

def mutate(rowset, updates):
    return _analysis("mutate", rowset = rowset, updates = updates)

def reorder(rowset, fields):
    return _analysis("reorder", rowset = rowset, fields = fields)

def stage(rowset, schema):
    return _analysis("stage", rowset = rowset, schema = schema)

def step_where(predicate):
    return {"__kind": "pipeline_step", "type": "where", "predicate": predicate}

def step_rename(mapping):
    return {"__kind": "pipeline_step", "type": "rename", "mapping": mapping}

def step_mutate(updates):
    return {"__kind": "pipeline_step", "type": "mutate", "updates": updates}

def step_select(fields):
    return {"__kind": "pipeline_step", "type": "select", "fields": fields}

def step_reorder(fields):
    return {"__kind": "pipeline_step", "type": "reorder", "fields": fields}

def step_sort(field, order = "asc"):
    return {"__kind": "pipeline_step", "type": "sort_by", "field": field, "order": order}

def step_limit(n):
    return {"__kind": "pipeline_step", "type": "limit", "n": n}

def _pipe_apply(rowset, step):
    if type(step) != "dict" or step.get("__kind") != "pipeline_step":
        return rowset
    step_type = step.get("type")
    if step_type == "where":
        return where(rowset, step.get("predicate"))
    if step_type == "rename":
        return rename(rowset, step.get("mapping"))
    if step_type == "mutate":
        return mutate(rowset, step.get("updates"))
    if step_type == "select":
        return select(rowset, step.get("fields"))
    if step_type == "reorder":
        return reorder(rowset, step.get("fields"))
    if step_type == "sort_by":
        return sort_by(rowset, step.get("field"), order = step.get("order", "asc"))
    if step_type == "limit":
        return limit(rowset, step.get("n"))
    return rowset

def pipe(seed, *steps):
    current = seed
    for step in steps:
        current = _pipe_apply(current, step)
    return current

def limit(rowset, n):
    return _analysis("limit", rowset = rowset, n = n)

def table_rows(rowset, fields = None, sort = None, order = "asc", take = None):
    table_rowset = rowset
    if fields != None:
        table_rowset = select(table_rowset, fields)
    if sort != None:
        table_rowset = sort_by(table_rowset, sort, order = order)
    if take != None:
        table_rowset = limit(table_rowset, take)
    return _analysis("table_rows", rowset = table_rowset)

def latest_days(rowset, field, days):
    return _analysis("latest_days", rowset = rowset, field = field, days = days)

def latest_months(rowset, field, months):
    return _analysis("latest_months", rowset = rowset, field = field, months = months)

def lookup_value(rowset, field, lookup_rowset, lookup_field, value_field, as_field):
    return _analysis("lookup_value", rowset = rowset, field = field, lookup_rowset = lookup_rowset, lookup_field = lookup_field, value_field = value_field, as_field = as_field)

def eq(field, value):
    return _analysis("eq", field = field, value = value)

def ne(field, value):
    return _analysis("ne", field = field, value = value)

def gt(field, value):
    return _analysis("gt", field = field, value = value)

def gte(field, value):
    return _analysis("gte", field = field, value = value)

def lt(field, value):
    return _analysis("lt", field = field, value = value)

def lte(field, value):
    return _analysis("lte", field = field, value = value)

def field_gt(left_field, right_field):
    return _analysis("field_gt", left_field = left_field, right_field = right_field)

def field_gte(left_field, right_field):
    return _analysis("field_gte", left_field = left_field, right_field = right_field)

def field_lt(left_field, right_field):
    return _analysis("field_lt", left_field = left_field, right_field = right_field)

def field_lte(left_field, right_field):
    return _analysis("field_lte", left_field = left_field, right_field = right_field)

def sub(left_field, right_field):
    return _analysis("sub", left_field = left_field, right_field = right_field)

def between(field, lower, upper):
    return _analysis("between", field = field, lower = lower, upper = upper)

def in_values(field, values):
    return _analysis("in_values", field = field, values = values)

def is_verified(field = "是否查实"):
    """Spreadsheet variants that mean verified (yes) for 是否查实-style columns."""
    return in_values(field, ["是", "查实", "已查实"])

def is_yes(field):
    """Spreadsheet yes cells: 是、是（2）等（与 Neverland yes_indicator 一致）。"""
    return and_(not_empty(field), contains(field, "是"))

def has_party_gov_sanction(field = "处理处分"):
    """党纪政务处分：处理处分含「第二种」形态（一个处理结果ID对应一人）。"""
    return contains(field, "第二种")

def not_empty(field):
    return _analysis("not_empty", field = field)

def present(field):
    """非空且非 —、无、待定 等占位（与 Neverland DrillFieldValue.present? 一致）。"""
    return _analysis("present", field = field)

def blank(field):
    return _analysis("blank", field = field)

def placeholder_only(field):
    return _analysis("placeholder_only", field = field)

def not_placeholder_text(field):
    """职务等字段：有内容且非 —— 等纯横线占位。"""
    return and_(present(field), not_(placeholder_only(field)))

def contains(field, value):
    return _analysis("contains", field = field, value = value)

def matches(field, pattern):
    return _analysis("matches", field = field, pattern = pattern)

def and_(*predicates):
    return _analysis("and", predicates = predicates)

def or_(*predicates):
    return _analysis("or", predicates = predicates)

def not_(predicate):
    return _analysis("not", predicate = predicate)

def number(source, field = None):
    return _analysis("number", source = source, field = field)

def text(source, field = None):
    return _analysis("text", source = source, field = field)

def date(source, field = None):
    return _analysis("date", source = source, field = field)

def extract_number(source = None, field = None, pattern = None):
    return _analysis("extract_number", source = source, field = field, pattern = pattern)

def split_text(rowset, field, delimiter = "、"):
    return _analysis("split_text", rowset = rowset, field = field, delimiter = delimiter)

def avg(value, fallback = 0):
    return _analysis("avg", value = value, fallback = fallback)

def min(value, fallback = 0):
    return _analysis("min", value = value, fallback = fallback)

def max(value, fallback = 0):
    return _analysis("max", value = value, fallback = fallback)

def median(value, fallback = 0):
    return _analysis("median", value = value, fallback = fallback)

def unique_count(value, fallback = 0):
    return _analysis("unique_count", value = value, fallback = fallback)

def item_count(value, fallback = 0):
    return _analysis("item_count", value = value, fallback = fallback)

def group_by(rowset, fields = None, by = None, universe = None, value = None, agg = "count"):
    resolved = fields
    if resolved == None and by != None:
        resolved = [by]
    return _analysis("group_by", rowset = rowset, fields = resolved, by = by, universe = universe, value = value, agg = agg)

def agg(grouped, metrics = [], sort = None, order = "desc", limit = None):
    return _analysis("agg", grouped = grouped, metrics = metrics, sort = sort, order = order, limit = limit)

def bucket_date(rowset, field, by = "month"):
    return _analysis("bucket_date", rowset = rowset, field = field, by = by)

def year_between(rowset, field, year):
    year_text = str(year)
    return where(rowset, between(field, year_text + "-01-01", year_text + "-12-31"))

def year_count(rowset, field, year):
    return count(year_between(rowset, field, year))

def year_sum(rowset, date_field, value_field, year):
    return sum(number(year_between(rowset, date_field, year), value_field))

def party_year_aggregate(rowset, party_field, date_field, value_field, years):
    return _analysis(
        "party_year_aggregate",
        rowset = rowset,
        party_field = party_field,
        date_field = date_field,
        value_field = value_field,
        years = years,
    )

def unpivot_columns(rowset, id_field, columns, year_field = "year", value_field = "value"):
    return _analysis(
        "unpivot_columns",
        rowset = rowset,
        id_field = id_field,
        columns = columns,
        year_field = year_field,
        value_field = value_field,
    )

def change_rate(current, base, mode = "growth", scale = 100):
    return _analysis("change_rate", current = current, base = base, mode = mode, scale = scale)

def trend_year_compare(rowset, date_field, value = None, agg = "count", limit = 6, years = None, month_label_field = "month", year_label_field = "year"):
    return _analysis(
        "trend_year_compare",
        rowset = rowset,
        date_field = date_field,
        value = value,
        agg = agg,
        limit = limit,
        years = years if years != None else [2024, 2025],
        month_label_field = month_label_field,
        year_label_field = year_label_field,
    )

def trend(rowset = None, date_field = None, value = None, by = "month", agg = "count", order = "asc", limit = None, id = None, label = None, source = None, grain = None):
    if rowset == None and (source != None or id != None or label != None or grain != None):
        return _explain_item(
            "trend",
            id = id if id != None else "trend",
            label = label,
            source = source,
            date_field = date_field,
            grain = grain if grain != None else by,
        )
    return _analysis("trend", rowset = rowset, date_field = date_field, value = value, by = by, agg = agg, order = order, limit = limit)

def mom(series, value_field = "value", label_field = "label", as_ = "mom"):
    return _analysis("mom", series = series, value_field = value_field, label_field = label_field, as_ = as_)

def yoy(series, value_field = "value", label_field = "label", as_ = "yoy"):
    return _analysis("yoy", series = series, value_field = value_field, label_field = label_field, as_ = as_)

def count_rows(fallback = 0):
    return _analysis("count", fallback = fallback)

def count_where_in_any(paths, values, fallback = 0):
    predicates = []
    for path in paths:
        predicates.append(in_values(path, values))
    return _analysis("count", rowset = where(_analysis("current_rows"), or_(*predicates)), fallback = fallback)

def sum_numeric_field(field = None, fields = None, fallback = 0):
    if fields != None and len(fields) > 0:
        return _analysis("sum_first_number", fields = fields, fallback = fallback)
    return _analysis("sum", value = number(_analysis("current_rows"), field), fallback = fallback)

def unique_count_by_field(field, fallback = 0):
    return _analysis("unique_count", value = text(_analysis("current_rows"), field), fallback = fallback)

def percent_of_eq(field, eq, fallback = 0):
    return _analysis("percent", rowset = _analysis("current_rows"), predicate = _analysis("eq", field = field, value = eq), fallback = fallback)

def sum_metric_refs(refs, fallback = 0):
    return {
        "type": "sum_metric_refs",
        "refs": refs,
        "fallback": fallback,
    }

def sum_rowset_counts(rowsets, fallback = 0):
    """Sum `count(*)` across multiple rowsets (e.g. cross-dataset cockpit totals)."""
    return _analysis("sum_rowset_counts", rowsets = rowsets, fallback = fallback)

def dedupe_first_count_eq(dedupe_field, field, eq, fallback = 0):
    return count(where(first_by(_analysis("current_rows"), dedupe_field), _analysis("eq", field = field, value = eq)), fallback = fallback)

def dedupe_first_count_nonempty(dedupe_field, field, fallback = 0):
    return count(where(first_by(_analysis("current_rows"), dedupe_field), not_empty(field)), fallback = fallback)

def dedupe_first_sum_numeric_loose(dedupe_field, field, fallback = 0):
    return sum(number(first_by(_analysis("current_rows"), dedupe_field), field), fallback = fallback)

def dedupe_first_count_split_items(dedupe_field, field, delimiter, fallback = 0):
    return item_count(split_text(first_by(_analysis("current_rows"), dedupe_field), field, delimiter = delimiter), fallback = fallback)

def dedupe_first_percent_eq(dedupe_field, field, eq, fallback = 0):
    return percent(first_by(_analysis("current_rows"), dedupe_field), _analysis("eq", field = field, value = eq), fallback = fallback)

def dedupe_first_sum_morph_people_in_text(dedupe_field, field, fallback = 0):
    return sum(extract_number(first_by(_analysis("current_rows"), dedupe_field), field, pattern = "种形态[\\s\\S]{0,64}?(\\d+)\\s*人"), fallback = fallback)

def metric(id = None, key = None, label = None, value = None, unit = None, where = None, drilldown = None, explain = None, analyses = None, binding = None):
    metric_id = id if id != None else key
    return _without_empty({
        "__kind": "metric",
        "key": metric_id,
        "label": label,
        "value": value,
        "unit": unit,
        "where": where,
        "drilldown": drilldown,
        "explain": explain,
        "analyses": analyses,
        "binding": binding,
    })

def scalar_map(id = None, key = None, label = None, values = None, unit = None, value_format = None, schema = None, note = None, basis_refs = None, recommended_dimensions = None, drilldown = None, explain = None, analyses = None, binding = None):
    return _data_product("scalar_map", id = id, key = key, label = label, values = values, unit = unit, value_format = value_format, schema = schema, note = note, basis_refs = basis_refs, recommended_dimensions = recommended_dimensions, drilldown = drilldown, explain = explain, analyses = analyses, binding = binding)

def dataframe(id = None, key = None, label = None, value = None, unit = None, schema = None, note = None, basis_refs = None, recommended_dimensions = None, drilldown = None, explain = None, analyses = None, binding = None):
    return _data_product("dataframe", id = id, key = key, label = label, value = value, unit = unit, schema = schema, note = note, basis_refs = basis_refs, recommended_dimensions = recommended_dimensions, drilldown = drilldown, explain = explain, analyses = analyses, binding = binding)

def count(id = None, label = None, unit = None, where = None, drilldown = None, fallback = 0, explain = None, analyses = None):
    if _is_analysis(id):
        return _analysis("count", rowset = id, fallback = fallback)
    value = _expr("count(*)")
    if id == None:
        return value
    return scalar_map(id = id, label = label, values = {"value": value}, unit = unit, drilldown = drilldown, explain = explain, analyses = analyses)

def sum(field, fallback = 0):
    if _is_analysis(field):
        return _analysis("sum", value = field, fallback = fallback)
    return _expr("sum(" + field + ")")

def ratio(numerator = None, denominator = None, fallback = 0, id = None, label = None, formula = None):
    if formula != None or id != None or label != None:
        return numerator_denominator(
            id = id if id != None else "numerator_denominator",
            label = label,
            numerator = numerator,
            denominator = denominator,
            formula = formula,
        )
    if _is_analysis(numerator) or _is_analysis(denominator):
        return _analysis("ratio", numerator = numerator, denominator = denominator, fallback = fallback)
    return _expr("ratio(" + numerator + ", " + denominator + ")")

def percent(expr, predicate = None, fallback = 0):
    if _is_analysis(expr):
        return _analysis("percent", rowset = expr, predicate = predicate, fallback = fallback)
    return _expr("percent(" + _expr_source(expr) + ")")

def month(field):
    return _expr("month(" + field + ")")

def last_days(field, days):
    return _expr(field + " >= max(" + field + ") - " + str(days) + "d")

def list_by(by, value, as_ = None, requires = None):
    return {
        "__expr": "list",
        "list": _without_empty({
            "by": by,
            "value": _expr_source(value),
            "as": as_,
        }),
        "requires": requires,
    }
