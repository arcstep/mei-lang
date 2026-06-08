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
2. 相关模板 / `_components`
3. `syntax-rules.md`、`components-reference.md`、`context.md`
4. 相近 example

作者态默认应是 **source-first + knowledge-first**。  
不要先把 `inspect summary` / `workspace summary` 当成当前源码的替代品。

### 2. 再建立机器反馈锚点

优先跑这些命令：

1. `mei check --app <app> --json`
2. `mei workspace summary --source-root <dir> --json`（需要 workspace 级 discover 视图时）

只有当源码里没有答案、需要 world/runtime 事实时，再补：

1. `mei query dataset --app <app> --id <dataset_id> --json`
2. `mei query metric --app <app> --id <dataset_id> --json`
3. `mei runtime peek --app <app> --json`
4. `mei inspect world --app <app> --json`
5. `mei inspect inventory --app <app> --json`
6. `mei inspect summary --app <app> --json`

不要因为能调到 runtime 工具，就跳过对当前 `.mei` 的直接阅读。

### 3. 最后才落盘

写文件前尽量先确认：

- 改动落在哪个 `scene`
- 用到的 `resource id` / `metric id` 是否真实存在
- layout area / component props 是否与现有 contract 对齐

改完后至少再跑一次：

- `mei check --app <app> --json`

## 主干规则

当前最稳定的作者态主干是：

- `app(..., default_scene=..., scene=...)` 与 `app.add_scene(...)` / `scene_file_ref(...)`
- `scene(id=..., world=..., flow=..., frame=...)`
- `world(id=..., resources=[...])`
- `flow(id=..., ...)`
- `frame(id=..., layout=...)`
- `frame.add_panel(...)`
- `component(...)`

复杂页面优先保持：

1. `app + scene` 路由（`scene_id` 与 `default_scene` 对齐）
2. `scene -> world / flow / frame`
3. `frame -> panel.blocks`

## 当前推荐写法

1. 先写 `app(...)`、`default_scene`，用 `app.add_scene(...)` / `app(scene=scene_file_ref(...))` 注册场景路由
2. 再定义 `scene(id=..., world=..., flow=..., frame=...)`
3. 资源放进具名 `world(id=..., resources=[...])`
4. UI 骨架放进具名 `frame(id=..., layout=...)`
5. 区块使用 `frame.add_panel(...)`
6. 组件统一放进 `panel.blocks`
7. 组件输入统一放进 `props`

## 布局规则

- 当前稳定布局原语是 `grid(...)` 与 `flex(...)`
- `grid(areas=...)` 时，`panel.area` 必须对齐命名区域
- 自动流布局使用 `area = "auto"`

## 绑定规则

- `props` 是唯一稳定绑定表面
- `world_ref(...)` 当前主要用于引用 `world.resources[id]`

## 文件组织

- 应用入口优先使用 `main.mei`
- 外部场景使用 `scene_file_ref(...)` 并在 `app` 侧注册路由
- 不要把运行态临时问答结果直接回写成正式作者态源码
- `inspect summary` / `workspace summary` 更适合作为作者态路由摘要，而不是源码理解主入口

## 版本与迁移

- 已移除 `entry(...)` 与 `app(..., entries=...)`
- 页面与编译选路统一为 **`app + scene`**（`scene_id` / `default_scene` / `?scene=`）
- 编辑侧若需要更自动化的机器对接，优先看 `mei mcp describe --surface author --json`（兼容 `editor`）
