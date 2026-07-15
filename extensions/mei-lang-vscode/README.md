# MeiLang VS Code / Cursor 扩展

为 `.mei` 注册正式 language id **`mei`**，提供 TextMate 着色（`source.mei`），并启动 **`mei-lsp`**。

扩展 **version 与仓库根 `Cargo.toml` 的 `[workspace.package].version` 对齐**（`npm run sync-version` / `npm run package` 会自动同步）。

作者态可打包说明见：`knowledge/editor-runtime/language-and-editor-recognition.md`（安装 runtime 后出现在 `.mei/knowledge/author/language-and-editor-recognition.md`）。

## 为什么需要本扩展

仅配置 `files.associations`（例如 `*.mei` → `python` / `starlark`）不够：

- 不会注册 language id `mei`
- 可能盖过正式扩展
- Agent / Glass / GitHub 等路径通常吃不到

正确路径：安装本扩展 → 右下角语言模式为 **MeiLang**。

MeiLang 是独立的 `mei-syntax` 作者 DSL（Python 风格表面），**不是** Starlark 方言。

## 本机安装

### 1. 依赖

```bash
cd extensions/mei-lang-vscode
npm install
```

### 2. 准备 mei-lsp（建议）

```bash
# 在 mei-lang 仓库根
cargo build -p mei-lang-lsp
# 或使用工作区已安装的 .mei/runtime/bin/mei-lsp
```

扩展查找顺序：

1. 设置 `mei.lsp.path`
2. 工作区祖先中的 `.mei/runtime/bin/mei-lsp`
3. `mei-lang/target/debug|release/mei-lsp`
4. `PATH` 上的 `mei-lsp`

找不到 LSP 时仍有着色，并弹出警告。

### 3. Extension Development Host

1. 用 Cursor / VS Code **单独打开**本目录
2. 按 **F5**（launch：`Extension: MeiLang`）
3. 在新窗口打开含 `.mei` 的工作区，或打开 `examples/hello.mei`
4. 右下角应为 **MeiLang**；`app_skeleton(` 等构造器应有 declaration 着色

### 4. 侧载到日常 Cursor / VS Code

```bash
npm run package
cursor --install-extension ./mei-lang-*.vsix
# 或：Extensions → Install from VSIX…
# 或：Install from Location… 指向本目录（需已 npm install）
```

若仍显示 Starlark / Python：检查用户/工作区 `files.associations` 是否把 `*.mei` 绑走了，删掉后 Reload Window。

## 设置

| 键 | 说明 |
|----|------|
| `mei.lsp.path` | `mei-lsp` 绝对路径；空则自动发现 |
| `mei.lsp.trace.server` | LSP 跟踪：`off` / `messages` / `verbose` |

## 当前范围

- ✅ `.mei` language id + TextMate + language-configuration
- ✅ 挂载 `mei-lsp`
- ❌ Stage MDX（`*.stage.mdx` / `*.deck.mdx`）专用 grammar / LSP
- ❌ Marketplace / Open VSX 发布（后续）

## 关键字来源

声明与关键字对齐当前 **`mei-syntax` 作者面**（构造器 + 字面量），不以 Starlark/Python 控制流为准。宿主内嵌 CodeMirror mode 可能仍带历史关键字表；长期应抽单一真源再生成 TextMate / CodeMirror。
