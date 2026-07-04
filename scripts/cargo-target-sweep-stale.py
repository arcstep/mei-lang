#!/usr/bin/env python3
"""Remove stale Cargo target artifacts before a full `cargo clean`.

Phases (when enabled via flags / env-backed CLI args):
  1. Drop the inactive profile directory (debug vs release).
  2. Remove workspace packages outside the runtime dependency closure.
  3. Remove integration-test fingerprints and binaries.
  4. Drop duplicate / orphaned deps for superseded fingerprint hashes.
  5. Optionally wipe incremental (phase-2 hygiene; slower rebuild than clean).
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

HASH_RE = re.compile(r"-([0-9a-f]{16})(?:\.|$)")
FP_HASH_RE = re.compile(r"^[0-9a-f]{16}$")


def dir_size(path: Path) -> int:
    total = 0
    if not path.exists():
        return 0
    if path.is_file():
        return path.stat().st_size
    for child in path.rglob("*"):
        if child.is_file():
            total += child.stat().st_size
    return total


def _remove_path(path: Path, dry_run: bool) -> int:
    size = dir_size(path)
    if dry_run:
        return size
    if path.is_file() or path.is_symlink():
        path.unlink(missing_ok=True)
    elif path.is_dir():
        shutil.rmtree(path, ignore_errors=True)
    return size


def _split_fingerprint_name(name: str) -> tuple[str, str] | tuple[None, None]:
    crate_key, hash_val = name.rsplit("-", 1)
    if not FP_HASH_RE.fullmatch(hash_val):
        return None, None
    return crate_key, hash_val


def _fingerprint_kinds(fp_dir: Path) -> set[str]:
    kinds: set[str] = set()
    for child in fp_dir.iterdir():
        if not child.is_file():
            continue
        name = child.name
        if name.startswith("bin-"):
            kinds.add(f"bin:{name[4:]}")
        elif name.startswith("dep-bin-"):
            kinds.add(f"bin:{name[8:]}")
        elif name.startswith("dep-lib-"):
            kinds.add("lib")
        elif name.startswith("dep-test-"):
            kinds.add(f"test:{name[9:]}")
        elif "build-script" in name:
            kinds.add("build-script")
        elif name.startswith("run-build-script"):
            kinds.add("run-build-script")
        else:
            kinds.add("other")
    return kinds or {"other"}


def _package_to_crate_key(package_name: str) -> str:
    return package_name.replace("_", "-")


def _artifact_stem(package_name: str) -> str:
    return package_name.replace("-", "_")


def resolve_keep_crate_keys(manifest: Path, keep_packages: list[str]) -> set[str]:
    """Transitive dependency closure crate keys (dash form) for keep roots."""
    proc = subprocess.run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            str(manifest),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return {_package_to_crate_key(name) for name in keep_packages}

    meta = json.loads(proc.stdout)
    id_to_name = {package["id"]: package["name"] for package in meta["packages"]}
    root_ids = [package["id"] for package in meta["packages"] if package["name"] in keep_packages]
    node_deps = {
        node["id"]: [dep["pkg"] for dep in node.get("deps", [])] for node in meta["resolve"]["nodes"]
    }

    closure: set[str] = set(root_ids)
    queue = list(root_ids)
    while queue:
        package_id = queue.pop()
        for dep_id in node_deps.get(package_id, []):
            if dep_id not in closure:
                closure.add(dep_id)
                queue.append(dep_id)

    return {_package_to_crate_key(id_to_name[package_id]) for package_id in closure}


def workspace_packages_outside_closure(manifest: Path, keep_crate_keys: set[str]) -> set[str]:
    proc = subprocess.run(
        [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            str(manifest),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return set()

    meta = json.loads(proc.stdout)
    outside: set[str] = set()
    for package in meta["packages"]:
        if package.get("source") is not None:
            continue
        crate_key = _package_to_crate_key(package["name"])
        if crate_key not in keep_crate_keys:
            outside.add(crate_key)
    return outside


def fingerprint_groups(fp_root: Path) -> tuple[set[str], list[Path]]:
    live_hashes: set[str] = set()
    stale_dirs: list[Path] = []
    groups: dict[tuple[str, str], list[tuple[str, float, Path]]] = defaultdict(list)

    for entry in fp_root.iterdir():
        if not entry.is_dir():
            continue
        crate_key, hash_val = _split_fingerprint_name(entry.name)
        if not crate_key:
            continue
        kinds = _fingerprint_kinds(entry)
        ts = entry / "invoked.timestamp"
        mtime = ts.stat().st_mtime if ts.exists() else 0.0
        for kind in kinds:
            groups[(crate_key, kind)].append((hash_val, mtime, entry))

    seen_dirs: set[Path] = set()
    for entries in groups.values():
        if len(entries) == 1:
            live_hashes.add(entries[0][0])
            continue
        max_mtime = max(item[1] for item in entries)
        keep = {item[0] for item in entries if item[1] == max_mtime and max_mtime > 0}
        if not keep:
            keep = {max(entries, key=lambda item: item[1])[0]}
        live_hashes.update(keep)
        for hash_val, _mtime, path in entries:
            if hash_val not in keep and path not in seen_dirs:
                stale_dirs.append(path)
                seen_dirs.add(path)

    return live_hashes, stale_dirs


def sweep_duplicate_incremental_dirs(incremental: Path, dry_run: bool) -> int:
    if not incremental.is_dir():
        return 0
    groups: dict[str, list[Path]] = defaultdict(list)
    for entry in incremental.iterdir():
        if not entry.is_dir():
            continue
        stem = entry.name.rsplit("-", 1)[0] if "-" in entry.name else entry.name
        groups[stem].append(entry)

    freed = 0
    for dirs in groups.values():
        if len(dirs) <= 1:
            continue
        dirs.sort(key=lambda path: path.stat().st_mtime, reverse=True)
        for stale in dirs[1:]:
            freed += _remove_path(stale, dry_run)
    return freed


def sweep_incremental_all(incremental: Path, dry_run: bool) -> int:
    if not incremental.is_dir():
        return 0
    return _remove_path(incremental, dry_run)


def sweep_profile_root_artifacts(profile_dir: Path, crate_keys: set[str], dry_run: bool) -> int:
    freed = 0
    for crate_key in crate_keys:
        stem = _artifact_stem(crate_key)
        for pattern in (crate_key, stem, f"lib{stem}"):
            for path in profile_dir.glob(f"{pattern}*"):
                if path.name in {"deps", "build", "incremental", ".fingerprint", "examples"}:
                    continue
                freed += _remove_path(path, dry_run)
    return freed


def sweep_hashes_from_fingerprints(
    profile_dir: Path,
    removed_hashes: set[str],
    dry_run: bool,
) -> int:
    freed = 0
    if not removed_hashes:
        return 0

    deps = profile_dir / "deps"
    if deps.is_dir():
        for artifact in deps.iterdir():
            if not artifact.is_file():
                continue
            match = HASH_RE.search(artifact.name)
            if match and match.group(1) in removed_hashes:
                freed += _remove_path(artifact, dry_run)

    build_root = profile_dir / "build"
    if build_root.is_dir():
        for entry in build_root.iterdir():
            _crate_key, hash_val = _split_fingerprint_name(entry.name)
            if hash_val and hash_val in removed_hashes:
                freed += _remove_path(entry, dry_run)

    return freed


def sweep_out_of_scope_workspace(
    profile_dir: Path,
    outside_crate_keys: set[str],
    dry_run: bool,
) -> int:
    if not outside_crate_keys:
        return 0

    freed = 0
    removed_hashes: set[str] = set()
    fp_root = profile_dir / ".fingerprint"
    if fp_root.is_dir():
        for entry in list(fp_root.iterdir()):
            if not entry.is_dir():
                continue
            crate_key, hash_val = _split_fingerprint_name(entry.name)
            if crate_key and crate_key in outside_crate_keys and hash_val:
                removed_hashes.add(hash_val)
                freed += _remove_path(entry, dry_run)

    freed += sweep_hashes_from_fingerprints(profile_dir, removed_hashes, dry_run)
    freed += sweep_profile_root_artifacts(profile_dir, outside_crate_keys, dry_run)
    freed += sweep_duplicate_incremental_dirs(profile_dir / "incremental", dry_run)
    return freed


def sweep_test_artifacts(profile_dir: Path, dry_run: bool) -> int:
    freed = 0
    removed_hashes: set[str] = set()
    fp_root = profile_dir / ".fingerprint"
    if fp_root.is_dir():
        for entry in list(fp_root.iterdir()):
            if not entry.is_dir():
                continue
            kinds = _fingerprint_kinds(entry)
            if not any(kind.startswith("test:") for kind in kinds):
                continue
            _crate_key, hash_val = _split_fingerprint_name(entry.name)
            if hash_val:
                removed_hashes.add(hash_val)
            freed += _remove_path(entry, dry_run)

    freed += sweep_hashes_from_fingerprints(profile_dir, removed_hashes, dry_run)
    return freed


def sweep_stale_hashes(profile_dir: Path, dry_run: bool) -> int:
    freed = 0
    fp_root = profile_dir / ".fingerprint"
    if not fp_root.is_dir():
        return 0

    live_hashes, stale_fp_dirs = fingerprint_groups(fp_root)

    deps = profile_dir / "deps"
    if deps.is_dir():
        for artifact in deps.iterdir():
            if not artifact.is_file():
                continue
            match = HASH_RE.search(artifact.name)
            if not match:
                continue
            if match.group(1) not in live_hashes:
                freed += _remove_path(artifact, dry_run)

    build_root = profile_dir / "build"
    if build_root.is_dir():
        for entry in build_root.iterdir():
            _crate_key, hash_val = _split_fingerprint_name(entry.name)
            if hash_val and hash_val not in live_hashes:
                freed += _remove_path(entry, dry_run)

    for stale in stale_fp_dirs:
        freed += _remove_path(stale, dry_run)

    return freed


def sweep_profile(
    profile_dir: Path,
    dry_run: bool,
    *,
    outside_crate_keys: set[str],
    sweep_tests: bool,
    sweep_incremental: bool,
) -> int:
    freed = 0
    if sweep_incremental:
        freed += sweep_incremental_all(profile_dir / "incremental", dry_run)
    else:
        freed += sweep_duplicate_incremental_dirs(profile_dir / "incremental", dry_run)

    freed += sweep_out_of_scope_workspace(profile_dir, outside_crate_keys, dry_run)
    if sweep_tests:
        freed += sweep_test_artifacts(profile_dir, dry_run)
    freed += sweep_stale_hashes(profile_dir, dry_run)
    return freed


def drop_inactive_profile(target_dir: Path, active_profile: str, dry_run: bool) -> int:
    freed = 0
    for profile in ("debug", "release"):
        if profile == active_profile:
            continue
        profile_dir = target_dir / profile
        if profile_dir.is_dir():
            freed += _remove_path(profile_dir, dry_run)
    return freed


def sweep_target(
    target_dir: Path,
    dry_run: bool,
    *,
    manifest: Path | None,
    keep_packages: list[str],
    active_profile: str,
    drop_other_profile: bool,
    sweep_tests: bool,
    sweep_incremental: bool,
    incremental_only: bool = False,
) -> int:
    if not target_dir.is_dir():
        return 0

    if incremental_only:
        total = 0
        for profile in ("debug", "release"):
            profile_dir = target_dir / profile
            if profile_dir.is_dir():
                total += sweep_incremental_all(profile_dir / "incremental", dry_run)
        return total

    total = 0
    if drop_other_profile and active_profile in {"debug", "release"}:
        total += drop_inactive_profile(target_dir, active_profile, dry_run)

    outside_crate_keys: set[str] = set()
    if manifest and keep_packages:
        keep_crate_keys = resolve_keep_crate_keys(manifest, keep_packages)
        outside_crate_keys = workspace_packages_outside_closure(manifest, keep_crate_keys)

    profiles = ("debug", "release") if not drop_other_profile else (active_profile,)
    for profile in profiles:
        profile_dir = target_dir / profile
        if profile_dir.is_dir():
            total += sweep_profile(
                profile_dir,
                dry_run,
                outside_crate_keys=outside_crate_keys,
                sweep_tests=sweep_tests,
                sweep_incremental=sweep_incremental,
            )
    return total


def _parse_keep_packages(raw: str | None) -> list[str]:
    if not raw:
        return []
    return [item.strip() for item in raw.split(",") if item.strip()]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target_dir", type=Path)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--manifest-path", type=Path, default=None)
    parser.add_argument(
        "--keep-packages",
        default="",
        help="comma-separated root package names (e.g. mei-compiler,mei-host-shell)",
    )
    parser.add_argument(
        "--active-profile",
        choices=("debug", "release", "both"),
        default="both",
        help="profile being built; used with --drop-inactive-profile",
    )
    parser.add_argument(
        "--drop-inactive-profile",
        action="store_true",
        help="remove the entire non-active profile directory",
    )
    parser.add_argument(
        "--sweep-tests",
        action="store_true",
        help="remove integration-test fingerprints and deps",
    )
    parser.add_argument(
        "--sweep-incremental",
        action="store_true",
        help="remove the entire incremental cache (phase-2 hygiene)",
    )
    parser.add_argument(
        "--incremental-only",
        action="store_true",
        help="only remove incremental caches (used by phase-2 hygiene)",
    )
    args = parser.parse_args()

    keep_packages = _parse_keep_packages(args.keep_packages)
    manifest = args.manifest_path.resolve() if args.manifest_path else None
    active_profile = args.active_profile
    if active_profile == "both":
        active_profile = "debug"

    freed = sweep_target(
        args.target_dir.resolve(),
        args.dry_run,
        manifest=manifest,
        keep_packages=keep_packages,
        active_profile=active_profile,
        drop_other_profile=args.drop_inactive_profile,
        sweep_tests=args.sweep_tests,
        sweep_incremental=args.sweep_incremental,
        incremental_only=args.incremental_only,
    )
    print(f"freed_bytes={freed}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
