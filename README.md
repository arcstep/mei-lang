AI-native scene orchestration language for building apps from world models, UI components, and capability modules.

## 本地启动

在 **`mei-lang` 仓库根目录**执行（当前工作目录会作为服务的 `package_root`，用于加载 `.env` 与解析相对路径）：

```bash
cd mei-lang
cargo run -p mei-lang-server -- serve
```

默认：

- 监听 **http://127.0.0.1:3000**
- 示例工程根目录为仓库下的 **`examples/`**

浏览器打开根路径即可；应用页面路由形如 **`/apps/manage/<app_id>`**（由服务端提供）。

### 常用参数

```bash
cargo run -p mei-lang-server -- serve --host 0.0.0.0 --port 3000 --source-root examples
```

| 参数 | 说明 |
|------|------|
| `--source-root` | Mei 源码根目录；相对路径相对于**启动时的当前工作目录** |
| `--host` | 绑定地址，默认 `127.0.0.1` |
| `--port` | 端口，默认 `3000` |

### OpenCode 与 `.env`

服务端会在 **`package_root`**（即你在上面 `cd` 到的目录）尝试加载 `.env`。若你把配置放在上级目录（例如单独的 monorepo 根），请复制或链接到 `mei-lang/.env`，或在启动前通过环境变量注入所需配置。
