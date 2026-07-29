AI-native scene orchestration language for building apps from world models, UI components, and capability modules.

## 下载安装包

预构建安装包发布在 GitHub Releases：桌面用 **mei-viewer**，服务器用 **mei-runtime**，
开发/LSP 用 **mei-toolchain**，编辑器用仓库内 [`extensions/mei-lang-vscode`](extensions/mei-lang-vscode) 的 VSIX。
说明见 [`desktop/README.md`](desktop/README.md#github-release)。正式发版推送与 Cargo 版本一致的 `v*` tag；日常 `git push` 不自动打安装包。

## 本地启动

### 1. 配置 `.env`（OpenAI 兼容）

`mei-lang` 的访问侧 AI 运行时可读取 OpenAI 兼容配置。

最小示例见 [`.env.example`](.env.example)（复制为 `.env` 后填写密钥）。

### 2. GIS 底图（Martin）

`map.maplibre` 需要 HTTP 瓦片（MBTiles）。**`mei-host-shell serve` 会在工作区存在 `stock/gis/tiles/*.mbtiles` 且能找到 `martin` 二进制时，自动在随机本地端口拉起 Martin**，浏览器仍走同源 `/gis`。

```bash
cd mei-lang
cargo run -p mei-host-shell -- serve --workspace <workspace-root>
```

- 应用：**http://127.0.0.1:9527**（默认端口 9527，避开 macOS AirPlay 占用的 5000）
- 已设 `MEI_GIS_PROXY_UPSTREAM` 时 **不**自动拉起（沿用外部 Docker/手动 Martin）
- 可选覆盖：`MEI_MARTIN_BIN`、`MEI_TILES_JSON_PATH`（见 `.env.example`）

### 3. 启动宿主（主路径：`mei-host-shell`）

```bash
cd mei-lang
cargo run -p mei-host-shell -- serve --workspace <workspace-root>

# 启用登录鉴权（须先完成下方配置，否则启动失败）
cargo run -p mei-host-shell -- serve --workspace <workspace-root> --auth
```

启用 `--auth` 前，先初始化 workspace-local host-state（写入 `{source_root}/.mei/local/hosts/*.state.json`）：

```bash
cargo run -p mei-host-shell -- auth ensure-keys --workspace <workspace-root> --json
cargo run -p mei-host-shell -- auth bootstrap-users --workspace <workspace-root> --json
```

旧入口 `mei-host-web` 已迁入 [`archive/mei-host-web/`](archive/mei-host-web/)（待手工清理）；请只使用 `mei-host-shell`。

默认行为：

- 监听 **http://127.0.0.1:9527**（可用 `--port` 覆盖）
- **必须**传 `--workspace <workspace-root>`（工作区源码根）
- 组件/模板：`mei workspace materialize` 物化到 profile 的 `stock/`（Git 跟踪）；运行时目录由工作区约定生成；未物化时只读 `mei-lang/stock/`
- **默认不要求登录**；传 **`--auth`** 后除登录页与静态资源外，访问页面/API 均需先登录（且须已配置用户与密钥，否则启动失败）
- 密码规则（新建用户、改密、`bootstrap-users`）：**至少 8 位**，且须含大写/小写/数字/符号；明文密码只能从 **stdin** 或浏览器 **RSA-OAEP(SHA-256)** 加密后提交
- 启动后提供默认宿主/runtime；访问侧 AI 若启用，将按 `.env` 中 OpenAI 兼容配置连接模型服务

## 当前边界

- **编辑侧**：默认交给 `Cursor / Codex / Claude Code / OpenCode` 等外部开发工具；`mei-lang` 提供 DSL、编译/lowering、宿主/runtime，以及后续可供这些工具消费的 `CLI / LSP / MCP` 接口。
- **编辑器识别**：Cursor / VS Code 安装本仓库扩展 [`extensions/mei-lang-vscode`](extensions/mei-lang-vscode)（language id `mei` + TextMate + `mei-lsp`；推荐配套 Even Better TOML 以校验 `app.toml`）。作者态说明见 [`agent/knowledge/editor-runtime/language-and-editor-recognition.md`](agent/knowledge/editor-runtime/language-and-editor-recognition.md)。
- **访问侧**：`mei-lang` 宿主内置访问侧 AI，围绕当前 `scene/world/runtime` 做问答、查询、解释与临时视图。
- **仓库内 skill / agent 相关实现**：当前仍有部分历史 authoring Agent 代码与配置表面，正在逐步从编辑侧主线退出。

## 编辑侧 CLI

新的编辑侧主线优先通过 `mei-toolchain` CLI 被外部工具消费；兼容入口 `mei` 仍保留，但推荐新脚本逐步切到双入口：

```bash
cd mei-lang

# 编译 / 诊断（示例应用见 workspaces/ws-dev/examples）
cargo run -p mei-lang-server --bin mei-toolchain -- check --workspace ws-dev --app examples/core/01-single-file-doc --json

# world / inventory
cargo run -p mei-lang-server --bin mei-toolchain -- inspect world --workspace ws-dev --app examples/core/01-single-file-doc --json
cargo run -p mei-lang-server --bin mei-toolchain -- inspect inventory --workspace ws-dev --app examples/ds/01-dataset-baseline --json

# 数据 / 指标 / runtime
cargo run -p mei-lang-server --bin mei-toolchain -- query dataset --workspace ws-dev --app examples/ds/01-dataset-baseline --id sample_rows --json
cargo run -p mei-lang-server --bin mei-toolchain -- query metric --workspace ws-dev --app examples/ds/01-dataset-baseline --id sample_count --json
cargo run -p mei-lang-server --bin mei-toolchain -- runtime peek --workspace ws-dev --app examples/core/01-single-file-doc --json

# 机器可读 MCP surface 描述
cargo run -p mei-lang-server --bin mei-toolchain -- mcp describe --surface author --json
cargo run -p mei-lang-server --bin mei-toolchain -- mcp describe --surface access --json

# host auth / describe（mei-host-shell）
cargo run -p mei-host-shell -- auth describe --workspace <workspace-root> --json

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
- 瓦片：Host 默认可自动托管 Martin（见上文 GIS）；外部服务可用 `./scripts/martin/start_martin.sh` 或 Docker，并设 `MEI_GIS_PROXY_UPSTREAM`
- 分发含 `bin/martin`：`mei-lang/scripts/desktop/collect-desktop-sidecars.sh`（经 `fetch-martin-sidecar.sh`）

## 最少配置

至少需要一组 OpenAI 兼容补全模型配置（见 `.env.example`）。若有多供应商，可通过 `OPENAI_IMITATORS` 追加前缀并配置对应的 `*_BASE_URL` / `*_API_KEY` / `*_COMPLETION_MODEL`。
