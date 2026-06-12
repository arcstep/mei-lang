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
cargo run -p mei-lang-server --bin mei-host-web -- serve
```

- 应用：**http://127.0.0.1:9527**（默认端口 9527，避开 macOS AirPlay 占用的 5000）
- 浏览器瓦片默认：**同源 `/gis`**；宿主默认将 `/gis` 代理到 **http://127.0.0.1:8080**，TileJSON 路径 **`/shapingba-z10-16`**

在 `mei-lang/.env` 中可改：

```bash
MEI_GIS_PROXY_UPSTREAM=http://127.0.0.1:8080
MEI_TILES_JSON_PATH=/shapingba-z10-16
```

未在 `.mei` 里写 `mapSpec.basemap` 时，预览页会默认走 `/gis`；实际代理上游由上述环境变量决定。更完整的安装与排错见 monorepo **`gis/spb/docs/martin-setup.md`**。

停止 Martin：`./scripts/stop_martin_docker.sh`（在 mei-projects 根目录）。

### 3. 启动 `mei-lang`

```bash
cd mei-lang
cargo run -p mei-lang-server --bin mei-host-web -- serve

# 启用登录鉴权（须先完成下方配置，否则启动失败）
cargo run -p mei-lang-server --bin mei-host-web -- serve --auth

# 仅发布 access host（不暴露 build/config/upload）
cargo run -p mei-lang-server --bin mei-host-web -- serve --host-surface access-only
```

启用 `--auth` 前，先初始化 workspace-local host-state（写入 `{source_root}/.mei/local/hosts/*.state.json`）：

```bash
# 生成 JWT / RSA 密钥（写入 .mei/local/hosts/*.state.json）
cargo run -p mei-lang-server --bin mei-host-web -- host auth ensure-keys --source-root ../workspaces/ws-dev --json

# 方案 A1：一次性初始化 super/admin/guest（各账号随机临时密码，只打印一次）
cargo run -p mei-lang-server --bin mei-host-web -- host auth bootstrap-users --source-root ../workspaces/ws-dev --json

# 方案 A2：本地调试可用统一初始密码（super/admin/guest 共用，从 stdin 读取）
printf '%s' 'Debug1!pwd' | cargo run -p mei-lang-server --bin mei-host-web -- host auth bootstrap-users \
  --source-root ../workspaces/ws-dev --default-password-stdin --json

# 方案 B1：手工编辑 auth.users[] 时，用 stdin 生成 Argon2 哈希
printf '%s' 'YourPwd1!complex' | cargo run -p mei-lang-server --bin mei-host-web -- host auth hash-password --json

# 方案 B2：工具链直接新增单个用户（同样从 stdin 读密码）
printf '%s' 'YourPwd1!complex' | cargo run -p mei-lang-server --bin mei-host-web -- host auth add-user \
  --source-root ../workspaces/ws-dev --username guest01 --role guest --password-stdin --json
```

默认行为：

- `mei-lang` 监听 **http://127.0.0.1:9527**（可用 `--port` 覆盖）
- 工作区默认 **`--workspace ws-dev`**（等价 `--source-root ../workspaces/ws-dev`）；生产对照用 **`--workspace ws-spbjw`**
- 组件/模板：`mei workspace materialize` 物化到 profile 的 `.stock/`（Git 跟踪）；`.mei/` 仅运行时；未物化时只读 `mei-lang/stock/`
- **默认不要求登录**（顶栏无账户入口，认证 API 不可用）；传 **`--auth`** 后除登录页与静态资源外，访问页面/API 均需先登录（且须已配置用户与密钥，否则启动失败）
- 密码规则（新建用户、改密、`bootstrap-users`）：**至少 8 位**，且须含大写/小写/数字/符号；明文密码只能从 **stdin** 或浏览器 **RSA-OAEP(SHA-256)** 加密后提交，禁止命令行参数与登录 API 明文 `password` 字段（HTTP 内网不依赖 SSL，登录页内置纯 JS 加密）
- 启动时**不会**自动同步 MeiLang skill；需要时显式传 **`--sync-agent-skill`**（或与 **`--auto-agent`** 联用）
- 启动后提供默认宿主/runtime；访问侧 AI 若启用，将按 `.env` 中 OpenAI 兼容配置连接模型服务

## 当前边界

- **编辑侧**：默认交给 `Cursor / Codex / Claude Code / OpenCode` 等外部开发工具；`mei-lang` 提供 DSL、编译/lowering、宿主/runtime，以及后续可供这些工具消费的 `CLI / LSP / MCP` 接口。
- **访问侧**：`mei-lang` 宿主内置访问侧 AI，围绕当前 `scene/world/runtime` 做问答、查询、解释与临时视图。
- **仓库内 skill / agent 相关实现**：当前仍有部分历史 authoring Agent 代码与配置表面，正在逐步从编辑侧主线退出。

## 编辑侧 CLI

新的编辑侧主线优先通过 `mei-toolchain` CLI 被外部工具消费；兼容入口 `mei` 仍保留，但推荐新脚本逐步切到双入口：

```bash
cd mei-lang

# 编译 / 诊断
cargo run -p mei-lang-server --bin mei-toolchain -- check --app spbjw --json

# world / inventory
cargo run -p mei-lang-server --bin mei-toolchain -- inspect world --app spbjw --json
cargo run -p mei-lang-server --bin mei-toolchain -- inspect inventory --app spbjw --json

# 数据 / 指标 / runtime
cargo run -p mei-lang-server --bin mei-toolchain -- query dataset --app spbjw --id enterprise_profiles --json
cargo run -p mei-lang-server --bin mei-toolchain -- query metric --app spbjw --id enterprise_profiles --json
cargo run -p mei-lang-server --bin mei-toolchain -- runtime peek --app spbjw --json

# 机器可读 MCP surface 描述
cargo run -p mei-lang-server --bin mei-toolchain -- mcp describe --surface author --json
cargo run -p mei-lang-server --bin mei-toolchain -- mcp describe --surface access --json

# host runtime contract 描述
cargo run -p mei-lang-server --bin mei-host-web -- host describe --json

# author MCP adapter（stdio）
npm run mcp:author-adapter
npm run test:mcp:author-adapter

# standalone author runtime
cargo run -p mei-lang-server --bin mei-toolchain -- editor-runtime describe --json
cargo run -p mei-lang-server --bin mei-toolchain -- editor-runtime doctor --json
cargo run -p mei-lang-server --bin mei-toolchain -- knowledge --surface author --include-content --json
cargo run -p mei-lang-server --bin mei-toolchain -- workspace bootstrap --source-root /tmp/mei-demo --app hello --tool cursor --json
cargo run -p mei-lang-server --bin mei-toolchain -- workspace create-app another-app --source-root /tmp/mei-demo --json
```

浏览器打开根路径即可；应用页面路由形如 **`/apps/manage/<app_id>`**。

## 停止服务

- 停止 `mei-lang`：在 `mei serve` 所在终端按 `Ctrl+C`；若端口仍被占用，可执行 `lsof -ti tcp:9527 | xargs kill`
- 日志里的 `synced MeiLang skill` 仅表示 skill 文件同步，**不是**自动拉起外部 Agent 进程；默认启动不会做这一步
- 停止 Martin（Docker）：`./scripts/stop_martin_docker.sh`（mei-projects 根目录）

## 最少配置

至少需要一组 OpenAI 兼容补全模型配置（见 `.env.example`）。若有多供应商，可通过 `OPENAI_IMITATORS` 追加前缀并配置对应的 `*_BASE_URL` / `*_API_KEY` / `*_COMPLETION_MODEL`。
