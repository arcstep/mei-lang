# Feasibility spike（非产品 crate）

验证 Martin 与 mei-lang 协作的两条路径，**不**加入 `mei-lang` workspace members。

```bash
cd mei-lang
cargo run --manifest-path crates/mei-martin-spike/Cargo.toml -- both
# 或: library | subprocess
```

| 模式 | 依赖 | 结果（2026-07-18） |
|------|------|-------------------|
| `library` | `martin-core`（feature `mbtiles`） | 打开 `huale-z10-16.mbtiles`，`get_tile` 返回非空 PBF |
| `subprocess` | 本机 `martin` 二进制 + 随机端口 | 挂 `stock/gis/tiles/`，`/catalog` 列出全部 source，HTTP 取瓦片成功 |

目标产品形态见 `docs/tools/gis/05-martin-runtime.md` §「Host 托管 Martin」。
