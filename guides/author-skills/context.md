# MeiLang Context

## 必读来源

按优先级读取：

1. 当前任务相关的 `.mei` 文件
2. 对应 example
3. `.mei/profiles/author.md`
4. 同目录的 `syntax-rules.md`、`dsl-reference.md`、`namespace-reference.md`、`components-reference.md`
5. `.mei/knowledge/author/workspace-config-reference.md`
6. `.mei/knowledge/author/components/*`、`.mei/knowledge/author/templates/*`
7. `.mei/knowledge/author/authoring-overview.md`、`workflow-recipes.md`、`build-debug-ops.md`
8. `.mei/knowledge/author/extension-authoring.md`
9. `mei-toolchain knowledge --surface author --include-content --json`
10. 仍不足时再读 `.stock/**/README.md`
11. 最后才看相关 component 实现

## 读取原则

- 只读与当前任务直接相关的文件
- 先看 workspace-local knowledge，再看实现细节
- 先看例子，再抽象规则
- `inspect summary` / `workspace summary` 只当路由摘要，不当源码替代品
- 不要默认回源 `docs/` 目录去找另一套规则；优先使用 packaged knowledge
- bootstrap / config / theme / upload 任务优先看 `workspace-config-reference.md`
- 新组件 / 新模板任务优先看 `extension-authoring.md`

## 需要确认的上下文

编辑前先确认：

1. 当前 `default_scene` 指向哪个 `scene`，以及是否存在 `app_add_scene(scene = scene_ref(...))`
2. 当前场景里有哪些 `world.resources`（必要时再查 inspect/query）
3. 当前 `frame.layout` 使用的是 `grid` 还是 `flex`
4. `panel.area` 与 `layout.areas` 是否一致
5. 目标组件或模板是否已在 `.stock/components` / `.stock/templates` 的公开 contract 中出现
6. 组件需要的 `props` 结构、example 与 template clone 路径是什么

## 何时读组件实现

只有在下面几种情况才读组件实现：

- 公共 component contract、pack guide 与 example 仍然不能回答 props 结构
- 需要确认 renderer 私有行为，而这些行为当前尚未公开承诺
- 需要验证某个 edge case 是否只是实现细节，而不是公共 contract

## 何时不要扩读

不要因为写一个 `.mei` 文件就去扫：

- 无关 example
- 旧 DSL 文档
- 与当前任务无关的 Rust 模块
- 与 standalone author package 无关的源码仓库背景文档
