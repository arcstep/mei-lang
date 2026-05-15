AI-native scene orchestration language for building apps from world models, UI components, and capability modules.

## 本地启动

推荐把 `OpenCode` 和 `MeiLang` 分别启动、分别管理。

### 1. 启动 `opencode-server`

```bash
opencode serve --hostname 127.0.0.1 --port 4099
```

本地默认建议保持这条最小访问边界：

- 继续监听 `127.0.0.1`
- 不改成 `0.0.0.0`
- 暂不启用 `--mdns`
- 暂不额外配置 `--cors`

这样 `opencode-server` 默认只对本机开放，`mei-lang` 通过 `localhost` 连接即可。

### 2. 启动 `mei-lang`

```bash
cd mei-lang
./scripts/tailwind-build.sh
cargo run -p mei-lang-server -- serve
```

默认行为：

- `mei-lang` 监听 **http://127.0.0.1:3000**
- 源码工作区根目录由 **`--source-root`** 控制（未传参时使用服务端默认值）
- 默认按 `external` 模式连接 **http://127.0.0.1:4099**

浏览器打开根路径即可；应用页面路由形如 **`/apps/manage/<app_id>`**。

### 3. 样式增量构建（可选）

如果你在开发过程中频繁调整宿主 UI 样式，可以开一个并行终端：

```bash
cd mei-lang
./scripts/tailwind-build.sh --watch
```

`tailwind-build.sh` 使用 Tailwind standalone CLI，会在首次执行时自动下载对应平台二进制并生成 `app/assets/tailwind.css`。

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
