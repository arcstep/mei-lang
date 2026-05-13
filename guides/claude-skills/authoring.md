# MeiLang Authoring

## 主干规则

当前最稳定的作者态主干是：

- `app(..., entries=[entry(...)])`
- `scene(id=..., world=..., flow=..., frame=...)`
- `world(id=..., resources=[...])`
- `flow(id=..., ...)`
- `frame(id=..., layout=...)`
- `frame.add_panel(...)`
- `component(...)`

复杂页面优先保持：

1. `app.entries -> active entry`
2. `scene -> world / flow / frame`
3. `frame -> panel.blocks`

## 当前推荐写法

1. 先写 `app(...)`、`default_scene`、`entries=[entry(...)]`
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
- `scene_ref(...)` 当前用于把整份 `SceneContract` 注入组件

## 兼容层说明

以下写法当前仍可运行，但不应作为新脚本默认主线：

- `app.add_scene(...)`
- `scene.set_world(...)`
- `scene.set_flow(...)`
- `scene.set_frame(...)`

## 当前不要写成已支持

- `entity_ref(...)`
- `data_ref(...)`
- `metric_ref(...)`
- `frame_ref(...)`
- `component_ref(...)`

## 自检

- 只使用当前已实现语法
- `app -> entry -> scene -> frame -> panel -> blocks` 结构清晰
- `area` 与 `layout.areas` 一致
- 组件使用 manifest 中真实存在的 type key
- `props` 中引用的资源 id 实际存在
