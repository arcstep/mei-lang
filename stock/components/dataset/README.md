# dataset 组件包

通用数据观察组件包。Build 预览见 [`previews/`](previews/)（`{use_key}.mei` 约定）。

当前导出：

| use key | 说明 |
|---------|------|
| `dataset.table` | manage 风格通用表格；支持 toolbar、server paging、列配置 UI、`query_state` 联动 |
| `dataset.filter-bar` | 最小过滤 UI；把命名 `query_state` 写入宿主共享状态 |
| `dataset.summary-cards` | 围绕 dataset / rowset / `metric_ref(...)` 展示摘要卡 |

## 分层位置

`dataset` 包同时包含：

1. **renderer 壳**
   - `table.js`
   - `filter-bar.js`
   - `summary-cards.js`
2. **共享 runtime**
   - `runtime-query.js`
   - `table-runtime/`

在当前体系中：

- `dataset.table` 是 **manage renderer**
- `cockpit.data-table` 是 **cockpit renderer**
- popup / drilldown 是 **宿主桥接路径**

因此 `dataset` 包不等于“表格唯一组件”，但它承载了当前最稳定的共享表格内核。

## `dataset.table`

入口：`table.js` → `mei-dataset-table`

职责：

- 消费静态 `data` / `dataset`，或运行时 `data_ref(...)` / `metric_ref(...)`
- 自动判断是否进入 server paging
- 合并共享 `query_state.filters` 与本地 toolbar 过滤
- 通过 `/api/datasets/query` 请求分页行集
- 消费 `column_meta`、`summary`、`query_state_echo` 等表格 contract V1 响应
- 解释 `column_state` / `columnFormats` / `columnRules` 并渲染列顺序、隐藏、宽度、格式化、tone/tag

特点：

- 语义 `<table>`
- 完整 toolbar：search / filter field / filter value / page size / prev / next / columns
- 表头单列排序入口、列显示切换、拖拽换列
- `query_state` 当前正式共享的是 `filters`；`sort` / `column_state` 已可进入请求链与回显，但仍未冻结为统一作者态

### server paging 判定

`dataset.table` 当前通过 `resolveServerPaging()` 做启发式判断，典型触发条件：

- shared runtime rows query capability 可用（读取 `props._mei.runtime_capabilities.rows_query`）
- 数据源是文件/DB 背后数据集
- props 里带运行时 `__mei_runtime_ref`
- 或绑定了 `query_state`

这条逻辑属于 **manage renderer 私有壳**，不进入共享 `table-runtime`。

## `dataset.filter-bar`

职责：

- 提供最小过滤表单
- 写入命名 `query_state`

不负责：

- 私自发 `/api/datasets/query`
- 私自维护某个 panel 的局部真值

## `dataset.summary-cards`

职责：

- 若绑定 `metric_ref(...)`，通过 runtime metric query 围绕当前 scene + query state 重算
- 若绑定 rowset / dataset，则做最小 fallback 摘要

## 共享 runtime

### `runtime-query.js`

承担宿主交互面：

- `fetchDatasetRows()` / `fetchRuntimeMetrics()`
- `query_state` store 与事件
- `deferUntilDisplayed()`
- runtime diagnostics / perf
- `__mei_runtime_ref` 解析

它是当前所有表格观察面的 **host bridge 封装**。

### `table-runtime/`

承担 renderer 无关逻辑：

- `state.js`：filters / sort / `column_state` 与 `query_state` 合并
- `query.js`：query 响应归一化
- `cells.js`：单元格截断、全文弹层、预览交互
- `format.js`：共享列描述、数字/时间/文本格式化、tone 规则

详见 [`table-runtime/README.md`](./table-runtime/README.md)。

## 与其它 renderer 的关系

`cockpit.data-table` 直接复用本包中的：

- `runtime-query.js`
- `table-runtime/state.js`
- `table-runtime/query.js`
- `table-runtime/cells.js`
- `table-runtime/format.js`

但它仍保留 cockpit 私有壳：

- `layoutPreset`
- `embedded`
- 轮播 / client paging 体验
- 业务 tone/tag 皮肤

popup / drilldown 则通过宿主侧 [`mei-lang/app/assets/spa-navigation/`](../../mei-lang/app/assets/spa-navigation/) 桥接到同一套 query contract；它不是 `dataset` 包中的第三个 renderer。

## 当前边界

### 共享内核

- query payload
- `query_state` store
- `sort` / `filters` / `column_state` 合并
- query 响应归一化
- 共享列描述与格式化
- 单元格全文弹层

### renderer 私有壳

- manage toolbar
- server paging 启发式判定
- cockpit `layoutPreset` / `embedded` / `carousel`
- popup overlay 生命周期与 scene projection

## 文档索引

- host contract：`docs/mei-lang/implementation/host/44-host-datasets-query-api-and-compile-file-cache.md`
- query_state 与宿主联动：`docs/mei-lang/implementation/host/45-host-page-query-state-and-manage-shell-data-integration.md`
- lazy/runtime 语义：`docs/mei-lang/implementation/syntax/08-lazy-sources-and-runtime-query-semantics.md`
- 页级 `query_state` 语义：`docs/mei-lang/implementation/syntax/10-page-query-state-and-runtime-metric-semantics.md`
- 场景示例：`workspaces/examples/ds/04-data-table-features/README.md`
