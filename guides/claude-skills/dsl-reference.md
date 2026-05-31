# MeiLang DSL Reference

## 当前最小场景骨架

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
    layout = flex(direction = "column", gap = "16px", padding = "20px"),
)

frame.add_panel(
    id = "main",
    area = "auto",
    blocks = [],
)
```

## 当前 dataset 场景骨架

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
                "dataset": world_ref("sales_data"),
            },
        ),
    ],
)
```

## 当前 chart 场景骨架

```python
frame.add_panel(
    id = "chart_panel",
    area = "chart",
    blocks = [
        component(
            "chart.bar-mini",
            area = "auto",
            props = {
                "dataset": world_ref("monthly_data"),
                "labelField": "month",
                "valueField": "revenue",
            },
        ),
    ],
)
```

## 当前推荐结构

1. `app(..., entries=[entry(...)])`
2. `scene(id=..., world=..., flow=..., frame=...)`
3. `world(id=...)`
4. `flow(id=...)`（按需）
5. `frame(id=...)`
6. `frame.add_panel(...)`
7. `component(...)`

## 兼容层（不作新脚本默认）

- `app.add_scene(...)`
- `scene.set_world(...)`
- `scene.set_flow(...)`
- `scene.set_frame(...)`

## 当前不要套用为已实现

- 完整 `dataset(...)` 作者态 DSL
- `world_file_ref(...)` / `flow_file_ref(...)` / `frame_file_ref(...)`
- `data_ref(...)`
- `metric_ref(...)`
- `frame_ref(...)`
- old cockpit-only 写法作为主规范
