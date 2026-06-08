# Cockpit GIS 地图模板包

**只做地图**：`panel-gis-map-fullscreen.mei` + `assets/map/geo/`。

| 内容 | 说明 |
|------|------|
| `basemap` | Martin 瓦片 URL、`tilesJsonPath`、`center`、缩放、`roadClasses`、标注语言 |
| `layers[]` | GeoJSON 业务层：`url`、`label`、`visible`、`choropleth`、`outlineOnly`、`style` |
| `valueByCode` | 主图街道设色 join 示意值（业务可覆写） |

不含 `chart.*`；图表在业务 `gis_stage` 栈上绝对定位叠放（半透明 panel 即可透出底图）。

## 全幅驾驶舱 + 中间观察区（`mapViewport`）

模板内预设 `MAP_VIEWPORT`（传给 `map.maplibre`）：

| 字段 | 说明 |
|------|------|
| `mode` | `"cockpitBleed"`：地图铺满组件，无圆角边框 |
| `focusInset` | `{ top, right, bottom, left }`（建议 `px`）：中间观察区；**图层切换、状态栏、缩放按钮**落在此矩形内 |
| `showFocusGuide` | `true` 时显示虚线观察区（调试用） |

MapLibre 会对画布 `setPadding`，默认中心/缩放以观察区为准。漂浮 panel 尺寸变化时，应同步增大对应方向的 `focusInset`（略大于 panel 占位 + 边距）。

业务编排：`gis_stage`（`position: relative`）→ 底层 `panel_ref(gis_map)` 全幅 + 上层 chart panel。

## 引用

```mei
COCKPIT_TPL = "../../../templates/cockpit"
GIS_MAP = COCKPIT_TPL + "/panel/panel-gis-map-fullscreen.mei"

frame.add_panel(
    base = panel_ref(id = "gis_map", scene_file = GIS_MAP),
    id = "gis_map",
    area = "body",
)
```

## GeoJSON

```bash
python3 gis/spb/admin-boundaries/scripts/build_demo_map_layers.py
```

默认输出：`workspaces/templates/cockpit/assets/map/geo/`

换区域：替换 geo 文件并改模板内 `MAP_SPEC`（`center`、`layers[].url` 等）。

## 预览

`templates/cockpit?scene=map`
