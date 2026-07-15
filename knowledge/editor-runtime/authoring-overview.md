# MeiLang Editor Runtime Overview

This package is the standalone authoring runtime for external MeiLang development tools.

## What it provides

- `mei-toolchain` for `check`, `inspect`, `query`, `workspace`, `knowledge`, and `editor-runtime`
- `mei-lsp` for in-editor diagnostics and language services
- **`mei-lang-vscode`** (in the `mei-lang` source tree: `extensions/mei-lang-vscode`) for language id `mei` + TextMate highlighting + LSP client
- editor MCP adapters for agent-style tools
- packaged authoring knowledge, examples, and platform assets
- canonical profiles for `author` and `access`

## What it does not require

- a checkout of the `mei-lang` source repository (except when installing/updating the VS Code extension from source)
- host-web authoring mode as the default source editing surface

## Editor recognition (required for comfortable `.mei` editing)

1. Install `mei-lang-vscode` (VSIX / Install from Location / F5). Details: `.mei/knowledge/author/language-and-editor-recognition.md`.
2. Confirm the status-bar language mode is **MeiLang**, not Python or Starlark.
3. Prefer the extension over `files.associations` remaps (`*.mei` → `python`/`starlark`). Associations are transitional only and can override the extension.
4. Point the editor at workspace-local `.mei/runtime/bin/mei-lsp` (or set `mei.lsp.path`).

MeiLang authoring is an independent `mei-syntax` DSL with a Python-like surface; it is **not** a Starlark dialect.

## Recommended authoring loop

1. Treat the checked-in workspace files (`.mei-workspace.json`, app sources, `.stock/`, `start.sh`) as the source-of-truth layer.
2. Run `mei-toolchain workspace bootstrap --source-root <workspace> [--app <app>] [--tool <tool>] --json` for the one-command path, or run `workspace runtime install` / `workspace runtime update` when the source workspace already exists.
3. Treat `.mei/runtime/bin/mei-toolchain`, `.mei/runtime/bin/mei-lsp`, and `.mei/runtime/bin/mei-host-web` as the canonical local binaries for the installed runtime layer.
4. Install / enable `mei-lang-vscode` so `.mei` files use language id `mei` with highlighting and LSP.
5. Read `.mei/profiles/author.md`, `.mei/skills/meilang-author/*`, and `.mei/knowledge/author/*` as the workspace-local authoring truth.
6. Use `mei-lsp` for editor feedback and use author-side MCP only for read-only agent tooling.
7. Run `mei-toolchain check --app <app> --source-root <workspace>` for compile validation.
8. Use `mei-toolchain knowledge --surface author --source-root <workspace> --include-content --json` when an agent needs packaged docs, contracts, or examples.

## Access handoff

When the question becomes runtime/data-facing instead of source-editing:

1. Switch to `.mei/profiles/access.md` and `.mei/skills/meilang-access/*`.
2. Use `mei-toolchain knowledge --surface access --source-root <workspace> --include-content --json`.
3. Prefer `dataset_query`, `dataset_metric`, `resource_business_summary`, and `resource_runtime_peek`.
