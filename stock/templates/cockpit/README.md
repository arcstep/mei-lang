# Cockpit 公共模板库（唯一真源）

驾驶舱 `metric_card` / `panel` 声明模板的公共目录。示例应用（如 `workspaces/examples/cockpit/05-panel`）**只引用**本目录，不再在 `examples/**/templates` 维护副本。

## 目录结构

```text
templates/cockpit/
├── main.mei              # 入口说明（scene: home）
├── metric-card.mei       # 指标卡预览（scene: metric）
├── map.mei               # GIS 主图预览（scene: map）
├── map/README.md         # 地图模板包说明（资产 + mapSpec 分工）
├── assets/
│   ├── metrics/ header/  # 指标卡、顶栏切图
│   └── map/geo/          # 区划 GeoJSON（沙坪坝示例，可替换）
├── panel/                # panel 壳与内容区模板
└── metric-card/          # metric_card / compound 壳模板
```

## 与组件包分工

| 层 | 路径 | 职责 |
|----|------|------|
| 模板 | `templates/cockpit/` | panel / metric_card 壳、**GIS mapSpec + GeoJSON**、默认 props |
| 专题组件 | `_components/cockpit/` | `cockpit.header-brand`、`cockpit.data-table`、`cockpit.panel-title` 等 |
| 图表 | `_components/chart/echarts/` | 渲染器（含 `chart.geo`），**无区域业务数据** |
| 地图 | `_components/map/maplibre/` | `map.maplibre`，**无 MBTiles / 图层 URL** |
| 数据 | `_components/dataset/` | 表格、查询等 |

`panel/panel-screen-header.mei` 是**引用模板**；标题视觉逻辑在 `cockpit.header-brand`。

### 表格组件边界

与本模板库相关的表格能力按“共享内核 + 多 renderer”维护：

- `dataset.table`（`_components/dataset/table.js`）用于 manage 通用表格交互；
- `cockpit.data-table`（`_components/cockpit/data-table.js`）用于驾驶舱皮肤、embedded、轮播等；
- 两者共享 `dataset/runtime-query.js` 与 `dataset/table-runtime/*`，模板侧不自行维护私有 dataset POST 逻辑。

因此模板库不提供“单组件 + theme”封装；是否单组件化属于后续内核收敛完成后的延后决策。

## 资产

静态资源目录：`assets/`（含 `assets/metrics/`、`assets/header/`）。模板内统一使用：

```text
/workspace-app-assets/templates/cockpit/assets/...
```

预览时 `source_root` 为 `workspaces/`，`app_id` 为 `templates/cockpit`。

## 模板清单

### `panel/`

| 文件 | panel id | 说明 |
|------|----------|------|
| `panel-titled-shell.mei` | `titled_shell` | 板块标题壳（内嵌 54px 标题栏 + 内容区，如 522×232 预览框） |
| `panel-screen-header.mei` | `screen_header_shell` | 大屏顶栏（1920×72；`screen-title-bg` + `screen-title-center` + `cockpit.header-brand`） |
| `panel-gis-map-fullscreen.mei` | `gis_map` | **GIS 地图**：`map.maplibre` + mapSpec（底图、GeoJSON 业务层、图层勾选） |
| `panel-map-shell.mei` | — | 已弃用（旧三栏壳），请用 `panel-gis-map-fullscreen` |
| `metrics-auto-body.mei` | `metrics_auto_body` | metrics_auto 内容区 |

### `metric-card/`

| 文件 | panel id | 说明 |
|------|----------|------|
| `metric-card-narrow-stack.mei` | `card_normal` / `card_selected` | 114×128 stack 窄卡，默认 / 强调皮肤 |
| `metric-card-solid-row.mei` | `card_solid_row_accent` / `card_solid_row_compact` | 横排纯色：宽松 132×50 / 紧凑 132×32 |
| `metric-card-solid-stack.mei` | `card_solid_stack_plain` / `card_solid_stack_corner` | 叠排纯色 152×54：纯底 / 四角装饰 |
| `metric-card-stack-desc.mei` | `card_stack_desc_mid` | 150×128 stack_desc + mid 底图；desc 用 `desc_shell` 角标 |
| `metric-card-stack-progress.mei` | `card_stack_progress_clean` | 132×80 进度卡；背景 `assets/metrics/metric-bg-clean@3x.svg`；比例走 `desc` 槽 + `metric_desc_mode=progress`（见文件头注释） |
| `metric-card-plain.mei` | `card_plain` | compound 内层透明卡 |
| `metric-card-icon-left.mei` | `card_icon_left` | 152×74 左图 + stack；消费方 `props.background.image` 换图 |
| `metric-card-strip-icon-left.mei` | `card_strip_icon_left` | 472×74 左图 + 横排 |
| `metric-wide-compound.mei` | `wide_compound_shell` | 宽卡 compound 壳（234×128，上横排 + 下多子卡） |
| `metric-card-long-compound.mei` | `long_compound_shell` | 长卡 compound 壳（463×80，左 1/3 stack + 右 2/3 上下横排子卡；底图 `metric-bg-long@3x.svg`） |

## 预览应用

本目录 `app_id` 为 `templates/cockpit`，在管理端 **「模板库 → Cockpit 模板预览」** 打开。

| 文件 | 场景 | 说明 |
|------|------|------|
| `main.mei` | `home`（默认） | 模板库入口说明 |
| `metric-card.mei` | `metric` | 指标卡模板画廊（`?scene=metric`） |
| `map.mei` | `map` | GIS 地图模板预览（`?scene=map`） |

大屏顶栏请单独对照 `panel/panel-screen-header.mei`（1920×72，不宜与窄栏指标卡同页预览）。

## 从其它应用引用

```mei
COCKPIT_TPL = "../../../templates/cockpit"
SHELL = COCKPIT_TPL + "/panel/panel-titled-shell.mei"
CARD = COCKPIT_TPL + "/metric-card/metric-card-narrow-stack.mei"
GIS_MAP = COCKPIT_TPL + "/panel/panel-gis-map-fullscreen.mei"
```

`scene_file` 路径相对**消费方应用根目录**（含该应用 `main.mei` 的目录）解析，可含子目录（如 `panel/`、`metric-card/`）。`metric-wide-compound.mei` 仅提供壳（空 `blocks`），子卡由消费方 `panel(blocks=[...])` 注入。

GIS 地图模板只管 **底图 + GeoJSON 图层**（`map/README.md`）；`chart.*` 由业务应用自行编排，不纳入模板库。
