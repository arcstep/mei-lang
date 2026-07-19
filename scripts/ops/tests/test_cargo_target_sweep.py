from __future__ import annotations

import importlib.util
import json
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "cargo-target-sweep-stale.py"
SPEC = importlib.util.spec_from_file_location("cargo_target_sweep", SCRIPT)
assert SPEC and SPEC.loader
SWEEP = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = SWEEP
SPEC.loader.exec_module(SWEEP)


class CargoTargetSweepTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.target = Path(self.temp_dir.name)
        self.debug = self.target / "debug"
        (self.debug / ".fingerprint").mkdir(parents=True)
        (self.debug / "deps").mkdir()
        (self.debug / "build").mkdir()
        (self.debug / "incremental").mkdir()

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def fingerprint(
        self,
        crate: str,
        hash_value: str,
        *,
        features: list[str],
        age: int,
    ) -> Path:
        path = self.debug / ".fingerprint" / f"{crate}-{hash_value}"
        path.mkdir()
        (path / f"lib-{crate}.json").write_text(
            json.dumps(
                {
                    "features": features,
                    "declared_features": features,
                    "target": 1,
                    "profile": 1,
                    "path": 1,
                }
            )
        )
        timestamp = path / "invoked.timestamp"
        timestamp.touch()
        modified = time.time() - age * 86_400
        os.utime(timestamp, (modified, modified))
        os.utime(path, (modified, modified))
        artifact = self.debug / "deps" / f"lib{crate}-{hash_value}.rlib"
        artifact.write_bytes(b"x" * 16)
        os.utime(artifact, (modified, modified))
        return path

    def run_sweep(self, *, dry_run: bool = False) -> int:
        return SWEEP.sweep_profile(
            self.debug,
            dry_run,
            max_age_days=30,
            incremental_max_age_days=14,
            link_max_age_days=7,
            keep_fingerprint_variants=2,
            keep_incremental_sessions=2,
            outside_crate_keys=set(),
            sweep_tests=False,
            sweep_incremental_all=False,
            prune_link_intermediates=True,
        )

    def test_distinct_feature_variants_are_not_collapsed(self) -> None:
        stream_hash = "1111111111111111"
        blocking_hash = "2222222222222222"
        stream = self.fingerprint(
            "reqwest", stream_hash, features=["stream"], age=90
        )
        blocking = self.fingerprint(
            "reqwest", blocking_hash, features=["stream", "blocking"], age=90
        )

        self.run_sweep()

        self.assertTrue(stream.exists())
        self.assertTrue(blocking.exists())

    def test_only_aged_superseded_identity_is_removed(self) -> None:
        oldest = self.fingerprint(
            "serde", "1111111111111111", features=["derive"], age=90
        )
        middle = self.fingerprint(
            "serde", "2222222222222222", features=["derive"], age=60
        )
        newest = self.fingerprint(
            "serde", "3333333333333333", features=["derive"], age=1
        )

        self.run_sweep()

        self.assertFalse(oldest.exists())
        self.assertTrue(middle.exists())
        self.assertTrue(newest.exists())
        self.assertFalse(
            (self.debug / "deps" / "libserde-1111111111111111.rlib").exists()
        )

    def test_orphans_and_old_link_objects_are_removed(self) -> None:
        orphan = self.debug / "deps" / "liborphan-aaaaaaaaaaaaaaaa.rlib"
        orphan.write_bytes(b"x")
        link_object = self.debug / "deps" / "runtime-bbbbbbbbbbbbbbbb.rcgu.o"
        link_object.write_bytes(b"x")
        modified = time.time() - 20 * 86_400
        os.utime(link_object, (modified, modified))

        self.run_sweep()

        self.assertFalse(orphan.exists())
        self.assertFalse(link_object.exists())

    def test_incremental_keeps_two_recent_sessions(self) -> None:
        sessions = []
        for index, age in enumerate((40, 30, 1), start=1):
            session = self.debug / "incremental" / f"mei_app-{index}"
            session.mkdir()
            (session / "data").write_bytes(b"x")
            modified = time.time() - age * 86_400
            os.utime(session, (modified, modified))
            sessions.append(session)

        self.run_sweep()

        self.assertFalse(sessions[0].exists())
        self.assertTrue(sessions[1].exists())
        self.assertTrue(sessions[2].exists())

    def test_dry_run_reports_without_deleting(self) -> None:
        orphan = self.debug / "deps" / "liborphan-aaaaaaaaaaaaaaaa.rlib"
        orphan.write_bytes(b"x" * 8)

        freed = self.run_sweep(dry_run=True)

        self.assertGreaterEqual(freed, 8)
        self.assertTrue(orphan.exists())

    def test_pressure_preserves_live_link_objects(self) -> None:
        older = self.debug / "deps" / "older-aaaaaaaaaaaaaaaa.rcgu.o"
        newer = self.debug / "deps" / "newer-bbbbbbbbbbbbbbbb.rcgu.o"
        older.write_bytes(b"x" * 8)
        newer.write_bytes(b"x" * 8)
        for crate, hash_value in (
            ("older", "aaaaaaaaaaaaaaaa"),
            ("newer", "bbbbbbbbbbbbbbbb"),
        ):
            fingerprint = self.debug / ".fingerprint" / f"{crate}-{hash_value}"
            fingerprint.mkdir()
            (fingerprint / f"lib-{crate}.json").write_text(
                json.dumps(
                    {
                        "features": [],
                        "declared_features": [],
                        "target": crate,
                        "profile": 1,
                        "path": crate,
                    }
                )
            )
        now = time.time()
        os.utime(older, (now - 100, now - 100))
        os.utime(newer, (now, now))

        freed = SWEEP.pressure_reclaim(
            self.target,
            False,
            8,
            keep_fingerprint_variants=2,
            keep_incremental_sessions=2,
        )

        self.assertEqual(freed, 0)
        self.assertTrue(older.exists())
        self.assertTrue(newer.exists())

    def test_pressure_preserves_two_full_fingerprint_identities(self) -> None:
        oldest = self.fingerprint(
            "serde", "1111111111111111", features=["derive"], age=3
        )
        middle = self.fingerprint(
            "serde", "2222222222222222", features=["derive"], age=2
        )
        newest = self.fingerprint(
            "serde", "3333333333333333", features=["derive"], age=1
        )
        stream = self.fingerprint(
            "reqwest", "4444444444444444", features=["stream"], age=3
        )
        blocking = self.fingerprint(
            "reqwest",
            "5555555555555555",
            features=["stream", "blocking"],
            age=3,
        )

        freed = SWEEP.pressure_reclaim(
            self.target,
            False,
            1_000_000,
            keep_fingerprint_variants=2,
            keep_incremental_sessions=2,
        )

        self.assertGreater(freed, 0)
        self.assertFalse(oldest.exists())
        self.assertTrue(middle.exists())
        self.assertTrue(newest.exists())
        self.assertTrue(stream.exists())
        self.assertTrue(blocking.exists())

    def test_pressure_preserves_two_incremental_sessions(self) -> None:
        sessions = []
        for index in range(3):
            session = self.debug / "incremental" / f"mei_app-{index}"
            session.mkdir()
            (session / "data").write_bytes(b"x" * 8)
            modified = time.time() - (3 - index) * 60
            os.utime(session, (modified, modified))
            sessions.append(session)

        SWEEP.pressure_reclaim(
            self.target,
            False,
            1_000_000,
            keep_fingerprint_variants=2,
            keep_incremental_sessions=2,
        )

        self.assertFalse(sessions[0].exists())
        self.assertTrue(sessions[1].exists())
        self.assertTrue(sessions[2].exists())

    def test_deep_pressure_can_evict_single_test_artifact(self) -> None:
        hash_value = "aaaaaaaaaaaaaaaa"
        fingerprint = self.debug / ".fingerprint" / f"mei-host-shell-{hash_value}"
        fingerprint.mkdir()
        (fingerprint / "dep-test-conformance.json").write_text(
            json.dumps(
                {
                    "features": [],
                    "declared_features": [],
                    "target": "conformance",
                    "profile": 1,
                    "path": 1,
                }
            )
        )
        artifact = self.debug / "deps" / f"conformance-{hash_value}"
        artifact.write_bytes(b"x" * 32)

        freed = SWEEP.pressure_reclaim(
            self.target,
            False,
            1_000_000,
            keep_fingerprint_variants=1,
            keep_incremental_sessions=1,
            include_tests=True,
        )

        self.assertGreater(freed, 0)
        self.assertFalse(fingerprint.exists())
        self.assertFalse(artifact.exists())


if __name__ == "__main__":
    unittest.main()
