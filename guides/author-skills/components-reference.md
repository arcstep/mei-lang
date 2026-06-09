# MeiLang Components Reference

## 组件使用原则

- 组件必须来自当前 example 或 manifest 中已注册的 type key
- 组件输入统一走 `props`
- 优先传语义对象，不优先传宿主内部字段路径

## 当前常见组件

### 文档

- `doc.markdown`

常见写法：

```python
doc.markdown(
    area = "auto",
    resource = world_ref("dataset_doc"),
)
```

### Dataset

- `dataset.table`
- `dataset.summary-cards`

常见写法：

```python
component(
    "dataset.table",
    area = "auto",
    props = {
        "dataset": world_ref("sales_data"),
    },
)
```

### Chart

- `chart.bar-mini`

常见写法：

```python
component(
    "chart.bar-mini",
    area = "auto",
    props = {
        "title": "月度营收",
        "dataset": world_ref("monthly_data"),
        "labelField": "month",
        "valueField": "revenue",
    },
)
```

### Scene

- `sim.scene`

常见写法：

```python
component(
    "sim.scene",
    area = "auto",
    props = {
        "scene": scene_ref("self"),
    },
)
```

## 组件选择规则

- 预览 dataset：优先 `dataset.*`
- 展示最小图表位：优先当前已存在的 `chart.*` 外部组件
- 展示整份场景：优先消费 `scene_ref(...)`

## 当前不要假定

- 任意 `chart.*` 都已经存在
- 组件会自动推断不存在的资源
- 组件能消费未实现的 `metric_ref(...)` 或 `data_ref(...)`
