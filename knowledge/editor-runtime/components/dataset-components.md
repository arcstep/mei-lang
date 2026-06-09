# Dataset Components

This guide is the standalone-friendly public summary for the `dataset` pack.

## Public components

- `dataset.table`: manage-oriented data table
- `dataset.filter-bar`: named query-state filter UI
- `dataset.summary-cards`: summary cards around dataset or metric results

## Shared rules

- Dataset-facing components should consume current-scene `dataset_ref(...)` / `metric_ref(...)`.
- `query_state` is the public way to coordinate filter UI, tables, cards, and charts.
- Shared formatting belongs in `columnFormats` / `columnRules`, not in per-renderer ad hoc props.

## `dataset.table`

Prefer these props first:

- `data`
- `query_state`
- `headers`
- `columns`
- `column_state`
- `columnFormats`
- `columnRules`

Use `dataset.table` when you want a manage-style table with toolbar, sorting, and optional runtime paging.

## `dataset.filter-bar`

Prefer these props first:

- `title`
- `query_state`
- `fields`

`dataset.filter-bar` writes a named query-state. It does not own the table truth by itself.

## `dataset.summary-cards`

Prefer these props first:

- `value`
- `query_state`

The preferred public path is a scalar `metric_ref(...)`; rowset/dataset fallback remains available but is less explicit.

## Recommended examples

- `examples/dataset-baseline.mei`
- `examples/filter-reactivity.mei`
- `examples/data-table-runtime.mei`
