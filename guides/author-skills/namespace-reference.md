# MeiLang Namespace Reference

## 应优先使用的名字

### App / Navigation

```python
app_skeleton(id = "pretty-panels", title = "...", default_scene = "home")
navigation(key = "access:home", scene = "home", url = "...", assembly = assembly_ref(...))
```

### Scene / Layout 链

```python
scene(..., planes = [plane_ref("app/home/t1"), ...])

plane_layout(id = "t1", key = "...", tier = "t1", layout = grid(...), regions = [region_ref(...)])
region_layout(id = "...", key = "...", area = "...", layout = grid(...), sections = [section_ref(...)])
section_layout(id = "...", key = "...", area = "...", shell = section_shell(...))

section_shell(title = "...", width = "100%", padding_profile = "...", body = panel_ref(...))

content_panel(id = "...", chrome = "bare", layout = grid(...), blocks = [...])
# 样板源码常见同义写法：content_panel(...)
```

### Refs（结构）

```python
plane_ref("pretty-panels/home/t1")
region_ref("pretty-panels/home/t1/r-left-rail")
section_ref("pretty-panels/home/t1/r-left-rail/s-enforcement")
panel_ref("content/enforcement-stats")
assembly_ref("home@src/scene/home/assembly.mei")
assembly_ref("mini-park/home/t2/r-drilldown/c-park-point-1")
```

### World / 资源

```python
world(...)
resource(...)
world_ref(...)
map_ref(...)
view_ref(...)
resource_ref(id = "welcome_doc")
```

### Layout 原语

```python
grid(...)
viewport(...)
budget(...)
rail_standard_gap()   # 或其它 gap_profile 宏，以 stock 为准
```

### Document / UI 块

```python
doc.markdown(...)
component(...)
metric_card(...)
metric_card_ref(...)
```

### T2 / Link

```python
link_decl(key = "...", type = "popup", target = assembly_ref(...), ...)
link_ref("mini-park/home/t2/park-point-1")
```

T2 叶子按 **page_instance** 理解；实现文件里可能仍出现 `page_instance(...)`（改名中），不要把它写成新稿推荐构造器名。

### Data / Config refs

```python
dataset_ref(id = "sales_data")
metric_ref(id = "sales_total")
theme_ref("cockpit")
source_ref("uploaded_sales")
basemap_ref("city_map")
ops_param_ref("default_region")
```

## Dataset 相关

```python
resource(
    id = "sales_data",
    kind = "dataset",
    title = "销售样本 CSV",
    source = ds.csv(path = "data/sales.csv"),
)

world.add_dataset(
    id = "uploaded_sales",
    source = source_ref("uploaded_sales"),
    schema = [ds.column("month", "string"), ds.column("amount", "number")],
)

component(
    "dataset.table",
    area = "auto",
    props = {"data": dataset_ref(id = "sales_data")},
)
```

## 当前不要写

```python
frame(...)
frame.add_panel(...)
frame_ref(...)          # 不作布局主路径
flex(...)               # 不作默认布局
titled_shell(...)
row_budgets = [...]
assembly_view(...)
board_assembly(...)      # 用 page_instance
panel_contract(...)      # 用 content_panel
entry(...)
app(..., entries=[entry(...)])
app(...)                # 用 app_skeleton
world_file_ref(...)
flow_file_ref(...)
frame_file_ref(...)
data_ref(...)
component_ref(...)
```

## ref 口径

- 公开主语法：`*_ref(...)`
- 结构跳转：`plane_ref` / `region_ref` / `section_ref` / `panel_ref` / `assembly_ref`
- 组件 props：`dataset_ref` / `metric_ref` / `resource_ref`
- `world_ref` 不是 world 内资源 id
- `content_panel` / `page_instance` 为作者与 BlockId 正式名（`content_panel:...` / `page_instance:...`）
