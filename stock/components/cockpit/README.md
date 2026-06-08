# cockpit 组件包

驾驶舱**专题运行时组件**包。

| use key | 说明 |
|---------|------|
| `cockpit.header-brand` | 大屏标题（1920×72；全宽底图 + 居中帽檐 + 主标题） |
| `cockpit.data-table` | 嵌入/弹层通用表格（dataframe 指标、分页、预警色条、可选轮播） |
| `cockpit.donut-trio` | 三列分组占比环形图（total + numer 双 metric；右侧共用图例） |
| `cockpit.park-amount-list` | 园区罚没金额 Top N 列表（万元） |
| `cockpit.panel-title` | 板块标题条（宽/紧凑、切图底纹与 caret） |
| `cockpit.metric-progress` | 指标卡内嵌进度条 |

## 标题 props（常用）

- `title` — 主标题文字
- `assets.title_bg` / `assets.title_mid` — 两层 SVG 路径
- `capMinWidth` — 帽檐最小宽度（默认 633）
- `titleColor` / `titleFontSize` / `titleLineHeight` / `titleLetterSpacing` — 文字样式

## `cockpit.donut-trio` props（常用）

- `totalMetric` — 分母 metric（如检查总量）
- `numerMetric` — 分子 metric（与分母按 `groupField` 对齐后算占比）；兼容 `noViolMetric`
- `groupField` — 两行 metric 的对齐维度字段名（如 `"园区名称"`）；未设时尝试 `label` / `name` / `category` 等
- `limit` — 展示分组数，默认 3
- `chartHeight` — 组件总高度（px），含图例时会为图例预留约 20px
- `showLegend` — `true` 时在**三图最右侧**显示共用竖向图例（非 ECharts 内置 legend）
- `legendWidth` — 右侧图例栏宽度（px），默认 52
- `legendOkLabel` / `legendViolLabel` — 双色说明（默认「无违规」「违规」）
- `legendOkColor` / `legendViolColor` — 与环图扇区一致（默认 `#62beeb` / `#c47a3a`）
- `legendRateLabel` / `legendRestLabel` 等 — 与 ok/viol 同义，兼容旧 props
- `legend` — 自定义两项数组 `[{"label":"无违规","color":"#62beeb"},{"label":"违规","color":"#c47a3a"}]`

## `cockpit.data-table` props（常用）

- `dataset` — `metric_ref(...)` 或带 `__mei_runtime_ref` 的 dataframe
- `headers` / `columns` — 表头与列 key（日期列 key 建议含「日期/时间」以便格式化）
- `column_state` — 列顺序/隐藏/宽度/对齐的最小视图状态
- `columnFormats` / `columnRules` — 共享格式化与 tone 规则
- `layoutPreset` — 如 `warnings`（列宽与等级色）
- `embedded` — 嵌入 panel 时 `true`，占满父级高度
- `pageSize` + `pagination` + `paginationMode: "client"` — 客户端分页
- `carousel` — `true` 时按页自动轮播（需 client 分页且总页数 > 1）
- `carouselIntervalMs` — 轮播间隔，默认 5000，最小 2000
- `carouselPauseOnHover` — 悬停暂停，默认开启
- `carouselShowPager` — 轮播时仍显示上一页/下一页，默认关闭

## 表格分层边界（2026-05）

`cockpit.data-table` 与 `dataset.table` 不走“一个组件 + 主题开关”的路线，当前边界是：

- **共享内核（dataset 包）**
  - `../dataset/runtime-query.js`：query payload、`__mei_runtime_ref` 解析、query_state 订阅、runtime diagnostics/perf
  - `../dataset/table-runtime/query.js`：query 响应归一化（`rows`/`total`/`column_meta`/`summary`）
  - `../dataset/table-runtime/state.js`：共享 filters / sort / `column_state` 与 query_state 合并
  - `../dataset/table-runtime/format.js`：共享列描述、数字/时间/文本格式化、tone 规则
  - `../dataset/table-runtime/cells.js`：单元格截断、全文弹层、预览交互
- **renderer 私有壳（cockpit 包）**
  - `layoutPreset` 列宽与预警 tone/tag 皮肤
  - `embedded` 外观与高度策略
  - 轮播与驾驶舱分页体验

## 与模板库

引用入口见 [templates/cockpit/panel/panel-screen-header.mei](../../templates/cockpit/panel/panel-screen-header.mei)。

本包自包含 `shared.js`、`tokens.js` 及上表各组件实现脚本。
