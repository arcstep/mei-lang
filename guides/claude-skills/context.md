# MeiLang Context

## 必读来源

按优先级读取：

1. 当前任务相关的 `.mei` 文件
2. 对应 example
3. `docs/mei-lang/implementation/syntax/`
4. `docs/mei-lang/implementation/extensions/`
5. 必要时读取相关 web component 或宿主代码

## 读取原则

- 只读与当前任务直接相关的文件
- 先看当前实现文档，再看代码
- 先看例子，再抽象规则

## 需要确认的上下文

编辑前先确认：

1. 当前 entry 指向哪个 `scene`（scene id 或外部 `.mei` 文件）
2. 当前场景里有哪些 `world.resources`
3. 当前 `frame.layout` 使用的是 `grid` 还是 `flex`
4. `panel.area` 与 `layout.areas` 是否一致
5. 目标组件是否已经在 manifest 中注册
6. 组件需要的 `props` 结构是什么

## 何时读组件实现

只有在下面几种情况才读组件实现：

- 需要确认 `props` 字段名
- 需要确认组件消费的是 document、dataset 还是整份 scene
- 需要确认 example 已经验证过的 contract

## 何时不要扩读

不要因为写一个 `.mei` 文件就去扫：

- 无关 example
- 旧 DSL 文档
- 与当前任务无关的 Rust 模块
