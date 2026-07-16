# Workspace Bootstrap, Config, And Theme

This guide is the standalone-friendly public reference for creating a new workspace, creating a new app, and understanding the two JSON config files that AI tools should read before guessing paths.

## Bootstrap a source workspace

Prefer the one-command bootstrap path:

```bash
mei-toolchain workspace bootstrap --source-root /path/to/workspace --app hello --tool cursor --json
```

This creates a new source workspace and immediately installs a local runtime:

- `.mei-workspace.json`
- `stock/`（components、templates；Git 真源。`authoring` 已退役）
- workspace-local `.mei/` runtime metadata
- `.mei/runtime/bin/mei-toolchain`
- `.mei/runtime/bin/mei-lsp`
- `.mei/runtime/bin/mei-host-shell`
- MCP adapters and packaged knowledge
- tool glue such as `.cursor/rules/` and `.cursor/mcp.json`
- optional minimal app skeleton

Git 真源仍然是源码工作区本身；`.mei/` 只是安装后的消费面，不是默认提交对象。

If you need the staged flow, use:

```bash
mei-toolchain workspace init --standalone --source-root /path/to/workspace --materialize --json
mei-toolchain workspace runtime install --source-root /path/to/workspace --json
mei-toolchain editor-runtime scaffold --target-root /path/to/workspace --tool cursor --json
mei-toolchain workspace create-app hello --source-root /path/to/workspace --json
```

- `workspace init` creates the workspace root, `workspace.json`, `stock/`, and optional runtime scaffolding.
- `workspace runtime install` writes the workspace-local runtime metadata, packaged docs, local binaries under `.mei/runtime/bin/`, and a workspace-root `./start.sh` launcher.
- `editor-runtime scaffold` writes tool glue only. It should not replace runtime metadata or host-local state.

## Restore runtime in an existing source workspace

If the workspace already exists in Git, install or refresh `.mei/` locally instead of re-running bootstrap:

```bash
mei-toolchain workspace runtime install --source-root /path/to/workspace --force --json
mei-toolchain workspace runtime update --source-root /path/to/workspace --json
```

## Start the workspace host

After bootstrap or `workspace runtime install`, launch the browser host from the workspace root:

```bash
./start.sh
```

Defaults to **http://127.0.0.1:9527**. The script runs the workspace-local `.mei/runtime/bin/mei-host-shell` by default. `MEI_HOST_WEB_BIN` or PATH `mei-host-shell` are recovery overrides, not the normal source-workspace flow.

Optional flags are forwarded to `mei-host-shell serve`, for example:

```bash
./start.sh --auth
./start.sh --host-surface access-only
```

## Create a new app

Create a new app after the workspace exists:

```bash
mei-toolchain workspace create-app my-app --source-root /path/to/workspace --json
```

The current gold scaffold is:

```text
my-app/
  app.toml                 # App root (title / default_stage)
  src/
    stage/home.stage.mdx   # Stage Registry
    scene/
      home.mei             # scene(...) entry
      home/t1/plane.mei    # plane_layout(...)
```

Classic catalog / legacy scaffolds may still use `src/main.mei` or a flat `main.mei`; product gold apps do **not** require `main.mei`.

Recommended next steps:

1. Keep `app.toml` as the app root marker (plus Stage MDX).
2. Author scene structure under `src/scene/` (`home.mei`, planes, regions, sections).
3. Prefer `plane_ref` / `panel_ref` / `theme_ref` over legacy `world` / `frame` helpers.
4. Keep app-specific `ops.*` and upload/prototype paths in app-local config when needed.

## Config boundary

Use the two JSON files for different concerns:

| File | Scope | What belongs here |
|------|-------|-------------------|
| `workspace.json` | workspace root | workspace id/label, **`stock`** catalog filters, stock paths, discover rules, menu, runtime file cache, compliance, **`ops.shellTheme`**, **`ops.themes`** (shell chrome only) |
| `<app>/.mei-config.json` | app root | app entry, app-local paths, host feature flags, `ops.themes` (scene / `theme_ref`), `ops.sources`, `ops.basemaps`, `ops.params` |

Do not treat them as interchangeable.

## `workspace.json` and `stock`

Minimal shape (schemaVersion 2):

```json
{
  "schemaVersion": 2,
  "workspace": {
    "id": "ws-demo",
    "label": "Demo Workspace"
  },
  "paths": {
    "stock": "stock",
    "components": "stock/components",
    "templates": "stock/templates",
    "authoring": "stock/authoring"
  },
  "stock": {
    "bootstrap": { "source": "platform-default", "refresh": true },
    "catalog": {
      "components": { "enabled": true, "exclude": [] },
      "templates": { "enabled": true, "exclude": ["**/assets/**"] },
      "authoring": { "enabled": true, "exclude": [] }
    },
    "preview": {
      "workspaceOnly": true,
      "contracts": "stock/authoring/component-contracts.json"
    },
    "sources": []
  }
}
```

- **`bootstrap.refresh`**: when `true` (default), startup/prebuild re-copies missing or newer platform stock files into the workspace. Set `false` for **workspace-owned** stock (deleted examples/previews stay deleted). Explicit `mei-toolchain workspace stock sync` still force-copies.
- **`preview.workspaceOnly`**: when true, Build preview reads only workspace stock (no package fallback).
- **Build component preview**: `{paths.components}/{pack}/previews/{use_key}.mei` beside each manifest (see host doc 87). Optional for gold-sample workspaces that view components via apps instead.
- **`preview.contracts`**: optional Agent/LSP overlay for tutorial `preferred_example_ids` — not the Build preview router. Use `null` when unused.

Stock maintenance commands:

```bash
mei-toolchain workspace stock sync --source-root /path/to/workspace [--force] --json
mei-toolchain workspace stock doctor --source-root /path/to/workspace --json
mei-toolchain workspace stock migrate-paths --source-root /path/to/workspace --json
```

See `docs/archive/mei-lang-v1/implementation/host/87-workspace-stock-ssot-and-catalog.md` for SSOT invariants.

## Legacy `.mei-workspace.json` note

Older workspaces may still use `.mei-workspace.json` and `.stock/` paths. Run `workspace stock migrate-paths` and prefer `workspace.json` + `stock/` layout.

## Config fields (workspace root)

Common fields:

- `workspace.id`, `workspace.label`, `workspace.deployHost`, `workspace.defaultApp`（登录后访问 `/` 时的默认应用 id；须为 discover 到的 app 或 `discover.appAliases` 别名；不可编译时回退到首个可编译 app）
- `discover.skip_directories`, `discover.appAliases`
- `menu`
- `runtime.file_cache`
- `compliance`
- `ops.shellTheme` — id of workspace shell theme (host chrome)
- `ops.themes` — shell theme definitions (e.g. `host`); injected on `<body>`

Shell theme shape (see `docs/archive/mei-lang-v1/topics/theme-token-contract.md`):

```json
{
  "schemaVersion": 1,
  "workspace": { "id": "ws-demo", "label": "Demo Workspace" },
  "ops": {
    "shellTheme": "host",
    "themes": {
      "host": {
        "font": { "1": "11px", "2": "13px", "3": "15px", "4": "18px" },
        "tokens": {
          "shell": {
            "bg": "radial-gradient(circle at top, #1a2a41 0%, #080d16 68%)",
            "text": "#dbe8f6",
            "stage": "linear-gradient(180deg, rgba(20,31,47,.56), rgba(8,13,21,.26))",
            "stage_border": "rgba(124,145,173,.12)",
            "chrome_top_bg": "linear-gradient(180deg, rgba(18,32,51,.97), rgba(9,18,30,.97))",
            "chrome_bottom_bg": "linear-gradient(180deg, rgba(8,15,25,.97), rgba(5,10,18,.98))",
            "chrome_border_top": "rgba(96,165,250,.24)",
            "chrome_border_bottom": "rgba(45,212,191,.22)",
            "family_ui": "\"Microsoft YaHei\", \"PingFang SC\", sans-serif"
          },
          "color": {
            "text_primary": "#e2e8f0",
            "text_muted": "#94a3b8",
            "text_body": "#cbd5e1",
            "text_inverse": "#f8fafc",
            "panel_bg": "rgba(2,6,23,.38)",
            "border_default": "rgba(96,165,250,.16)",
            "border_nav": "rgba(100,116,139,.32)",
            "manage_panel_bg": "linear-gradient(180deg, rgba(12,18,31,.92), rgba(2,6,23,.76))",
            "host_page_bg": "linear-gradient(180deg, #0b1220 0%, #070d18 100%)",
            "watermark": "rgba(251,113,133,.11)"
          }
        }
      }
    }
  }
}
```

Shell `tokens.color` keys must be semantic `snake_case` names. Do not use `literal_*` or hash keys; run `node scripts/lint-workspace-shell-theme.mjs` in `mei-lang`.

Authoring rule:

- Put workspace-wide discover/menu/runtime defaults here.
- Put **host shell** fonts/colors in `ops.themes` + `ops.shellTheme`.
- App scene themes (`theme_ref`) stay in `<app>/.mei-config.json` only.

## `.mei-config.json`

Minimal shape:

```json
{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" }
}
```

Ops-aware shape:

```json
{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" },
  "paths": {
    "upload": "upload",
    "prototype": "prototype"
  },
  "ops": {
    "themes": {
      "cockpit_dark": {
        "palette": {
          "background": "#08121f",
          "text": "#e7f5ff"
        }
      }
    },
    "sources": {
      "uploaded_sales": {
        "kind": "xlsx",
        "path": "upload/sales.xlsx",
        "sheet": "Sheet1",
        "header_row": 1
      }
    }
  }
}
```

Common fields:

- `entry.main`
- `paths.upload`, `paths.prototype`
- `features.aiChat`
- `ops.themes`, `ops.sources`, `ops.basemaps`, `ops.params`

Authoring rule:

- App-local sources, theme refs, basemap refs, and ops params live here.
- Legacy workspace fields in `.mei-config.json` are compatibility-only and should not be used for new work.
- Host auth state belongs in `.mei/local/hosts/*.state.json`, not in publish-safe Git config.

## Theme selection

There are two public theme paths:

### Built-in scene preset

Use a literal preset when you only need the stock page/cockpit/game look:

```mei
scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
    theme = "cockpit",
)
```

Use this when:

- the app only needs a built-in visual preset
- there is no app-local theme registry yet

### `theme_ref(...)`

Use `theme_ref(...)` when the theme should come from app config:

```mei
scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
    theme = theme_ref("cockpit_dark"),
)
```

The referenced value must exist in `.mei-config.json -> ops.themes`.

Use this when:

- ops or deployment should control theme tokens
- multiple named themes must be switchable
- the theme is part of the app contract instead of a built-in preset

### Theme token rules

Shell and scene use **separate** token tracks (see `docs/archive/mei-lang-v1/topics/29-shell-scene-theme-separation.md`):

- **Shell** (workspace `ops.themes[shellTheme]`): literals in `tokens.shell`, `tokens.color`, `font` → `--mei-shell-*`, `--mei-shell-color-*`, `--mei-shell-font-*` on `<body>`.
- **Scene** (app `ops.themes` + `theme_ref`): literals in `theme.tokens.color`, `tokens.gradient`, `theme.font` → `--mei-color-*`, `--mei-font-*` on preview viewport only.

**Definition layer** only: literal `#hex`, `rgba()`, and `Npx`.

**Reference layer** (scene/frame/panel props): semantic names only — `color = "text_primary"`, `font = "3"`. Literals compile as errors.

**Consumption layer** (CSS/JS): `var(--mei-*)` only; no `var(--mei-*, #hex)` fallbacks.

Required token keys: `docs/archive/mei-lang-v1/topics/theme-token-contract.md`.

## Source refs and upload-backed datasets

Prefer `source_ref(...)` when the data source should come from `.mei-config.json` instead of a hard-coded path:

```mei
resource(
    id = "sales_data",
    kind = "dataset",
    source = source_ref("uploaded_sales"),
)
```

The referenced source lives in `.mei-config.json -> ops.sources`.

Use this pattern when:

- the dataset comes from `upload/*.csv` or `upload/*.xlsx`
- ops should switch the source without editing `.mei`
- the authoring task must stay portable across environments

## AI reading order

When the task involves bootstrapping, config, upload paths, or themes, read in this order:

1. `.mei-workspace.json`
2. `<app>/.mei-config.json`
3. `.mei/profiles/author.md`
4. `.mei/skills/meilang-author/SKILL.md`
5. this guide
6. the nearest example and diagnostics

Only fall back to source-tree docs or implementation when the public config contract still does not answer the task.
