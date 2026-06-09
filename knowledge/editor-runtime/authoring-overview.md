# MeiLang Editor Runtime Overview

This package is the standalone authoring runtime for external MeiLang development tools.

## What it provides

- `mei-toolchain` for `check`, `inspect`, `query`, `workspace`, `knowledge`, and `editor-runtime`
- `mei-lsp` for in-editor diagnostics and language services
- editor MCP adapters for agent-style tools
- packaged authoring knowledge, examples, and platform assets

## What it does not require

- a checkout of the `mei-lang` source repository
- host-web authoring mode as the default source editing surface

## Recommended authoring loop

1. Create or initialize a standalone workspace.
2. Materialize `.stock/` when the workspace needs built-in components or templates.
3. Use `mei-lsp` and the editor runtime templates in your IDE.
4. Run `mei-toolchain check --app <app> --source-root <workspace>` for compile validation.
5. Use `mei-toolchain knowledge export --surface editor --include-content --json` when an agent needs bundled docs or examples.
