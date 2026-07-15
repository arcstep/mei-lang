# Cockpit Components

This guide is the standalone-friendly public summary for the `cockpit` pack.

## Public components

- `cockpit.header-brand`
- `cockpit.data-table`
- `cockpit.donut-trio`
- `cockpit.panel-title`
- `cockpit.metric-progress`

## Pack boundary

The cockpit pack is for cockpit-specific presentation and runtime chrome.

- Reusable shells live in `.stock/templates/cockpit/`
- Cockpit components provide the visuals or renderer skin
- Shared table/query behavior still comes from the dataset runtime core
- Complex shell semantics should come from `template-contracts.json`, not from guessing template internals

## Common usage

### `cockpit.header-brand`

Use for full-width or shell header branding.

Common props:

- `title`
- `assets`
- `titleColor`

### `cockpit.data-table`

Use when the table should look and behave like a cockpit panel.

Common props:

- `dataset`
- `embedded`
- `layoutPreset`
- `column_state`
- `columnFormats`
- `columnRules`
- `carousel`

### `cockpit.donut-trio`

Use for grouped numerator/denominator ratios.

Common props:

- `totalMetric`
- `numerMetric`
- `groupField`
- `limit`

## Authoring rules

- Prefer template shells for layout and skins, then place cockpit components inside those shells.
- Do not treat `cockpit.data-table` as a separate data model. It still consumes the same dataset/metric contract.
- When a screen only needs shell reuse, prefer `panel_ref(...)` / `metric_card_ref(...)` over inventing a cockpit-only DSL.
- For nested metric rows or compound shells, prefer the public cockpit template pack before reading `.stock/templates/**/README.md`.

## Recommended examples

- `examples/cockpit-panel.mei`
- `examples/template-clone.mei`
- `examples/data-table-runtime.mei`
