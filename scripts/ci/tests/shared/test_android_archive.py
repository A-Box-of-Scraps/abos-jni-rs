import json
import tarfile
import tempfile
import unittest
from pathlib import Path

from scripts.ci.cross_platform import archive as ci

SHA = "a" * 40


class AndroidArchiveTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.destination = self.root / "artifacts"

    def test_android_archive_includes_both_abis_apks_and_logs(self):
        files = [
            "target/aarch64-linux-android/debug/libtestactivity.so",
            "target/x86_64-linux-android/debug/libtestactivity.so",
            "build/test-apks/arm64-v8a/app-debug.apk",
            "build/test-apks/x86_64/app-debug.apk",
            "build/ci-logs/logcat.txt",
        ]
        excluded = [
            "target/aarch64-linux-android/debug/incremental/cache",
            "target/x86_64-linux-android/debug/incremental/cache",
            "target/debug/host-tool",
            "build/jni-temporary/libtestactivity.so",
        ]
        for name in [*files, *excluded]:
            path = self.root / ci.ANDROID_PROJECT / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)
        archive = ci.archive_debug(self.root, self.destination, "android", SHA)
        assert archive is not None
        self.assertEqual(archive.name, f"debug-android-{SHA}.tar.gz")
        with tarfile.open(archive) as bundle:
            names = bundle.getnames()
            for name in files:
                self.assertIn(f"{ci.ANDROID_PROJECT}/{name}", names)
            for name in excluded:
                self.assertNotIn(f"{ci.ANDROID_PROJECT}/{name}", names)
            self.assertFalse(any("incremental" in Path(name).parts for name in names))
            metadata = bundle.extractfile("build-info.json")
            assert metadata is not None
            self.assertEqual(json.load(metadata), {"target": "android", "sha": SHA, "profile": "debug"})

    def test_android_partial_build_or_failure_logs_are_archived(self):
        for directory in ci.ANDROID_DIRECTORIES:
            with self.subTest(directory=directory), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                output = root / directory / "output"
                output.parent.mkdir(parents=True)
                output.write_text("partial output")
                archive = ci.archive_debug(root, self.destination, "android", SHA)
                assert archive is not None
                with tarfile.open(archive) as bundle:
                    self.assertIn(f"{directory}/output", bundle.getnames())

    def test_android_does_not_archive_unrelated_native_outputs(self):
        (self.root / ci.DEBUG_DIRECTORY).mkdir(parents=True)
        self.assertIsNone(ci.archive_debug(self.root, self.destination, "android", SHA))
        self.assertFalse(self.destination.exists())


if __name__ == "__main__":
    unittest.main()
