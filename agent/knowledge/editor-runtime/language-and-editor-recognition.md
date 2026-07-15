# Language And Editor Recognition

This guide explains how `.mei` is recognized in external editors (Cursor / VS Code and friends).

## What authors need

For `.mei` editing you want three layers:

1. **Language id `mei`** — so the editor knows the file is MeiLang, not Python/Starlark
2. **TextMate grammar (`source.mei`)** — syntax highlighting
3. **`mei-lsp`** — diagnostics, symbols, hover, definition, completion

The compiler (`mei-syntax`) remains the semantic source of truth. The editor grammar is an approximation for readability.

## Install the VS Code / Cursor extension

Source path in the `mei-lang` repository:

```text
extensions/mei-lang-vscode/
```

Quick install (from that directory):

```bash
npm install
npm run package
cursor --install-extension ./mei-lang-*.vsix
# or: Extensions → Install from VSIX…
```

Development Host: open `extensions/mei-lang-vscode`, press **F5**, then open any workspace containing `.mei`.

After install, open a `.mei` file and confirm the status-bar language mode is **MeiLang**.

### Settings

| Key | Purpose |
|-----|---------|
| `mei.lsp.path` | Absolute path to `mei-lsp` (empty = auto-detect) |
| `mei.lsp.trace.server` | `off` / `messages` / `verbose` |

`mei-lsp` discovery order:

1. `mei.lsp.path`
2. workspace ancestor `.mei/runtime/bin/mei-lsp`
3. `mei-lang/target/debug|release/mei-lsp`
4. `PATH`

Highlighting still works if LSP is missing; diagnostics will not.

## Do not rely on `files.associations`

Avoid long-term settings like:

```json
"files.associations": { "*.mei": "python" }
```

or `"*.mei": "starlark"`.

Those remaps only help the classic editor look colored; they do **not** register language id `mei`, and they can override the official extension. Prefer installing `mei-lang-vscode`.

`editor-runtime scaffold` may still emit associations as a transitional fallback when the extension is not installed.

## What MeiLang is (for highlighting)

MeiLang authoring is an **independent DSL** parsed by `mei-syntax`. The surface looks Python-like (`#` comments, `True`/`False`/`None`, call-style constructors), but it is **not** a Starlark dialect.

Author policy rejects tokens such as `for`, `while`, `lambda`, `load`, `import`, `open` (with limited world-file exceptions). Do not teach agents to write Starlark/Python control flow in `.mei`.

## Stage MDX

`*.stage.mdx` / `*.deck.mdx` / `home.mdx` are a separate authoring surface:

- Today: use a generic MDX extension if needed
- Not covered by the `.mei` TextMate grammar
- Dedicated grammar / LSP is future work

## Cross-tool summary

| Asset | Consumers |
|-------|-----------|
| language id `mei` | Cursor, VS Code, fences `` ```mei `` |
| TextMate `source.mei` | VS Code-compatible hosts, later Linguist |
| `mei-lsp` binary | Cursor, VS Code, Neovim, Zed, OpenCode, … |

Other editors should adapt the same three assets; do not invent a second language identity.

## Related packaged docs

- `.mei/knowledge/author/authoring-overview.md`
- `.mei/knowledge/author/build-debug-ops.md`
- Extension how-to: `extensions/mei-lang-vscode/README.md` (in the `mei-lang` source tree)
