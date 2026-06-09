# MeiLang Namespace Reference

## 应优先使用的名字

### App / Scene

```python
app(...)
app_add_scene(scene = scene_ref(scene_file = "home.mei", scene_id = "home"))
scene(...)
scene_ref(scene_file = "home.mei", scene_id = "home")
```

### World

```python
world(...)
resource(...)
world_ref(scene_file = "worlds/base.mei", scene_id = "base")
resource_ref(id = "welcome_doc")
```

### Flow

```python
flow(...)
flow_ref(scene_file = "flows/base.mei", scene_id = "base")
```

### Frame / Layout

```python
frame(...)
frame_ref(scene_file = "frames/base.mei", scene_id = "base")
frame.add_panel(...)
panel(...)
panel_ref(id = "summary_panel", scene_file = "panels/base.mei")
metric_card(...)
metric_card_ref(id = "metric_shell", scene_file = "templates/metric-shell.mei")
component(...)
grid(...)
flex(...)
```

### Document

```python
doc.markdown(...)
```

### Refs

```python
dataset_ref(id = "sales_data")
metric_ref(id = "sales_total")
resource_ref(id = "welcome_doc")
scene_ref("self")
```

## 当前扩展相关写法

### Dataset 资源

```python
resource(
    id = "sales_data",
    kind = "dataset",
    title = "销售样本 CSV",
    source = ds.csv(path = "data/sales.csv"),
)
```

### 外部组件消费 dataset

```python
component(
    "dataset.table",
    area = "auto",
    props = {
        "data": dataset_ref(id = "sales_data"),
    },
)
```

## 当前不要写

```python
entry(...)
app(..., entries=[entry(...)])
world_file_ref(...)
flow_file_ref(...)
frame_file_ref(...)
data_ref(...)
component_ref(...)
```

## 当前 ref / file_ref 口径

- 当前公开主语法统一为 `*_ref(...)`
- `scene_ref(...)` / `world_ref(...)` / `flow_ref(...)` / `frame_ref(...)` 主要进入 owner 槽位
- `panel_ref(...)` / `metric_card_ref(...)` 用于跨文件模板与 panel 复用
- `dataset_ref(...)` / `metric_ref(...)` / `resource_ref(...)` 主要作为组件 props 的稳定值来源
- `*_file_ref(...)` 仅保留兼容/迁移语义，不再作为公开主示例
- `world_ref(...)` 不再表示 world 内部某个资源 id
