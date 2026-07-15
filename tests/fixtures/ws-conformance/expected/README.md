# Optional stable JSON golden outputs for conformance cases.

Update only with:

```bash
MEI_UPDATE_GOLDEN=1 cargo test -p mei-host-graph conformance_ -- --nocapture
```

CI never writes files here. See `docs/mei-lang-v2/07-workspace-lifecycle/0710-mei-lang-conformance-fixtures.md`.

## Deprecated legacy

`crates/mei-host-graph/tests/fixtures/stage_architecture/*.runtime.json` and
`mei-compiler/.../tests/fixtures/stage_architecture/*.compiler.json` remain short-term
(for private-source baselines). Prefer promoting new cases into `ws-conformance` apps
instead of extending those zhifa/mini-park named goldens.
