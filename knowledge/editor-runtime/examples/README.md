# Author Example Pack

This directory is the public authoring example pack for standalone workspaces.

## Reading order

Start with the example that matches the contract you need to prove, not the largest example.

| File | What it proves | Best starting point for |
|------|----------------|-------------------------|
| `dataset-baseline.mei` | `app + scene`, dataset resource, `dataset.table`, `dataset.summary-cards` | A minimal data app |
| `filter-reactivity.mei` | `dataset.filter-bar` + shared `query_state` + linked table/chart | Filter-driven pages |
| `chart-baseline.mei` | `chart.*` common contract (`data` + `mapping`) | Basic chart scenes |
| `data-table-runtime.mei` | `cockpit.data-table` runtime/table skin contract | Cockpit table panels |
| `cockpit-panel.mei` | `panel(base = panel_ref(...))` with cockpit shell templates | Cockpit shells |
| `template-clone.mei` | `metric_card(base = metric_card_ref(...))` | Template clone / card reuse |
| `sim-baseline.mei` | `sim.scene` with `scene_ref("self")` | Simulation scenes |
| `map-baseline.mei` | `map.maplibre` + `mapSpec` | GIS map panels |

## Rules

- These examples are curated proof points, not a dump of historical workspace examples.
- Use them to confirm the public contract first.
- If an example still leaves a gap, read the matching component/template guide before reading implementation code.
- Do not mechanically copy optional props that your target scene does not need.

## Supporting data

The `data/` subdirectory contains tiny CSV fixtures for the dataset/chart examples so the pack can be copied into a standalone workspace without reusing source-repo example data.
