import io
import json
import os
import subprocess
import tempfile
import unittest
import urllib.error
import urllib.parse
from email.message import Message
from pathlib import Path
from unittest.mock import patch

from scripts.ci.cross_platform import prepare as ci

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

    def test_each_native_target_selects_one_runner(self):
        for target, runner in ci.NATIVE_RUNNERS.items():
            with self.subTest(target=target):
                self.assertEqual(ci.select_matrix(target), {"include": [{"target": target, "runner": runner}]})

    def test_android_uses_a_separate_job_without_native_runners(self):
        self.assertEqual(ci.select_matrix("android"), {"include": []})

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
            for target in ci.TARGETS
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

    def test_native_only_success_does_not_suppress_android_coverage(self):
        jobs = [job for job in self.successful_jobs() if job["name"] != "Check and build (android)"]
        self.respond({"workflow_runs": [self.scheduled_run()]}, {"jobs": jobs})
        self.assertTrue(ci.should_run(SHA))

    def test_each_platform_must_succeed(self):
        for index, target in enumerate(ci.TARGETS):
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

    def test_prepare_selects_native_and_android_jobs(self):
        for target in ["all", *ci.TARGETS]:
            for run in [True, False]:
                with self.subTest(target=target, run=run), tempfile.TemporaryDirectory() as directory:
                    output = Path(directory) / "output"
                    with (
                        patch.dict(os.environ, {"GITHUB_OUTPUT": str(output), "CI_TARGET": target}),
                        patch.object(ci.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, SHA + "\n")),
                        patch.object(ci, "should_run", return_value=run),
                    ):
                        ci.prepare()
                    values = dict(line.split("=", 1) for line in output.read_text().splitlines())
                    self.assertEqual(values["sha"], SHA)
                    self.assertEqual(values["run-native"], str(run and target != "android").lower())
                    self.assertEqual(values["run-android"], str(run and target in {"all", "android"}).lower())
                    self.assertEqual(json.loads(values["matrix"]), ci.select_matrix(target))


if __name__ == "__main__":
    unittest.main()
