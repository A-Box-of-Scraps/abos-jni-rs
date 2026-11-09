"""SDK setup tests use disposable layouts and never download or accept licenses."""

import contextlib
import io
import os
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

import android_env as environment
import android_tools as tools


class EnvironmentTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="android tests ")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.sdk = self.root / "SDK with spaces"

    def installed(self):
        for package, version in environment.PACKAGES.items():
            directory = self.sdk / package
            directory.mkdir(parents=True, exist_ok=True)
            (directory / "source.properties").write_text(f"Pkg.Revision={version}\n", encoding="utf-8")

    def test_host_support_is_explicit(self):
        cases = [
            ("Linux", "x86_64", "linux-x86_64"),
            ("Windows", "AMD64", "windows-x86_64"),
            ("Darwin", "arm64", "darwin-x86_64"),
            ("Darwin", "x86_64", "darwin-x86_64"),
        ]
        for system, machine, expected in cases:
            with self.subTest(system=system, machine=machine):
                self.assertEqual(environment.supported_host(system, machine), expected)
        for system, machine in [("Linux", "aarch64"), ("Windows", "ARM64"), ("FreeBSD", "x86_64")]:
            with self.subTest(system=system, machine=machine), self.assertRaisesRegex(RuntimeError, "Unsupported"):
                environment.supported_host(system, machine)

    def test_sdk_selection_and_conflicting_environment(self):
        with patch.dict(os.environ, {"ANDROID_HOME": str(self.sdk)}, clear=True):
            self.assertEqual(environment.sdk_root(), self.sdk)
        with patch.dict(os.environ, {"ANDROID_SDK_ROOT": str(self.sdk)}, clear=True):
            self.assertEqual(environment.sdk_root(), self.sdk)
        with patch.dict(os.environ, {"ANDROID_HOME": str(self.sdk), "ANDROID_SDK_ROOT": str(self.root)}, clear=True):
            with self.assertRaisesRegex(RuntimeError, "disagree"):
                environment.sdk_root()
            self.assertEqual(environment.sdk_root(self.sdk), self.sdk)

    def test_default_sdk_roots_for_each_os(self):
        for system, variable, base in [
            ("Windows", "LOCALAPPDATA", self.root / "Local App Data"),
            ("Linux", "XDG_DATA_HOME", self.root / "share"),
        ]:
            with (
                self.subTest(system=system),
                patch.object(environment.platform, "system", return_value=system),
                patch.dict(os.environ, {variable: str(base)}, clear=True),
            ):
                self.assertEqual(environment.sdk_root(), base / "abos/android-sdk")
        with (
            patch.object(environment.platform, "system", return_value="Darwin"),
            patch.object(Path, "home", return_value=self.root),
            patch.dict(os.environ, {}, clear=True),
        ):
            self.assertEqual(environment.sdk_root(), self.root / "Library/Application Support/abos/android-sdk")

    def test_configure_selects_pinned_ndk_and_sdk_for_children(self):
        with patch.dict(os.environ, {}, clear=True):
            environment.configure(self.sdk)
            self.assertEqual(os.environ["ANDROID_HOME"], str(self.sdk))
            self.assertEqual(os.environ["ANDROID_SDK_ROOT"], str(self.sdk))
            self.assertEqual(os.environ["ANDROID_NDK_HOME"], str(self.sdk / "ndk" / environment.NDK))

    def test_missing_revisions_and_shared_sdk_protection(self):
        self.assertEqual(len(environment.missing_packages(self.sdk)), len(environment.PACKAGES))
        self.installed()
        self.assertEqual(environment.missing_packages(self.sdk), [])
        platform = self.sdk / f"platforms/android-{environment.API}/source.properties"
        platform.write_text("Pkg.Revision = 2\n", encoding="utf-8")
        self.assertEqual(environment.missing_packages(self.sdk), [])
        platform.write_text("Pkg.Revision = 999\n", encoding="utf-8")
        with self.assertRaisesRegex(RuntimeError, "separate --sdk-root"):
            environment.missing_packages(self.sdk)

    def test_license_gate_cannot_be_implicitly_accepted(self):
        with (
            patch.object(environment, "check_host"),
            patch.object(tools, "execute") as execute,
            patch.object(environment.sys.stdin, "isatty", return_value=False),
            contextlib.redirect_stdout(io.StringIO()),
        ):
            with self.assertRaisesRegex(RuntimeError, "accept-licenses"):
                environment.setup(self.sdk, [environment.DEFAULT_TARGET])
            execute.assert_not_called()
            self.assertFalse(self.sdk.exists())
        with (
            patch.object(environment.sys.stdin, "isatty", return_value=True),
            patch("builtins.input", return_value="no"),
            contextlib.redirect_stdout(io.StringIO()),
            self.assertRaisesRegex(RuntimeError, "cancelled"),
        ):
            environment.confirm_licenses(False)

    def test_explicit_setup_installs_exact_revisions_and_disables_metrics(self):
        with (
            patch.object(environment, "check_host"),
            patch.object(environment, "check_environment"),
            patch.object(tools, "executable"),
            patch.object(tools, "execute") as execute,
            contextlib.redirect_stdout(io.StringIO()),
        ):
            environment.setup(self.sdk, [environment.DEFAULT_TARGET], accepted=True)
        commands = [call.args for call in execute.call_args_list]
        packages = [f"{name}@{version}" for name, version in environment.PACKAGES.items()]
        self.assertEqual([command[-1] for command in commands[:-1]], packages)
        for command in commands[:-1]:
            self.assertIn("--no-metrics", command)
            self.assertIn(f"--sdk={self.sdk}", command)
            self.assertNotIn("--force", command)
        self.assertEqual(commands[-1], ("rustup", "target", "add", environment.DEFAULT_TARGET))

    def test_setup_reuses_installed_packages_without_license_prompt(self):
        self.installed()
        with (
            patch.object(environment, "check_host"),
            patch.object(environment, "check_environment"),
            patch.object(environment, "confirm_licenses") as confirm,
            patch.object(tools, "execute") as execute,
            contextlib.redirect_stdout(io.StringIO()),
        ):
            environment.setup(self.sdk, [environment.DEFAULT_TARGET])
        confirm.assert_not_called()
        execute.assert_called_once_with("rustup", "target", "add", environment.DEFAULT_TARGET, capture=False)

    def test_check_detects_incomplete_installation_and_missing_target(self):
        self.installed()
        with patch.object(environment, "check_host"), self.assertRaisesRegex(RuntimeError, "android.jar"):
            environment.check_environment(self.sdk)
        with (
            patch.object(environment, "check_host"),
            patch.object(environment, "require_file"),
            patch.object(tools, "PROJECT", self.root),
            patch.object(tools, "execute", return_value=""),
            self.assertRaisesRegex(RuntimeError, "Missing Rust target"),
        ):
            environment.check_environment(self.sdk)
        (self.root / "local.properties").write_text("sdk.dir=C\\:\\other\n", encoding="utf-8")
        with (
            patch.object(environment, "check_host"),
            patch.object(environment, "require_file"),
            patch.object(tools, "PROJECT", self.root),
            patch.object(tools, "execute", return_value=environment.DEFAULT_TARGET),
            self.assertRaisesRegex(RuntimeError, "sdk.dir"),
        ):
            environment.check_environment(self.sdk)

    def test_pins_match_gradle_and_mise_tasks_are_sorted(self):
        catalog = tomllib.loads((tools.PROJECT / "gradle/libs.versions.toml").read_text(encoding="utf-8"))
        versions = catalog["versions"]
        self.assertEqual(int(versions["compileSdk"]), environment.API)
        self.assertEqual(int(versions["targetSdk"]), environment.API)
        self.assertEqual(int(versions["minSdk"]), environment.MIN_API)
        self.assertEqual(versions["buildTools"], environment.BUILD_TOOLS)
        self.assertEqual(versions["ndk"], environment.NDK)
        config = tomllib.loads((tools.PROJECT.parents[1] / "mise.toml").read_text(encoding="utf-8"))
        self.assertEqual(list(config["tasks"]), sorted(config["tasks"]))
        self.assertTrue(config["tasks"]["setup:android"]["raw"])
        self.assertEqual(config["tools"]["android-cli"], "1.1.16717390")
        self.assertEqual(config["tools"]["cargo:cargo-ndk"], "4.1.2")
