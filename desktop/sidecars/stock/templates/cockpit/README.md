# Cockpit 公共模板库（唯一真源）

驾驶舱 `metric_card` / section 壳 / 语义布局宏的公共目录。示例应用只引用本目录。

## 目录结构

```text
templates/cockpit/
├── main.mei              # 入口说明（scene: home）
├── metric-card.mei       # 指标卡预览（scene: metric）
├── map.mei               # GIS 主图预览（scene: map）
├── object-recipes.mei    # 编译器 stock recipe 的稳定 source anchor
├── object-defaults.mei   # 内部展开种子 / 旧生成代码兼容层
├── map/README.md         # 地图模板包说明（资产 + mapSpec 分工）
├── business-layouts.mei  # 业务语义布局宏（Fill-down）
├── assets/
│   ├── metrics/ header/  # 指标卡、顶栏切图
│   └── map/geo/          # 区划 GeoJSON（沙坪坝示例，可替换）
├── panel/                # section 壳与顶栏 / 地图模板
│   └── shell-macros.mei  # section_shell / screen_header / content_fill_props
├── metric-card/          # metric_card / compound 壳模板
└── drilldown/            # T2 page 壳（*-page.mei + frame-macros）
```

## 与组件包分工

| 层 | 路径 | 职责 |
|----|------|------|
| 模板 | `templates/cockpit/` | section 壳、metric_card 壳、**GIS mapSpec + GeoJSON**、语义布局宏 |
| 专题组件 | `_components/cockpit/` | `cockpit.header-brand`、`cockpit.data-table`、`cockpit.panel-title` 等 |
| 图表 | `_components/chart/echarts/` | 渲染器（含 `chart.geo`），**无区域业务数据** |
| 地图 | `_components/map/maplibre/` | `map.maplibre`，**无 MBTiles / 图层 URL** |
| 数据 | `_components/dataset/` | 表格、查询等 |

`panel/panel-screen-header.mei` 是**引用模板**；标题视觉逻辑在 `cockpit.header-brand`。

`business-layouts.mei` 提供**业务语义布局宏**（不是结构树节点）：`content_block`、`metric_triptych_body`、`chart_with_summary_body`、`table_with_summary_body`、`story_evidence_frame` 等。展开为 panel + grid；content 用 `content_fill_props` / `1fr`，禁止 `row_budgets` 撑高。

间距合同：

- rail / shell：`rail_standard_gap()`、`shell_body_padding_compact()`、`shell_body_padding_dense()`、`padding_profile`
- 语义宏内部 gap：`6px` 主 gap、`8px` table gap、`2px` compound gap

当前已进入真实业务文件使用的宏包括：

- `content_block`（原 `micro_panel`）
- `metric_triptych_body`
- `chart_with_summary_body`
- `table_with_summary_body`
- `story_opinion_block`
- `story_evidence_frame`
- `metric_quad_body`
- `status_triptych_summary_body`
- `wide_metric_compound_body`
- `metric_list_body`
- `evidence_pair_body`

### 表格组件边界

- `dataset.table`（`_components/dataset/table.js`）用于 manage 通用表格交互；
- `cockpit.data-table`（`_components/cockpit/data-table.js`）用于驾驶舱皮肤、embedded、轮播等；
- 两者共享 `dataset/runtime-query.js` 与 `dataset/table-runtime/*`。

## 资产

静态资源目录：`assets/`（含 `assets/metrics/`、`assets/header/`）。模板内统一使用：

```text
/workspace-app-assets/templates/cockpit/assets/...
```

预览时 `source_root` 为 `workspaces/`，`app_id` 为 `templates/cockpit`。

## 模板清单

### `panel/`

| 文件 | id / 宏 | 说明 |
|------|----------|------|
| `shell-macros.mei` | `section_shell` / `screen_header` / `content_fill_props` | Fill-down section 壳与 content fill |
| `panel-screen-header.mei` | `screen_header_shell` | 大屏顶栏（1920×72；`cockpit.header-brand`） |
| `panel-gis-map-fullscreen.mei` | `gis_map` | **GIS 地图**：`map.maplibre` + mapSpec |
| `panel-map-shell.mei` | — | 已弃用，请用 `panel-gis-map-fullscreen` |
| `metrics-auto-body.mei` | `metrics_auto_body` | 兼容 body；新写法请显式 `grid(...)` |

### `metric-card/`

| 文件 | panel id | 说明 |
|------|----------|------|
| `macros.mei` | — | 指标内容模板真源 |
| `metric-card-*.mei` / `metric-*-compound.mei` | 各 preset | legacy 预设；新样板优先 `business-layouts.mei` + macros |

### 对象 recipe 边界

`cockpit.alert`、`cockpit.case`、`cockpit.place`、`cockpit.event` 是编译期/运行时内部 stock recipe，不是第二套作者 UI DSL。作者只写高层对象意图：

```mei
object(
    type = "ops.Alert",
    source = dataset_ref("alerts"),
    identity = field_ref("alert_id"),
    recipe = stock_ref("alert"),
    slots = {
        "label": field_ref("title"),
        "severity": field_ref("severity"),
        "occurredAt": field_ref("occurred_at"),
        "status": field_ref("status"),
    },
)
```

编译器将 recipe 展开为薄的 projection / responder / interaction metadata，按 `local > domain > app > stock > placeholder > no_projection` 解析 override；override 不得改变 identity、object type 或 source。缺槽位只会隐藏、降级或留下无数据 placeholder，不会伪造字段值。

首批合同：

- `alert`：必选 `label/severity/occurredAt/status`；可选 `place/detail/explain`；
- `case`：必选 `label/status/occurredAt`；可选 `attachments/evidence/result/detail`，附件和证据默认附带 PII 脱敏提示；
- `place`：必选 `label/entityId/viewpoint`；可选 `world/rough3d/narration`，复用现有 Map / World / narration 表面；
- `event`：必选 `label/occurredAt`；可选 `severity/playbackAt/media/place/chart/t2/detail`，默认 secondary selection。

recipe 只保存 slot 名、降级规则和 owner/component 的薄引用；不会保存 dataset payload、ECharts option、GeoJSON、World geometry、HTML、媒体正文或讲稿正文。

`object-recipes.mei` 仅提供稳定 stock source anchor，不导出作者宏。`object-defaults.mei` 已降为内部展开种子和旧生成代码兼容层；新代码不得显式调用 `object_binding_props`、`object_viewpoint`、`object_metric_card` 或 action helper。

### `drilldown/`

| 文件 | 说明 |
|------|------|
| `*-page.mei` | T2 page scene 壳（filter / chart / table / tabs） |
| `frame-macros.mei` | `analytics_frame` 等 frame_export 宏 |
| `drilldown-kit.mei` | 多 scene_export 资源容器 |

> 构造器名：本阶段仍写 `content_panel` / `page_instance`；后续将更名为 `content_panel` / `page_instance`。

## 预览应用

| 文件 | 场景 | 说明 |
|------|------|------|
| `main.mei` | `home`（默认） | 模板库入口说明 |
| `metric-card.mei` | `metric` | 指标卡模板画廊（`?scene=metric`） |
| `map.mei` | `map` | GIS 地图模板预览（`?scene=map`） |

## 从其它应用引用

```mei
use template "cockpit/panel/shell-macros"
use template "cockpit/business-layouts" as biz
# section 壳
shell = section_shell(title = "板块", body = panel_ref("content/..."))

# content fill（禁止 row_budgets）
props = content_fill_props()
```

GIS 地图模板只管 **底图 + GeoJSON 图层**（`map/README.md`）；`chart.*` 由业务应用自行编排。
