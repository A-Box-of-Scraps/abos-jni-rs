import contextlib
import importlib.util
import io
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / "profile_abos_jni.py"
SPEC = importlib.util.spec_from_file_location("profile_abos_jni", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise ImportError(f"Unable to load script: {SCRIPT}")
profiler = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(profiler)


class ProfileJniTests(unittest.TestCase):
    def test_defaults_and_project_root(self):
        with patch.object(profiler, "profile", return_value=0) as run:
            self.assertEqual(profiler.main([]), 0)
        run.assert_called_once_with(SCRIPT.parent.parent, "^jni_call_static_abs_method_safe$", 15, package="abos-jni")

    def test_jvmti_defaults_and_overrides(self):
        with patch.object(profiler, "profile", return_value=0) as run:
            self.assertEqual(profiler.main(["--package", "abos-jvmti"]), 0)
            run.assert_called_once_with(SCRIPT.parent.parent, "^jvmti/phase/wrapper$", 15, package="abos-jvmti")
            self.assertEqual(profiler.main(["--package", "abos-jvmti", "jvmti/monitor", "--seconds", "2"]), 0)
            run.assert_called_with(SCRIPT.parent.parent, "jvmti/monitor", 2, package="abos-jvmti")

    def test_jvmti_command_and_output(self):
        with (
            tempfile.TemporaryDirectory(prefix="jvmti profile ") as directory,
            patch.object(profiler.platform, "system", return_value="Linux"),
            patch.object(profiler.shutil, "which", return_value="/usr/bin/perf"),
            patch.object(profiler.subprocess, "run") as run,
        ):
            root = Path(directory)
            run.return_value.returncode = 0
            self.assertEqual(profiler.profile(root, "^jvmti/phase/wrapper$", 2, package="abos-jvmti"), 0)
            command = run.call_args.args[0]
            self.assertEqual(command[command.index("--package") + 1], "abos-jvmti")
            self.assertEqual(command[command.index("--features") + 1], "invocation")
            self.assertEqual(command[command.index("--bench") + 1], "api_calls")
            self.assertEqual(command[command.index("--output") + 1], str(root / "target/profiling/abos-jvmti.svg"))
            self.assertEqual(command[-5:], ["--", "--bench", "^jvmti/phase/wrapper$", "--profile-time", "2"])

    def test_jvmti_mise_environment_arguments(self):
        with (
            patch.dict(os.environ, {"usage_filter": "jvmti/monitor", "usage_seconds": "30"}),
            patch.object(profiler, "profile", return_value=0) as run,
        ):
            self.assertEqual(profiler.main(["--package", "abos-jvmti"]), 0)
            run.assert_called_once_with(SCRIPT.parent.parent, "jvmti/monitor", 30, package="abos-jvmti")

    def test_jvmti_errors_are_reported(self):
        with (
            patch.object(profiler, "profile", side_effect=FileNotFoundError("cargo")),
            contextlib.redirect_stderr(io.StringIO()) as output,
        ):
            self.assertEqual(profiler.main(["--package", "abos-jvmti"]), 1)
        self.assertIn("Unable to profile JVMTI benchmark: cargo", output.getvalue())

    def test_invalid_duration_does_not_run_profiler(self):
        for seconds in ("0", "-1", "abc"):
            with (
                self.subTest(seconds=seconds),
                patch.object(profiler, "profile") as run,
                contextlib.redirect_stderr(io.StringIO()),
                self.assertRaises(SystemExit) as error,
            ):
                profiler.main(["--seconds", seconds])
            self.assertEqual(error.exception.code, 2)
            run.assert_not_called()

    def test_prerequisites(self):
        for system, perf in (("FreeBSD", "/usr/bin/perf"), ("Linux", None)):
            with (
                self.subTest(system=system, perf=perf),
                patch.object(profiler.platform, "system", return_value=system),
                patch.object(profiler.shutil, "which", return_value=perf),
                patch.object(profiler.subprocess, "run") as run,
                contextlib.redirect_stderr(io.StringIO()) as output,
            ):
                self.assertEqual(profiler.main([]), 1)
                self.assertIn("Unable to profile", output.getvalue())
                run.assert_not_called()

    def test_command_environment_outputs_and_exit_status(self):
        with (
            tempfile.TemporaryDirectory(prefix="jni profile ") as directory,
            patch.object(profiler.platform, "system", return_value="Linux"),
            patch.object(profiler.shutil, "which", return_value="/usr/bin/perf"),
            patch.object(profiler.subprocess, "run") as run,
            patch.dict(os.environ, {"CARGO_PROFILE_BENCH_DEBUG": "false", "JAVA_HOME": "/test/jdk"}),
        ):
            root = Path(directory)
            run.return_value.returncode = 7
            self.assertEqual(profiler.profile(root, "^jni_get_string$", 30, package="abos-jni"), 7)
            self.assertTrue((root / "target" / "profiling").is_dir())
            command = run.call_args.args[0]
            self.assertEqual(command[:2], ["cargo", "flamegraph"])
            self.assertEqual(command[-5:], ["--", "--bench", "^jni_get_string$", "--profile-time", "30"])
            self.assertEqual(command[command.index("--output") + 1], str(root / "target/profiling/abos-jni.svg"))
            self.assertIn("-o perf.data", command[command.index("--cmd") + 1])
            self.assertIn("--user-callchains", command[command.index("--cmd") + 1])
            self.assertEqual(Path(run.call_args.kwargs["cwd"]).parent, root / "target/profiling")
            self.assertEqual(command[command.index("--manifest-path") + 1], str(root / "Cargo.toml"))
            self.assertEqual(run.call_args.kwargs["env"]["CARGO_PROFILE_BENCH_DEBUG"], "true")
            self.assertEqual(run.call_args.kwargs["env"]["JAVA_HOME"], "/test/jdk")
            self.assertEqual(os.environ["CARGO_PROFILE_BENCH_DEBUG"], "false")

    def test_windows_selects_bundled_blondie(self):
        env = {"DTRACE": "installed-dtrace.exe"}
        with patch.object(profiler.platform, "system", return_value="Windows"):
            self.assertEqual(profiler.backend_options(env), [])
        self.assertEqual(env["DTRACE"], "")

    def test_macos_resolves_xctrace(self):
        env = {}
        with (
            patch.object(profiler.platform, "system", return_value="Darwin"),
            patch.object(profiler.shutil, "which", return_value="/usr/bin/xcrun"),
            patch.object(profiler.subprocess, "run") as run,
        ):
            run.return_value.returncode = 0
            run.return_value.stdout = "/Applications/Xcode.app/usr/bin/xctrace\n"
            self.assertEqual(profiler.backend_options(env), [])
            run.assert_called_once_with(["xcrun", "--find", "xctrace"], capture_output=True, text=True, check=False)
        self.assertEqual(env["XCTRACE"], "/Applications/Xcode.app/usr/bin/xctrace")

    def test_macos_respects_xctrace_override(self):
        env = {"XCTRACE": "/custom/xctrace"}
        with (
            patch.object(profiler.platform, "system", return_value="Darwin"),
            patch.object(profiler.subprocess, "run") as run,
        ):
            self.assertEqual(profiler.backend_options(env), [])
            run.assert_not_called()
        self.assertEqual(env["XCTRACE"], "/custom/xctrace")

    def test_macos_requires_xcode(self):
        with (
            patch.object(profiler.platform, "system", return_value="Darwin"),
            patch.object(profiler.shutil, "which", return_value=None),
            self.assertRaisesRegex(RuntimeError, "Xcode"),
        ):
            profiler.backend_options({})
        with (
            patch.object(profiler.platform, "system", return_value="Darwin"),
            patch.object(profiler.shutil, "which", return_value="/usr/bin/xcrun"),
            patch.object(profiler.subprocess, "run") as run,
            self.assertRaisesRegex(RuntimeError, "Xcode"),
        ):
            run.return_value.returncode = 1
            profiler.backend_options({})

    def test_mise_environment_arguments(self):
        with (
            patch.dict(os.environ, {"usage_filter": "^jni_get_string$", "usage_seconds": "30"}),
            patch.object(profiler, "profile", return_value=0) as run,
        ):
            self.assertEqual(profiler.main([]), 0)
            run.assert_called_once_with(SCRIPT.parent.parent, "^jni_get_string$", 30, package="abos-jni")
            self.assertEqual(profiler.main(["native_call_function", "--seconds", "2"]), 0)
            run.assert_called_with(SCRIPT.parent.parent, "native_call_function", 2, package="abos-jni")

    def test_os_errors_are_reported(self):
        with (
            patch.object(profiler, "profile", side_effect=FileNotFoundError("cargo")),
            contextlib.redirect_stderr(io.StringIO()) as output,
        ):
            self.assertEqual(profiler.main([]), 1)
        self.assertIn("Unable to profile JNI benchmark: cargo", output.getvalue())


if __name__ == "__main__":
    unittest.main()
