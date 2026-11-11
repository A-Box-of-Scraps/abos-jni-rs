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

    def respond(self, *pages):
        self.urlopen.side_effect = [io.BytesIO(json.dumps(page).encode()) for page in pages]

    def scheduled_run(self, number=199, sha=SHA, conclusion="success", status="completed"):
        return {
            "id": number * 10,
            "run_number": number,
            "head_sha": sha,
            "status": status,
            "conclusion": conclusion,
        }

    def successful_jobs(self):
        return [
            {"name": f"Check and build ({target})", "status": "completed", "conclusion": "success"}
            for target in ci.RUNNERS
        ]

    def test_first_schedule_runs(self):
        self.respond({"workflow_runs": []})
        self.assertTrue(ci.should_run(SHA))

    def test_unchanged_commit_with_successful_matrix_skips(self):
        self.respond({"workflow_runs": [self.scheduled_run()]}, {"jobs": self.successful_jobs()})
        self.assertFalse(ci.should_run(SHA))

    def test_unsuccessful_runs_do_not_suppress_retry(self):
        for conclusion in ["failure", "cancelled", "timed_out", "action_required", "skipped", "neutral", None]:
            with self.subTest(conclusion=conclusion):
                self.respond({"workflow_runs": [self.scheduled_run(conclusion=conclusion)]})
                self.assertTrue(ci.should_run(SHA))

    def test_incomplete_run_does_not_suppress_retry(self):
        self.respond({"workflow_runs": [self.scheduled_run(status="in_progress")]})
        self.assertTrue(ci.should_run(SHA))

    def test_changed_commit_runs(self):
        self.respond({"workflow_runs": [self.scheduled_run(sha=OTHER_SHA)]})
        self.assertTrue(ci.should_run(SHA))

    def test_current_and_newer_runs_do_not_suppress_retry(self):
        self.respond({"workflow_runs": [self.scheduled_run(number=201), self.scheduled_run(number=200)]})
        self.assertTrue(ci.should_run(SHA))

    def test_green_workflow_without_matrix_does_not_suppress_retry(self):
        self.respond(
            {"workflow_runs": [self.scheduled_run()]},
            {"jobs": [{"name": "prepare", "status": "completed", "conclusion": "success"}]},
        )
        self.assertTrue(ci.should_run(SHA))

    def test_each_platform_must_succeed(self):
        for index, target in enumerate(ci.RUNNERS):
            for conclusion in ["failure", "cancelled", "skipped", None]:
                with self.subTest(target=target, conclusion=conclusion):
                    jobs = self.successful_jobs()
                    jobs[index]["conclusion"] = conclusion
                    self.respond({"workflow_runs": [self.scheduled_run()]}, {"jobs": jobs})
                    self.assertTrue(ci.should_run(SHA))

    def test_each_platform_must_complete(self):
        jobs = self.successful_jobs()
        jobs[0]["status"] = "in_progress"
        self.respond({"workflow_runs": [self.scheduled_run()]}, {"jobs": jobs})
        self.assertTrue(ci.should_run(SHA))

    def test_missing_platform_does_not_suppress_retry(self):
        self.respond({"workflow_runs": [self.scheduled_run()]}, {"jobs": self.successful_jobs()[:-1]})
        self.assertTrue(ci.should_run(SHA))

    def test_partial_matrices_from_separate_runs_are_not_combined(self):
        jobs = self.successful_jobs()
        self.respond(
            {"workflow_runs": [self.scheduled_run(), self.scheduled_run(number=198)]},
            {"jobs": jobs[:3]},
            {"jobs": jobs[3:]},
        )
        self.assertTrue(ci.should_run(SHA))

    def test_searches_past_skipped_matrices_for_successful_build(self):
        self.respond(
            {"workflow_runs": [self.scheduled_run(), self.scheduled_run(number=198)]},
            {"jobs": [{"name": "Check and build", "status": "completed", "conclusion": "skipped"}]},
            {"jobs": self.successful_jobs()},
        )
        self.assertFalse(ci.should_run(SHA))

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

    def test_api_request_filters_workflow_event_branch_sha_and_success(self):
        self.respond({"workflow_runs": []})
        self.assertTrue(ci.should_run(SHA))
        request = self.urlopen.call_args.args[0]
        url = urllib.parse.urlsplit(request.full_url)
        self.assertEqual(url.path, "/repos/owner/repo/actions/workflows/cross-platform.yml/runs")
        self.assertEqual(
            urllib.parse.parse_qs(url.query),
            {
                "event": ["schedule"],
                "branch": ["default/branch"],
                "head_sha": [SHA],
                "status": ["success"],
                "per_page": ["100"],
                "page": ["1"],
            },
        )
        self.assertEqual(request.get_header("Authorization"), "Bearer test-token")
        self.assertEqual(self.urlopen.call_args.kwargs["timeout"], 30)

    def test_jobs_request_uses_latest_results(self):
        self.respond({"workflow_runs": [self.scheduled_run()]}, {"jobs": self.successful_jobs()})
        self.assertFalse(ci.should_run(SHA))
        request = self.urlopen.call_args.args[0]
        url = urllib.parse.urlsplit(request.full_url)
        self.assertEqual(url.path, "/repos/owner/repo/actions/runs/1990/jobs")
        self.assertEqual(urllib.parse.parse_qs(url.query), {"filter": ["latest"], "per_page": ["100"], "page": ["1"]})
        self.assertEqual(request.get_header("Authorization"), "Bearer test-token")
        self.assertEqual(self.urlopen.call_args.kwargs["timeout"], 30)

    def test_paginates_workflow_runs(self):
        self.respond(
            {"workflow_runs": [self.scheduled_run(number=number) for number in range(299, 199, -1)]},
            {"workflow_runs": [self.scheduled_run()]},
            {"jobs": self.successful_jobs()},
        )
        self.assertFalse(ci.should_run(SHA))
        self.assertIn("page=2", self.urlopen.call_args_list[1].args[0].full_url)

    def test_paginates_jobs(self):
        self.respond(
            {"workflow_runs": [self.scheduled_run()]},
            {
                "jobs": [
                    {"name": f"Other job {index}", "status": "completed", "conclusion": "success"}
                    for index in range(100)
                ]
            },
            {"jobs": self.successful_jobs()},
        )
        self.assertFalse(ci.should_run(SHA))
        self.assertIn("page=2", self.urlopen.call_args.args[0].full_url)

    def test_api_errors_are_not_treated_as_empty_history(self):
        for endpoint in ["runs", "jobs"]:
            for status in [403, 404, 429, 500]:
                with self.subTest(endpoint=endpoint, status=status):
                    error = urllib.error.HTTPError("https://api.github.com", status, "error", Message(), io.BytesIO())
                    self.addCleanup(error.close)
                    responses = []
                    if endpoint == "jobs":
                        responses.append(io.BytesIO(json.dumps({"workflow_runs": [self.scheduled_run()]}).encode()))
                    self.urlopen.side_effect = [*responses, error]
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
