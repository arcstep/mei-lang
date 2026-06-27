# mei-compiler

MeiLang 原生表面语言编译器。

- **v0 轨**：`.mei` → Decl IR JSON（ws-hello golden）
- **v2 轨**：MeiLang 2.0 → 图 Block JSON（parse → 宏展开 → lower）

设计与路线图见 monorepo 文档：`docs/mei-compiler/`。

```bash
cargo build -p mei-compiler
cargo test -p mei-compiler-tests

# v0
mei-compiler emit-decl --file path/to/file.mei
mei-compiler check --workspace ../workspaces/ws-hello --app hello

# v2
mei-compiler compile-v2 --workspace ../workspaces/ws-demo-v2 --app data-demo
mei-compiler parse-v2 --file path/to/file.mei --expand
```
