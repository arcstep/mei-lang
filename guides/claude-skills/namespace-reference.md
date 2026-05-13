# MeiLang Namespace Reference

## 应优先使用的名字

### App / Entry / Scene

```python
app(id, title=None, default_scene=None, entries=None)
entry(scene=None, frame=None, id=None, title=None)
scene_file_ref(path, id=None)
scene(id, world=None, flow=None, frame=None, profile=None, theme=None, summary=None, goal=None, state=None)
```

### World

```python
world(id, resources=[], topology=None, entities=[])
resource(id, kind, title=None, content=None, source=None)
```

### Flow

```python
flow(id, start=None, interactions=[], timer=None, outcome=None)
```

### Frame / Layout

```python
frame(id, title=None, layout=None, viewport=None)
frame.add_panel(id=None, title=None, area=None, blocks=[], layout=None, props=None)
panel(id=None, title=None, area=None, blocks=[], layout=None, props=None)
component(use, area=None, props=None)
grid(columns=None, rows=None, areas=None, gap=None, padding=None)
flex(direction, gap=None, padding=None)
```

### Document

```python
doc.markdown(area=None, resource=None, content=None)
```

### Refs

```python
world_ref(id)
scene_ref(id)
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
        "dataset": world_ref("sales_data"),
    },
)
```

## 当前不要写

```python
world_file_ref(...)
flow_file_ref(...)
frame_file_ref(...)
data_ref(...)
metric_ref(...)
frame_ref(...)
component_ref(...)
```

## 当前 ref / file_ref 口径

- `*_file_ref(...)` 用来引用其他文件
- `*_ref(...)` 用来引用当前组合后作用域里的对象
- 当前已进入最小支持的是 `scene_file_ref(...)`
- `world_file_ref(...)` / `flow_file_ref(...)` / `frame_file_ref(...)` 是推荐命名方向，当前还未形成稳定实现
- `world_ref(...)` 当前仍主要兼容引用 `world.resources[id]`
