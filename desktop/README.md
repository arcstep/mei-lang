# mei-viewer（desktop）

本地桌面薄壳：监督 `mei-host-shell`，用系统浏览器打开同源 Web UI。  
瓦片服务由 **Host 自动托管** bundled `martin` sidecar（工作区 `stock/gis/tiles/*.mbtiles`）；启动器**不再**独立管理 Martin。  
设计见 `docs/mei-lang-v2/05-host/0541-desktop-viewer-implementation-plan.md`。

> **产物不在工作区里。**  
> 不会生成到 `workspaces/ws-demo-v2/`；固定输出在 `mei-lang/desktop/src-tauri/target/...` 与 `desktop/dist/`。

## 默认工作区与导入

- **默认工作区**：`~/Library/Application Support/MeiViewer/workspace`（Windows：`%AppData%\MeiViewer\workspace`）。
- 首次启动自动创建骨架（`workspace.json` + 空 `apps/` + `stock/gis/tiles`）。
- **导入快照**：合并到默认工作区——覆盖包内 app 与同名 stock 文件，**不删除**其中其它 app。
- 也可手工把工作区文件拷进该目录；「打开其它工作区…」留给开发/高级用户。

## 地图瓦片（Martin）

安装包**内置** `sidecars/bin/martin`。打开工作区后若存在 `stock/gis/tiles/*.mbtiles`，Host 自动拉起 Martin 并经 `/gis` 代理。  
高级用户仍可设 `MEI_GIS_PROXY_UPSTREAM` / `MEI_MARTIN_BIN` 覆盖。

## 本机构建（macOS）

**推荐一键**（在 `mei-lang` 根，或工作区根 `mei-projects/`）：

```bash
# 工作区根 → 转发到 mei-lang
./scripts/desktop/build-desktop-viewer.sh

# 或 mei-lang 根（真源）
cd mei-lang && ./scripts/desktop/build-desktop-viewer.sh

# 开发热跑：collect (debug) + tauri dev
./scripts/desktop/build-desktop-viewer.sh --dev

# 仅重打安装包（sidecar 已收集过）
./scripts/desktop/build-desktop-viewer.sh --skip-collect
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
mei-lang/desktop/dist/mei-viewer-<version>-aarch64-apple-darwin.manifest.json

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

sidecar 收集与 `scripts/build/build.sh` 一样会做 **cargo target hygiene**（超预算才 clean）；二进制相对 `Cargo.lock` 未过期时默认跳过 cargo（`MEI_DESKTOP_FORCE_BUILD=1` 强制重编）。

开发热跑：

```bash
./scripts/desktop/build-desktop-viewer.sh --dev
# 或：cd desktop && npm run dev:all
```

环境变量：

- `MEI_DESKTOP_BIN`：sidecar 二进制目录（默认 `desktop/sidecars/bin`）

行为说明：

- **能力**：打开默认工作区 · 打开其它工作区 · 导出快照（`.mei-snapshot.zip`）· 导入合并到默认工作区。
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
- **mei-runtime**：服务运行所需四个二进制及 app/stock 资源
- **mei-toolchain**：runtime + `snapshot` / `mei-lsp` / `mei-toolchain` 及 app/stock 资源
- **VS Code 扩展**：`mei-lang-*.vsix`
- **发布元数据**：release manifest、SHA-256、SPDX SBOM 与 GitHub attestation

Actions 中手动执行 `Release` 只构建并验收候选产物，不创建 GitHub
Release；只有来自默认分支、且与 Cargo 版本完全一致的 `v*` tag 才会正式发布。

本地打 runtime + toolchain 归档：

```bash
./scripts/release/package-release-bundles.sh
# → dist/bundles/mei-{runtime,toolchain}-<ver>-<triple>.tar.gz|zip
```

## 快照（GUI 与 CLI）

启动器：打开工作区后**多选** app →「导出快照…」（默认 **portable v2**：含可移植配置、parquet、assets、csv/json、**工作区 stock/gis**；视频默认外置，可勾选「包含大媒体」）。对方「导入快照到默认工作区…」即可 merge。  
外置视频等可在导入后手工放入对应 `upload/` 路径，或下次导出时勾选「包含大媒体」。

无 GUI 时：

```bash
# v2 portable（推荐；默认含 stock/gis）
mei-snapshot pack --workspace ../workspaces/ws-demo-v2 --app mini-data --app zhifa --portable \
  --out /tmp/demo.mei-snapshot.zip
# 含大媒体
mei-snapshot pack --workspace ../workspaces/ws-demo-v2 --app zhifa --portable --include-media \
  --out /tmp/zhifa-full.mei-snapshot.zip
# v1 兼容
mei-snapshot pack --workspace ../workspaces/ws-demo-v2 --app mini-data --out /tmp/mini-data.mei-snapshot.zip --include-data
mei-snapshot unpack --archive /tmp/demo.mei-snapshot.zip --into /tmp/snap-out
```
