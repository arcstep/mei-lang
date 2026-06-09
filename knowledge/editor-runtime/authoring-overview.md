# MeiLang Editor Runtime Overview

This package is the standalone authoring runtime for external MeiLang development tools.

## What it provides

- `mei-toolchain` for `check`, `inspect`, `query`, `workspace`, `knowledge`, and `editor-runtime`
- `mei-lsp` for in-editor diagnostics and language services
- editor MCP adapters for agent-style tools
- packaged authoring knowledge, examples, and platform assets
- canonical profiles for `author` and `access`

## What it does not require

- a checkout of the `mei-lang` source repository
- host-web authoring mode as the default source editing surface

## Recommended authoring loop

1. Run `mei-toolchain workspace bootstrap --source-root <workspace> [--app <app>] [--tool <tool>] --json` for the one-command path, or stage `workspace init` -> `workspace runtime install` -> `editor-runtime scaffold` when you need separation.
2. Treat `.mei/runtime/bin/mei-toolchain`, `.mei/runtime/bin/mei-lsp`, and `.mei/runtime/bin/mei-host-web` as the canonical local binaries for a qualified workspace.
3. Read `.mei/profiles/author.md`, `.mei/skills/meilang-author/*`, and `.mei/knowledge/author/*` as the workspace-local authoring truth.
4. Use `mei-lsp` for editor feedback and use author-side MCP only for read-only agent tooling.
5. Run `mei-toolchain check --app <app> --source-root <workspace>` for compile validation.
6. Use `mei-toolchain knowledge --surface author --source-root <workspace> --include-content --json` when an agent needs packaged docs, contracts, or examples.

## Access handoff

When the question becomes runtime/data-facing instead of source-editing:

1. Switch to `.mei/profiles/access.md` and `.mei/skills/meilang-access/*`.
2. Use `mei-toolchain knowledge --surface access --source-root <workspace> --include-content --json`.
3. Prefer `dataset_query`, `dataset_metric`, `resource_business_summary`, and `resource_runtime_peek`.
