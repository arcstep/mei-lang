# Cockpit Template Index

This guide is the standalone-friendly public summary for `.stock/templates/cockpit/`.

## What this pack is for

The cockpit template pack is the public shell/template layer for cockpit-style screens.

- Use templates for panel shells, metric-card shells, and GIS map shells.
- Use cockpit components for renderer skin or interactive behavior inside those shells.
- Use chart/dataset/map components for data presentation inside the shell.

## Main entry paths

- `.stock/templates/cockpit/panel/panel-titled-shell.mei`
- `.stock/templates/cockpit/panel/metrics-auto-body.mei`
- `.stock/templates/cockpit/panel/panel-gis-map-fullscreen.mei`
- `.stock/templates/cockpit/metric-card/metric-card-narrow-stack.mei`
- `.stock/templates/cockpit/metric-card/metric-card-solid-row.mei`
- `.stock/templates/cockpit/metric-card/metric-card-solid-stack.mei`
- `.stock/templates/cockpit/metric-card/metric-card-plain.mei`
- `.stock/templates/cockpit/metric-card/metric-wide-compound.mei`

## Public authoring patterns

### Panel shell clone

```mei
COCKPIT_TPL = ".stock/templates/cockpit"
SHELL = COCKPIT_TPL + "/panel/panel-titled-shell.mei"

panel(
    base = panel_ref(id = "titled_shell", scene_file = SHELL),
    id = "summary_panel",
    title = "业务概览",
)
```

### Metric-card shell clone

```mei
COCKPIT_TPL = ".stock/templates/cockpit"
CARD = COCKPIT_TPL + "/metric-card/metric-card-narrow-stack.mei"

metric_card(
    base = metric_card_ref(id = "card_normal", scene_file = CARD),
    id = "metric_sales",
    source = {"label": "销售额", "value": "120", "unit": "万"},
)
```

## GIS shell contract

The GIS shell path is:

- `.stock/templates/cockpit/panel/panel-gis-map-fullscreen.mei`

Its public contract assumes:

- the shell hosts `map.maplibre`
- the main data contract comes from `mapSpec`
- `mapViewport.focusInset` controls the visual focus window for overlay panels

## Recommended examples

- `examples/cockpit-panel.mei`
- `examples/template-clone.mei`
- `examples/map-baseline.mei`
