---
name: meilang-author
description: 创建、修改、审查或修复 MeiLang `.mei` 时使用。编辑侧主线是“文档 + mei CLI + mei-lsp + 外部开发工具”，而不是宿主内置 agent tool。
---

# MeiLang Author（短入口）

这个 skill 包当前是 **MeiLang toolchain capability catalog** 导出的作者态入口之一；它不应与宿主内置提示、访问态 prompt 或其它 AI 工具维护漂移的平行规则。

MeiLang 是 scene-first 作者态 DSL（`scene / world / flow / frame`）。

编辑侧主线先记住三条：

1. 先看仓库文档与本 skill 附带文档，不要假设宿主内有 `skill_list` / `skill_read`。
2. 先用 `mei` CLI / `mei-lsp` 获取 diagnostics、world、inventory、query 结果，再做编辑。
3. 文件写入由外部开发工具直接完成；`mei-lang` 提供的是语义/编译/查询接口，不是默认作者态聊天宿主。
4. `components/templates/world能力/宿主扩展能力` 应继续以 toolchain 导出的 catalog 为准，不要假设宿主内部另有一套更完整说明。

## 推荐顺序

1. 读 `authoring.md`、`syntax-rules.md` 和 `../../README.md`。
2. 跑 `mei check --app <app> --json` 看 diagnostics。
3. 跑 `mei inspect world --app <app> --json` / `mei inspect inventory --app <app> --json` 理解 scene/world/resources。
4. 涉及数据问题时，跑：
   - `mei query dataset --app <app> --id <dataset_id> --json`
   - `mei query metric --app <app> --id <dataset_id> --json`
   - `mei runtime peek --app <app> --json`
5. 再按需读取目标 `.mei` 与相关模板文件。

## 常用命令

- `mei check --app <app> --scene <scene> --json`
- `mei inspect world --app <app> --scene <scene> --json`
- `mei inspect inventory --app <app> --scene <scene> --json`
- `mei query dataset --app <app> --id <dataset_id> --scene <scene> --json`
- `mei query metric --app <app> --id <dataset_id> --scene <scene> --json`
- `mei runtime peek --app <app> --scene <scene> --json`
- `mei mcp describe --surface editor --json`

## 触发

- `.mei` / MeiLang / `scene` / `world` / `frame` / `panel` / `component` / dataset / chart 等。

## 禁止

- 不猜组件名、资源 id、布局 area。
- 不把设计文档里的未来能力写成已支持语法。
- 不再依赖 `skill_list` / `skill_read` / `rewrite_current_mei` 这类宿主内作者态工具。
