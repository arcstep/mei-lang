# MeiLang VS Code / Cursor 扩展

为 `.mei` 注册正式 language id **`mei`**，提供 TextMate 着色（`source.mei`）、语言默认图标，并启动 **`mei-lsp`**。另贡献 **`app.toml` JSON Schema**（由 Even Better TOML 消费）。

扩展 **version 与仓库根 `Cargo.toml` 的 `[workspace.package].version` 对齐**（`npm run sync-version` / `npm run package` 会自动同步）。

作者态说明：`knowledge/editor-runtime/language-and-editor-recognition.md`（runtime 安装后在 `.mei/knowledge/author/`）。设计 SSOT：`docs/mei-lang-v2/08-agent-skills/0807-…`。

## 为什么需要本扩展

仅配置 `files.associations`（例如 `*.mei` → `python` / `starlark`）不够：

- 不会注册 language id `mei`
- 可能盖过正式扩展
- Agent / Glass / GitHub 等路径通常吃不到

正确路径：安装本扩展 → 右下角语言模式为 **MeiLang**。

MeiLang 是独立的 `mei-syntax` 作者 DSL（Python 风格表面），**不是** Starlark 方言。产品 App 根是 **`app.toml`**，不是 `main.mei`。

## 推荐配套扩展

| 扩展 | 用途 |
|------|------|
| **Even Better TOML**（`tamasfe.even-better-toml`） | TOML 着色；消费本扩展的 `tomlValidation` → `app.toml` 补全/校验 |

本扩展 **不** 内置 TOML TextMate。

## 安装

### 市场（目标路径）

Open VSX / Cursor 扩展面板搜索 **MeiLang**（`mei-lang.mei-lang`）。上架步骤见 [PUBLISH.md](PUBLISH.md)。**当前若尚未上架，请用侧载。**

### 侧载 VSIX

```bash
cd extensions/mei-lang-vscode
npm install
npm run package

# PATH 上有 cursor 时：
cursor --install-extension ./mei-lang-*.vsix

# macOS Cursor.app 完整路径：
/Applications/Cursor.app/Contents/Resources/app/bin/cursor \
  --install-extension ./mei-lang-*.vsix

# 或：经典 Editor 中 Cmd+Shift+P → Extensions: Install from VSIX…
```

**务必在经典 Editor 中安装/验证**（Agents Window 常无 Extensions 面板，且自定义 grammar 常不生效）。命令面板：`Open IDE`。

### 准备 mei-lsp（建议）

```bash
# 在 mei-lang 仓库根
cargo build -p mei-lang-lsp
# 或使用工作区已安装的 .mei/runtime/bin/mei-lsp
```

查找顺序：`mei.lsp.path` → `.mei/runtime/bin/mei-lsp` → `target/debug|release/mei-lsp` → `PATH`。

找不到 LSP 时仍有着色，并弹出警告。

### Extension Development Host

1. 单独打开本目录，按 **F5**
2. 新窗口打开含 `.mei` / `app.toml` 的工作区
3. `.mei` 右下角应为 **MeiLang**；`app.toml`（已装 Even Better TOML）应有 schema 提示

若仍显示 Starlark / Python：删掉绑走 `*.mei` 的 `files.associations`，Reload Window。  
若图标仍是通用文档：File Icon Theme 不要用 `None` / `Minimal`（推荐 Seti）。

## 设置

| 键 | 说明 |
|----|------|
| `mei.lsp.path` | `mei-lsp` 绝对路径；空则自动发现 |
| `mei.lsp.trace.server` | LSP 跟踪：`off` / `messages` / `verbose` |

## 当前范围

- ✅ `.mei` language id + TextMate + language-configuration
- ✅ 资源管理器语言默认图标（梅花铜钱 `icons/mei.svg`）
- ✅ 挂载 `mei-lsp`（app 根：`app.toml` / 兼容路径）
- ✅ `app.toml` JSON Schema + `tomlValidation`（需 Even Better TOML）
- ❌ Stage MDX 专用 grammar / LSP
- ❌ Open VSX / Marketplace **已上架**（发布准备见 PUBLISH.md；分发契约见仓库 `0605`）

## 关键字来源

声明与关键字对齐当前 **`mei-syntax` 作者面**。宿主 CodeMirror mode 可能仍带历史表；长期应抽单一真源再生成 TextMate / CodeMirror。
