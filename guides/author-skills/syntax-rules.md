# MeiLang Syntax Rules

## 当前可写主线

当前应优先使用：

- `app(...)`
- `entry(...)`
- `scene(...)`
- `world(...)`
- `resource(...)`
- `flow(...)`
- `frame(...)`
- `frame.add_panel(...)`
- `panel(...)`
- `component(...)`
- `grid(...)`
- `flex(...)`
- `doc.markdown(...)`
- `scene_file_ref(...)`
- `world_ref(...)`
- `scene_ref(...)`

当前主线还应遵守：

- `default_scene` + `entries = [entry(...)]` 表达 app 默认入口
- `scene` 当前优先只收敛一个主 `world / flow / frame`
- `frame` 当前优先通过 `area` 组织多个 `panel`
- `frame / panel` 的 `title` 默认可省略
- `*_file_ref(...)` 用来引用其他文件
- `*_ref(...)` 用来引用当前组合后作用域里的对象

## 组件绑定

- 组件输入统一写入 `props`
- `world_ref(...)` 作为 `props` 的值来源
- `scene_ref(...)` 作为 `props` 的值来源
- 不再发明与 `props` 平行的第二套绑定语法

## 当前已实现边界

- `world_ref(...)`：部分实现，当前主要引用 `world.resources[id]`
- `scene_ref(...)`：部分实现，当前是整份 scene contract 注入
- `scene_file_ref(...)`：可用，当前用于 app 绑定外部 scene 文件
- `grid / flex`：可用
- `grid areas`：可用

## 当前不要误写成已实现

- `world_file_ref(...)`
- `flow_file_ref(...)`
- `frame_file_ref(...)`
- `entity_ref(...)`
- `data_ref(...)`
- `metric_ref(...)`
- `frame_ref(...)`
- richer `component` contract
- `chart.mapping`
- capability registry
- profile 包装层

## 扩展规则

- dataset 当前是 `world.resource(kind = "dataset")`
- chart 当前优先通过外部组件接入
- capability 当前先看 contract / registry 设计，不当作现成作者态语法

## 写作要求

- 先主干，后细节
- 先 current syntax，再考虑设计中的未来能力
- 能用现有 example 证明的，才写进脚本规范
