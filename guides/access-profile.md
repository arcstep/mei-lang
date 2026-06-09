# MeiLang Access Profile

## 当前定位

访问态不是“换个地方继续写 `.mei`”。

它的主任务是：

- 理解当前 `scene / world / runtime`
- 回答 dataset / metric / runtime state 问题
- 基于宿主当前 query state 注入有界求值结果

因此，访问态默认应是 **world-first + eval-first**，而不是 source-first。

## 主输入

访问态优先依赖这些输入：

1. `mei-toolchain inspect world --app <app> --json`
2. `mei-toolchain inspect inventory --app <app> --json`
3. `mei-toolchain inspect summary --app <app> --json`
4. `mei-toolchain query dataset --app <app> --id <dataset_id> --json`
5. `mei-toolchain query metric --app <app> --id <dataset_id> --json`
6. `mei-toolchain runtime peek --app <app> --json`
7. 当前宿主浏览器传入的 `browser_context` / `query_state`

## 默认顺序

1. 先读 world/runtime/catalog 摘要，确定当前问的是哪个 app / scene / scope。
2. 再读取浏览器当前 `query_state`，把筛选条件并入默认求值范围。
3. 如果 prompt 已注入 metric 预览，先用它回答简单聚合问题。
4. 不够时再调用 `dataset_metric` / `dataset_query` / `resource_runtime_peek`。
5. 只有在需要 verbatim DSL 证据时，才小范围 `read_file`。

## 关键边界

- 不猜 dataset 字段、metric id、resource id。
- 不把源码静态声明误当成 runtime 真值。
- 不把访问态问题默认转成作者态修改建议。
- 不越过当前 `resource_visibility` / inventory reachability。

## `summary` 的角色

`inspect summary` / `workspace summary` 在访问态里仍然有用，但角色应固定为：

- 语义路由摘要
- app/scene 类型快速判断
- 帮助决定下一步应注入哪些求值结果

它们**不是**访问态的最终事实层；真正回答数据问题时，仍应以 dataset/metric/runtime 求值为准。
