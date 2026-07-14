---
name: meilang-author
description: 创建、修改、审查、修复、编译、预构建、发布前校验或性能比较 MeiLang `.mei` / scene / dataset / metric 时使用。作者态主线是“源码 + knowledge + mei-toolchain CLI + mei-lsp + 外部开发工具”，默认优先 scoped build / 增量 prebuild，而不是提示重启宿主。
---

# MeiLang Author（短入口）

这个 skill 包是 **MeiLang toolchain capability catalog** 导出的作者态短入口。  
公开 profile 与 knowledge surface 都是 `author`；不要再把 `editor` 当作作者态公开角色名。

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
6. `.mei/knowledge/author/workspace-config-reference.md`
7. `.mei/knowledge/author/components/*`
8. `.mei/knowledge/author/templates/*`
9. `.mei/knowledge/author/examples/*`
10. `.mei/knowledge/author/extension-authoring.md`

源码包目录对应：

- `guides/author-profile.md`
- `guides/author-skills/*.md`
- `knowledge/editor-runtime/**/*`

独立 workspace 的公开消费面始终是 `.mei/`，不是源码仓路径。

## 布局主线（v2 SSOT）

作者只记这一条结构链：

```text
scene → plane_layout → region_layout → section_layout + section_shell → content_panel
```

| 层级 | 构造器 | 推荐文件 |
|------|--------|----------|
| scene | `scene(...)` + `plane_ref(...)` | `assembly.mei` |
| plane | `plane_layout(...)` | `t*/layout.mei` |
| region | `region_layout(...)` | `r-*/layout.mei` |
| section | `section_layout(...)` + `section_shell(...)` | `s-*/layout.mei` |
| content | `content_panel(...)` | `content.mei` |

`content_panel` 是 `content.mei` 的作者构造器名；当前编译器仍可能写作 `content_panel`——guides 统一用 `content_panel`，改源码时与现有样板对齐即可。

**Fill-down**：region 用 `Nfr` 分格 → section `stretch` 填满格子 → slot fill → **content 不设 px 高度**。

Runnable 样板：

- `workspaces/ws-demo-v2/apps/zhifa/src/scene/**`
- `workspaces/ws-demo-v2/apps/mini-park/src/scene/**`

目录形态：`t*/r-*/s-*/content.mei`（扁平前缀，不用 `planes/regions/sections` 硬目录）。

T2 用 **page_instance** 术语（实现侧 `page_instance` 正在改名）；不要把 `page_instance` / `page_instance` 当作推荐作者路径。

## 推荐顺序

1. 先读当前任务相关的 `.mei`、相邻 `layout.mei` / `content.mei`、`stock/templates` 与 `stock/components`。
2. 再读 `.mei/profiles/author.md` 与同目录 skill companion 文档。
3. 跑 `mei-toolchain check --app <app> --source-root <workspace> --json`；编辑器内反馈走 `mei-lsp`。
4. 需要 app / 别名 / discover 视图时再跑 `mei-toolchain workspace summary --source-root <workspace> --json`。
5. bootstrap / create-app / 配置 / 主题：补读 `.mei/knowledge/author/workspace-config-reference.md`。
6. 新组件 / 新模板：先读 `.mei/knowledge/author/extension-authoring.md`。
7. 源码与 packaged knowledge 仍不足时，再读 `stock/**/README.md`。
8. 仍需 runtime/world 真值时，再跑 `inspect/query/runtime`。

## 运行中宿主的默认处理原则

1. 先区分变更是否只落在单个 `sceneId + targetFile`
2. 单 scope 修改优先 scoped build
3. 多 scope / 数据输入变化优先单 app 增量 `prebuild`
4. 发布前优先 `prebuild --verify` 或 `fail-fast-verify`
5. 只有改了 Rust 宿主、前端 bundle、启动参数或运行时二进制时，才建议重启服务

不要把“重启宿主”当作 `.mei` 作者态修改的默认建议。

## 当前作者态主线

- 应用入口：`app.mei` 中的 `app_skeleton(...)` + `navigation(...)`
- 场景入口：`scene/.../assembly.mei` 中的 `scene(...)` + `planes = [plane_ref(...)]`
- 结构主线：上表 layout 链；`region_layout` 只挂 `section_ref(...)`
- UI 内容：`content_panel`（`content.mei`）内 `grid` + slot + `component` / `metric_card` / 业务宏
- 绑定主线：`dataset_ref` / `metric_ref` / `resource_ref` / `world_ref` / `map_ref` / `view_ref` / `link_ref`
- 复用主线：`plane_ref` / `region_ref` / `section_ref` / `panel_ref` / `assembly_ref` / `metric_card_ref`

## 常用命令

- `mei-toolchain check --app <app> --source-root <workspace> --json`
- `mei-toolchain workspace bootstrap --source-root <workspace> --app <app> --tool cursor --json`
- `mei-toolchain workspace init --standalone --source-root <workspace> --materialize --json`
- `mei-toolchain workspace runtime install --source-root <workspace> --json`
- `mei-toolchain editor-runtime scaffold --target-root <workspace> --tool cursor --json`
- `mei-toolchain workspace create-app <app> --source-root <workspace> --json`
- `./.mei/runtime/bin/mei-toolchain check --app <app> --source-root <workspace> --json`
- `mei-toolchain workspace summary --source-root <workspace> --json`
- `mei-toolchain knowledge --surface author --source-root <workspace> --include-content --json`
- `mei-toolchain knowledge --surface author --source-root <workspace> --topic config --include-content --json`
- `mei-toolchain knowledge --surface author --source-root <workspace> --topic syntax --include-content --json`
- `mei-toolchain mcp describe --surface author --source-root <workspace> --json`
- `mei-toolchain knowledge --surface access --source-root <workspace> --include-content --json`
- `mei-toolchain mcp describe --surface access --source-root <workspace> --json`
- `mei-toolchain inspect world --app <app> --source-root <workspace> --json`
- `mei-toolchain inspect inventory --app <app> --source-root <workspace> --json`
- `mei-toolchain inspect summary --app <app> --source-root <workspace> --json`
- `mei-toolchain query dataset --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain query metric --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain runtime peek --app <app> --source-root <workspace> --json`
- `curl -X POST http://127.0.0.1:9527/api/host/build -H 'Content-Type: application/json' -d '{"appId":"<app>","mode":"build","sceneId":"<scene>","targetFile":"<target>"}'`
- `mei-toolchain prebuild --workspace <workspace> --app <app> --json`
- `mei-toolchain prebuild --workspace <workspace> --app <app> --verify --json`
- `mei-toolchain prebuild --workspace <workspace> --app <app> --hot-only --json`
- `node ./scripts/host-perf-sample.mjs --scenario-file ./scripts/perf-scenarios/<app>.json`
- `node ./scripts/host-perf-report.mjs --sample <sample.jsonl> --scenario-file ./scripts/perf-scenarios/<app>.json --mode auto`

## 性能与发布判断

- 局部修改反馈：看 `local_edit_feedback_ms`、`compile_ms`、`dependency_graph_build_ms`
- 访问态 ready：看 `host_access_ready`、`host_full_warmup_ready`、`/api/host/ready`
- 发布前：`prebuild --verify` 或 `fail-fast-verify`
- 长期是否有效：`host-perf-sample` + `host-perf-report` 与 ledger / pinned baseline 比较

细节见同目录 `authoring.md` 与 `.mei/knowledge/author/build-debug-ops.md`。

## 禁止

- 不写 `frame(...)` / `frame.add_panel(...)` / 以 `flex(...)` 为默认布局。
- 不写 `titled_shell`、`row_budgets`、micro-layout 结构层。
- 不把 `page_instance` / `page_instance` 当作新稿推荐路径（T2 用 page_instance 叙事）。
- 不在 `region_layout` 上直挂 `content(...)` / `blocks`；必须 `sections = [section_ref(...)]`。
- 不在 content 层手写 px 高度撑 section。
- 不在组件 `props` 中直接跨文件消费外部 `dataset_ref` / `metric_ref`。
- 不把 `world_ref(...)` 当成资源 id 选择器。
- 不猜组件名、模板 id、资源 id、布局 area 或 props 字段。
- 不把设计文档里的未来能力写成已支持语法。
- 不再依赖 `skill_list` / `skill_read` / `rewrite_current_mei`。
- 不把 `inspect summary` / `workspace summary` 当成当前源码的替身。
