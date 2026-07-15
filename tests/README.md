# tests

| 路径 | 职责 |
|------|------|
| [`e2e/`](e2e/) | Playwright 浏览器端到端规格 |

各 crate 内的 Rust `tests/`、`mei-compiler-tests` 仍跟 Cargo 约定留在原处。Playwright 配置见仓库根 `playwright.config.mjs`（`testDir` 指向本目录）。
