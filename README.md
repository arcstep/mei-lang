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

## 进一步阅读

- OpenCode 服务边界与设计：`../docs/mei-lang/topics/opencode-service-boundary.md`
- OpenCode 运维与排障：`../docs/mei-lang/implementation/extensions/05-opencode-service-operations.md`
