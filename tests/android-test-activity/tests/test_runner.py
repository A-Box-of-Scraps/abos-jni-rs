"""Test Android runner orchestration without invoking Android tools."""
import contextlib
import io
import os
import subprocess
import unittest
from unittest.mock import patch

import run as runner


class AndroidRunnerTests(unittest.TestCase):
    def invoke(self, log, *, extra=(), environment=None):
        calls = []

        def execute(*args):
            calls.append(args)
            if "getprop" in args:
                return "arm64-v8a\n"
            if "logcat" in args:
                return log
            return ""

        env = {"ANDROID_HOME": "/sdk", "ANDROID_TEST_SERIAL": "device"} if environment is None else environment
        with patch.dict(os.environ, env, clear=True), patch("sys.argv", ["run.py", *extra]), \
                patch.object(runner, "run", side_effect=execute), patch.object(runner.uuid, "uuid4") as uuid, \
                patch.object(runner.time, "sleep"), contextlib.redirect_stdout(io.StringIO()), \
                contextlib.redirect_stderr(io.StringIO()):
            uuid.return_value.hex = "current-run"
            runner.main()
        return calls

    def test_builds_selected_abi_and_accepts_current_success(self):
        calls = self.invoke("ABOS_TEST_PASSED:current-run", extra=("--features", "sdk_os_build"))
        build = next(call for call in calls if call[:2] == ("cargo", "ndk"))
        self.assertIn("aarch64-linux-android", build)
        self.assertEqual(build[-2:], ("--features", "sdk_os_build"))
        launch = next(call for call in calls if "start" in call)
        self.assertEqual(launch[-3:], ("--es", "abos.test.run-id", "current-run"))
        self.assertFalse(any("-c" in call for call in calls))

    def test_current_failure_overrides_success(self):
        with self.assertRaisesRegex(RuntimeError, "ABOS_TEST_FAILED"):
            self.invoke("ABOS_TEST_FAILED:current-run\nABOS_TEST_PASSED:current-run")

    def test_requires_explicit_device_and_sdk(self):
        for environment in [{}, {"ANDROID_TEST_SERIAL": "device"}]:
            with self.subTest(environment=environment), self.assertRaises(SystemExit):
                self.invoke("", environment=environment)

    def test_rejects_nonpositive_timeout(self):
        with self.assertRaises(SystemExit):
            self.invoke("", extra=("--timeout", "0"))

    def test_stale_success_does_not_pass(self):
        with patch.object(runner.time, "monotonic", side_effect=[0, 0, 121]):
            with self.assertRaisesRegex(RuntimeError, "timeout"):
                self.invoke("ABOS_TEST_PASSED:previous-run")

    def test_command_failures_propagate(self):
        with patch.object(subprocess, "run", side_effect=subprocess.CalledProcessError(1, ["adb"])):
            with self.assertRaises(subprocess.CalledProcessError):
                runner.run("adb", "devices")


if __name__ == "__main__":
    unittest.main()
