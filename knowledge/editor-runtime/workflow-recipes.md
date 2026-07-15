# MeiLang Workflow Recipes

## Bootstrap a source workspace

```bash
mei-toolchain workspace bootstrap --source-root /path/to/workspace --app my-app --tool cursor --json
```

Use the staged flow only when you intentionally want to separate the steps:

- `workspace init` creates the workspace skeleton
- `workspace runtime install` writes `.mei/` runtime assets, local binaries, and `./start.sh`
- `editor-runtime scaffold` writes tool glue only

```bash
mei-toolchain workspace init --standalone --source-root /path/to/workspace --materialize --json
mei-toolchain workspace runtime install --source-root /path/to/workspace --json
mei-toolchain editor-runtime scaffold --target-root /path/to/workspace --tool cursor --json
mei-toolchain workspace create-app my-app --source-root /path/to/workspace --json
```

## Restore runtime after cloning a workspace

```bash
mei-toolchain workspace runtime install --source-root /path/to/workspace --force --json
```

Use `workspace runtime update` for day-2 refreshes when the source workspace already exists and you want to rebuild `.mei/` without touching `.mei/local/**`.

## Recognize `.mei` in Cursor / VS Code

Install the MeiLang language extension from the `mei-lang` source tree:

```bash
cd /path/to/mei-lang/extensions/mei-lang-vscode
npm install
npm run package
cursor --install-extension ./mei-lang-*.vsix
```

Then open any `.mei` file and confirm the language mode is **MeiLang**. Prefer this over `files.associations` remaps to Python/Starlark.

Packaged guide after runtime install: `.mei/knowledge/author/language-and-editor-recognition.md`.

## Start the host

```bash
./start.sh
```

This should launch the workspace-local `.mei/runtime/bin/mei-host-web`.

## Create a new app

```bash
mei-toolchain workspace create-app my-app --source-root /path/to/workspace --scaffold --tool cursor --json
```

The current scaffold writes:

- `my-app/main.mei`
- `my-app/scenes/home.mei`
- `my-app/.mei-config.json`

## Configure workspace and app JSON

```bash
mei-toolchain knowledge --surface author --source-root /path/to/workspace --topic config --include-content --json
```

Read `.mei-workspace.json` for workspace-wide paths/discover/menu/runtime defaults, and read `my-app/.mei-config.json` for app entry, upload/prototype paths, and `ops.*`.

## Configure a theme or upload-backed source

```json
{
  "ops": {
    "themes": {
      "cockpit_dark": {}
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

Then consume them in `.mei`:

```mei
scene(theme = theme_ref("cockpit_dark"))
world.add_dataset(id = "uploaded_sales", source = source_ref("uploaded_sales"), schema = [])
```

## Export packaged knowledge for an AI tool

```bash
mei-toolchain knowledge --surface author --source-root /path/to/workspace --include-content --json
mei-toolchain knowledge --surface author --source-root /path/to/workspace --topic author_profile --include-content --json
mei-toolchain knowledge --surface author --source-root /path/to/workspace --topic components --include-content --json
mei-toolchain knowledge --surface author --source-root /path/to/workspace --topic examples --include-content --json
mei-toolchain knowledge --surface access --source-root /path/to/workspace --include-content --json
```

## Describe and verify the local editor runtime

```bash
mei-toolchain editor-runtime describe --json
mei-toolchain editor-runtime doctor --json
```

## Validate an app in a runtime-installed workspace

```bash
mei-toolchain check --app my-app --source-root /path/to/workspace --json
```
