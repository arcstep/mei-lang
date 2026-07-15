# MeiLang Author Profile

## 当前定位

作者态不是“在宿主里顺手改几行 `.mei`”。

它的主任务是：

- 围绕当前 `.mei` 源码做结构化修改
- 读取稳定语法、组件与示例知识
- 在 Cursor / VS Code 中用 **language id `mei`**（扩展 `mei-lang-vscode`）获得着色，并用 `mei-lsp` / `mei-toolchain check` 收敛 diagnostics
- 只在需要 runtime/world 事实时调用 `inspect/query/runtime`

因此，作者态默认应是 **source-first + diagnostics-first**，而不是 world-first。

## 编辑器识别（先装再写）

1. 安装 `mei-lang` 仓库中的 [`extensions/mei-lang-vscode`](../extensions/mei-lang-vscode/README.md)（VSIX / Install from Location / F5）。
2. 打开 `.mei` 后，状态栏语言模式应为 **MeiLang**。
3. **不要**长期依赖 `"files.associations": { "*.mei": "python" }` 或 `starlark`；那只是过渡，且可能盖过正式扩展。
4. 确保 `mei-lsp` 可达：工作区 `.mei/runtime/bin/mei-lsp`，或设置 `mei.lsp.path`。

打包说明（runtime install 后）：`.mei/knowledge/author/language-and-editor-recognition.md`。

MeiLang 作者面是独立的 `mei-syntax` DSL（Python 风格表面），**不是** Starlark 方言。

## Catalog 真源

作者态 profile、knowledge bundle 与 MCP surface 由 toolchain capability catalog 统一导出：

```bash
mei-toolchain mcp catalog --json
mei-toolchain mcp describe --surface author --json
mei-toolchain knowledge --surface author --json
```

公开 profile / knowledge / MCP surface 统一为 `author`。  
`editor-runtime` 只是技术外壳名，不再作为作者态公开角色名。

## 主输入

作者态优先依赖这些输入：

1. 当前目标 `.mei` 源码
2. `.mei/profiles/author.md`
3. `.mei/skills/meilang-author/SKILL.md`
4. `.mei/skills/meilang-author/authoring.md`
5. `.mei/skills/meilang-author/syntax-rules.md`
6. `.mei/skills/meilang-author/dsl-reference.md`
7. `.mei/skills/meilang-author/namespace-reference.md`
8. `.mei/skills/meilang-author/components-reference.md`
9. `.mei/skills/meilang-author/context.md`
10. `.mei/knowledge/author/workspace-config-reference.md`
11. `.mei/knowledge/author/language-and-editor-recognition.md`
12. `.mei/knowledge/author/components/*`
13. `.mei/knowledge/author/templates/*`
14. `.mei/knowledge/author/examples/*`
15. `.mei/knowledge/author/extension-authoring.md`
16. `mei-toolchain check --app <app> --source-root <workspace> --json`
17. `mei-lsp` diagnostics / symbol / hover / definition / completion

## 默认顺序

1. 先读当前 target `.mei` 与相邻 scene / template / `_components`。
2. 确认编辑器已用 **MeiLang** 识别 `.mei`（扩展 + `mei-lsp`）；详见 `language-and-editor-recognition.md`。
3. 再读 workspace-local packaged authoring knowledge，而不是去源码仓库里额外找一套 docs。
4. 先跑 `mei-toolchain check` 或看 `mei-lsp`，把 diagnostics 当成主要机器反馈。
5. 涉及 bootstrap、create-app、`.mei-workspace.json`、`.mei-config.json`、`theme_ref(...)` 或 upload source 时，先读 `.mei/knowledge/author/workspace-config-reference.md`。
6. 涉及新组件 / 新模板 / provider 扩展时，先读 `.mei/knowledge/author/extension-authoring.md`，明确任务是否已经离开普通 author 链。
7. 只有当 packaged knowledge 仍然没有答案时，才把 `.stock/**/README.md` 或实现文件当成最后兜底来源。
8. 正式文件写入由外部开发工具完成；MeiLang 提供的是语义后端，不是默认作者态写入宿主。

## Access 的边界

- 不把 `inspect summary` / `workspace summary` 当成源码替代品。
- 不把 runtime/world 事实误当成当前源码已经声明的真值。
- 不把访问态 world-first 工具默认拿来做作者态结构改写。
- 不依赖宿主内置 `skill_list` / `skill_read` / `rewrite_current_mei` 这类旧 authoring loop。

## 推荐工具面

### 语言服务

- Cursor / VS Code 扩展：`mei-lang-vscode`（language id `mei` + TextMate + LSP 客户端）
- `mei-lsp`
- `mei-toolchain check --app <app> --source-root <workspace> --json`
- 打包说明：`.mei/knowledge/author/language-and-editor-recognition.md`

### 结构与 discover

- `mei-toolchain workspace summary --source-root <workspace> --json`
- `mei-toolchain inspect summary --app <app> --source-root <workspace> --json`

### bootstrap / config / theme

- `mei-toolchain workspace bootstrap --source-root <workspace> --app <app> --tool cursor --json`
- `mei-toolchain workspace init --standalone --source-root <workspace> --materialize --json`
- `mei-toolchain workspace runtime install --source-root <workspace> --json`
- `mei-toolchain editor-runtime scaffold --target-root <workspace> --tool cursor --json`
- `mei-toolchain workspace create-app <app> --source-root <workspace> --json`
- `./.mei/runtime/bin/mei-toolchain check --app <app> --source-root <workspace> --json`
- `mei-toolchain knowledge --surface author --source-root <workspace> --topic config --include-content --json`

### runtime/world 补充

- `mei-toolchain inspect world --app <app> --source-root <workspace> --json`
- `mei-toolchain inspect inventory --app <app> --source-root <workspace> --json`
- `mei-toolchain query dataset --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain query metric --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain runtime peek --app <app> --source-root <workspace> --json`

## 何时切到 access

出现下面情况时，优先切到 `access` 而不是继续按作者态猜：

- 用户问当前筛选下某个指标值是多少
- 用户问 dataset 行数、分组、趋势或 runtime phase/result
- 用户给了浏览器 `query_state` 或当前宿主访问态上下文

切换后优先读取：

- `.mei/profiles/access.md`
- `.mei/skills/meilang-access/SKILL.md`
- `mei-toolchain knowledge --surface access --source-root <workspace> --include-content --json`
