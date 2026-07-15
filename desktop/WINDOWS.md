# Windows notes (Mei Viewer)

## Build

Do **not** cross-compile Windows installers from macOS. Use:

- GitHub Actions `windows-latest` (`.github/workflows/desktop-viewer.yml`), or
- a Windows VM / native machine: `scripts/collect-desktop-sidecars.sh --release` then `cd desktop && npm run build`

## Symlink / `env/current`

Viewer snapshot slots materialize `apps/{app}/env/current/` as a **real directory** (not a symlink) so Windows does not require Developer Mode or elevation for junctions.

When opening a normal workspace that still uses Unix `env/current` → `WS-*` symlinks:

- Prefer enabling Windows Developer Mode, or
- ensure the active generation path is resolvable without following symlink (host already pins generation in runtime state; see 0537).

Desktop shell never invokes bash `deploy/*.sh`; it only spawns `.exe` sidecars.
