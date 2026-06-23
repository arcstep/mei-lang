# MeiLang Authoring

## 当前边界

作者态的默认协作方式已经切换为：

- **外部开发工具**负责读写文件、重构、搜索、多文件修改
- **MeiLang CLI / LSP** 负责 diagnostics，以及在需要 runtime/world 事实时提供 inspect/query 能力
- **MeiLang 宿主内置 AI** 不再承担编辑侧主线

所以，写 `.mei` 时不要再假设存在这些宿主内作者态工具：

- `skill_list`
- `skill_read`
- `rewrite_current_mei`

## 推荐工作流

### 1. 先读目标源码与知识

优先读取：

1. 当前 `main.mei` / 目标 scene 文件
2. 相关 `.stock/templates` / `.stock/components`
3. `syntax-rules.md`、`dsl-reference.md`、`namespace-reference.md`、`components-reference.md`、`context.md`
4. `.mei/knowledge/author/workspace-config-reference.md`
5. `.mei/knowledge/author/components/*`、`.mei/knowledge/author/templates/*`
6. 相近 example

作者态默认应是 **source-first + knowledge-first**。  
不要先把 `inspect summary` / `workspace summary` 当成当前源码的替代品。

### 2. 再建立机器反馈锚点

优先跑这些命令：

1. `mei-toolchain check --app <app> --source-root <workspace> --json`
2. `mei-toolchain workspace summary --source-root <workspace> --json`（需要 workspace 级 discover 视图时）

只有当源码里没有答案、需要 world/runtime 事实时，再补：

1. `mei-toolchain query dataset --app <app> --source-root <workspace> --id <dataset_id> --json`
2. `mei-toolchain query metric --app <app> --source-root <workspace> --id <dataset_id> --json`
3. `mei-toolchain runtime peek --app <app> --source-root <workspace> --json`
4. `mei-toolchain inspect world --app <app> --source-root <workspace> --json`
5. `mei-toolchain inspect inventory --app <app> --source-root <workspace> --json`
6. `mei-toolchain inspect summary --app <app> --source-root <workspace> --json`

不要因为能调到 runtime 工具，就跳过对当前 `.mei` 的直接阅读。

### 3. 最后才落盘

写文件前尽量先确认：

- 改动落在哪个 `scene`
- 用到的 `resource id` / `metric id` 是否真实存在
- layout area / component props 是否与现有 contract 对齐

改完后至少再跑一次：

- `mei-toolchain check --app <app> --source-root <workspace> --json`

### 3.1 改完后如何让运行中的宿主生效

默认不要先说“重启服务”。更稳妥的顺序是：

1. **单个 scene / board / drilldown target 修改**
   - 优先对运行中的宿主发 scoped build
   - 示例：

```bash
curl -X POST http://127.0.0.1:9527/api/host/build \
  -H 'Content-Type: application/json' \
  -d '{
    "appId": "zhifa",
    "mode": "build",
    "sceneId": "inspection_week_analytics_board",
    "targetFile": "scenes/02-行政检查.board.mei"
  }'
```

2. **多个相关 target、dataset/metric 绑定、upload source 或数据输入变化**
   - 优先跑单 app 增量 `prebuild`

```bash
mei-toolchain prebuild --workspace <workspace> --app <app> --json
```

3. **只想先把热点访问链恢复 ready**
   - 优先 `--hot-only`

```bash
mei-toolchain prebuild --workspace <workspace> --app <app> --hot-only --json
```

4. **发布前 / access-only 校验**
   - 优先 `--verify`，或用 `fail-fast-verify` 启动

```bash
mei-toolchain prebuild --workspace <workspace> --app <app> --verify --json
```

只有当你改的是：

- `mei-host-web` / `mei-toolchain` Rust 实现
- `app/assets/**` bundle
- 启动参数、端口、鉴权、startup policy

时，才把“重启宿主”当成默认动作。

### 3.2 不要误判 access-only 的运行方式

当前 access strict AOT / artifact-first 的边界是：

- 页面请求不会现场触发 compile
- 缺 artifact 时应先补 build / prebuild
- `/api/host/ready` 才是访问态是否 ready 的正式锚点

因此：

- “改了 `.mei` 后页面没立刻生效”不应直接归因为“服务该重启了”
- 先检查 scoped build / prebuild 是否成功、artifact 是否已更新、ready 是否恢复

### 3.3 性能优化要回到长期台账比较

若任务涉及：

- “为什么局部修改反馈还是慢”
- “这次 scoped build 是否真的缩了范围”
- “这次整改是否值得保留到长期主线”

不要只看单次体感，优先回到长期采样与比较：

```bash
node ./scripts/host-perf-sample.mjs \
  --scenario-file ./scripts/perf-scenarios/zhifa.json
```

```bash
node ./scripts/host-perf-report.mjs \
  --sample <sample.jsonl> \
  --scenario-file ./scripts/perf-scenarios/zhifa.json \
  --mode auto
```

比较模式含义固定为：

- `auto`：先比 pinned baseline；无 pinned 时回退到 ledger 最近历史
- `latest`：只比最近历史
- `pinned`：只比固定基线；适合发布前和 compile 专项

读报告时优先回答四个问题：

1. 退化落在 `compile_ms` 还是 `metric_total_ms`
2. `local_edit_feedback_ms` 是否真的下降
3. `dependency_graph_build_ms` / `catalog_compile_ms` 是否仍是主矛盾
4. scoped build / hot-only prebuild 是真的缩了 scope，还是只是缓存碰巧命中

### 3.4 发布前的最小动作

若任务已经进入“准备交付 / 发布 / smoke”：

1. 跑 `mei-toolchain check`
2. 跑 `prebuild --verify`
3. 需要严格准入时，用 `serve --startup-policy fail-fast-verify`
4. 涉及性能整改时，补一轮 `host-perf-sample` + `host-perf-report`

## 当前主线

当前最稳定的应用入口与场景组织方式是：

1. `main.mei` 中声明唯一 `app(...)`
2. 单文件时直接 inline `scene(...)`
3. 多文件时用 `app_add_scene(scene = scene_ref(scene_file = "...", scene_id = "..."))`
4. `scene(...)` 绑定当前 active `world / flow / frame`
5. `frame.add_panel(...)` / `panel(...)` 组织 UI
6. `component(...)` 与 `metric_card(...)` 消费当前 scene 可见对象

复杂页面优先保持：

1. `app + scene` 路由（`scene_id` 与 `default_scene` 对齐）
2. `scene -> world / flow / frame`
3. `frame -> panels / blocks`
4. 数据先进入 `world` 账本，再由 `dataset_ref(...)` / `metric_ref(...)` / `resource_ref(...)` 消费

## 当前推荐写法

1. 先写 `app(...)` 与 `default_scene`
2. 单文件场景直接写 `scene(...)`
3. 外部场景用 `app_add_scene(scene = scene_ref(...))` 注册
4. 资源、dataset、metric、template clone 先进入 owner 账本
5. UI 骨架放进具名 `frame(id=..., layout=...)`
6. 区块使用 `frame.add_panel(...)` 或 `panel(...)`
7. 组件输入统一放进 `props`，并优先消费本地可见的 typed refs
8. 主题、upload source、basemap、ops param 统一走 `.mei-config.json -> ops.*` 与 `*_ref(...)`

## typed ref 主线

当前作者态应优先把下面这些名字当作正式主线：

- 结构槽位：`scene_ref(...)`、`world_ref(...)`、`flow_ref(...)`、`frame_ref(...)`
- 集合复用：`panel_ref(...)`、`metric_card_ref(...)`
- 数据绑定：`dataset_ref(...)`、`metric_ref(...)`、`resource_ref(...)`

其中要特别记住：

- `world_ref(...)` 是 world 对象引用，不再表示 world 内某个资源 id。
- 组件 `props` 中优先消费 **本地可见 id** 的 `dataset_ref(...)` / `metric_ref(...)` / `resource_ref(...)`。
- 若要跨文件使用外部 dataset / metric / resource，应先通过 world 引入或 scene owner 绑定进入当前账本，再在组件中按本地 id 消费。

## 布局规则

- 当前稳定布局原语是 `grid(...)` 与 `flex(...)`
- `grid(areas=...)` 时，`panel.area` 必须对齐命名区域
- 自动流布局使用 `area = "auto"`

## 绑定规则

- `props` 是唯一稳定绑定表面
- 组件 props 的稳定值来源优先是 `dataset_ref(...)`、`metric_ref(...)`、`resource_ref(...)`、`scene_ref(...)`
- `theme_ref(...)` / `source_ref(...)` / `basemap_ref(...)` / `ops_param_ref(...)` 是当前 config refs 主线
- `world_ref(...)` 不作为资源选择器使用
- `panel(base = panel_ref(...))` 与 `metric_card(base = metric_card_ref(...))` 是当前模板克隆主线

## 文件组织

- 应用入口优先使用 `main.mei`
- 外部场景优先使用 `scene_ref(scene_file = ...)` 并在 `app_add_scene(...)` 中注册
- `.stock/templates/**` 是公共模板消费面；不要把 `workspaces/**` 相对路径当成 standalone 默认写法
- 不要把运行态临时问答结果直接回写成正式作者态源码
- `inspect summary` / `workspace summary` 更适合作为作者态路由摘要，而不是源码理解主入口
- 新组件 / 新模板任务先切到 `.mei/knowledge/author/extension-authoring.md`，不要假装它仍是普通 author 任务

## 版本与迁移

- `entry(...)` 与 `app(..., entries=...)` 不再作为新作者态默认主线
- `scene_file_ref(...)` / `world_file_ref(...)` / `frame_file_ref(...)` 仅保留兼容/迁移语义，不再作为公开主示例
- 页面与编译选路统一为 **`app + scene`**（`default_scene` + inline scene 或 `app_add_scene(scene = scene_ref(...))`）
- 编辑侧自动化接线统一看 `mei-toolchain mcp describe --surface author --json`
