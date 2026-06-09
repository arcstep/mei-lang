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

1. Create or initialize a standalone workspace.
2. Materialize `.stock/` when the workspace needs built-in components or templates.
3. Read `.mei/profiles/author.md`, `.mei/skills/meilang-author/*`, and `.mei/knowledge/author/*` as the workspace-local authoring truth.
4. Use `mei-lsp` for editor feedback and use author-side MCP only for read-only agent tooling.
5. Run `mei-toolchain check --app <app> --source-root <workspace>` for compile validation.
6. Use `mei-toolchain knowledge --surface author --source-root <workspace> --include-content --json` when an agent needs packaged docs, contracts, or examples.
