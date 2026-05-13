AI-native scene orchestration language for building apps from world models, UI components, and capability modules.

## 本地启动

推荐把 `OpenCode` 和 `MeiLang` 分别启动、分别管理。

### 1. 启动 `opencode-server`

```bash
opencode serve --hostname 127.0.0.1 --port 4099
```

### 2. 启动 `mei-lang`

```bash
cd mei-lang
cargo run -p mei-lang-server -- serve
```

默认行为：

- `mei-lang` 监听 **http://127.0.0.1:3000**
- 示例工程根目录为仓库下的 **`examples/`**
- 默认按 `external` 模式连接 **http://127.0.0.1:4099**

浏览器打开根路径即可；应用页面路由形如 **`/apps/manage/<app_id>`**。

## 布局容器

`MeiLang` 现在支持把**容器级背景**与**组件内部装饰**分层处理：

- `frame(props=...)`：负责页面/舞台级背景、边框、圆角、固定比例舞台等能力
- `panel(props=...)`：负责某个容器区块本身的背景、边框、是否显示默认标题栏
- `grid`：只负责布局，不负责某个格子内部的装饰
- 具体组件：负责自己那一格里的背景、标题托底、图标、视觉特效

推荐的责任划分：

- 整体大屏背景放在 `frame`
- 某个内容容器的底板放在 `panel`
- 例如标题栏中间的专属托底，放在对应的 header 组件内部

### `frame(props=...)`

`frame` 支持 `props`，常用字段包括：

- `background`
  - 直接传字符串：作为 CSS `background`
  - 或传对象：支持 `color / image / size / position / repeat / attachment / blend_mode`
- `padding`
- `margin`
- `border`
- `radius`
- `box_shadow`
- `overflow`
- `min_height`
- `min_width`

示例：

```python
scene.set_frame(
    layout = grid(
        columns = ["1fr"],
        areas = [["main"]],
        gap = "0",
        padding = "0",
    ),
    props = {
        "background": {
            "image": "linear-gradient(180deg, #050b14 0%, #0a1628 40%, #071018 100%)",
            "position": "center",
            "repeat": "no-repeat",
        },
        "border": "1px solid rgba(56,189,248,.18)",
        "radius": "8px",
        "overflow": "hidden",
    },
)
```

### `panel(props=...)`

`panel` 也支持容器级视觉属性，字段与 `frame` 基本一致，并额外支持：

- `show_heading = False`：隐藏预览层默认标题栏
- 或 `chrome = "bare"`：同样表示作为“纯容器”使用

示例：

```python
frame.add_panel(
    id = "main",
    area = "main",
    props = {
        "show_heading": False,
        "background": {
            "color": "rgba(2,6,23,.18)",
            "image": "linear-gradient(180deg, rgba(8,16,28,.24), rgba(8,16,28,.06))",
        },
        "border": "none",
        "padding": "0",
    },
    blocks = [
        component("cockpit.demo", area = "auto"),
    ],
)
```

## 固定比例舞台

`frame.props.viewport` 用于把整个 `frame` 渲染成一个**固定设计尺寸的大屏舞台**，并按视口做统一缩放。

这套缩放是对**整个 stage** 做 `transform: scale(...)`，因此：

- 文字会一起缩放
- 图片会一起缩放
- 边框、圆角、间距会一起缩放
- 不会出现“容器缩了，但文字字号不变”的挤压变形

### 支持字段

- `enabled`: 是否启用
- `design_width`: 设计宽度，例如 `1920`
- `design_height`: 设计高度，例如 `1080`
- `scale_mode`
  - `contain`: 完整显示，不裁切
  - `cover`: 铺满区域，可能裁切
- `align`
  - `center`
  - `top-center`
  - `top-left`
  - `top-right`
  - `bottom-center`
  - `bottom-left`
  - `bottom-right`
  - `center-left`
  - `center-right`
- `align_x` / `align_y`
  - 更细粒度控制，优先级高于 `align`
- `safe_padding`
  - 四边统一安全边距
- `safe_inset`
  - 单独指定 `top / right / bottom / left`

### 示例：`1920x1080` 大屏

```python
scene.set_frame(
    layout = grid(
        columns = ["minmax(0,1fr)"],
        rows = ["minmax(640px,auto)"],
        areas = [["main"]],
        gap = "0",
        padding = "0",
    ),
    props = {
        "viewport": {
            "enabled": True,
            "design_width": 1920,
            "design_height": 1080,
            "scale_mode": "contain",
            "align": "top-center",
            "safe_inset": {
                "top": 12,
                "right": 16,
                "bottom": 12,
                "left": 16,
            },
        },
        "background": {
            "image": "radial-gradient(120% 80% at 50% -10%, rgba(14,165,233,.22), transparent 55%), radial-gradient(80% 50% at 100% 50%, rgba(59,130,246,.12), transparent 45%), linear-gradient(180deg, #050b14 0%, #0a1628 40%, #071018 100%)",
            "position": "center",
            "repeat": "no-repeat",
        },
        "border": "1px solid rgba(56,189,248,.18)",
        "radius": "8px",
        "overflow": "hidden",
        "padding": "0",
    },
)
```

可以参考示例工程：`examples/032-cockpit/main.mei`。

## 停止服务

- 停止 `opencode-server`：在它自己的终端里按 `Ctrl+C`
- 停止 `mei-lang`：在 `mei serve` 所在终端里按 `Ctrl+C`

## 最少配置

如果需要覆盖默认 OpenCode 地址：

```bash
export MEI_OPENCODE_URL=http://127.0.0.1:4099
```

如果你确实要恢复“启动 `mei` 时顺带拉起托管 OpenCode”，显式使用：

```bash
cargo run -p mei-lang-server -- serve --auto-opencode
```
