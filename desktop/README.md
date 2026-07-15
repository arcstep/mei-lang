# mei-viewer（desktop）

本地桌面薄壳：监督 `mei-host-shell`，用系统 WebView 打开同源 Web UI。  
设计 / 阶段见 `docs/mei-lang-v2/05-host/0541-desktop-viewer-implementation-plan.md`。

> **产物不在工作区里。**  
> 不会生成到 `workspaces/ws-demo-v2/`；固定输出在 `mei-lang/desktop/src-tauri/target/...`。

## 本机构建（macOS）

在 **`mei-lang/desktop`** 下执行（不要设嵌套的 `CARGO_TARGET_DIR=./src-tauri/target`，否则会落到错误的 `src-tauri/src-tauri/target`）：

```bash
# 1) 收集 sidecar（在 mei-lang 根）
cd /Users/xuehongwei/codeup/mei-projects/mei-lang
./scripts/collect-desktop-sidecars.sh --release   # 开发可用 --debug

# 2) 安装依赖并打包
cd desktop
npm install
npm run build
```

产物：

```text
# 安装包本体（Finder 里的应用名）
mei-lang/desktop/src-tauri/target/release/bundle/macos/mei-viewer.app

# 带版本号、方便分发/下载的 zip（推荐）
mei-lang/desktop/dist/mei-viewer-<version>-aarch64-apple-darwin.zip
mei-lang/desktop/dist/MANIFEST.json
```

`npm run build` 会在 `tauri build` 之后自动跑 `scripts/package-release.mjs`，生成版本化 zip（版本 = `tauri.conf.json#version` + git 短哈希）。

打开：

```bash
open "/Users/xuehongwei/codeup/mei-projects/mei-lang/desktop/src-tauri/target/release/bundle/macos/mei-viewer.app"

# 或解压 dist 里的 zip 后再 open
# 直开 demo 工作区：
open -na "mei-viewer" --args "/Users/xuehongwei/codeup/mei-projects/workspaces/ws-demo-v2"
```

开发热跑：

```bash
cd mei-lang/desktop
npm run dev
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

## 快照（GUI 与 CLI）

启动器：打开工作区后选 app →「导出快照…」；对方「导入快照…」即可。

无 GUI 时：

```bash
mei-snapshot pack --workspace ../workspaces/ws-demo-v2 --app mini-data --out /tmp/mini-data.mei-snapshot.zip --include-data
mei-snapshot unpack --archive /tmp/mini-data.mei-snapshot.zip --into /tmp/snap-out
```
