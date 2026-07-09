# MeiLang Components Reference

## 组件使用原则

- 组件必须来自 public contract、example pack 或 `.stock/components/**/manifest.json` 已注册 type key
- 输入统一走 `props`
- 优先传语义对象，不传宿主内部字段路径
- props 优先消费当前 scene 可见的 `dataset_ref` / `metric_ref` / `resource_ref`
- pack 规则：`.mei/knowledge/author/components/component-contracts.json`
- 模板壳：`.mei/knowledge/author/templates/template-contracts.json`
- 新增组件 / 模板：先读 `.mei/knowledge/author/extension-authoring.md`

组件挂在 **content_panel**（`content.mei`）的 `blocks` 里，不挂在 `frame.add_panel` 上。  
结构壳用 `section_shell`；不要用 `titled_shell`。

## 当前常见组件

### 文档

- `doc.markdown`
- `mei.text`

```python
doc.markdown(
    area = "auto",
    resource = resource_ref(id = "dataset_doc"),
)

component(
    "mei.text",
    area = "auto",
    props = {"content": "Standalone author package"},
)
```

### Dataset

- `dataset.table`
- `dataset.filter-bar`
- `dataset.summary-cards`

```python
component(
    "dataset.table",
    area = "auto",
    props = {"data": dataset_ref(id = "sales_data")},
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
    props = {"value": metric_ref(id = "sales_overview")},
)
```

upload / 环境切换：source 放进 `.mei-config.json -> ops.sources`，world 用 `source_ref(...)`。

### Chart

- `chart.line`
- `chart.bar-mini`
- `chart.ranking`

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
```

### Cockpit / 业务宏

- `cockpit.data-table` / `cockpit.header-brand` / `cockpit.panel-title` / `cockpit.donut-trio`
- stock 宏：`use template "cockpit/business-layouts"`、`cockpit/metric-card/macros`、`cockpit/panel/shell-macros`

典型 content 骨架（Fill-down）：

```python
use template "cockpit/panel/shell-macros" as shell

content_panel(
    id = "enforcement-stats",
    variant = "container",
    chrome = "bare",
    props = shell.content_fill_props() | {"width": "100%"},
    layout = grid(
        rows = ["1fr"],
        columns = ["1fr"],
        areas = [["strip"]],
        align = "stretch",
        justify = "stretch",
    ),
    blocks = [
        # biz.metric_triptych_compound_body(...) 等；slot fill，无 row_budgets
    ],
)
```

样板真源：`pretty-panels/.../s-enforcement/content.mei`、`mini-park/.../s-lake-pavilion/content.mei`。  
源码里构造器名可能是 `content_panel`——与 `content_panel` 同义。

section 标题壳：

```python
shell = section_shell(
    title = "执法要素",
    width = "100%",
    padding_profile = "dense_strip_100",
    body = panel_ref("content/enforcement-stats"),
)
```

不要：`titled_shell`、content 层 px 高度、`row_budgets`。

### Simulation / Map

- `sim.scene`
- `map.maplibre`

```python
component(
    "sim.scene",
    area = "auto",
    props = {"scene": scene_ref("self")},
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

地图 / stage 常落在 T0 `content.mei`，经 section bare shell 透传（见 mini-park / pretty-panels `t0/**`）。

## 组件选择规则

- dataset / query_state：`dataset.*`
- 最小图表位：`chart.*`
- 驾驶舱指标与标题皮肤：`cockpit.*` + stock 宏 + `section_shell`
- 整份场景：`sim.scene`
- GIS：`map.maplibre` 或 cockpit GIS shell
- upload / ops / basemap / theme：`.mei/knowledge/author/workspace-config-reference.md`

## 当前不要假定

- 任意 `chart.*` 都已存在
- 组件会自动推断不存在的资源
- props 可直接跨文件消费外部 dataset/metric locator
- 未列入 public contract 的私有 renderer props 已稳定承诺
- **不可**再用 `frame.add_panel` / `titled_shell` / `row_budgets` 组织 UI（已删除）
