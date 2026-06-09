---
name: meilang-author
description: 创建、修改、审查或修复 MeiLang `.mei` 时使用。编辑侧主线是“文档 + mei-toolchain CLI + mei-lsp + 外部开发工具”，而不是宿主内置 agent tool。
---

# MeiLang Author（短入口）

这个 skill 包是 **MeiLang toolchain capability catalog** 导出的作者态短入口。  
它对应的公开 profile 与 knowledge surface 都是 `author`；不要再把 `editor` 当作作者态公开角色名。

MeiLang 当前作者态的主线是：

1. **先读当前 `.mei` 源码与 workspace-local 文本知识**
2. **再用 `mei-toolchain check` / `mei-lsp` 收 diagnostics**
3. **只在需要 runtime/world 事实时才用 inspect/query/runtime**
4. **正式文件写入始终由外部开发工具完成**

## Workspace-local 入口

在已安装 runtime 的 workspace 中，优先按下面顺序阅读：

1. `.mei/profiles/author.md`
2. 同目录下的 `authoring.md`、`syntax-rules.md`、`dsl-reference.md`、`namespace-reference.md`、`components-reference.md`、`context.md`
3. `.mei/knowledge/author/authoring-overview.md`
4. `.mei/knowledge/author/workflow-recipes.md`
5. `.mei/knowledge/author/build-debug-ops.md`
6. `.mei/knowledge/author/components/*`
7. `.mei/knowledge/author/templates/*`
8. `.mei/knowledge/author/examples/*`

如果你现在看到的是源码包目录，上述文件会分别来自：

- `guides/author-profile.md`
- `guides/author-skills/*.md`
- `knowledge/editor-runtime/**/*`

但对独立 workspace 使用者来说，**公开消费面始终是 `.mei/`，不是源码仓路径。**

## 推荐顺序

1. 先读当前任务相关的 `.mei` 文件、相邻 scene、`.stock/templates` 引用与 `.stock/components` 使用点。
2. 再读 `.mei/profiles/author.md` 与同目录 skill companion 文档，不要把 `summary` 当源码替代品。
3. 跑 `mei-toolchain check --app <app> --source-root <workspace> --json`；需要编辑器内反馈时走 `mei-lsp`。
4. 如需理解当前 workspace 的 app、别名与 discover 结果，再跑 `mei-toolchain workspace summary --source-root <workspace> --json`。
5. 只有当源码与 packaged knowledge 仍不足时，再读 `.mei/knowledge/author/components/*`、`.mei/knowledge/author/templates/*` 与最接近的 example。
6. 只有当仍需要 runtime/world 真值时，再跑 `inspect/query/runtime`。

## 当前作者态主线

- 单文件主线：`main.mei` 中的 `app(...)` + inline `scene(...)`
- 多文件主线：`main.mei` 中 `app(...)` + `app_add_scene(scene = scene_ref(...))`
- 结构主线：`scene -> world / flow / frame`
- UI 主线：`frame.add_panel(...)` / `panel(...)` / `component(...)`
- 绑定主线：`dataset_ref(...)`、`metric_ref(...)`、`resource_ref(...)` 等 typed refs 作为稳定值来源
- 复用主线：`scene_ref` / `world_ref` / `frame_ref` / `panel_ref` / `metric_card_ref`

## 常用命令

- `mei-toolchain check --app <app> --source-root <workspace> --json`
- `mei-toolchain workspace summary --source-root <workspace> --json`
- `mei-toolchain knowledge --surface author --source-root <workspace> --include-content --json`
- `mei-toolchain knowledge --surface author --source-root <workspace> --topic syntax --include-content --json`
- `mei-toolchain mcp describe --surface author --source-root <workspace> --json`
- `mei-toolchain inspect world --app <app> --source-root <workspace> --json`
- `mei-toolchain inspect inventory --app <app> --source-root <workspace> --json`
- `mei-toolchain inspect summary --app <app> --source-root <workspace> --json`
- `mei-toolchain query dataset --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain query metric --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain runtime peek --app <app> --source-root <workspace> --json`

## 禁止

- 不把 `entry(...)` / `entries=[entry(...)]` 当作新作者态默认主线。
- 不在组件 `props` 中直接跨文件消费外部 `dataset_ref` / `metric_ref`。
- 不把 `world_ref(...)` 当成资源 id 选择器。
- 不猜组件名、模板 id、资源 id、布局 area 或 props 字段。
- 不把设计文档里的未来能力写成已支持语法。
- 不再依赖 `skill_list` / `skill_read` / `rewrite_current_mei` 这类宿主内旧作者态工具。
- 不把 `inspect summary` / `workspace summary` 当成当前源码的替身。
