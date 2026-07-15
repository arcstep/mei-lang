# MeiLang Access Workflow

## 当前边界

访问态负责回答运行态问题，而不是默认修改 `.mei`：

- dataset / metric / resource 问题
- 当前浏览器 query state 下的有界求值
- runtime phase / result / actions 的解释
- 当前世界里哪些资源与场景最相关

## 输入优先级

按下面顺序取证：

1. `.mei/profiles/access.md`
2. `inspect world` / `inspect inventory` / `inspect summary`
3. 浏览器当前 `query_state`
4. `dataset_query` / `dataset_metric`
5. `resource_runtime_peek` / `resource_runtime_trace_export`
6. 仍需 verbatim DSL 证据时，再读小段 `.mei`

## 工具选型

### 数据问题

- 字段、样例行、schema：`dataset_query`
- 聚合、分组、指标：`dataset_metric`

### 资源问题

- 当前可见资源目录：`resource_list`
- 某个资源的具体 payload：`resource_get`

### 运行态问题

- 当前 phase / result / actions：`resource_runtime_peek`
- 需要 trace envelope：`resource_runtime_trace_export`
- 需要高层业务摘要：`resource_business_summary`

## 浏览器 query state

访问态回答默认应合并当前浏览器 `query_state`：

- 先确认当前 tab / overlay / active filters
- 再判断注入的 metric preview 是否已经覆盖问题
- 若 preview 不够，再用 bounded dataset/metric 工具补充

不要忽略浏览器 query state 后直接回答聚合问题。

## 何时读 `.mei`

只有下面几种情况才读源码：

- 需要引用一小段 DSL 作为 verbatim 证据
- 需要确认当前 target file 与 runtime scope 的关系
- 需要解释某个资源/组件在源码里是如何命名的

不要把访问态默认退化成“先读大段 `.mei` 再猜”。

## 何时切换回 author

出现下面情况时，应明确切回 author 链：

- 用户要修改 `.mei`
- 用户要新增 scene / frame / panel / component 布局
- 用户要创建或重构 app 结构
- 用户要修复编译 diagnostics

访问态回答可指出应切到 `author`，但不应直接把 world-first 问题改造成作者态工作流。
