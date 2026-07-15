# Stock Pack 文档与示例检索（Agent）

MeiLang 语法与内置模块见 `knowledge/editor-runtime/authoring-overview.md`。**组件/模板的使用**以 workspace `stock/` 为真源，而非散落示例。

## 检索顺序

1. **组件 pack**：`stock/components/{category}/{pack}/docs/README.md`
2. **Build 预览 scene**：`stock/components/{category}/{pack}/previews/{use_key}.mei`
3. **示例 CSV/数据**：`stock/authoring/examples/data/`
4. **模板库**：`stock/templates/{category}/docs/README.md`
5. **契约摘要**：`knowledge/editor-runtime/components/*.md`

## 分类索引

| Build 树分组 | 文档 | 预览 |
|--------------|------|------|
| chart/echarts | `stock/components/chart/echarts/docs/README.md` | `.../previews/chart.*.mei` |
| cockpit | `stock/components/cockpit/docs/README.md` | `.../previews/cockpit.*.mei` |
| dataset | `stock/components/dataset/README.md` | `.../previews/dataset.*.mei` |
| Templates/cockpit | `stock/templates/cockpit/docs/README.md` | Build 树 template 节点 |

## 路径约定

- `dataset` CSV：`path` 相对 **app 根** → `../../stock/authoring/examples/data/...`
- Build 预览 target：`stock/components/.../previews/{use_key}.mei`
- 同步：``mei-toolchain workspace stock sync --source-root <ws>``

## SKILL 集成建议

在 Agent SKILL 中增加一步：**先 Glob `stock/components/**/docs/README.md` 与 `previews/*.mei`，再改 app scene。**
