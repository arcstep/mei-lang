# MeiLang Access Profile

## 当前定位

访问态不是“换个地方继续写 `.mei`”。

它的主任务应收敛为两件事：

- 围绕当前 `scene / world / runtime / query_state` 回答业务问题
- 在未来若需要讲解、演示、引导时，再驱动宿主做受控 presentation 动作

因此，访问态默认应是：

- **world-first + eval-first**
- **business-viewpoint-first**
- UI 不应成为 access-agent 的主要知识来源；未来若扩展演示能力，再把它作为受控执行器

也就是说，访问侧更应该回答：

- 当前值是多少
- 口径是什么
- 为什么在当前筛选下是这个结果
- 这条结论沿哪条观点路径 / 血缘链得到

而不是默认回答：

- 这块 panel 是从哪段 `.mei` 布局挂出来的
- 当前 rail / overlay 的源码层级关系是什么

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

后续若访问侧继续强化讲解与演示，更推荐把：

- 高亮
- 隐藏
- 放大
- explain/detail/lineage 弹窗
- TTS 播放

这类能力继续收敛为 host presentation callbacks，而不是把 panel 级实现细节混进 canonical access catalog。  
但在当前阶段，它们应优先被理解为**未来规划**，不是 access-agent 的默认能力。

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
11. metric / resource 可导出的 `analysis_contract`、business summary 与 lineage 摘要
12. 宿主未来可选注入的 presentation catalog（业务观察面目录，而不是源码布局目录）

这里需要明确一条新的优先级：

- **world / metric / lineage / query_state** 是访问态主真源
- `.mei` 源码与 layout 结构只作为按需取证材料

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
4. 若已注入 metric 预览、business summary 或观点路径摘要，先用这些结构化结果回答。
5. 不够时再调用 `mei_access_knowledge` / `dataset_metric` / `dataset_query` / `resource_business_summary` / `resource_runtime_peek` / `resource_runtime_trace_export`。
6. 如果问题涉及“为什么”或“口径如何成立”，优先展开 `analysis_contract`、detail scope 与 lineage，而不是先读 layout 文件。
7. 只有在需要 verbatim DSL 证据时，才小范围 `read_file`。

## 观点路径优先

访问态不应只把 metric 当成一个孤立数值，还应尽量围绕“观点路径”组织回答。

一条典型观点路径至少应包含：

- 当前业务观察面 / `viewpoint_id`
- 对应的 `dataset_id` / `metric_id` / `resource_id`
- 指标口径
- 当前 `query_state`
- detail rowset 或 explain/detail 入口
- 必要时的 lineage / basis refs

例如：

```text
viewpoint: warnings_total
-> dataset: warning_list
-> metric: warnings_count
-> definition: 按预警 ID 去重后，对预警条数求和
-> filters: warningLevel / agency / supervisionCategory
-> detail_scope: warning_list
-> runtime_value: 18
```

当访问态能直接拿到这类结构时，应优先用它回答“是什么 / 为什么 / 还能看哪里”，而不是沿 UI 布局层回溯。

## 未来可扩展的演示辅助，而不是布局解说

访问态如果未来要承担讲解与辅助演示，更推荐通过宿主开放的业务观察面目录与 presentation callbacks 工作。

更合适的未来能力是：

- 聚焦某个业务观察面
- 高亮或隐藏部分内容
- 放大重点观察面
- 打开 explain / detail / lineage / summary 飘窗
- 聚焦或切换 `query_state`
- 播放与讲解内容同步的 TTS

访问态不应默认承担：

- 解释 panel 挂载链
- 讲解 `.mei` 布局结构
- 依赖 rail / panel / DOM 细节来组织回答

也就是说，访问侧若未来需要“带着页面说话”，更合适的形态是：

- Agent 给出业务结论
- Agent 触发 presentation script / callback
- 宿主负责把业务观察面映射成真实 UI 动作

## 关键边界

- 不猜 dataset 字段、metric id、resource id。
- 不把源码静态声明误当成 runtime 真值。
- 不把访问态问题默认转成作者态修改建议。
- 不越过当前 `resource_visibility` / inventory reachability。
- 不把访问态默认退回到作者态 `.mei` 全文阅读。
- 不把 UI 布局层级当成访问态的主要知识组织方式。
- 不让 presentation callbacks 直接暴露宿主源码或 DOM 细节。

## `summary` 的角色

`inspect summary` / `workspace summary` 在访问态里仍然有用，但角色应固定为：

- 语义路由摘要
- app/scene 类型快速判断
- 帮助决定下一步应注入哪些求值结果

它们**不是**访问态的最终事实层；真正回答数据问题时，仍应以 dataset/metric/runtime 求值为准。

## 当前能力 vs 未来 Roadmap

### 当前（access-agent 能力基线）

- `access` + `ask` 只读问答，围绕 world / metric / `query_state` / runtime
- 工具：`dataset_query`、`dataset_metric`、`resource_*`、`mei_access_knowledge`（catalog）；宿主 overlay：`read_file`、`propose_session_patch`
- 回答优先：观点路径、口径、血缘、`analysis_contract`，而非 layout / `panel_ref` 链

### 未来（保留在设计里，非当前默认）

| 阶段 | 内容 |
|------|------|
| R1 | 业务观察面目录（`viewpoint_id`） |
| R2 | 上下文默认注入观点路径 / lineage；清理 layout trace 语料 |
| R3 | Presentation callbacks（高亮、隐藏、弹窗、`focus_query_state`） |
| R4–R5 | 声明式演示脚本、TTS 与讲解同步 |
| R6 | access eval 语料迁到口径 / 血缘主线 |

细节与边界见 `docs/archive/mei-lang-v1/implementation/agent/62-access-agent-world-model-and-presentation-runtime.md`。

## 一句话总结

如果只记一句话：

- **access-agent 当前应主要围绕 world 语义、观点路径、指标口径与血缘解释回答问题；UI 演示执行器与 TTS 可先保留为未来计划，而不是默认去理解布局源码结构。**
