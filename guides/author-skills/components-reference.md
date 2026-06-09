# MeiLang Components Reference

## 组件使用原则

- 组件必须来自当前 public contract、example pack 或 `.stock/components/**/manifest.json` 中已注册的 type key
- 组件输入统一走 `props`
- 优先传语义对象，不优先传宿主内部字段路径
- 组件 props 优先消费当前 scene 可见的 `dataset_ref(...)`、`metric_ref(...)`、`resource_ref(...)`
- 需要 pack 级规则时，先读 `.mei/knowledge/author/components/component-contracts.json` 与同目录 pack guide

## 当前常见组件

### 文档

- `doc.markdown`
- `mei.text`

常见写法：

```python
doc.markdown(
    area = "auto",
    resource = resource_ref(id = "dataset_doc"),
)

component(
    "mei.text",
    area = "auto",
    props = {
        "content": "Standalone author package",
    },
)
```

### Dataset

- `dataset.table`
- `dataset.filter-bar`
- `dataset.summary-cards`

常见写法：

```python
component(
    "dataset.table",
    area = "auto",
    props = {
        "data": dataset_ref(id = "sales_data"),
    },
)

component(
    "dataset.filter-bar",
    area = "auto",
    props = {
        "query_state": "sales_filters",
        "fields": [
            {"key": "region", "label": "Region", "options": ["East", "West"]},
        ],
    },
)

component(
    "dataset.summary-cards",
    area = "auto",
    props = {
        "value": metric_ref(id = "sales_overview"),
    },
)
```

### Chart

- `chart.line`
- `chart.bar-mini`
- `chart.ranking`

常见写法：

```python
component(
    "chart.line",
    area = "auto",
    props = {
        "title": "月度营收",
        "data": dataset_ref(id = "monthly_data"),
        "mapping": {
            "x": [{"field": "month", "name": "Month"}],
            "y": [{"field": "revenue", "name": "Revenue"}],
        },
    },
)

component(
    "chart.ranking",
    area = "auto",
    props = {
        "title": "区域排名",
        "data": dataset_ref(id = "monthly_data"),
        "showBackground": True,
        "rankingLayout": "side",
        "mapping": {
            "x": [{"field": "label", "name": "Region"}],
            "y": [{"field": "value", "name": "Value"}],
        },
    },
)
```

### Cockpit / Template

- `cockpit.data-table`
- `cockpit.header-brand`
- `cockpit.panel-title`
- `cockpit.donut-trio`

常见写法：

```python
component(
    "cockpit.data-table",
    area = "auto",
    props = {
        "dataset": metric_ref(id = "alerts_table"),
        "embedded": True,
        "layoutPreset": "warnings",
    },
)

panel(
    base = panel_ref(
        id = "titled_shell",
        scene_file = ".stock/templates/cockpit/panel/panel-titled-shell.mei",
    ),
    id = "summary_panel",
    title = "业务概览",
)
```

### Simulation / Map

- `sim.scene`
- `map.maplibre`

常见写法：

```python
component(
    "sim.scene",
    area = "auto",
    props = {
        "scene": scene_ref("self"),
    },
)

component(
    "map.maplibre",
    area = "auto",
    props = {
        "mapSpec": {
            "basemap": {
                "tilesUrl": "http://127.0.0.1:3000",
                "tilesJsonPath": "/tiles/city.json",
                "center": [106.55, 29.56],
                "zoom": 11,
            },
            "layers": [],
        },
    },
)
```

## 组件选择规则

- 预览 dataset / query_state 联动：优先 `dataset.*`
- 展示最小图表位：优先 `chart.*`
- 驾驶舱表格与标题皮肤：优先 `cockpit.*` + `.stock/templates/cockpit`
- 展示整份场景：优先 `sim.scene`
- GIS 地图：优先 `map.maplibre` 或 cockpit GIS shell

## 当前不要假定

- 任意 `chart.*` 都已经存在
- 组件会自动推断不存在的资源
- 组件 props 可以直接跨文件消费外部 dataset/metric/resource locator
- 未列进 public contract 的私有 renderer props 已经稳定承诺
