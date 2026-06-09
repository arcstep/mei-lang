---
name: meilang-author
description: 创建、修改、审查或修复 MeiLang `.mei` 时使用。编辑侧主线是“文档 + mei-toolchain CLI + mei-lsp + 外部开发工具”，而不是宿主内置 agent tool。
---

# MeiLang Author（短入口）

这个 skill 包当前是 **MeiLang toolchain capability catalog** 导出的作者态入口之一；它对应的是 `author` profile，不应与宿主内置提示、访问态 prompt 或其它 AI 工具维护漂移的平行规则。

MeiLang 是 scene-first 作者态 DSL（`scene / world / flow / frame`）。

编辑侧主线先记住四条：

1. 先读当前 `.mei` 源码、相关文档和例子，不要把 runtime tool 当成作者态主入口。
2. `mei-toolchain check` / `mei-lsp` 是作者态最重要的机器反馈；`inspect/query/runtime` 只在需要 runtime/world 事实时补充。
3. 文件写入由外部开发工具直接完成；`mei-lang` 提供的是语义/编译/查询接口，不是默认作者态聊天宿主。
4. `components/templates/world能力/宿主扩展能力` 应继续以 toolchain 导出的 catalog 为准，不要假设宿主内部另有一套更完整说明。

## 推荐顺序

1. 读 `authoring.md`、`syntax-rules.md`、`components-reference.md`、`context.md` 和 `../../README.md`。
2. 直接读当前任务相关的 `.mei` 源码、引用的 scene/template/_components，不要先把 `summary` 当成源码替代品。
3. 跑 `mei-toolchain check --app <app> --json` 看 diagnostics；需要更紧密的编辑反馈时走 `mei-lsp`。
4. 如需先理解当前 workspace 有哪些 app、发现规则和别名，再跑 `mei-toolchain workspace summary --source-root <dir> --json`。
   - 这个命令更适合作为 discover/layout/semantic 路由摘要，不等于完整语义有效性校验。
5. 只有在源码里没有答案时，再跑 `mei-toolchain inspect world --app <app> --json` / `mei-toolchain inspect inventory --app <app> --json` / `mei-toolchain inspect summary --app <app> --json`。
   - `inspect summary` 适合快速判断 app/scene 的业务轮廓，但不替代直接阅读目标 `.mei`。
6. 涉及数据与运行态事实时，跑：
   - `mei-toolchain query dataset --app <app> --id <dataset_id> --json`
   - `mei-toolchain query metric --app <app> --id <dataset_id> --json`
   - `mei-toolchain runtime peek --app <app> --json`
7. 最后再在外部开发工具里修改源码，并回到 `mei-toolchain check` / `mei-lsp` 验证。

## 常用命令

- `mei-toolchain check --app <app> --scene <scene> --json`
- `mei-toolchain workspace summary --source-root <dir> --json`
- `mei-toolchain inspect world --app <app> --scene <scene> --json`
- `mei-toolchain inspect inventory --app <app> --scene <scene> --json`
- `mei-toolchain inspect summary --app <app> --scene <scene> --json`
- `mei-toolchain query dataset --app <app> --id <dataset_id> --scene <scene> --json`
- `mei-toolchain query metric --app <app> --id <dataset_id> --scene <scene> --json`
- `mei-toolchain runtime peek --app <app> --scene <scene> --json`
- `mei-toolchain mcp describe --surface author --json`
- `mei-toolchain mcp describe --surface editor --json`

## 触发

- `.mei` / MeiLang / `scene` / `world` / `frame` / `panel` / `component` / dataset / chart 等。

## 禁止

- 不猜组件名、资源 id、布局 area。
- 不把设计文档里的未来能力写成已支持语法。
- 不再依赖 `skill_list` / `skill_read` / `rewrite_current_mei` 这类宿主内作者态工具。
- 不把 `inspect summary` / `workspace summary` 当成当前源码的替身。
