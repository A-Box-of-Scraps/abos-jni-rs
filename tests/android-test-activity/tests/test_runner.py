"""Test Android runner orchestration without invoking Android tools."""

import contextlib
import io
import json
import os
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

import run as runner


def event(**values):
    return runner.EVENT_PREFIX + json.dumps({"run_id": "current-run", **values})


def report(features=(), **counts):
    names = runner.SMOKE_TESTS | {runner.SDK_TESTS[feature] for feature in features}
    lines = [event(type="result", name=name, status="passed", message="failed errors: 0") for name in sorted(names)]
    lines.append(
        event(
            type="summary",
            status="passed",
            **{
                "selected": len(names),
                "executed": len(names),
                "passed": len(names),
                "failed": 0,
                "skipped": 0,
                **counts,
            },
        )
    )
    return "\n".join(lines)


class AndroidRunnerTests(unittest.TestCase):
    def invoke(self, log, *, extra=(), environment=None, api="35", abi="arm64-v8a"):
        calls = []

        def execute(*args):
            calls.append(args)
            if args[-1] == "ro.build.version.sdk":
                return api
            if args[-1] == "ro.product.cpu.abi":
                return abi
            if "logcat" in args:
                return log
            return ""

        def package(sdk, target, features):
            calls.append(("package", str(sdk), target, *features))
            return Path("verified.apk")

        env = {"ANDROID_HOME": "/sdk", "ANDROID_TEST_SERIAL": "device"} if environment is None else environment
        with (
            patch.dict(os.environ, env, clear=True),
            patch.object(Path, "home", return_value=runner.PROJECT),
            patch("sys.argv", ["run.py", *extra]),
            patch.object(runner, "run", side_effect=execute),
            patch.object(runner.uuid, "uuid4") as uuid,
            patch.object(runner.environment, "adb", return_value=Path("adb")),
            patch.object(runner.environment, "check_environment"),
            patch.object(runner.build_apk, "package", side_effect=package),
            patch.object(runner.time, "sleep"),
            contextlib.redirect_stdout(io.StringIO()),
            contextlib.redirect_stderr(io.StringIO()),
        ):
            uuid.return_value.hex = "current-run"
            runner.main()
        return calls

    def test_builds_selected_abi_and_accepts_current_success(self):
        calls = self.invoke(report(["sdk_os_build"]), extra=("--features", "sdk_os_build"))
        build = next(call for call in calls if call[0] == "package")
        self.assertEqual(build[2:], ("aarch64-linux-android", "sdk_os_build"))
        launch = next(call for call in calls if "start" in call)
        self.assertEqual(launch[-3:], ("--es", "abos.test.run-id", "current-run"))
        self.assertFalse(any("-c" in call for call in calls))

    def test_default_suite_enables_every_sdk_feature(self):
        calls = self.invoke(report(runner.SDK_TESTS))
        build = next(call for call in calls if call[0] == "package")
        self.assertEqual(build[3:], tuple(sorted(runner.SDK_TESTS)))
        features = tomllib.loads((runner.PROJECT / "Cargo.toml").read_text())["features"]
        self.assertEqual(set(runner.SDK_TESTS), set(features))

    def test_smoke_selection_excludes_sdk_features(self):
        calls = self.invoke(report(), extra=("--suite", "smoke"))
        build = next(call for call in calls if call[0] == "package")
        self.assertEqual(len(build), 3)

    def test_compile_only_does_not_require_or_touch_a_device(self):
        calls = self.invoke("", extra=("--compile-only",), environment={"ANDROID_HOME": "/sdk"})
        self.assertEqual(len(calls), 1)
        self.assertIn("check", calls[0])
        self.assertEqual(calls[0][-1], ",".join(sorted(runner.SDK_TESTS)))

    def test_device_api_matches_selected_suite(self):
        with self.assertRaises(SystemExit):
            self.invoke("", api="34")
        self.invoke(report(), extra=("--suite", "smoke"), api="31")
        for api in ["30", "unknown"]:
            with self.subTest(api=api), self.assertRaises(SystemExit):
                self.invoke("", extra=("--suite", "smoke"), api=api)
        with self.assertRaises(SystemExit):
            self.invoke("", abi="unsupported")

    def test_current_failure_overrides_success(self):
        with self.assertRaisesRegex(RuntimeError, "Android test failed"):
            self.invoke(
                report() + "\n" + event(type="result", name="setup", status="failed"), extra=("--suite", "smoke")
            )

    def test_requires_explicit_device(self):
        with self.assertRaises(SystemExit):
            self.invoke("", environment={})

    def test_package_mode_does_not_touch_devices(self):
        calls = self.invoke("", extra=("--package-only", "--target", "x86_64-linux-android"), environment={})
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][0], "package")
        self.assertEqual(calls[0][2], "x86_64-linux-android")
        self.assertEqual(calls[0][3:], tuple(sorted(runner.SDK_TESTS)))

    def test_rejects_invalid_options(self):
        for extra in [
            ("--compile-only", "--package-only"),
            ("--timeout", "0"),
            ("--features", ""),
            ("--features", "typo"),
            ("--features", "sdk_os_build,sdk_os_build"),
            ("--suite", "smoke", "--features", "sdk_os_build"),
        ]:
            with self.subTest(extra=extra), self.assertRaises(SystemExit):
                self.invoke("", extra=extra)

    def test_stale_success_does_not_pass(self):
        with (
            patch.object(runner.time, "monotonic", side_effect=[0, 0, 121]),
            self.assertRaisesRegex(RuntimeError, "timeout"),
        ):
            self.invoke(report().replace("current-run", "previous-run"))

    def test_rejects_incomplete_or_incorrect_reports(self):
        for log in [
            report(executed=0),
            report(passed=True),
            report(skipped=1),
            report().splitlines()[-1],
            report() + "\n" + report(),
            event(type="mystery", status="passed"),
        ]:
            with self.subTest(log=log), self.assertRaises(RuntimeError):
                runner.read_report(log, "current-run", runner.SMOKE_TESTS)

    def test_truncated_logs_cannot_pass_or_poison_a_later_run(self):
        truncated = runner.EVENT_PREFIX + '{"run_id":'
        self.assertIsNone(runner.read_report(truncated, "current-run", runner.SMOKE_TESTS))
        self.assertIsNotNone(runner.read_report(truncated + "\n" + report(), "current-run", runner.SMOKE_TESTS))

    def test_exact_run_id_match_and_explicit_status(self):
        self.assertIsNone(
            runner.read_report(report().replace("current-run", "current-run-extra"), "current-run", runner.SMOKE_TESTS)
        )
        self.assertIsNotNone(runner.read_report(report(), "current-run", runner.SMOKE_TESTS))

    def test_command_failures_propagate(self):
        with (
            patch.object(runner.tools, "execute", side_effect=RuntimeError("command failed")),
            self.assertRaises(RuntimeError),
        ):
            runner.run("adb", "devices")
