---
name: meilang-author
description: 在用户要求创建、修改、审查或修复 MeiLang `.mei` 脚本时使用。先读取当前实现文档、相关 example 和必要源码，再按本目录规则编辑；不要从旧 DSL、Rust 细节或未实现设计反推可写语法。
---

# MeiLang Author

MeiLang 是一个 scene-first 的作者态 DSL，用来装配 `scene / world / flow / frame`。
当前智能体写脚本时，应以已实现语法为准，优先使用声明式 `entry/scene/world/flow/frame` 绑定与 `panel(...)`、`component(...)`、`props`、`world_ref(...)`、`scene_ref(...)`。

## 触发

用户提到以下任一内容时使用本 skill：

- MeiLang / `.mei`
- `scene` / `world` / `flow` / `frame`
- `panel(...)` / `component(...)`
- `world_ref(...)` / `scene_ref(...)`
- dataset / chart / capability 示例装配

## 必要输入

1. 目标文件或应用目录
2. 任务目标
3. 当前已实现语法边界

## 阅读顺序

1. `authoring.md`
2. `context.md`
3. `syntax-rules.md`
4. `namespace-reference.md`
5. `components-reference.md`
6. `dsl-reference.md`

## 工作流程

1. 先确认任务落点和相关 example
2. 只读取与任务直接相关的 `.mei`、组件和当前实现文档
3. 声明式入口优先：`app(entries=[entry(...)]) + scene(world/frame/flow=...)`
4. 组件输入统一走 `props`
5. 资源绑定优先使用当前已实现的 `world_ref(...)` / `scene_ref(...)`
6. 不把 `data_ref(...)`、`metric_ref(...)`、`frame_ref(...)` 当成当前可写主线
7. 修改后验证相关文件与 diagnostics

## 禁止

- 不从 old `mei-lang` DSL 直接搬语法
- 不从 Rust / prelude 细节反推作者态写法
- 不猜组件名、资源 id、布局 area
- 不把设计文档里的未来能力写成当前已支持语法
- 不把兼容层 `app.add_scene(...)` / `scene.set_*` 当作新脚本默认主线
