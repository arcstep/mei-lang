# MeiLang VS Code / Cursor 扩展

为 `.mei` 注册正式 language id **`mei`**，提供 TextMate 着色，并启动已有的 **`mei-lsp`**。

扩展 **version 与 `mei-lang/Cargo.toml` 的 `[workspace.package].version` 对齐**（当前脚本 `npm run sync-version` / `npm run package` 会自动同步）。

设计说明见：`docs/mei-lang-v2/08-agent-skills/0807-language-ecosystem-grammar-and-editor-recognition.md`。

## 本机验证（推荐）

### 1. 安装依赖

```bash
cd mei-lang/extensions/mei-lang-vscode
npm install
```

### 2. 准备 mei-lsp（可选，但建议）

```bash
cd mei-lang
cargo build -p mei-lang-lsp
# 或使用工作区已安装的 .mei/runtime/bin/mei-lsp
```

扩展会按顺序查找：

1. 设置 `mei.lsp.path`
2. 工作区祖先中的 `.mei/runtime/bin/mei-lsp`
3. `mei-lang/target/debug|release/mei-lsp`
4. `PATH` 上的 `mei-lsp`

找不到 LSP 时仍有着色，并弹出警告。

### 3. Extension Development Host

1. 用 Cursor / VS Code **单独打开**本目录：`mei-lang/extensions/mei-lang-vscode`
2. 按 **F5**（或运行 launch 配置 `Extension: MeiLang`）
3. 在新窗口打开任意含 `.mei` 的工作区，或打开本目录 `examples/hello.mei`
4. 右下角语言模式应为 **MeiLang**；声明符（如 `app(`）与关键字应有独立着色

### 4. 侧载到日常 Cursor（可选）

```bash
cd mei-lang/extensions/mei-lang-vscode
npm install
npm run package
```

生成 `mei-lang-<workspace.version>.vsix` 后：Command Palette → **Extensions: Install from VSIX…**

或 CLI：

```bash
cursor --install-extension ./mei-lang-*.vsix
```

或在 Cursor 中：**Extensions: Install from Location…** 指向本目录（需已 `npm install`）。

## 设置

| 键 | 说明 |
|----|------|
| `mei.lsp.path` | `mei-lsp` 绝对路径；空则自动发现 |
| `mei.lsp.trace.server` | LSP 通信跟踪：`off` / `messages` / `verbose` |

## 范围（0.1）

- ✅ `.mei` language id + TextMate + language-configuration
- ✅ 挂载 `mei-lsp`
- ❌ Stage MDX（`*.stage.mdx` / `*.deck.mdx`）专用 grammar / LSP
- ❌ Marketplace / Open VSX 发布（后续）

## 关键字来源

声明与关键字应对齐 **当前 `mei-syntax` 作者面**（见 `0210-syntax`），而不是 Starlark/Python 控制流。宿主 CodeMirror mode 仍可能带历史关键字表；扩展 grammar 以构造器 + 字面量为主，后续应抽成单一真源。
