# Cockpit 组件 Pack 用法

Cockpit 组件用于大屏/驾驶舱布局（指标卡、环图组、数据表等）。Build 预览见 `../previews/{use_key}.mei`。

## 数据型 vs 静态 props

| 模式 | 示例 use_key | props 形态 |
|------|--------------|------------|
| 静态演示 | `cockpit.donut-trio` | `items: [{label, value}, ...]` |
| 数据集驱动 | `cockpit.data-table` | `data` + `columns` / runtime query |

## 示例：环图 trio（无需 CSV）

```mei
component(
    "cockpit.donut-trio",
    area = "auto",
    props = {
        "items": [
            {"label": "A", "value": 40},
            {"label": "B", "value": 35},
            {"label": "C", "value": 25},
        ],
    },
)
```

Build 预览：`../previews/cockpit.donut-trio.mei`

## 模板资产

SVG/PNG 等静态资源在 `stock/templates/cockpit/assets/`，组件 props 中引用 workspace 路径或 `/workspace-app-assets/templates/cockpit/...`。

## Agent 检索

1. `stock/components/cockpit/docs/README.md`
2. `stock/components/cockpit/previews/`
3. `knowledge/editor-runtime/components/cockpit-components.md`
