# Publishing MeiLang VS Code extension

Artifact name aligns with release contract `0605`:

```text
mei-lang-<version>.vsix
```

Version must match `Cargo.toml` `[workspace.package].version` (`npm run check-version`).

## Package locally

```bash
cd extensions/mei-lang-vscode
npm install
npm run package
# → mei-lang-<version>.vsix
```

Sideload for verification before any marketplace publish.

## Open VSX (prep)

1. Create a publisher on [Open VSX](https://open-vsx.org/) matching `package.json` `"publisher": "mei-lang"`.
2. Create a personal access token with publish rights.
3. Export `OVSX_PAT` (never commit the token).
4. Publish:

```bash
cd extensions/mei-lang-vscode
npm run publish:ovsx
# equivalent:
# npx ovsx publish ./mei-lang-<version>.vsix -p "$OVSX_PAT"
```

Optional CI: [`.github/workflows/vscode-extension-publish.yml`](../../.github/workflows/vscode-extension-publish.yml) is **workflow_dispatch only** and requires repository secret `OVSX_PAT`. It does **not** run on every push and must not depend on sibling `workspaces/`.

## Microsoft Marketplace (optional, later)

Use `vsce publish` with a Marketplace PAT after publisher verification. Cursor / Codium users primarily need Open VSX.

## LSP note

VSIX does **not** embed `mei-lsp` (see `0605`). Authors use Toolchain / `.mei/runtime/bin/mei-lsp` / `mei.lsp.path`.
