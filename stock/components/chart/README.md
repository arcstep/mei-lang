# Chart Components

`chart/echarts/` 存放基于 ECharts 渲染的图表组件实现与注册文件。

当前结构：

- `echarts/engine.js` 为 ECharts 图表共享引擎
- `echarts/manifest.json` 负责注册 `chart.*` 组件
- `chart.ranking`：横向排名条形图，按数值降序；`showBackground` 全宽背景条 + 高亮数据条；`rankingLayout`：`side`（左侧标签）或 `above`（标签置顶，适合长事项名）；`label_max_chars` / 宽度估算控制省略号截断；悬停 tooltip / 点击柱条查看全文；可选 `barColor` / `barBackground`
- `echarts/*.js` 为具体图表入口

组件加载器会递归扫描 `_components/**/manifest.json`，并自动把 `script` 解析为相对 `_components/` 根目录的路径。
