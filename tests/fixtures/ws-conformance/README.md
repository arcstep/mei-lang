# ws-conformance

In-repo platform Golden Case workspace for public `mei-lang` CI.

- `apps/fx-structure` — T1 + T2 catalog / scene examples
- `apps/fx-data` — small CSV + one metric bundle + warmup
- `apps/fx-admin-mei` — Admin v2 MDX entry + shared scene/data provider binding
- `apps/fx-deck-minimal` — two-page slides Stage
- `apps/fx-dual-stage` — cockpit + slides Stage Registry
- `apps/fx-narration-journey` — current compiled Stage MDX narration (`@step`)
- `apps/fx-page-report` — page-profile document Stage
- `apps/fx-diag-*` — negative diagnostics (`link_decl_*` / `grid_track_unresolved` / `unknown_component` / `warmup_focus_not_found` / `row_drilldown_filter_key_mismatch`)
- `apps/fx-diag-admin-*` — one-violation Admin v2 diagnostics

Do not copy private product workspaces here.

`stock/` is materialized at test time from `mei-lang/stock` (not committed).
