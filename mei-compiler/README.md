# mei-compiler

MeiLang 2.0 编译器。**`.meibundle` 交换产物仅由此二进制产生**，不经 `mei-toolchain` / `mei-host-web`。

- **v0 遗留**：`emit-decl` / `check`（ws-hello Decl IR golden）
- **主路径**：`.mei` → `compile` → `.meibundle`（parse → 宏展开 → lower → manifest + zstd）

设计与路线图见 monorepo 文档：`docs/mei-compiler/`。

```bash
cargo build -p mei-compiler
cargo test -p mei-compiler-tests

# v0 遗留
mei-compiler emit-decl --file path/to/file.mei
mei-compiler check --workspace ../workspaces/ws-hello --app hello

# 编译（默认写 apps/{app}/.mei/compile/{app}.meibundle）
mei-compiler compile --workspace ../workspaces/ws-demo-v2 --app data-demo

# 人类查看
mei-compiler bundle inspect apps/data-demo/.mei/compile/data-demo.meibundle --pretty
mei-compiler bundle stats apps/data-demo/.mei/compile/data-demo.meibundle

# 调试：stdout 去重 JSON
mei-compiler compile --workspace ../workspaces/ws-demo-v2 --app data-demo --format json --pretty

mei-compiler parse --file path/to/file.mei --expand
```

Crate 布局：`mei-syntax`、`mei-graph`、`mei-bundle`、`compiler`（bin）。
