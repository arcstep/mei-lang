# MeiLang Authoring

## 当前边界

作者态的默认协作方式已经切换为：

- **外部开发工具**负责读写文件、重构、搜索、多文件修改
- **MeiLang CLI / LSP** 负责 diagnostics、world/inventory 查询、dataset/metric/runtime 查询
- **MeiLang 宿主内置 AI** 不再承担编辑侧主线

所以，写 `.mei` 时不要再假设存在这些宿主内作者态工具：

- `skill_list`
- `skill_read`
- `rewrite_current_mei`

## 推荐工作流

### 1. 先建立语义锚点

优先跑这些命令：

1. `mei check --app <app> --json`
2. `mei inspect world --app <app> --json`
3. `mei inspect inventory --app <app> --json`

涉及数据与运行态时再补：

1. `mei query dataset --app <app> --id <dataset_id> --json`
2. `mei query metric --app <app> --id <dataset_id> --json`
3. `mei runtime peek --app <app> --json`

### 2. 再读目标源码

按需读取：

- 目标 `main.mei`
- 当前 `scene` 文件
- 相关模板 / `_components`
- 文档中的稳定规则与边界说明

不要在没看 diagnostics / world / inventory 的前提下凭印象大改。

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

## 版本与迁移

- 已移除 `entry(...)` 与 `app(..., entries=...)`
- 页面与编译选路统一为 **`app + scene`**（`scene_id` / `default_scene` / `?scene=`）
- 编辑侧若需要更自动化的机器对接，优先看 `mei mcp describe --surface editor --json`
