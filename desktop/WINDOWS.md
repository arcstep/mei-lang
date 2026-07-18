# Windows notes (Mei Viewer)

## Build

Do **not** cross-compile Windows installers from macOS. Use:

- GitHub Actions `windows-latest` (`.github/workflows/desktop-viewer.yml`), or
- a Windows VM / native machine: `scripts/desktop/collect-desktop-sidecars.sh --release` then `cd desktop && npm run build`

## Symlink / `env/current`

Viewer snapshot slots materialize:

- `apps/{app}/env/WS-yyyymmdd.n/` with `build/exchange/*.meibundle` (and optional data-snapshots)
- **Unix:** `env/current` → symlink to that generation
- **Windows:** `env/current/` as a real directory containing `.mei-build-target` pointing at `WS-yyyymmdd.n` (no Developer Mode / junction required)

Host / kernel only accept a symlink `current` on Unix; a plain directory named `current` with bundle content inside is rejected (`missing env/current`).

When opening a normal workspace that still uses Unix `env/current` → `WS-*` symlinks on Windows:

- Prefer enabling Windows Developer Mode, or
- ensure the active generation path is resolvable without following symlink (host already pins generation in runtime state; see 0537).

Desktop shell never invokes bash `deploy/*.sh`; it only spawns `.exe` sidecars.
