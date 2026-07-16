# mei-viewer（desktop）

本地桌面薄壳：监督 `mei-host-shell`，用系统浏览器打开同源 Web UI。  
可选监督 **Martin** 瓦片服务（从 GitHub 拉取本机二进制，无需 Docker）。  
设计 / 阶段见 `docs/mei-lang-v2/05-host/0541-desktop-viewer-implementation-plan.md`。

> **产物不在工作区里。**  
> 不会生成到 `workspaces/ws-demo-v2/`；固定输出在 `mei-lang/desktop/src-tauri/target/...`。

## 地图瓦片（Martin）

安装包**不内置** Martin。在启动器「地图瓦片」或菜单「地图瓦片」中：

1. **下载 / 更新** — 官方 Release（钉死与 Docker 对齐的版本）
2. **选择 MBTiles…** — 本地 `.mbtiles`（记住路径；开发机若存在 `gis/.../shapingba-z10-16.mbtiles` 可作默认）
3. **启动** — `127.0.0.1:8080`；打开工作区时自动注入 `MEI_GIS_PROXY_UPSTREAM` / `MEI_TILES_JSON_PATH`

数据目录：`~/Library/Application Support/MeiViewer/martin/`。详情见 `gis/spb/docs/martin-setup.md`。

## 本机构建（macOS）

**推荐一键**（在 `mei-lang` 根，或工作区根 `mei-projects/`）：

```bash
# 工作区根 → 转发到 mei-lang
./scripts/build-desktop-viewer.sh

# 或 mei-lang 根（真源）
cd mei-lang && ./scripts/build-desktop-viewer.sh

# 开发热跑：collect (debug) + tauri dev
./scripts/build-desktop-viewer.sh --dev

# 仅重打安装包（sidecar 已收集过）
./scripts/build-desktop-viewer.sh --skip-collect
```

等价 npm（在 `desktop/` 下）：

```bash
npm run build:all   # = collect --release + build
npm run dev:all     # = collect --debug + dev
```

不要设嵌套的 `CARGO_TARGET_DIR=./src-tauri/target`，否则会落到错误的 `src-tauri/src-tauri/target`。

产物：

```text
# 推荐：直接打开（无需解压；已 gitignore）
mei-lang/desktop/dist/mei-viewer.app

# 分发用 zip
mei-lang/desktop/dist/mei-viewer-<version>-aarch64-apple-darwin.zip
mei-lang/desktop/dist/MANIFEST.json

# Tauri 原始 bundle（也不会被 package 脚本删掉）
mei-lang/desktop/src-tauri/target/release/bundle/macos/mei-viewer.app
```

`npm run build` 会在 `tauri build` 之后跑 `scripts/package-release.mjs`：复制稳定 `.app` 到 `dist/`，并打版本化 zip。

打开：

```bash
open mei-lang/desktop/dist/mei-viewer.app

# 直开 demo 工作区：
open -na "mei-lang/desktop/dist/mei-viewer.app" --args "/Users/xuehongwei/codeup/mei-projects/workspaces/ws-demo-v2"
```

sidecar 收集与 `scripts/build.sh` 一样会做 **cargo target hygiene**（超预算才 clean）；二进制相对 `Cargo.lock` 未过期时默认跳过 cargo（`MEI_DESKTOP_FORCE_BUILD=1` 强制重编）。

开发热跑：

```bash
./scripts/build-desktop-viewer.sh --dev
# 或：cd desktop && npm run dev:all
```

环境变量：

- `MEI_DESKTOP_BIN`：sidecar 二进制目录（默认 `desktop/sidecars/bin`）

行为说明：

- **三能力**：打开工作区目录 · 导出快照（`.mei-snapshot.zip`）· 导入快照。
- **端口**：每次启动由 OS 分配空闲端口（`127.0.0.1:0`），不再固定 9527。
- **工作区直开**：若启动 cwd 或 CLI 参数是含 `workspace.json` 的目录，则跳过启动器，直接起 host；可用 ⌘/Ctrl+L 回到启动器导出快照或看日志。
- **macOS sidecar 签名**：`collect-desktop-sidecars.sh` 拷贝后会 adhoc resign；否则 `mei-app-runtime` 可能被 SIGKILL，启动 app 返回 503。

## Windows

见 [WINDOWS.md](WINDOWS.md)。在 Windows 本机或 CI runner 上同样 `collect` + `npm run build`。  
不要在 Mac 上交叉编译正式 Windows 安装包。

## GitHub Release

打 tag（版本需与 `Cargo.toml` `[workspace.package].version` 一致）会触发
[`.github/workflows/release.yml`](../.github/workflows/release.yml)，自动发布：

- **mei-viewer**：macOS zip、Windows NSIS setup
- **mei-toolchain**：`host-shell` / `compiler` / `app-runtime` / `plug-ds` / `snapshot` / `mei-lsp` / `mei-toolchain` 多平台归档
- **VS Code 扩展**：`mei-lang-*.vsix`

也可在 Actions 里手动 `Release` → `workflow_dispatch`（默认 draft）。

本地仅打 toolchain 归档：

```bash
./scripts/package-toolchain-release.sh
# → dist/toolchain/mei-toolchain-<ver>-<triple>.tar.gz
```

## 快照（GUI 与 CLI）


启动器：打开工作区后**多选** app →「导出快照…」（默认打 **portable v2**：含可移植配置、parquet、assets、csv/json，视频/底图外置）；对方「导入快照…」即可。  
导入后若有外置资源，用「待补齐资源」面板选择文件自动落位。

无 GUI 时：

```bash
# v2 portable（推荐）
mei-snapshot pack --workspace ../workspaces/ws-demo-v2 --app mini-data --app zhifa --portable \
  --out /tmp/demo.mei-snapshot.zip
# v1 兼容
mei-snapshot pack --workspace ../workspaces/ws-demo-v2 --app mini-data --out /tmp/mini-data.mei-snapshot.zip --include-data
mei-snapshot unpack --archive /tmp/demo.mei-snapshot.zip --into /tmp/snap-out
```
