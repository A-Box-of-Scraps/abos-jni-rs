"""Native argument lists work without Windows batch-file or POSIX shell quoting."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import android_tools as tools


class ToolTests(unittest.TestCase):
    def test_native_command_preserves_spaces_and_shell_metacharacters(self):
        arguments = ["java", "-jar", "C:/SDK with spaces/lib/apksigner.jar", "verify", "C:/a&b/%name%!^.apk"]
        result = subprocess.CompletedProcess(arguments, 0, stdout="verified", stderr="")
        with patch.object(tools.shutil, "which", return_value="C:/Java Home/bin/java.exe"), \
                patch.object(subprocess, "run", return_value=result) as run:
            self.assertEqual(tools.execute(*arguments), "verified")
        self.assertEqual(run.call_args.args[0], ["C:/Java Home/bin/java.exe", *arguments[1:]])
        self.assertFalse(run.call_args.kwargs.get("shell", False))

    def test_batch_shims_are_rejected(self):
        for path in ["C:/tools/java.cmd", "C:/tools/gradlew.BAT"]:
            with self.subTest(path=path), patch.object(tools.shutil, "which", return_value=path):
                with self.assertRaisesRegex(RuntimeError, "batch shim"):
                    tools.executable("tool")

    def test_missing_tools_and_failed_commands_are_actionable(self):
        with patch.object(tools.shutil, "which", return_value=None):
            with self.assertRaisesRegex(RuntimeError, "Missing rustup"):
                tools.executable("rustup")
        error = subprocess.CalledProcessError(1, ["cargo"], output="output", stderr="link failed")
        with patch.object(tools, "executable", return_value="cargo"), \
                patch.object(subprocess, "run", side_effect=error):
            with self.assertRaisesRegex(RuntimeError, "link failed"):
                tools.execute("cargo", "build")
        with patch.object(tools, "executable", return_value="adb"), \
                patch.object(subprocess, "run", side_effect=subprocess.TimeoutExpired(["adb"], 10)):
            with self.assertRaisesRegex(RuntimeError, "timed out"):
                tools.execute("adb", timeout=10)

    def test_gradle_uses_java_entry_point_on_every_host(self):
        for system in ["Windows", "Darwin", "Linux"]:
            with self.subTest(system=system), tempfile.TemporaryDirectory(prefix="java home ") as temporary:
                home = Path(temporary)
                binary = home / "bin" / tools.executable_name("java", system)
                binary.parent.mkdir()
                binary.touch()
                with patch.dict(os.environ, {"JAVA_HOME": str(home)}), \
                        patch.object(tools.platform, "system", return_value=system):
                    command = tools.gradle_command("-PabosJniLibs=C:/space & chars", ":app:assembleDebug")
                self.assertEqual(command[0], str(binary))
                self.assertIn("org.gradle.wrapper.GradleWrapperMain", command)
                self.assertEqual(command[-2:], ["-PabosJniLibs=C:/space & chars", ":app:assembleDebug"])
                self.assertFalse(any(arg.endswith((".bat", ".cmd")) for arg in command))

    def test_invalid_java_home_does_not_silently_use_another_jdk(self):
        with tempfile.TemporaryDirectory() as home, patch.dict(os.environ, {"JAVA_HOME": home}):
            with self.assertRaisesRegex(RuntimeError, "JAVA_HOME"):
                tools.java_tool()
