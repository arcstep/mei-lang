# MeiLang Access Profile

## 当前定位

访问态不是“换个地方继续写 `.mei`”。

它的主任务是：

- 理解当前 `scene / world / runtime`
- 回答 dataset / metric / runtime state 问题
- 基于宿主当前 query state 注入有界求值结果

因此，访问态默认应是 **world-first + eval-first**，而不是 source-first。

## Catalog 真源

访问态 profile 与 MCP surface 由 toolchain capability catalog 统一导出：

```bash
mei-toolchain mcp catalog --json
mei-toolchain mcp describe --surface access --json
mei-toolchain knowledge --surface access --json
```

外部 stdio MCP 适配器：

```bash
npm run mcp:access-adapter
npm run test:mcp:access-adapter
```

宿主内 agent 仍应绑定同一套 access surface 工具名，而不是维护平行清单。

当前宿主内 access runtime 已改为直接消费 access surface descriptor，再叠加最小 host 绑定补丁：

- `app` / `source_root` 在宿主内由当前会话绑定，不再要求模型重复传入
- `dataset_query` / `dataset_metric` 使用 `dataset_id`
- `resource_get` 使用 `resource_id`
- `resource_visibility=local_only` 时，会隐藏 `scene_id` / `target_file` 覆盖参数
- `propose_session_patch` 仍是 host-only 附加工具，不属于 canonical access catalog

也就是说：

- canonical access tool schema 仍由 catalog 决定
- 宿主只允许做参数绑定、可见性裁剪和 host-only 工具追加
- 宿主不应再维护一套平行的 access tools 定义

## 主输入

访问态优先依赖这些输入：

1. `.mei/profiles/access.md`
2. `.mei/skills/meilang-access/SKILL.md`
3. `.mei/skills/meilang-access/workflow.md`
4. `mei-toolchain inspect world --app <app> --json`
5. `mei-toolchain inspect inventory --app <app> --json`
6. `mei-toolchain inspect summary --app <app> --json`
7. `mei-toolchain query dataset --app <app> --id <dataset_id> --json`
8. `mei-toolchain query metric --app <app> --id <dataset_id> --json`
9. `mei-toolchain runtime peek --app <app> --json`
10. 当前宿主浏览器传入的 `browser_context` / `query_state`

## Access Surface 工具名

与 catalog / host agent 对齐的正式工具名：

- `mei_access_knowledge`
- `dataset_query`
- `dataset_metric`
- `resource_list`
- `resource_get`
- `resource_runtime_peek`
- `resource_runtime_trace_export`
- `resource_business_summary`

其中常用输入形状在宿主内统一为：

- `mei_access_knowledge({ topic?, include_content? })`
- `dataset_query({ dataset_id, ... })`
- `dataset_metric({ dataset_id, metric_ids?, ... })`
- `resource_get({ resource_id, ... })`
- `resource_runtime_peek({ trace_limit?, ... })`
- `resource_runtime_trace_export({ trace_limit?, ... })`
- `resource_business_summary({ ... })`

## 默认顺序

1. 先读 `.mei/profiles/access.md` 与 `meilang-access` skill companion，确认当前是 world-first。
2. 再读 world/runtime/catalog 摘要，确定当前问的是哪个 app / scene / scope。
3. 再读取浏览器当前 `query_state`，把筛选条件并入默认求值范围。
4. 如果 prompt 已注入 metric 预览，先用它回答简单聚合问题。
5. 不够时再调用 `mei_access_knowledge` / `dataset_metric` / `dataset_query` / `resource_business_summary` / `resource_runtime_peek` / `resource_runtime_trace_export`。
6. 只有在需要 verbatim DSL 证据时，才小范围 `read_file`。

## 关键边界

- 不猜 dataset 字段、metric id、resource id。
- 不把源码静态声明误当成 runtime 真值。
- 不把访问态问题默认转成作者态修改建议。
- 不越过当前 `resource_visibility` / inventory reachability。
- 不把访问态默认退回到作者态 `.mei` 全文阅读。

## `summary` 的角色

`inspect summary` / `workspace summary` 在访问态里仍然有用，但角色应固定为：

- 语义路由摘要
- app/scene 类型快速判断
- 帮助决定下一步应注入哪些求值结果

它们**不是**访问态的最终事实层；真正回答数据问题时，仍应以 dataset/metric/runtime 求值为准。
