# MeiLang Syntax Rules

## 当前可写主线

当前应优先使用：

- `app(...)`
- `app_add_scene(...)`
- `scene(...)`
- `world(...)`
- `resource(...)`
- `flow(...)`
- `frame(...)`
- `frame.add_panel(...)`
- `panel(...)`
- `component(...)`
- `metric_card(...)`
- `grid(...)`
- `flex(...)`
- `doc.markdown(...)`
- `scene_ref(...)`
- `world_ref(...)`
- `flow_ref(...)`
- `frame_ref(...)`
- `panel_ref(...)`
- `metric_card_ref(...)`
- `dataset_ref(...)`
- `metric_ref(...)`
- `resource_ref(...)`

当前主线还应遵守：

- app 入口统一收口为 `default_scene` + inline `scene(...)` 或 `app_add_scene(scene = scene_ref(...))`
- `scene` 当前优先只收敛一个主 `world / flow / frame`
- 单例槽位优先使用 typed ref：`scene.world = world_ref(...)`、`scene.frame = frame_ref(...)`
- `frame` 当前优先通过 `area` 组织多个 `panel`
- `frame / panel` 的 `title` 默认可省略
- `panel(base = panel_ref(...))` / `metric_card(base = metric_card_ref(...))` 是当前模板克隆主线
- `*_ref(...)` 用来引用当前组合后作用域里的对象或 owner 槽位目标

## 组件绑定

- 组件输入统一写入 `props`
- `dataset_ref(...)` / `metric_ref(...)` / `resource_ref(...)` 作为 `props` 的稳定值来源
- `scene_ref(...)` 可作为整份 scene contract 的值来源
- 不再发明与 `props` 平行的第二套绑定语法
- 不在组件 `props` 中直接写跨文件 locator；外部对象应先进入当前 world/scene 账本
- `mapping`、`headers`、`columns` 等组件私有字段属于组件 contract，不是独立 DSL 函数

## 当前已实现边界

- `scene_ref(...)`：可用，当前用于 `app_add_scene(...)` 与 scene contract 注入
- `world_ref(...)`：可用，当前用于 scene owner 槽位绑定 world
- `frame_ref(...)` / `panel_ref(...)`：可用，当前用于 frame/panel 模板与跨文件复用
- `dataset_ref(...)` / `metric_ref(...)` / `resource_ref(...)`：可用，当前作为组件 props 的主要对象引用
- `grid / flex`：可用
- `grid areas`：可用

## 当前不要误写成已实现

- `entry(...)`
- `app(..., entries=[entry(...)])`
- `scene_file_ref(...)` / `world_file_ref(...)` / `frame_file_ref(...)` 作为公开主语法
- `world_file_ref(...)`
- `flow_file_ref(...)`
- `frame_file_ref(...)`
- `entity_ref(...)`
- `data_ref(...)`
- richer `component` contract
- capability registry
- profile 包装层

## 扩展规则

- dataset 当前是 `world.resource(kind = "dataset")`
- chart 当前优先通过外部组件接入
- capability 当前先看 contract / registry 设计，不当作现成作者态语法
- `world_ref(...)` 不再作为资源 id 选择器；资源绑定请用 `dataset_ref(...)` / `metric_ref(...)` / `resource_ref(...)`

## 写作要求

- 先主干，后细节
- 先 current syntax，再考虑设计中的未来能力
- 能用现有 example 证明的，才写进脚本规范
