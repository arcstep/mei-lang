# mei-lang/scripts

本目录是 **平台工程脚本**（构建、门禁、回归、发布），不是应用业务代码。按类别分子目录，避免根目录脚本海。

## 目录

| 目录 | 用途 | 典型入口 |
|------|------|----------|
| [`build/`](build/) | 前端 assets / scene bundle / 体积扫描 | `npm run assets:build` |
| [`env/`](env/) | **冷启动**：ensure / fill / bootstrap（三场景；见 SSOT 0608） | `./scripts/env/bootstrap.sh` |
| [`theme/`](theme/) | Shell 主题 lint、token、fragment | `npm run test:theme-tokens` |
| [`check/`](check/) | 回归门禁 shell | `npm run test:auth-regressions` 等 |
| [`test/`](test/) | 静态/单元级 Node 测试 | `npm run test:spa-static` |
| [`audit/`](audit/) | Eval / SPA / surface 手工或套件审计 | `npm run test:eval-suite` |
| [`perf/`](perf/) | 性能采样与场景配置 | `npm run perf:sample` |
| [`release/`](release/) | 版本同步、manifest、release bundle、Martin sidecar | `npm run release:test` |
| [`mcp/`](mcp/) | Author / Access MCP stdio adapter | `npm run mcp:author-adapter` |
| [`ops/`](ops/) | cargo target 清理、快照对比、presentation 编译等运维 | 手工 |
| [`lib/`](lib/) | 被上述脚本复用的库代码 | — |

Desktop Viewer 已迁至 monorepo `tools/mei-viewer/`（独立仓）。

## 常用命令

```bash
# 资产构建（0 warning 约定）
npm run assets:build

# SPA / artifact 静态门禁
npm run test:spa-static
npm run test:artifact-static

# Eval 回归套件（需本地 host）
npm run test:eval-suite

# Release bundle
./scripts/release/package-release-bundles.sh
```

## 约定

1. **公开仓只放平台工具**；业务探针（如 `perf/zhifa-runtime-perf.mjs`）若仍保留，仅作可选本地联调，不作为 CI 硬门禁。
2. 脚本定位 `mei-lang` 根目录时，子目录脚本使用 `../..`（相对 `scripts/<category>/`）。
3. 新增脚本请放入对应分类，并在本 README 补一行；`package.json` / `.github/workflows` 同步改路径。
4. 本地采样产物（`perf-wave*.jsonl` 等）已 gitignore，勿提交。

## 路径迁移（2026-07）

旧扁平路径（如 `scripts/cargo-target-gc.sh`、`scripts/build.sh`）已迁入上表分类目录。  
工作区 `deploy/lib.sh` / `install.sh` 与 `mei-lang/stock/workspace/deploy/*` 须引用：

- `scripts/ops/cargo-target-gc.sh`
- `scripts/build/build.sh`

完整对照见本目录各子文件夹；`package.json` 与 `.github/workflows` 已同步。
