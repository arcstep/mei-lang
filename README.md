AI-native scene orchestration language for building apps from world models, UI components, and capability modules.

## 本地启动

### 1. 配置 `.env`（OpenAI 兼容）

`mei-lang` 的访问侧 AI 运行时可读取 OpenAI 兼容配置。

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

# 仅发布 access host（不暴露 build/config/upload）
cargo run -p mei-lang-server -- serve --host-surface access-only
```

默认行为：

- `mei-lang` 监听 **http://127.0.0.1:9527**（可用 `--port` 覆盖）
- 源码根目录 **`--source-root`** 未传时默认为仓库内 **`../workspaces`**
- 启动后提供默认宿主/runtime；访问侧 AI 若启用，将按 `.env` 中 OpenAI 兼容配置连接模型服务

## 当前边界

- **编辑侧**：默认交给 `Cursor / Codex / Claude Code / OpenCode` 等外部开发工具；`mei-lang` 提供 DSL、编译/lowering、宿主/runtime，以及后续可供这些工具消费的 `CLI / LSP / MCP` 接口。
- **访问侧**：`mei-lang` 宿主内置访问侧 AI，围绕当前 `scene/world/runtime` 做问答、查询、解释与临时视图。
- **仓库内 skill / agent 相关实现**：当前仍有部分历史 authoring Agent 代码与配置表面，正在逐步从编辑侧主线退出。

## 编辑侧 CLI

新的编辑侧主线优先通过 `mei` CLI 被外部工具消费：

```bash
cd mei-lang

# 编译 / 诊断
cargo run -p mei-lang-server -- check --app spbjw --json

# world / inventory
cargo run -p mei-lang-server -- inspect world --app spbjw --json
cargo run -p mei-lang-server -- inspect inventory --app spbjw --json

# 数据 / 指标 / runtime
cargo run -p mei-lang-server -- query dataset --app spbjw --id enterprise_profiles --json
cargo run -p mei-lang-server -- query metric --app spbjw --id enterprise_profiles --json
cargo run -p mei-lang-server -- runtime peek --app spbjw --json

# 机器可读 MCP surface 描述
cargo run -p mei-lang-server -- mcp describe --surface editor --json
cargo run -p mei-lang-server -- mcp describe --surface access --json

# host runtime contract 描述
cargo run -p mei-lang-server -- host describe --json

# editor MCP adapter（stdio）
npm run mcp:editor-adapter
npm run test:mcp:editor-adapter
```

浏览器打开根路径即可；应用页面路由形如 **`/apps/manage/<app_id>`**。

## 停止服务

- 停止 `mei-lang`：在 `mei serve` 所在终端按 `Ctrl+C`
- 停止 Martin（Docker）：`./scripts/stop_martin_docker.sh`（mei-projects 根目录）

## 最少配置

至少需要一组 OpenAI 兼容补全模型配置（见 `.env.example`）。若有多供应商，可通过 `OPENAI_IMITATORS` 追加前缀并配置对应的 `*_BASE_URL` / `*_API_KEY` / `*_COMPLETION_MODEL`。
