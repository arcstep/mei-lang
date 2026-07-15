# MeiLang Authoring

## 当前边界

作者态默认协作方式：

- **外部开发工具**负责读写文件、重构、搜索、多文件修改
- **`mei-lang-vscode`** 负责 language id `mei`、语法着色，并挂载 `mei-lsp`
- **MeiLang CLI / LSP** 负责 diagnostics，以及在需要 runtime/world 事实时提供 inspect/query
- **MeiLang 宿主内置 AI** 不再承担编辑侧主线

不要再假设存在：`skill_list` / `skill_read` / `rewrite_current_mei`。

不要把 `.mei` 长期 `files.associations` 到 `python` / `starlark`；那只是未装扩展时的过渡。安装与排障见 `.mei/knowledge/author/language-and-editor-recognition.md`。

## 布局心智（先记这一条）

```text
viewport 定舞台
  → plane / region：grid + gap + Nfr
    → section：标题固定 + body fill（section_shell）
      → slot：分位 + fill + chrome
        → content_panel：metric / chart / text（禁止撑布局）
```

结构链：

```text
scene → plane_layout → region_layout → section_layout + section_shell → content_panel
```

`content_panel` = `content.mei` 作者构造器；样板源码里常见 `content_panel`，语义同一。

**Fill-down**：region `Nfr` → section stretch → slot fill → content **不设 px 高度**。调占比只改 region `fr` 或 Theme Profile，不改 content 行高。

样板：

- `workspaces/ws-demo-v2/apps/zhifa/src/scene/**`
- `workspaces/ws-demo-v2/apps/mini-park/src/scene/**`

## 推荐工作流

### 1. 先读目标源码与知识

1. 当前 `app.mei` / `assembly.mei` / 目标 `layout.mei` / `content.mei`
2. 相邻 `t*/r-*/s-*` 树与 `stock/templates` / `stock/components`
3. `syntax-rules.md`、`dsl-reference.md`、`namespace-reference.md`、`components-reference.md`、`context.md`
4. `.mei/knowledge/author/language-and-editor-recognition.md`（确认 MeiLang 语言模式与 `mei-lsp`）
5. `.mei/knowledge/author/workspace-config-reference.md`
6. `.mei/knowledge/author/components/*`、`templates/*`
7. 相近 example（优先 zhifa / mini-park）

作者态默认 **source-first + knowledge-first**。不要先把 `inspect summary` / `workspace summary` 当源码替代品。

### 2. 再建立机器反馈锚点

1. `mei-toolchain check --app <app> --source-root <workspace> --json`
2. 需要 discover 视图时：`mei-toolchain workspace summary --source-root <workspace> --json`

需要 world/runtime 事实时再补 `query` / `runtime peek` / `inspect`。

### 3. 落盘前确认

- 改动落在哪个 `scene` / `t*` / `r-*` / `s-*`
- `key` 与 `*_ref` 是否镜像目录路径（`{app}/{scene}/t*/r-*/s-*`）
- `region_layout.sections` 是否全是 `section_ref`
- content 是否未手写 px 高度 / `row_budgets`
- resource / metric id、component props 是否真实存在

改完至少再跑一次 `mei-toolchain check`。

### 3.1 改完后如何让运行中的宿主生效

默认不要先说“重启服务”：

1. **单个 scene / section / content 修改** → scoped build

```bash
curl -X POST http://127.0.0.1:9527/api/host/build \
  -H 'Content-Type: application/json' \
  -d '{
    "appId": "zhifa",
    "mode": "build",
    "sceneId": "home",
    "targetFile": "src/scene/home/t1/r-left-rail/s-enforcement/content.mei"
  }'
```

2. **多 target / dataset·metric 绑定 / upload source 变化** → 单 app 增量 `prebuild`
3. **只恢复热点访问链** → `prebuild --hot-only`
4. **发布前** → `prebuild --verify` 或 `fail-fast-verify`

只有改了 Rust 宿主、`app/assets/**` bundle、启动参数/端口/鉴权时，才默认重启宿主。

### 3.2 access-only 边界

- 页面请求不会现场 compile；缺 artifact 先补 build / prebuild
- `/api/host/ready` 才是访问态 ready 锚点
- “改了 `.mei` 页面没立刻生效”先查 scoped build / prebuild / artifact，不要直接归因重启

### 3.3 性能与发布

性能整改用 `host-perf-sample` + `host-perf-report`（`auto` / `latest` / `pinned`）。  
发布前最小动作：`check` → `prebuild --verify` → 需要时 `serve --startup-policy fail-fast-verify`。

## 当前主线写法

1. `app.toml`：`title` / `default_stage`（App 根；无 `src/app.mei`）
2. `src/stage/{id}.stage.mdx`：Stage Program（access navigation 由编译器从 MDX 合成）
3. `src/scene/{id}/assembly.mei`：薄 `scene` + `plane_ref`
4. `t*/layout.mei`：`plane_layout` + `region_ref`
5. `r-*/layout.mei`：`region_layout` + `section_ref`（仅此）
6. `s-*/layout.mei`：`section_layout` + `section_shell`（或裸 `content_panel` shell）
7. `content.mei`：`content_panel` + `grid` + blocks（组件 / metric / 业务宏）
8. T2：同构树 + `link_decl`；叶子按 **page_instance** 理解（实现名可能仍是 `page_instance`）
9. world / map / view 外置，content 层 `*_ref` 引用

## typed ref

- 结构：`plane_ref` / `region_ref` / `section_ref` / `panel_ref` / `assembly_ref`
- 数据：`dataset_ref` / `metric_ref` / `resource_ref`
- config：`theme_ref` / `source_ref` / `basemap_ref` / `ops_param_ref`

注意：`world_ref` 是 world 对象引用，不是资源 id 选择器。组件 `props` 只消费当前账本内本地 id。

## 文件组织

```text
app.toml
src/
  stage/home.stage.mdx
  scene/home/
    assembly.mei
    t0/ ... t1/ ... t2/
      layout.mei
      r-*/layout.mei
      r-*/s-*/layout.mei
      r-*/s-*/content.mei
    t2/links/*.mei          # link_decl
  world/  map/  view/
  data/metrics/
```

- 不要新建 `overlay/**`、`content/**` 旁路树、`planes/regions/sections` 硬目录
- `stock/templates/**` 是公共模板面
- 新组件 / 新模板任务切到 `.mei/knowledge/author/extension-authoring.md`

## 明确删除的作者路径

| 旧写法 | 替代 |
|--------|------|
| `frame` / `frame.add_panel` | `plane_layout` → … → `content_panel` |
| 默认 `flex(...)` | `grid(...)` |
| `titled_shell` | `section_shell` |
| `page_instance` / 推荐 `page_instance` | `scene` 树 + T2 **page_instance** |
| micro-layout 结构层 | `grid` + slot + content |
| `row_budgets` 撑高 | region `Nfr` + Fill-down |

## 版本口径

- 应用入口：`app.toml`（不是 `app.mei` / 旧 `app(...)` / `entry.main`）
- 默认 Stage：`app.toml` 的 `default_stage`；Registry 枚举 Stage MDX
- 页面选路：Stage MDX `@scene` → Scene assembly
- 编辑侧自动化：`mei-toolchain mcp describe --surface author --json`
