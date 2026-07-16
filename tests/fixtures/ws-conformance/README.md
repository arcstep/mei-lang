# ws-conformance

In-repo platform Golden Case workspace for public `mei-lang` CI.

- `apps/fx-structure` — T1 + T2 catalog / scene examples
- `apps/fx-data` — small CSV + one metric bundle + warmup
- `apps/fx-diag-*` — negative diagnostics (`link_decl_*` / `grid_track_unresolved` / `unknown_component` / `warmup_focus_not_found` / `row_drilldown_filter_key_mismatch`)

Do not copy private `ws-demo-v2` apps here. See `docs/mei-lang-v2/07-workspace-lifecycle/0710-mei-lang-conformance-fixtures.md`.

`stock/` is materialized at test time from `mei-lang/stock` (not committed).
