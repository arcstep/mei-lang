# Chart Pack（ECharts）用法真源

本目录与同级 `previews/`、`manifest.json` 同属 **chart/echarts** pack。Build 视图预览只读 `previews/{use_key}.mei`，示例数据在 `stock/authoring/examples/data/`。

## 快速引用

| 图表类型 | use_key | 示例数据 | mapping 要点 |
|----------|---------|----------|--------------|
| 折线/面积/趋势/柱 | `chart.line` 等 | `chart-lab.csv` | `x=month`, `y=revenue`, `group=region` |
| 饼/环/玫瑰/排名 | `chart.pie` 等 | `chart-composition.csv` | `label=category`, `y=revenue` |
| 雷达 | `chart.radar` | `chart-radar.csv` | `label=series`, 多 `y` 通道 |
| 箱线 | `chart.boxplot` | `chart-boxplot.csv` | `x=category`, `y=value` |
| 地理 | `chart.geo` | `chart-geo.csv` | `geojsonUrl` + `joinKey=code`, `y=value` |

## 最小 scene 模板

```mei
world(
    id = "home_world",
    resources = [
        resource(
            id = "chart_data",
            kind = "dataset",
            title = "Sample",
            source = ds.csv(path = "../../stock/authoring/examples/data/chart-composition.csv"),
        ),
    ],
)

component(
    "chart.donut",
    area = "auto",
    props = {
        "title": "Revenue donut",
        "data": dataset_ref(id = "chart_data"),
        "mapping": {
            "label": [{"field": "category", "name": "Category"}],
            "y": [{"field": "revenue", "name": "Revenue"}],
        },
    },
)
```

路径约定：dataset `path` 相对 **app 根**（如 `apps/hello`），不是相对 `.mei` 文件。

## 参考文件

- Build 预览：`../previews/chart.donut.mei`（及同目录其它 `chart.*.mei`）
- 教程级多图示例：`stock/authoring/examples/chart-baseline.mei`、`chart-composition.mei`
- 公共契约摘要：`knowledge/editor-runtime/components/chart-components.md`

## Agent 检索提示

在 Cursor / Trae / OpenCode 中优先搜索：

1. `stock/components/chart/echarts/docs/README.md`（本文件）
2. `stock/components/chart/echarts/previews/`（与 Build 预览 1:1）
3. `stock/authoring/examples/data/chart-*.csv`（示例数据 SSOT）
