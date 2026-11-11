import importlib.util
import io
import json
import os
import subprocess
import tarfile
import tempfile
import unittest
import urllib.error
import urllib.parse
from email.message import Message
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "ci-cross-platform.py"
SPEC = importlib.util.spec_from_file_location("ci_cross_platform", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise ImportError(f"Unable to load script: {SCRIPT}")
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)
SHA = "a" * 40
OTHER_SHA = "b" * 40


class MatrixTests(unittest.TestCase):
    def test_all_contains_six_native_targets(self):
        matrix = ci.select_matrix("all")["include"]
        self.assertEqual(len(matrix), 6)
        self.assertEqual(
            {entry["target"] for entry in matrix},
            {f"{system}-{arch}" for system in ["linux", "windows", "macos"] for arch in ["amd64", "arm64"]},
        )
        self.assertEqual(len({entry["runner"] for entry in matrix}), 6)

    def test_each_target_selects_one_runner(self):
        for target, runner in ci.RUNNERS.items():
            with self.subTest(target=target):
                self.assertEqual(ci.select_matrix(target), {"include": [{"target": target, "runner": runner}]})

    def test_invalid_target_fails(self):
        with self.assertRaises(ValueError):
            ci.select_matrix("linux")


class ScheduleTests(unittest.TestCase):
    def setUp(self):
        self.enterContext(
            patch.dict(
                os.environ,
                {
                    "GITHUB_EVENT_NAME": "schedule",
                    "GITHUB_RUN_ATTEMPT": "1",
                    "GITHUB_API_URL": "https://api.github.com",
                    "GITHUB_REPOSITORY": "owner/repo",
                    "GITHUB_REF_NAME": "default/branch",
                    "GITHUB_WORKFLOW_REF": "owner/repo/.github/workflows/cross-platform.yml@refs/heads/default/branch",
                    "GITHUB_RUN_NUMBER": "200",
                    "GH_TOKEN": "test-token",
                },
            )
        )
        self.urlopen = self.enterContext(patch.object(ci.urllib.request, "urlopen"))

    def respond(self, runs):
        self.urlopen.return_value = io.BytesIO(json.dumps({"workflow_runs": runs}).encode())

    def test_first_schedule_runs(self):
        self.respond([{"run_number": 200, "head_sha": SHA}])
        self.assertTrue(ci.should_run(SHA))

    def test_unchanged_commit_skips_regardless_of_previous_result(self):
        for conclusion in ["success", "failure", "cancelled", None]:
            with self.subTest(conclusion=conclusion):
                self.respond([{"run_number": 199, "head_sha": SHA, "conclusion": conclusion}])
                self.assertFalse(ci.should_run(SHA))

    def test_changed_commit_runs(self):
        self.respond([{"run_number": 199, "head_sha": OTHER_SHA}])
        self.assertTrue(ci.should_run(SHA))

    def test_compares_only_to_preceding_schedule(self):
        self.respond(
            [
                {"run_number": 201, "head_sha": SHA},
                {"run_number": 200, "head_sha": SHA},
                {"run_number": 199, "head_sha": OTHER_SHA},
                {"run_number": 198, "head_sha": SHA},
            ]
        )
        self.assertTrue(ci.should_run(SHA))

    def test_manual_and_reusable_release_calls_bypass_history(self):
        for event in ["workflow_dispatch", "push", "workflow_call"]:
            with self.subTest(event=event), patch.dict(os.environ, {"GITHUB_EVENT_NAME": event}):
                self.assertTrue(ci.should_run(SHA))
        self.urlopen.assert_not_called()

    def test_scheduled_reusable_caller_bypasses_daily_history(self):
        with patch.dict(os.environ, {"GITHUB_WORKFLOW_REF": "owner/repo/.github/workflows/caller.yml@refs/heads/main"}):
            self.assertTrue(ci.should_run(SHA))
        self.urlopen.assert_not_called()

    def test_explicit_rerun_bypasses_history(self):
        with patch.dict(os.environ, {"GITHUB_RUN_ATTEMPT": "2"}):
            self.assertTrue(ci.should_run(SHA))
        self.urlopen.assert_not_called()

    def test_api_request_filters_workflow_event_and_branch(self):
        self.respond([])
        self.assertTrue(ci.should_run(SHA))
        request = self.urlopen.call_args.args[0]
        url = urllib.parse.urlsplit(request.full_url)
        self.assertEqual(url.path, "/repos/owner/repo/actions/workflows/cross-platform.yml/runs")
        self.assertEqual(
            urllib.parse.parse_qs(url.query),
            {"event": ["schedule"], "branch": ["default/branch"], "per_page": ["100"], "page": ["1"]},
        )
        self.assertEqual(request.get_header("Authorization"), "Bearer test-token")
        self.assertEqual(self.urlopen.call_args.kwargs["timeout"], 30)

    def test_paginates_when_current_or_newer_runs_fill_first_page(self):
        pages = [
            [{"run_number": number, "head_sha": SHA} for number in range(299, 199, -1)],
            [{"run_number": 199, "head_sha": OTHER_SHA}],
        ]
        self.urlopen.side_effect = [io.BytesIO(json.dumps({"workflow_runs": page}).encode()) for page in pages]
        self.assertEqual(ci.previous_scheduled_sha(), OTHER_SHA)
        self.assertIn("page=2", self.urlopen.call_args.args[0].full_url)

    def test_api_errors_are_not_treated_as_empty_history(self):
        for status in [403, 404, 429, 500]:
            with self.subTest(status=status):
                error = urllib.error.HTTPError("https://api.github.com", status, "error", Message(), io.BytesIO())
                self.addCleanup(error.close)
                self.urlopen.side_effect = error
                with self.assertRaises(urllib.error.HTTPError):
                    ci.should_run(SHA)

    def test_prepare_writes_single_line_action_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            with (
                patch.dict(os.environ, {"GITHUB_OUTPUT": str(output), "CI_TARGET": "windows-arm64"}),
                patch.object(ci.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, SHA + "\n")),
                patch.object(ci, "should_run", return_value=False),
            ):
                ci.prepare()
            values = dict(line.split("=", 1) for line in output.read_text().splitlines())
            self.assertEqual(values["sha"], SHA)
            self.assertEqual(values["should-run"], "false")
            self.assertEqual(json.loads(values["matrix"]), ci.select_matrix("windows-arm64"))


class ArchiveTests(unittest.TestCase):
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

    def test_rejects_unsafe_archive_names(self):
        for target, sha in [("../outside", SHA), ("linux-amd64", "../outside")]:
            with self.subTest(target=target, sha=sha), self.assertRaises(ValueError):
                ci.archive_debug(self.root, self.destination, target, sha)


if __name__ == "__main__":
    unittest.main()
