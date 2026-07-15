# Cockpit 模板库

模板是 **可克隆的 scene/frame 片段**（`.mei` + 资产），与 `stock/components/cockpit/` 组件 pack 配合使用。

## 目录结构

| 路径 | 说明 |
|------|------|
| `cockpit/main.mei` | 主屏 board 入口 |
| `cockpit/drilldown/` | 下钻 board |
| `cockpit/panel/`、`metric-card/` | 可复用 panel 片段 |
| `cockpit/assets/` | 图片、地图 GeoJSON 等 |

## 在 app 中引用

通过 `template_ref` / board 挂载（见 `stock/authoring/examples/cockpit-panel.mei`）。

Build 树 **Templates** 根下按子目录分组；点击模板 `.mei` 节点可预览整板。

## Agent 检索

1. `stock/templates/cockpit/docs/README.md`（本文件）
2. `knowledge/editor-runtime/templates/cockpit-template-index.md`
3. `stock/authoring/examples/cockpit-panel.mei`
