# Language And Editor Recognition

This guide explains how `.mei` (and `app.toml`) are recognized in external editors (Cursor / VS Code and friends).

## What authors need

For `.mei` editing you want three layers:

1. **Language id `mei`** — so the editor knows the file is MeiLang, not Python/Starlark
2. **TextMate grammar (`source.mei`)** — syntax highlighting
3. **`mei-lsp`** — diagnostics, symbols, hover, definition, completion

The compiler (`mei-syntax`) remains the semantic source of truth. The editor grammar is an approximation for readability.

Product apps use **`app.toml` + Stage MDX + `src/scene/…`**. Do not treat `main.mei` as the gold entry.

## Classic Editor vs Agents Window

Install extensions and verify highlighting in the **classic Editor** (Explorer / Extensions activity bar).

Cursor **Agents Window** often does **not** load custom TextMate grammars, language icons, or full LSP UX. If `Cmd+Shift+X` does nothing:

1. Command Palette → **Open IDE** (or File → Open Cursor Editor Window)
2. Or start classic mode: `open /Applications/Cursor.app --args --classic`

## Install the VS Code / Cursor extension

Source path in the `mei-lang` repository:

```text
tools/mei-lang-vscode/
```

### Preferred (when published)

Search the Extensions marketplace (Open VSX / Cursor proxy) for **MeiLang** (`mei-lang.mei-lang`).

### Sideload (today)

From that directory:

```bash
npm install
npm run package
# If `cursor` is not on PATH:
/Applications/Cursor.app/Contents/Resources/app/bin/cursor \
  --install-extension ./mei-lang-*.vsix
# or: Cmd+Shift+P → Extensions: Install from VSIX…
```

Development Host: open `tools/mei-lang-vscode`, press **F5**, then open any workspace containing `.mei`.

After install, open a `.mei` file and confirm the status-bar language mode is **MeiLang**.

File icons need a File Icon Theme other than `None` / `Minimal` (Seti is fine).

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

App root for LSP: `app.toml`, `app.config.json`, or `src/main.mei` (legacy). Missing root no longer means “need `main.mei` at the folder that contains the open file”.

## `app.toml` editing (TOML)

Do **not** rely on MeiLang for TOML syntax highlighting.

1. Install **Even Better TOML** (`tamasfe.even-better-toml`)
2. MeiLang ships `schemas/app.toml.json` and contributes `tomlValidation` for `app.toml`

Then you get completion / validation for App root fields (`title`, `default_stage`, `ops`, `warmup`, …).

Workspace scaffold may recommend both extensions via `.vscode/extensions.json`.

## Do not rely on `files.associations`

Avoid long-term settings like:

```json
"files.associations": { "*.mei": "python" }
```

or `"*.mei": "starlark"`.

Those remaps only help the classic editor look colored; they do **not** register language id `mei`, and they can override the official extension. Prefer installing `mei-lang-vscode`.

`editor-runtime scaffold` no longer defaults `*.mei` → python; it recommends the MeiLang extension instead.

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
| language id `mei` | Cursor, VS Code, Trae (VS Code shells), fences `` ```mei `` |
| TextMate `source.mei` | One VSIX for all VS Code-compatible hosts; later Linguist |
| `mei-lsp` binary | Cursor, VS Code, Neovim, Zed, OpenCode, … |
| `app.toml` JSON Schema | Even Better TOML + MeiLang contribution |

Other editors should adapt the same assets; do not invent a second language identity or fork a full grammar per IDE.

## Related packaged docs

- `.mei/knowledge/author/authoring-overview.md`
- `.mei/knowledge/author/build-debug-ops.md`
- Extension how-to: `tools/mei-lang-vscode/README.md` (in the `mei-lang` source tree)
- Design SSOT: `docs/mei-lang-v2/08-agent-skills/0807-language-ecosystem-grammar-and-editor-recognition.md`
