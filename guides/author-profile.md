# MeiLang Author Profile

## 当前定位

作者态不是“在宿主里顺手改几行 `.mei`”。

它的主任务是：

- 围绕当前 `.mei` 源码做结构化修改
- 读取稳定语法、组件与示例知识
- 依赖 `mei-toolchain check` / `mei-lsp` 收敛 diagnostics
- 只在需要 runtime/world 事实时调用 `inspect/query/runtime`

因此，作者态默认应是 **source-first + diagnostics-first**，而不是 world-first。

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
11. `.mei/knowledge/author/components/*`
12. `.mei/knowledge/author/templates/*`
13. `.mei/knowledge/author/examples/*`
14. `.mei/knowledge/author/extension-authoring.md`
15. `mei-toolchain check --app <app> --source-root <workspace> --json`
16. `mei-lsp` diagnostics / symbol / hover / definition / completion

## 默认顺序

1. 先读当前 target `.mei` 与相邻 scene / template / `_components`。
2. 再读 workspace-local packaged authoring knowledge，而不是去源码仓库里额外找一套 docs。
3. 先跑 `mei-toolchain check` 或看 `mei-lsp`，把 diagnostics 当成主要机器反馈。
4. 涉及 bootstrap、create-app、`.mei-workspace.json`、`.mei-config.json`、`theme_ref(...)` 或 upload source 时，先读 `.mei/knowledge/author/workspace-config-reference.md`。
5. 涉及新组件 / 新模板 / provider 扩展时，先读 `.mei/knowledge/author/extension-authoring.md`，明确任务是否已经离开普通 author 链。
6. 只有当 packaged knowledge 仍然没有答案时，才把 `.stock/**/README.md` 或实现文件当成最后兜底来源。
7. 正式文件写入由外部开发工具完成；MeiLang 提供的是语义后端，不是默认作者态写入宿主。

## Access 的边界

- 不把 `inspect summary` / `workspace summary` 当成源码替代品。
- 不把 runtime/world 事实误当成当前源码已经声明的真值。
- 不把访问态 world-first 工具默认拿来做作者态结构改写。
- 不依赖宿主内置 `skill_list` / `skill_read` / `rewrite_current_mei` 这类旧 authoring loop。

## 推荐工具面

### 语言服务

- `mei-lsp`
- `mei-toolchain check --app <app> --source-root <workspace> --json`

### 结构与 discover

- `mei-toolchain workspace summary --source-root <workspace> --json`
- `mei-toolchain inspect summary --app <app> --source-root <workspace> --json`

### bootstrap / config / theme

- `mei-toolchain workspace init --standalone --source-root <workspace> --materialize --json`
- `mei-toolchain workspace runtime install --source-root <workspace> --json`
- `mei-toolchain editor-runtime scaffold --target-root <workspace> --tool cursor --json`
- `mei-toolchain workspace create-app <app> --source-root <workspace> --scaffold --tool cursor --json`
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
