# MeiLang DSL Reference

## 当前最小单文件骨架

```python
app(
    id = "demo",
    title = "Demo",
    default_scene = "home",
)

scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
    summary = "示例场景。",
)

world(
    id = "home_world",
    resources = [],
)

frame(
    id = "home_frame",
    layout = grid(
        rows = ["1fr"],
        columns = ["1fr"],
        areas = [["main"]],
        gap = "16px",
        padding = "20px",
    ),
)

frame.add_panel(
    id = "main",
    area = "main",
    blocks = [],
)
```

## 当前多文件场景注册骨架

```python
app(
    id = "demo",
    title = "Demo",
    default_scene = "home",
)

app_add_scene(
    scene = scene_ref(
        scene_file = "scenes/home.mei",
        scene_id = "home",
    ),
)
```

如需第二个 scene，继续追加：

```python
app_add_scene(
    scene = scene_ref(
        scene_file = "scenes/insights.mei",
        scene_id = "insights",
    ),
)
```

## 当前 dataset 骨架

```python
world(
    id = "sales_world",
    resources = [
        resource(
            id = "sales_data",
            kind = "dataset",
            title = "销售样本 CSV",
            source = ds.csv(path = "data/sales.csv"),
        ),
    ],
)

frame(
    id = "sales_frame",
    layout = grid(
        columns = ["1fr", "2fr"],
        rows = ["auto", "minmax(220px, 1fr)"],
        areas = [
            ["doc", "table"],
            ["summary", "chart"],
        ],
        gap = "16px",
        padding = "20px",
    ),
)

frame.add_panel(
    id = "table_panel",
    area = "table",
    blocks = [
        component(
            "dataset.table",
            area = "auto",
            props = {
                "data": dataset_ref(id = "sales_data"),
            },
        ),
    ],
)
```

## upload / ops source 骨架

```python
world.add_dataset(
    id = "uploaded_sales",
    source = source_ref("uploaded_sales"),
    schema = [
        ds.column("month", "string"),
        ds.column("amount", "number"),
        ds.column("region", "string"),
    ],
)
```

`source_ref("uploaded_sales")` 的真值来自 app 根目录 `.mei-config.json -> ops.sources`。

## theme 骨架

```python
scene(
    id = "home",
    world = "home_world",
    frame = "home_frame",
    profile = "page",
    theme = theme_ref("cockpit_dark"),
)
```

若只需要内建预设，也可以直接用 `theme = "cockpit"`。

## 当前 chart 骨架

```python
frame.add_panel(
    id = "chart_panel",
    area = "chart",
    blocks = [
        component(
            "chart.bar-mini",
            area = "auto",
            props = {
                "data": metric_ref(id = "sales_ranking"),
                "labelField": "month",
                "valueField": "revenue",
            },
        ),
    ],
)
```

## 当前模板克隆骨架

```python
panel(
    base = panel_ref(
        id = "titled_shell",
        scene_file = ".stock/templates/cockpit/panel/panel-titled-shell.mei",
    ),
    id = "summary_panel",
    title = "业务概览",
)
```

## 当前推荐结构

1. `app(...) + default_scene`
2. `scene(id=..., world=..., flow=..., frame=...)`
3. 单文件时 inline `scene(...)`
4. 多文件时 `app_add_scene(scene = scene_ref(...))`
5. `world(id=...)`
6. `flow(id=...)`（按需）
7. `frame(id=..., layout = grid(...))`
8. `frame.add_panel(area = "...")`
9. `panel(...)` 中继续使用 `grid + slot + content`
10. `component(...)`
11. `panel(base = panel_ref(...))` / `metric_card(base = metric_card_ref(...))`

## 布局冻结口径

- MeiLang 作者 DSL 统一使用 `grid(...)`
- `absolute(...)` 只用于 placement，不再作为与 `grid` 并列的布局范式
- `slot` 负责占位、背景、皮肤、padding、gap 与 typography budget
- `metric_card(...)` 保留 `label / value / unit / desc` 等内容语义，但不再承担背景壳
- 历史上的 `micro-layout`、`flex(...)`、`panel_slot(...)`、`layout_policy` 都不再作为新脚本默认写法

## 当前 typed ref 主线

- owner 槽位：`scene_ref(...)`、`world_ref(...)`、`flow_ref(...)`、`frame_ref(...)`
- 集合/模板：`panel_ref(...)`、`metric_card_ref(...)`
- 数据绑定：`dataset_ref(...)`、`metric_ref(...)`、`resource_ref(...)`
- config refs：`theme_ref(...)`、`source_ref(...)`、`basemap_ref(...)`、`ops_param_ref(...)`

## 兼容层（不作新脚本默认）

- `scene_file_ref(...)`
- `world_file_ref(...)`
- `frame_file_ref(...)`
- `app(..., scene = scene_file_ref(...))`

## 当前不要套用为已实现

- 完整 `dataset(...)` 作者态 DSL
- `data_ref(...)`
- 在组件 `props` 中直接跨文件消费外部 `dataset_ref(...)` / `metric_ref(...)`
- `world_ref(...)` 作为资源 id 选择器
- old cockpit-only 写法作为主规范
