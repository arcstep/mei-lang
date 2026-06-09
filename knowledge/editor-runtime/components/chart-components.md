# Chart Components

This guide is the standalone-friendly public summary for the `chart` pack.

## What is stable

- `chart.*` is the public chart component family.
- The common public path is `props.data + props.mapping`.
- `props.data` should come from a current-scene `dataset_ref(...)` or `metric_ref(...)`.
- `props.mapping` describes axes/labels/groups; it is component contract, not a separate DSL function.

## Common variants

- `chart.line`, `chart.area`, `chart.trend`: 1D time or sequence trends
- `chart.column`, `chart.bar`, `chart.bar-mini`: comparisons and rankings
- `chart.pie`, `chart.donut`, `chart.rose`: composition
- `chart.scatter`: x/y/size/color projections
- `chart.radar`: multi-dimension comparison
- `chart.boxplot`: distribution summaries
- `chart.geo`: map-like thematic overlays
- `chart.ranking`: ranking bars with `showBackground` and `rankingLayout`

## Minimal contract

- `title`: optional chart title
- `data`: required dataset/dataframe input
- `mapping`: optional but usually recommended
- `query_state`: optional shared runtime filter channel

## Authoring rules

- Prefer current-scene data ids first; do not pass cross-file dataset/metric locators directly into chart props.
- For reusable chart layouts, keep the data contract stable and change only the mapping or title.
- If you need chart-specific knobs beyond the common contract, check `component-contracts.json` and then the nearest verified example before reading implementation code.
- If the chart should react to upload-backed or ops-managed data sources, resolve them in the world layer first through `source_ref(...)` and then pass the resulting `dataset_ref(...)`.

## Recommended examples

- `examples/chart-baseline.mei`
- `examples/filter-reactivity.mei`
