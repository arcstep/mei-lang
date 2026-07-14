# MeiLang Syntax Rules

## 当前可写主线

优先使用：

- `app_skeleton(...)`
- `navigation(...)`
- `scene(...)`
- `plane_layout(...)` / `region_layout(...)` / `section_layout(...)`
- `section_shell(...)`
- `content_panel(...)`（`content.mei` 作者名；源码样板常写作 `content_panel(...)`）
- `plane_ref(...)` / `region_ref(...)` / `section_ref(...)`
- `panel_ref(...)` / `assembly_ref(...)`
- `grid(...)`
- `viewport(...)`
- `budget(...)` / `padding_profile` 档位名
- `world(...)` / `resource(...)`
- `component(...)` / `metric_card(...)` / `doc.markdown(...)`
- `link_decl(...)` / `link_ref(...)`
- `metric_def_bundle(...)`（按样板）
- `dataset_ref(...)` / `metric_ref(...)` / `resource_ref(...)`
- `world_ref(...)` / `map_ref(...)` / `view_ref(...)`
- `theme_ref(...)` / `source_ref(...)` / `basemap_ref(...)` / `ops_param_ref(...)`
- `metric_card_ref(...)`

结构链（必须遵守）：

```text
scene → plane_layout → region_layout → section_layout + section_shell → content_panel
```

硬规则：

- `assembly.mei` 只挂 `planes = [plane_ref(...)]`
- `region_layout` 只允许 `sections = [section_ref(...)]`；禁止直挂 `content` / `blocks`
- section 有标题用 `section_shell`；裸 stage/map 用 bare `content_panel` shell + `panel_ref`
- 布局原语只有 `grid(...)`；不要把 `flex` 当默认
- Fill-down：region `Nfr` → section stretch → slot fill → content **不设 px 高度**
- `key` 镜像目录：`{app}/{scene}/t*/r-*/s-*`
- T2 文档术语：**page_instance**（实现叶子可能仍是 `page_instance`，勿当推荐新路径名）

## 组件绑定

- 输入统一写入 `props`
- 稳定值来源：`dataset_ref` / `metric_ref` / `resource_ref` / `scene_ref`（整份 contract）/ config refs
- 不在 `props` 里写跨文件 locator；外部对象先进入当前 world/scene 账本
- `mapping` / `headers` / `columns` 等是组件 contract 字段，不是独立 DSL 函数

## 已实现边界

| 构造器 | 状态 |
|--------|------|
| `plane_layout` / `region_layout` / `section_layout` | 已实现 |
| `section_shell` | 已实现 |
| `plane_ref` / `region_ref` / `section_ref` | 已实现 |
| `grid` + areas | 已实现 |
| `content_panel` 作者名 | guides 用语；编译器当前多为 `content_panel` |
| T2 page_instance | 语义已定；实现名迁移中 |

## 不要误写成已实现 / 不要再写

- `frame(...)` / `frame.add_panel(...)` / `frame_ref` 作为布局主路径
- `flex(...)` 作为默认布局
- `titled_shell` / `row_budgets` / micro-layout 结构层
- `page_instance` / 把 `page_instance` 当推荐作者 API（用 page_instance 叙事）
- `entry(...)` / `app(..., entries=...)`
- `app(...)` 旧入口（用 `app_skeleton`）
- `*_file_ref(...)` 作为公开主语法
- `entity_ref` / `data_ref` / 未落地的 capability registry

## 扩展规则

- dataset：`world.resource(kind = "dataset")` 或 `world.add_dataset` + `source_ref`
- chart / cockpit 组件：走已注册 type key + `props`
- `world_ref` 不是资源 id 选择器

## 写作要求

- 先主干结构，后 content 细节
- 能用 zhifa / mini-park 证明的，才写进脚本规范
- 先 current syntax，再考虑设计中的未来能力
