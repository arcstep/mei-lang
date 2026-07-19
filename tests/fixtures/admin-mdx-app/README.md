# Admin MDX CI fixture

Convention-only Admin discovery (`src/admin/**/*.admin.mdx`), no `[admin].manifest`.

- `organization` — `record_path` + `apply_policy=hot`
- `theme` — `config_path=ops.themes.cockpit` + `apply_policy=hot`（覆盖 config-path 读写）

Design SSOT: `docs/mei-lang-v2/05-host/0545`.
