AI-native scene orchestration language for building apps from world models, UI components, and capability modules.

## 本地启动

### 1. 配置 `.env`（OpenAI 兼容）

`mei-lang` 内置 agent 直接读取 OpenAI 兼容配置。

最小示例见 [`.env.example`](.env.example)（复制为 `.env` 后填写密钥）。

### 2. GIS 底图（Martin，与 mei 分开）

`map.maplibre` 需要 HTTP 瓦片（MBTiles，无 PostGIS）。在 monorepo 根目录单独起 Martin：

```bash
# 终端 A（mei-projects 根目录）
./scripts/start_martin_docker.sh

# 终端 B
cd mei-lang
cargo run -p mei-lang-server -- serve
```

- 应用：**http://127.0.0.1:9527**（默认端口 9527，避开 macOS AirPlay 占用的 5000）
- 瓦片默认：**http://127.0.0.1:8080**，TileJSON 路径 **`/shapingba-z10-16`**

在 `mei-lang/.env` 中可改：

```bash
MEI_TILES_BASE_URL=http://127.0.0.1:8080
MEI_TILES_JSON_PATH=/shapingba-z10-16
```

未在 `.mei` 里写 `mapSpec.basemap` 时，预览页会使用上述默认值。更完整的安装与排错见 monorepo **`gis/spb/docs/martin-setup.md`**。

停止 Martin：`./scripts/stop_martin_docker.sh`（在 mei-projects 根目录）。

### 3. 启动 `mei-lang`

```bash
cd mei-lang
cargo run -p mei-lang-server -- serve
```

默认行为：

- `mei-lang` 监听 **http://127.0.0.1:9527**（可用 `--port` 覆盖）
- 源码根目录 **`--source-root`** 未传时默认为仓库内 **`../workspaces`**
- 启动后直接使用内置 agent（按 `.env` 中 OpenAI 兼容配置连模型服务）

浏览器打开根路径即可；应用页面路由形如 **`/apps/manage/<app_id>`**。

## 停止服务

- 停止 `mei-lang`：在 `mei serve` 所在终端按 `Ctrl+C`
- 停止 Martin（Docker）：`./scripts/stop_martin_docker.sh`（mei-projects 根目录）

## 最少配置

至少需要一组 OpenAI 兼容补全模型配置（见 `.env.example`）。若有多供应商，可通过 `OPENAI_IMITATORS` 追加前缀并配置对应的 `*_BASE_URL` / `*_API_KEY` / `*_COMPLETION_MODEL`。
