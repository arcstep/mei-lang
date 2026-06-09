# MeiLang Build And Debug Ops

## Runtime inspection

- `mei-toolchain editor-runtime doctor --json`
- `mei-toolchain mcp describe --surface editor --json`
- `mei-toolchain knowledge export --surface editor --json`

## Workspace and stock

- `mei-toolchain workspace materialize --source-root <workspace> --json`
- `mei-toolchain workspace summary --source-root <workspace> --json`

## Compile diagnostics

- `mei-toolchain check --app <app> --source-root <workspace> --json`
- `mei-lsp` inside the editor for faster feedback

## Common standalone issues

- Missing `.stock/`: run `workspace materialize`
- Missing bundled docs or examples: run `knowledge export`
- Missing tool glue files: run `editor-runtime scaffold --tool <tool>`
- Missing package root discovery: set `MEI_PACKAGE_ROOT` or install the editor runtime package layout
