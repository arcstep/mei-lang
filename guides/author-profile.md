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
10. `.mei/knowledge/author/components/*`
11. `.mei/knowledge/author/templates/*`
12. `.mei/knowledge/author/examples/*`
13. `mei-toolchain check --app <app> --source-root <workspace> --json`
14. `mei-lsp` diagnostics / symbol / hover / definition / completion

## 默认顺序

1. 先读当前 target `.mei` 与相邻 scene / template / `_components`。
2. 再读 workspace-local packaged authoring knowledge，而不是去源码仓库里额外找一套 docs。
3. 先跑 `mei-toolchain check` 或看 `mei-lsp`，把 diagnostics 当成主要机器反馈。
4. 若公共 component/template contract 仍不足，再读 `.stock/components/**/README.md` / `.stock/templates/**/README.md`。
5. 只有当源码与 packaged knowledge 仍然没有答案时，再调用 `inspect/query/runtime`。
6. 正式文件写入由外部开发工具完成；MeiLang 提供的是语义后端，不是默认作者态写入宿主。

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

### runtime/world 补充

- `mei-toolchain inspect world --app <app> --source-root <workspace> --json`
- `mei-toolchain inspect inventory --app <app> --source-root <workspace> --json`
- `mei-toolchain query dataset --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain query metric --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain runtime peek --app <app> --source-root <workspace> --json`
