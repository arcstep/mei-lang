---
name: meilang-access
description: 回答 MeiLang 运行态、dataset、metric、resource、query_state 与 runtime trace 问题时使用。访问侧主线是 world-first + eval-first，而不是继续写 `.mei`。
---

# MeiLang Access（短入口）

这个 skill 包是 **MeiLang toolchain capability catalog** 导出的访问态短入口。  
它对应的公开 profile 与 knowledge surface 都是 `access`。

访问态主线是：

1. **先读 world / inventory / runtime 摘要**
2. **再合并浏览器 `query_state` 与当前 scope**
3. **优先用有界 dataset/metric/runtime 工具回答**
4. **只有需要 verbatim DSL 证据时才小范围读文件**

## Workspace-local 入口

在已安装 runtime 的 workspace 中，优先按下面顺序读取：

1. `.mei/profiles/access.md`
2. `.mei/skills/meilang-access/workflow.md`
3. `.mei/catalog/access-surface.json`
4. `mei-toolchain knowledge --surface access --source-root <workspace> --include-content --json`

如果你现在看到的是源码包目录，上述文件会分别来自：

- `guides/access-profile.md`
- `guides/access-skills/*.md`

但对独立 workspace 使用者来说，**公开消费面始终是 `.mei/`，不是源码仓路径。**

## 推荐顺序

1. 先确认当前 `app / scene / target_file / resource_visibility`。
2. 再读 `.mei/profiles/access.md` 与 `workflow.md`，确认访问态是 world-first。
3. 优先使用 `dataset_query`、`dataset_metric`、`resource_list`、`resource_get`、`resource_runtime_peek`、`resource_business_summary`。
4. 若 prompt 已注入 metric preview，先用它回答简单聚合问题。
5. 只有在需要小段 DSL 证据时，才读取目标 `.mei` 文件。

## 常用命令

- `mei-toolchain mcp describe --surface access --source-root <workspace> --json`
- `mei-toolchain knowledge --surface access --source-root <workspace> --include-content --json`
- `mei-toolchain inspect world --app <app> --source-root <workspace> --json`
- `mei-toolchain inspect inventory --app <app> --source-root <workspace> --json`
- `mei-toolchain inspect summary --app <app> --source-root <workspace> --json`
- `mei-toolchain query dataset --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain query metric --app <app> --source-root <workspace> --id <dataset_id> --json`
- `mei-toolchain runtime peek --app <app> --source-root <workspace> --json`

## 禁止

- 不把访问态问题默认转成作者态改代码建议。
- 不猜 dataset 字段、metric id、resource id、scene id。
- 不忽略浏览器 `query_state`。
- 不把静态 `.mei` 声明误当成 runtime 真值。
- 不越过当前 `resource_visibility`、inventory reachability 或宿主 scope。
