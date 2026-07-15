# MeiLang DSL Reference

布局 SSOT：

```text
scene → plane_layout → region_layout → section_layout + section_shell → content_panel
```

样板：`workspaces/ws-demo-v2/apps/zhifa`、`mini-park` 的 `src/scene/**`（`t*/r-*/s-*/content.mei`）。

## 应用入口

App 根是 `app.toml`（`title` / `default_stage`）。编译器从 toml 合成 `app_skeleton`，并从 Stage MDX 合成 access `navigation`；作者不必再写 `src/app.mei`。

```toml
# app.toml
title = "面板调试 · Zhifa"
default_stage = "home"
app_id = "zhifa"
```

```mdx
<!-- src/stage/home.stage.mdx -->
---
stage_id: home
profile: cockpit
title: Home
---
@scene(use="scene/home")
```

## Scene 入口（assembly.mei）

```python
scene(
    id = "home",
    key = "home@src/scene/home/assembly.mei",
    profile = "cockpit",
    theme = theme_ref("cockpit"),
    summary = "驾驶舱首页",
    canvas = viewport(
        design_width = 1920,
        design_height = 1080,
        aspect_ratio = "16:9",
        scale_mode = "contain",
        overflow = "clip",
        align = "center",
    ),
    layout = grid(
        columns = ["1fr"],
        rows = ["1080px"],
        areas = [["body"]],
        align = "stretch",
    ),
    planes = [
        plane_ref("zhifa/home/t0"),
        plane_ref("zhifa/home/t1"),
    ],
)
```

`assembly.mei` 只组织 `plane_ref(...)`；不在这里堆 panel。

## Plane（t*/layout.mei）

```python
plane_layout(
    id = "t1",
    key = "zhifa/home/t1",
    tier = "t1",
    layout = grid(
        rows = ["72px", "1fr"],
        columns = ["2fr", "3fr", "2fr"],
        areas = [
            ["header", "header", "header"],
            ["left_rail", "center_rail", "right_rail"],
        ],
        gap = rail_standard_gap(),
    ),
    regions = [
        region_ref("zhifa/home/t1/r-header"),
        region_ref("zhifa/home/t1/r-left-rail"),
        region_ref("zhifa/home/t1/r-center-rail"),
        region_ref("zhifa/home/t1/r-right-rail"),
    ],
)
```

业务栏位用 `Nfr`；固定标题行可用 theme 档位高度（如 `"72px"`），不要在 content 层再撑高。

## Region（r-*/layout.mei）

```python
region_layout(
    id = "left_rail",
    key = "zhifa/home/t1/r-left-rail",
    chrome_role = "rail",
    area = "left_rail",
    layout = grid(
        rows = ["1fr", "2.52fr", "2.33fr"],
        areas = [["enforcement"], ["inspection"], ["penalty"]],
        gap = rail_standard_gap(),
    ),
    sections = [
        section_ref("zhifa/home/t1/r-left-rail/s-enforcement"),
        section_ref("zhifa/home/t1/r-left-rail/s-inspection"),
        section_ref("zhifa/home/t1/r-left-rail/s-penalty"),
    ],
)
```

硬规则：`sections = [section_ref(...)]` 唯一子树入口；禁止直挂 `content(...)` / `blocks`。

## Section（s-*/layout.mei）

有标题壳：

```python
section_layout(
    id = "enforcement",
    key = "zhifa/home/t1/r-left-rail/s-enforcement",
    area = "enforcement",
    title = "执法要素",
    budget = budget(width = "100%", padding_profile = "dense_strip_100"),
    shell = section_shell(
        title = "执法要素",
        width = "100%",
        padding_profile = "dense_strip_100",
        body = panel_ref("content/enforcement-stats"),
    ),
)
```

裸 stage / map 透传（无标题壳）可用 `shell = content_panel(chrome = "bare", blocks = [panel_ref(...)])`——样板里常见写法仍是 `content_panel(...)`，语义相同。

**禁止** `section_shell` / shell 上手写 `height`；section 高度来自 region 格子 stretch。

## Content（content.mei）

作者名：`content_panel`（`content.mei` 的内容构造器）。  
当前编译器标识仍多为 `content_panel`——读样板时按 `content_panel` 写，guides 叙事用 `content_panel`。

```python
use template "cockpit/panel/shell-macros" as shell

content_panel(
    id = "enforcement-stats",
    variant = "container",
    chrome = "bare",
    props = shell.content_fill_props() | {
        "width": "100%",
    },
    layout = grid(
        rows = ["1fr"],
        columns = ["1fr"],
        areas = [["strip"]],
        gap = "0",
        align = "stretch",
        justify = "stretch",
    ),
    blocks = [
        # metric / chart / 业务宏；slot fill，不设 px 行高撑 section
    ],
)
```

Fill-down：`layout.rows = ["1fr", ...]` 填满父 body；**不要** `row_budgets`、不要 content 层 px 高度。

## T2 page_instance

T2 与 T1 同构：`t2/layout.mei` → `r-*/layout.mei` → `s-*/layout.mei` → `c-*/content.mei`。

文档术语用 **page_instance**。实现侧叶子文件里仍可能见到 `page_instance(...)`（正在改名）；新稿按 page_instance 理解，不要再推荐 `page_instance`。

入口联动用 `link_decl`（常聚合在 `t2/links/*.mei`），`target = assembly_ref(...)`。

## World / dataset

重资源外置，在 content 层引用：

```python
world(
    id = "home_world",
    resources = [
        resource(
            id = "sales_data",
            kind = "dataset",
            title = "销售样本 CSV",
            source = ds.csv(path = "data/sales.csv"),
        ),
    ],
)
```

upload / ops source：

```python
world.add_dataset(
    id = "uploaded_sales",
    source = source_ref("uploaded_sales"),
    schema = [
        ds.column("month", "string"),
        ds.column("amount", "number"),
    ],
)
```

## Theme

```python
scene(
    id = "home",
    theme = theme_ref("cockpit"),
    # ...
)
```

尺寸微调走 Theme Profile（字体 + 比例 + padding 同组），不在 content 手改字号/px 撑高。

## 布局冻结口径

- 唯一布局原语：`grid(...)`
- `absolute(...)` 只用于 placement，不是并列布局范式
- slot 负责占位、背景、皮肤、padding、gap
- content / metric 只负责语义，不撑布局
- **已删除的作者路径**：`frame.add_panel`、默认 `flex`、`titled_shell`、`page_instance` / `page_instance` 推荐路径、micro-layout 结构层、`row_budgets`

## typed ref 主线

- 结构：`plane_ref` / `region_ref` / `section_ref` / `panel_ref` / `assembly_ref`
- 数据：`dataset_ref` / `metric_ref` / `resource_ref`
- 资源：`world_ref` / `map_ref` / `view_ref` / `link_ref`
- config：`theme_ref` / `source_ref` / `basemap_ref` / `ops_param_ref`

## 不要当作已实现主线

- `frame(...)` / `frame.add_panel(...)` / `app(...)` 旧入口（用 `app_skeleton`）
- `entry(...)` / `entries=[entry(...)]`
- 在组件 `props` 中直接跨文件消费外部 dataset/metric locator
- `world_ref(...)` 作为资源 id 选择器
