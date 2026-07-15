# Demo fixtures for Mei Viewer

| File | Description |
|------|-------------|
| `mini-data.mei-snapshot.zip` | Packed from `workspaces/ws-demo-v2` app `mini-data` (`--include-data`) |

Regenerate:

```bash
cd mei-lang
cargo build -p mei-snapshot
./target/debug/mei-snapshot pack \
  --workspace ../workspaces/ws-demo-v2 \
  --app mini-data \
  --out desktop/fixtures/mini-data.mei-snapshot.zip \
  --include-data \
  --default-scene home
```
