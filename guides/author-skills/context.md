# MeiLang Context

## 必读来源

按优先级读取：

1. 当前任务相关的 `.mei` 文件（优先 `assembly.mei` / `layout.mei` / `content.mei`）
2. 对应 example：`pretty-panels`、`mini-park` 的 `src/scene/**`
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
- 不要默认回源 `docs/` 找另一套规则；优先 packaged knowledge
- bootstrap / config / theme / upload 优先 `workspace-config-reference.md`
- 新组件 / 新模板优先 `extension-authoring.md`

## 需要确认的上下文

编辑前先确认：

1. `default_scene` 与 `navigation` / `assembly_ref` 指向哪个 `scene`
2. 结构是否落在 `t*/r-*/s-*` 链：`plane_layout` → `region_layout` → `section_layout` → `content_panel`
3. `region_layout.sections` 是否全是 `section_ref`（未直挂 content）
4. section 是否用 `section_shell`（或裸 shell），且无手写 height / `row_budgets`
5. content 是否 Fill-down（`1fr` / fill props），未设 px 高度
6. 当前 scene 可见的 world resources / metric id（必要时再查 inspect/query）
7. 目标组件或模板是否已在 `.stock/components` / `.stock/templates` 公开 contract 中
8. T2 是否按 **page_instance** 理解（勿再走 `page_instance` 推荐路径）

## 何时读组件实现

只有在下面几种情况才读组件实现：

- 公共 component contract、pack guide 与 example 仍不能回答 props 结构
- 需要确认 renderer 私有行为，而这些行为当前尚未公开承诺
- 需要验证某个 edge case 是否只是实现细节，而不是公共 contract

## 何时不要扩读

不要因为写一个 `.mei` 文件就去扫：

- 无关 app 的全部 scene
- 全部 stock 实现源码
- 历史 v1 frame/panel 文档
