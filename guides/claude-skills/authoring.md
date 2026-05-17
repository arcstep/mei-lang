# MeiLang Authoring

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

- 应用入口优先使用 `main.mei`；外部场景使用 `scene_file_ref(...)` 并在 `app` 侧注册路由。

## 版本与迁移

- 已移除 `entry(...)` 与 `app(..., entries=...)`；页面与编译选路统一为 **`app + scene`**（`scene_id` / `default_scene` / `?scene=`）。
