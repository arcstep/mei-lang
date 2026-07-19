#!/usr/bin/env python3
"""Age-aware, fingerprint-safe reclamation for a Cargo target directory.

The default mode keeps every newest compilation identity, including distinct
feature sets and target kinds. It removes only orphaned hash artifacts, aged
superseded fingerprints, aged linker objects, and aged incremental sessions.
Destructive profile/test/package sweeps remain explicit options.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import time
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

HASH_RE = re.compile(r"-([0-9a-f]{16})(?:\.|$)")
FP_HASH_RE = re.compile(r"^[0-9a-f]{16}$")


@dataclass(frozen=True)
class ReclaimCandidate:
    category: str
    priority: int
    mtime: float
    paths: tuple[Path, ...]
    bytes: int


def dir_size(path: Path) -> int:
    if not path.exists():
        return 0
    if path.is_file() or path.is_symlink():
        return path.lstat().st_size
    return sum(
        child.lstat().st_size
        for child in path.rglob("*")
        if child.is_file() or child.is_symlink()
    )


def remove_path(path: Path, dry_run: bool) -> int:
    size = dir_size(path)
    if dry_run:
        return size
    if path.is_file() or path.is_symlink():
        path.unlink(missing_ok=True)
    elif path.is_dir():
        shutil.rmtree(path, ignore_errors=True)
    return size


def split_fingerprint_name(name: str) -> tuple[str, str] | tuple[None, None]:
    crate_key, hash_value = name.rsplit("-", 1)
    if not FP_HASH_RE.fullmatch(hash_value):
        return None, None
    return crate_key, hash_value


def artifact_stem(package_name: str) -> str:
    return package_name.replace("-", "_")


def age_days(path: Path, now: float) -> float:
    try:
        return max(0.0, (now - path.stat().st_mtime) / 86_400)
    except OSError:
        return 0.0


def fingerprint_mtime(path: Path) -> float:
    timestamp = path / "invoked.timestamp"
    try:
        return timestamp.stat().st_mtime
    except OSError:
        try:
            return path.stat().st_mtime
        except OSError:
            return 0.0


def stable_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), default=str)


def fingerprint_identity(path: Path) -> tuple[str, ...]:
    """Return the full Cargo unit identity, not merely crate/kind.

    Cargo legitimately stores multiple hashes for one crate when feature sets,
    targets, profiles, or build-script inputs differ. Those variants must not
    be collapsed by recency alone.
    """
    identities: list[str] = []
    for candidate in sorted(path.glob("*.json")):
        try:
            payload = json.loads(candidate.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        identities.append(
            stable_json(
                {
                    "file_kind": candidate.name.rsplit("-", 1)[0],
                    "features": payload.get("features"),
                    "declared_features": payload.get("declared_features"),
                    "target": payload.get("target"),
                    "profile": payload.get("profile"),
                    "path": payload.get("path"),
                }
            )
        )
    if identities:
        return tuple(identities)
    kinds = sorted(
        child.name.split("-", 1)[0]
        for child in path.iterdir()
        if child.is_file() and child.name != "invoked.timestamp"
    )
    return tuple(kinds or ["unknown"])


def package_to_crate_key(package_name: str) -> str:
    return package_name.replace("_", "-")


def cargo_metadata(manifest: Path) -> dict[str, Any] | None:
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
        print(
            f"warn: cargo metadata failed; package-scope sweep skipped: {proc.stderr.strip()}",
            file=sys.stderr,
        )
        return None
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as error:
        print(f"warn: invalid cargo metadata JSON: {error}", file=sys.stderr)
        return None


def workspace_packages_outside_closure(
    manifest: Path, keep_packages: list[str]
) -> set[str]:
    metadata = cargo_metadata(manifest)
    if metadata is None:
        return set()
    id_to_package = {package["id"]: package for package in metadata["packages"]}
    roots = [
        package_id
        for package_id, package in id_to_package.items()
        if package["name"] in keep_packages
    ]
    node_deps = {
        node["id"]: [dependency["pkg"] for dependency in node.get("deps", [])]
        for node in metadata["resolve"]["nodes"]
    }
    closure = set(roots)
    queue = list(roots)
    while queue:
        package_id = queue.pop()
        for dependency in node_deps.get(package_id, []):
            if dependency not in closure:
                closure.add(dependency)
                queue.append(dependency)
    return {
        package_to_crate_key(package["name"])
        for package_id, package in id_to_package.items()
        if package.get("source") is None and package_id not in closure
    }


def remove_hash_artifacts(
    profile_dir: Path, removed_hashes: set[str], dry_run: bool
) -> int:
    if not removed_hashes:
        return 0
    freed = 0
    deps = profile_dir / "deps"
    if deps.is_dir():
        for artifact in deps.iterdir():
            if not artifact.is_file():
                continue
            match = HASH_RE.search(artifact.name)
            if match and match.group(1) in removed_hashes:
                freed += remove_path(artifact, dry_run)
    build_root = profile_dir / "build"
    if build_root.is_dir():
        for entry in build_root.iterdir():
            _crate_key, hash_value = split_fingerprint_name(entry.name)
            if hash_value in removed_hashes:
                freed += remove_path(entry, dry_run)
    return freed


def hash_artifact_paths(profile_dir: Path, hash_value: str) -> list[Path]:
    paths: list[Path] = []
    deps = profile_dir / "deps"
    if deps.is_dir():
        for artifact in deps.iterdir():
            if not artifact.is_file():
                continue
            match = HASH_RE.search(artifact.name)
            if match and match.group(1) == hash_value:
                paths.append(artifact)
    build_root = profile_dir / "build"
    if build_root.is_dir():
        for entry in build_root.iterdir():
            _crate_key, entry_hash = split_fingerprint_name(entry.name)
            if entry_hash == hash_value:
                paths.append(entry)
    return paths


def remove_orphan_hash_artifacts(profile_dir: Path, dry_run: bool) -> int:
    fingerprint_root = profile_dir / ".fingerprint"
    live_hashes: set[str] = set()
    if fingerprint_root.is_dir():
        for entry in fingerprint_root.iterdir():
            if entry.is_dir():
                _crate_key, hash_value = split_fingerprint_name(entry.name)
                if hash_value:
                    live_hashes.add(hash_value)

    orphan_hashes: set[str] = set()
    for root_name in ("deps", "build"):
        root = profile_dir / root_name
        if not root.is_dir():
            continue
        for entry in root.iterdir():
            match = HASH_RE.search(entry.name)
            if match and match.group(1) not in live_hashes:
                orphan_hashes.add(match.group(1))
    return remove_hash_artifacts(profile_dir, orphan_hashes, dry_run)


def sweep_aged_fingerprints(
    profile_dir: Path, dry_run: bool, max_age_days: int, keep_per_identity: int
) -> int:
    fingerprint_root = profile_dir / ".fingerprint"
    if not fingerprint_root.is_dir():
        return 0
    now = time.time()
    groups: dict[tuple[str, tuple[str, ...]], list[Path]] = defaultdict(list)
    for entry in fingerprint_root.iterdir():
        if not entry.is_dir():
            continue
        crate_key, hash_value = split_fingerprint_name(entry.name)
        if crate_key and hash_value:
            groups[(crate_key, fingerprint_identity(entry))].append(entry)

    removed_hashes: set[str] = set()
    freed = 0
    for entries in groups.values():
        entries.sort(key=fingerprint_mtime, reverse=True)
        for stale in entries[max(1, keep_per_identity) :]:
            stale_age_days = max(
                0.0, (now - fingerprint_mtime(stale)) / 86_400
            )
            if stale_age_days < max_age_days:
                continue
            _crate_key, hash_value = split_fingerprint_name(stale.name)
            if hash_value:
                removed_hashes.add(hash_value)
            freed += remove_path(stale, dry_run)
    freed += remove_hash_artifacts(profile_dir, removed_hashes, dry_run)
    return freed


def sweep_incremental_sessions(
    profile_dir: Path,
    dry_run: bool,
    max_age_days: int,
    keep_per_crate: int,
) -> int:
    incremental = profile_dir / "incremental"
    if not incremental.is_dir():
        return 0
    now = time.time()
    groups: dict[str, list[Path]] = defaultdict(list)
    for entry in incremental.iterdir():
        if entry.is_dir():
            crate_name = entry.name.rsplit("-", 1)[0]
            groups[crate_name].append(entry)
    freed = 0
    for entries in groups.values():
        entries.sort(key=lambda path: path.stat().st_mtime, reverse=True)
        for stale in entries[max(1, keep_per_crate) :]:
            if age_days(stale, now) >= max_age_days:
                freed += remove_path(stale, dry_run)
    return freed


def sweep_linker_intermediates(
    profile_dir: Path, dry_run: bool, max_age_days: int
) -> int:
    deps = profile_dir / "deps"
    if not deps.is_dir():
        return 0
    now = time.time()
    freed = 0
    for artifact in deps.iterdir():
        if not artifact.is_file():
            continue
        if not (artifact.name.endswith(".o") or ".rcgu.o" in artifact.name):
            continue
        if age_days(artifact, now) >= max_age_days:
            freed += remove_path(artifact, dry_run)
    return freed


def pressure_reclaim(
    target_dir: Path,
    dry_run: bool,
    reclaim_bytes: int,
    *,
    keep_fingerprint_variants: int,
    keep_incremental_sessions: int,
    include_tests: bool = False,
) -> int:
    """Reclaim bounded local caches when the hard watermark is exceeded.

    TTL is intentionally ignored, but each full fingerprint identity and each
    incremental crate stem retains its newest sessions. Candidates are removed
    in increasing rebuild-cost order until the requested logical byte budget is
    reached. The caller re-measures physical `du` and may invoke another pass.
    """
    candidates: list[ReclaimCandidate] = []
    claimed_paths: set[Path] = set()

    def add_candidate(
        category: str,
        priority: int,
        mtime: float,
        paths: list[Path],
    ) -> None:
        unique_paths: list[Path] = []
        for path in paths:
            if path in claimed_paths or not path.exists():
                continue
            claimed_paths.add(path)
            unique_paths.append(path)
        candidate_bytes = sum(dir_size(path) for path in unique_paths)
        if unique_paths and candidate_bytes > 0:
            candidates.append(
                ReclaimCandidate(
                    category=category,
                    priority=priority,
                    mtime=mtime,
                    paths=tuple(unique_paths),
                    bytes=candidate_bytes,
                )
            )

    for profile in ("debug", "release"):
        profile_dir = target_dir / profile
        if not profile_dir.is_dir():
            continue
        fingerprint_root = profile_dir / ".fingerprint"
        live_hashes: set[str] = set()
        fingerprint_groups: dict[
            tuple[str, tuple[str, ...]], list[Path]
        ] = defaultdict(list)
        if fingerprint_root.is_dir():
            for entry in fingerprint_root.iterdir():
                if not entry.is_dir():
                    continue
                crate_key, hash_value = split_fingerprint_name(entry.name)
                if crate_key and hash_value:
                    live_hashes.add(hash_value)
                    fingerprint_groups[
                        (crate_key, fingerprint_identity(entry))
                    ].append(entry)

        orphan_paths_by_hash: dict[str, list[Path]] = defaultdict(list)
        for root_name in ("deps", "build"):
            root = profile_dir / root_name
            if not root.is_dir():
                continue
            for entry in root.iterdir():
                match = HASH_RE.search(entry.name)
                if match and match.group(1) not in live_hashes:
                    orphan_paths_by_hash[match.group(1)].append(entry)
        for paths in orphan_paths_by_hash.values():
            add_candidate(
                "orphan",
                0,
                min((path.stat().st_mtime for path in paths), default=0.0),
                paths,
            )

        incremental = profile_dir / "incremental"
        incremental_groups: dict[str, list[Path]] = defaultdict(list)
        if incremental.is_dir():
            for entry in incremental.iterdir():
                if entry.is_dir():
                    incremental_groups[entry.name.rsplit("-", 1)[0]].append(entry)
        for entries in incremental_groups.values():
            entries.sort(key=lambda path: path.stat().st_mtime, reverse=True)
            for stale in entries[max(1, keep_incremental_sessions) :]:
                add_candidate(
                    "incremental-session",
                    20,
                    stale.stat().st_mtime,
                    [stale],
                )

        for entries in fingerprint_groups.values():
            entries.sort(key=fingerprint_mtime, reverse=True)
            if include_tests:
                test_entries: set[Path] = set()
                for test_entry in list(entries):
                    is_test = any(
                        child.name.startswith(("dep-test-", "test-"))
                        for child in test_entry.iterdir()
                        if child.is_file()
                    )
                    if not is_test:
                        continue
                    test_entries.add(test_entry)
                    _crate_key, hash_value = split_fingerprint_name(test_entry.name)
                    paths = [test_entry]
                    if hash_value:
                        paths.extend(hash_artifact_paths(profile_dir, hash_value))
                    add_candidate(
                        "test-artifact",
                        5,
                        fingerprint_mtime(test_entry),
                        paths,
                    )
                entries = [entry for entry in entries if entry not in test_entries]
            for stale in entries[max(1, keep_fingerprint_variants) :]:
                _crate_key, hash_value = split_fingerprint_name(stale.name)
                paths = [stale]
                if hash_value:
                    paths.extend(hash_artifact_paths(profile_dir, hash_value))
                add_candidate(
                    "superseded-fingerprint",
                    30,
                    fingerprint_mtime(stale),
                    paths,
                )

    candidates.sort(
        key=lambda candidate: (
            candidate.priority,
            candidate.mtime,
            candidate.category,
        )
    )
    freed = 0
    removed_by_category: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for candidate in candidates:
        if freed >= reclaim_bytes:
            break
        candidate_freed = sum(
            remove_path(path, dry_run) for path in candidate.paths
        )
        freed += candidate_freed
        removed_by_category[candidate.category][0] += 1
        removed_by_category[candidate.category][1] += candidate_freed

    for category in sorted(removed_by_category):
        count, category_bytes = removed_by_category[category]
        print(
            f"pressure[{category}]: candidates={count} "
            f"bytes={category_bytes}",
            file=sys.stderr,
        )
    return freed


def sweep_selected_fingerprints(
    profile_dir: Path,
    dry_run: bool,
    *,
    outside_crate_keys: set[str],
    sweep_tests: bool,
) -> int:
    fingerprint_root = profile_dir / ".fingerprint"
    if not fingerprint_root.is_dir():
        return 0
    removed_hashes: set[str] = set()
    freed = 0
    for entry in list(fingerprint_root.iterdir()):
        if not entry.is_dir():
            continue
        crate_key, hash_value = split_fingerprint_name(entry.name)
        is_test = any(
            child.name.startswith(("dep-test-", "test-"))
            for child in entry.iterdir()
            if child.is_file()
        )
        if (crate_key in outside_crate_keys) or (sweep_tests and is_test):
            if hash_value:
                removed_hashes.add(hash_value)
            freed += remove_path(entry, dry_run)
    freed += remove_hash_artifacts(profile_dir, removed_hashes, dry_run)
    for crate_key in outside_crate_keys:
        stem = artifact_stem(crate_key)
        for pattern in (crate_key, stem, f"lib{stem}"):
            for path in profile_dir.glob(f"{pattern}*"):
                if path.name not in {"deps", "build", "incremental", ".fingerprint"}:
                    freed += remove_path(path, dry_run)
    return freed


def sweep_profile(
    profile_dir: Path,
    dry_run: bool,
    *,
    max_age_days: int,
    incremental_max_age_days: int,
    link_max_age_days: int,
    keep_fingerprint_variants: int,
    keep_incremental_sessions: int,
    outside_crate_keys: set[str],
    sweep_tests: bool,
    sweep_incremental_all: bool,
    prune_link_intermediates: bool,
) -> int:
    freed = remove_orphan_hash_artifacts(profile_dir, dry_run)
    freed += sweep_aged_fingerprints(
        profile_dir, dry_run, max_age_days, keep_fingerprint_variants
    )
    freed += sweep_selected_fingerprints(
        profile_dir,
        dry_run,
        outside_crate_keys=outside_crate_keys,
        sweep_tests=sweep_tests,
    )
    incremental = profile_dir / "incremental"
    if sweep_incremental_all:
        freed += remove_path(incremental, dry_run)
    else:
        freed += sweep_incremental_sessions(
            profile_dir,
            dry_run,
            incremental_max_age_days,
            keep_incremental_sessions,
        )
    if prune_link_intermediates:
        freed += sweep_linker_intermediates(
            profile_dir, dry_run, link_max_age_days
        )
    return freed


def drop_inactive_profile(
    target_dir: Path, active_profile: str, dry_run: bool
) -> int:
    freed = 0
    for profile in ("debug", "release"):
        if profile != active_profile:
            profile_dir = target_dir / profile
            if profile_dir.is_dir():
                freed += remove_path(profile_dir, dry_run)
    return freed


def parse_keep_packages(raw: str) -> list[str]:
    return [item.strip() for item in raw.split(",") if item.strip()]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target_dir", type=Path)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--manifest-path", type=Path)
    parser.add_argument("--keep-packages", default="")
    parser.add_argument(
        "--active-profile", choices=("debug", "release", "both"), default="both"
    )
    parser.add_argument("--max-age-days", type=int, default=30)
    parser.add_argument("--incremental-max-age-days", type=int, default=14)
    parser.add_argument("--link-max-age-days", type=int, default=7)
    parser.add_argument("--keep-fingerprint-variants", type=int, default=2)
    parser.add_argument("--keep-incremental-sessions", type=int, default=2)
    parser.add_argument("--drop-inactive-profile", action="store_true")
    parser.add_argument("--profile-drop-only", action="store_true")
    parser.add_argument("--sweep-tests", action="store_true")
    parser.add_argument("--sweep-out-of-scope", action="store_true")
    parser.add_argument("--sweep-incremental", action="store_true")
    parser.add_argument("--incremental-only", action="store_true")
    parser.add_argument("--no-prune-link-intermediates", action="store_true")
    parser.add_argument(
        "--pressure-reclaim-bytes",
        type=int,
        default=0,
        help="ignore TTL and reclaim this many logical bytes without full clean",
    )
    parser.add_argument(
        "--pressure-sweep-tests",
        action="store_true",
        help="allow hard-pressure mode to evict test artifacts",
    )
    args = parser.parse_args()

    target_dir = args.target_dir.resolve()
    active_profile = "debug" if args.active_profile == "both" else args.active_profile
    if args.profile_drop_only:
        freed = drop_inactive_profile(target_dir, active_profile, args.dry_run)
        print(f"freed_bytes={freed}")
        return 0
    if args.incremental_only:
        freed = sum(
            remove_path(target_dir / profile / "incremental", args.dry_run)
            for profile in ("debug", "release")
        )
        print(f"freed_bytes={freed}")
        return 0
    if args.pressure_reclaim_bytes > 0:
        freed = pressure_reclaim(
            target_dir,
            args.dry_run,
            args.pressure_reclaim_bytes,
            keep_fingerprint_variants=max(1, args.keep_fingerprint_variants),
            keep_incremental_sessions=max(1, args.keep_incremental_sessions),
            include_tests=args.pressure_sweep_tests,
        )
        print(f"freed_bytes={freed}")
        return 0

    freed = 0
    if args.drop_inactive_profile:
        freed += drop_inactive_profile(target_dir, active_profile, args.dry_run)

    outside_crate_keys: set[str] = set()
    keep_packages = parse_keep_packages(args.keep_packages)
    if args.sweep_out_of_scope and args.manifest_path and keep_packages:
        outside_crate_keys = workspace_packages_outside_closure(
            args.manifest_path.resolve(), keep_packages
        )

    profiles = (
        (active_profile,)
        if args.drop_inactive_profile
        else tuple(
            profile
            for profile in ("debug", "release")
            if (target_dir / profile).is_dir()
        )
    )
    for profile in profiles:
        freed += sweep_profile(
            target_dir / profile,
            args.dry_run,
            max_age_days=max(0, args.max_age_days),
            incremental_max_age_days=max(0, args.incremental_max_age_days),
            link_max_age_days=max(0, args.link_max_age_days),
            keep_fingerprint_variants=max(1, args.keep_fingerprint_variants),
            keep_incremental_sessions=max(1, args.keep_incremental_sessions),
            outside_crate_keys=outside_crate_keys,
            sweep_tests=args.sweep_tests,
            sweep_incremental_all=args.sweep_incremental,
            prune_link_intermediates=not args.no_prune_link_intermediates,
        )
    print(f"freed_bytes={freed}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
