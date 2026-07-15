# MeiLang Build And Debug Ops

## Runtime inspection

- `mei-toolchain editor-runtime doctor --json`
- `mei-toolchain mcp describe --surface author --json`
- `mei-toolchain knowledge --surface author --json`

## Workspace and stock

- `mei-toolchain workspace bootstrap --source-root <workspace> [--app <app>] --tool cursor --json`
- `mei-toolchain workspace init --standalone --source-root <workspace> --materialize --json`
- `mei-toolchain workspace runtime install --source-root <workspace> --json`
- `mei-toolchain workspace runtime update --source-root <workspace> --json`
- `./start.sh` from the workspace root after runtime install
- `./.mei/runtime/bin/mei-toolchain check --app <app> --source-root <workspace> --json`
- `mei-toolchain workspace create-app <app> --source-root <workspace> --json`
- `mei-toolchain workspace materialize --source-root <workspace> --json`
- `mei-toolchain workspace summary --source-root <workspace> --json`

## Compile diagnostics

- `mei-toolchain check --app <app> --source-root <workspace> --json`
- `mei-lsp` inside the editor for faster feedback
- Install `mei-lang-vscode` so `.mei` uses language id `mei` (see `language-and-editor-recognition.md`)

## Editor recognition issues

- `.mei` shows as Python / Starlark / plain text: install `mei-lang-vscode`, then remove any `files.associations` that force `*.mei` to another language, and reload the window
- Highlighting works but no diagnostics: build or install `mei-lsp` (`.mei/runtime/bin/mei-lsp`) and optionally set `mei.lsp.path`
- Status bar is not **MeiLang**: use “Change Language Mode” → MeiLang, or reinstall the extension

## Common standalone issues

- Fresh clone has no `.mei/`: run `workspace runtime install --force` before opening the workspace in an AI tool or starting `./start.sh`
- Missing `.stock/`: run `workspace materialize`
- Missing `.mei/` runtime assets after `workspace init`: run `workspace runtime install`
- Missing workspace-local binaries after bootstrap/install: rerun `workspace runtime install --force` and verify `.mei/runtime/bin/mei-toolchain`, `mei-lsp`, and `mei-host-shell`
- Missing bundled docs, profiles, contracts, or examples: run `knowledge --surface author`
- Missing access entry files or access guidance: run `knowledge --surface access`
- Missing tool glue files: run `editor-runtime scaffold --tool <tool>`
- `./start.sh` falling back to PATH `mei-host-shell`: repair the workspace-local runtime instead of treating PATH as the normal path
- Missing theme/source refs: inspect `<app>/.mei-config.json -> ops.*` and `workspace-config-reference.md`
