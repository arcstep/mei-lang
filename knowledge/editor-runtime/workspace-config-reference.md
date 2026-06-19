# Workspace Bootstrap, Config, And Theme

This guide is the standalone-friendly public reference for creating a new workspace, creating a new app, and understanding the two JSON config files that AI tools should read before guessing paths.

## Bootstrap a source workspace

Prefer the one-command bootstrap path:

```bash
mei-toolchain workspace bootstrap --source-root /path/to/workspace --app hello --tool cursor --json
```

This creates a new source workspace and immediately installs a local runtime:

- `.mei-workspace.json`
- `.stock/`
- workspace-local `.mei/` runtime metadata
- `.mei/runtime/bin/mei-toolchain`
- `.mei/runtime/bin/mei-lsp`
- `.mei/runtime/bin/mei-host-web`
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

- `workspace init` creates the workspace root, `.mei-workspace.json`, `.mei/`, and optional `.stock/`.
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

Defaults to **http://127.0.0.1:9527**. The script runs the workspace-local `.mei/runtime/bin/mei-host-web` by default. `MEI_HOST_WEB_BIN` or PATH `mei-host-web` are recovery overrides, not the normal source-workspace flow.

Optional flags are forwarded to `mei-host-web serve`, for example:

```bash
./start.sh --auth
./start.sh --host-surface access-only
```

## Create a new app

Create a new app after the workspace exists:

```bash
mei-toolchain workspace create-app my-app --source-root /path/to/workspace --json
```

The current scaffold writes this minimal file layout:

```text
my-app/
  main.mei
  .mei-config.json
  scenes/
    home.mei
```

Recommended next steps:

1. Keep `main.mei` as the app entry.
2. Keep `scenes/` for additional scene files.
3. Expand from `scene_ref(...)` and `app_add_scene(...)` when the app grows beyond one scene.
4. Keep app-specific `ops.*` and upload/prototype paths in the app-local `.mei-config.json`.

## Config boundary

Use the two JSON files for different concerns:

| File | Scope | What belongs here |
|------|-------|-------------------|
| `.mei-workspace.json` | workspace root | workspace id/label, stock paths, discover rules, menu, runtime file cache, compliance |
| `<app>/.mei-config.json` | app root | app entry, app-local paths, host feature flags, `ops.themes`, `ops.sources`, `ops.basemaps`, `ops.params` |

Do not treat them as interchangeable.

## `.mei-workspace.json`

Minimal shape:

```json
{
  "schemaVersion": 1,
  "workspace": {
    "id": "ws-demo",
    "label": "Demo Workspace"
  },
  "paths": {
    "components": ".stock/components",
    "templates": ".stock/templates"
  }
}
```

Common fields:

- `workspace.id`, `workspace.label`, `workspace.deployHost`
- `paths.components`, `paths.templates`
- `discover.skip_directories`, `discover.appAliases`
- `menu`
- `runtime.file_cache`
- `compliance`

Authoring rule:

- Put workspace-wide discover/menu/runtime defaults here.
- Do not keep app-local `ops.*` here.

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

All font/color values are managed through theme tokens (preview + editor shell):

- **Definition layer** (`theme.tokens.color`, `tokens.shell`, `tokens.gradient`, `theme.font`): literal `#hex`, `rgba()`, and `Npx` are allowed.
- **Reference layer** (scene/frame/panel props, `metric_*` roles): use semantic names only — `color = "text_primary"`, `font = "3"`. Literals compile as errors.

Required token keys and profile tables: `docs/mei-lang/topics/theme-token-contract.md`.

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
