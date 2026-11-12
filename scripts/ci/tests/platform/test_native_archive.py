import json
import os
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.ci.cross_platform import archive as ci

SHA = "a" * 40


class NativeArchiveTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.destination = self.root / "artifacts"

    def test_missing_debug_directory_does_not_upload_empty_artifacts(self):
        self.assertIsNone(ci.archive_debug(self.root, self.destination, "linux-amd64", SHA))
        self.assertFalse(self.destination.exists())

    def test_archive_preserves_debug_files_permissions_and_metadata(self):
        debug = self.root / "target" / "debug"
        files = [
            "deps/library.rlib",
            "binary.pdb",
            "binary.dSYM/Contents/symbols",
            ".fingerprint/library",
            "incremental/cache",
        ]
        for name in files:
            path = debug / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)
        executable = debug / "binary"
        executable.write_text("debug binary")
        executable.chmod(0o755)
        archive = ci.archive_debug(self.root, self.destination, "linux-amd64", SHA)
        assert archive is not None
        self.assertEqual(archive.name, f"debug-linux-amd64-{SHA}.tar.gz")
        with tarfile.open(archive) as bundle:
            names = bundle.getnames()
            for name in files[:-1]:
                self.assertIn(f"target/debug/{name}", names)
            self.assertFalse(any(name.startswith("target/debug/incremental") for name in names))
            binary = bundle.extractfile("target/debug/binary")
            assert binary is not None
            self.assertEqual(binary.read(), b"debug binary")
            if os.name != "nt":
                self.assertEqual(bundle.getmember("target/debug/binary").mode & 0o777, 0o755)
            metadata = bundle.extractfile("build-info.json")
            assert metadata is not None
            self.assertEqual(
                json.load(metadata),
                {"target": "linux-amd64", "sha": SHA, "profile": "debug"},
            )

    def test_main_only_sets_output_when_artifacts_exist(self):
        output = self.root / "github-output"
        output.write_text("existing=value\n", encoding="utf-8")
        environment = {
            "CI_TARGET": "linux-amd64",
            "CI_SHA": SHA,
            "RUNNER_TEMP": str(self.destination),
            "GITHUB_OUTPUT": str(output),
        }
        for has_artifacts in [False, True]:
            with self.subTest(has_artifacts=has_artifacts):
                if has_artifacts:
                    debug = self.root / "target/debug"
                    debug.mkdir(parents=True)
                    (debug / "library.rlib").write_text("debug library")
                with (
                    patch.dict(os.environ, environment),
                    patch("sys.argv", [ci.__file__]),
                    patch.object(ci.Path, "cwd", return_value=self.root),
                ):
                    ci.main()
                archive = self.destination / "cross-platform-artifacts" / f"debug-linux-amd64-{SHA}.tar.gz"
                self.assertEqual(archive.is_file(), has_artifacts)
                expected = "existing=value\n" + (f"path={archive}\n" if has_artifacts else "")
                self.assertEqual(output.read_text(encoding="utf-8"), expected)

    def test_rejects_unsafe_archive_names(self):
        for target, sha in [("../outside", SHA), ("linux-amd64", "../outside")]:
            with self.subTest(target=target, sha=sha), self.assertRaises(ValueError):
                ci.archive_debug(self.root, self.destination, target, sha)


if __name__ == "__main__":
    unittest.main()
