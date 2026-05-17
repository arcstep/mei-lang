---
name: meilang-author
description: 创建、修改、审查或修复 MeiLang `.mei` 时使用。先用 `skill_list` / `skill_read` 读规则，再用 `read_file` / `resource_*` 按需取证据；不要从旧 DSL 或未实现设计反推语法。
---

# MeiLang Author（短入口）

MeiLang 是 scene-first 作者态 DSL（`scene / world / flow / frame`）。**默认 system 只含索引**；写稿前按需读取：

1. `skill_read("authoring.md")`、`skill_read("syntax-rules.md")` 等（见 `skill_list`）。
2. 目标 `.mei`：`read_file("<app>/…")`（相对 workspace）。
3. 资源细节：`resource_list` → `resource_get`；运行态：`resource_runtime_peek`。

## 触发

- `.mei` / MeiLang / `scene` / `world` / `frame` / `panel` / `component` / dataset / chart 等。

## 禁止

- 不猜组件名、资源 id、布局 area。
- 不把设计文档里的未来能力写成已支持语法。
