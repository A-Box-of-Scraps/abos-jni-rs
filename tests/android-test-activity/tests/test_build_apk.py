"""Package validation and build orchestration without SDK tools or a device."""
import contextlib
import io
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import android_env as environment
import android_tools as tools
import build_apk


def elf(abi):
    elf_class, machine = build_apk.ELF_TARGETS[abi]
    header = bytearray(64)
    header[:6] = b"\x7fELF" + bytes([elf_class, 1])
    struct.pack_into("<HH", header, 16, 3, machine)
    return header


def write_apk(path, abi, library=None, extra=None):
    path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("AndroidManifest.xml", b"manifest")
        archive.writestr("classes.dex", b"dex\n")
        archive.writestr(f"lib/{abi}/libtestactivity.so", elf(abi) if library is None else library)
        if extra:
            archive.writestr(extra, b"stale")


class PackageTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="apk tests ")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.apk = self.root / "app-debug.apk"

    def test_each_abi_requires_the_correct_elf(self):
        for abi in build_apk.ELF_TARGETS:
            with self.subTest(abi=abi):
                write_apk(self.apk, abi)
                build_apk.verify_apk_contents(self.apk, abi)
                write_apk(self.apk, abi, library=b"not an ELF")
                with self.assertRaisesRegex(RuntimeError, "ELF"):
                    build_apk.verify_apk_contents(self.apk, abi)
        write_apk(self.apk, "arm64-v8a", library=elf("x86_64"))
        with self.assertRaisesRegex(RuntimeError, "ELF"):
            build_apk.verify_apk_contents(self.apk, "arm64-v8a")

    def test_rejects_stale_abis_missing_native_code_and_incomplete_apks(self):
        write_apk(self.apk, "arm64-v8a", extra="lib/x86_64/libtestactivity.so")
        with self.assertRaisesRegex(RuntimeError, "exactly"):
            build_apk.verify_apk_contents(self.apk, "arm64-v8a")
        with zipfile.ZipFile(self.apk, "w") as archive:
            archive.writestr("classes.dex", "dex")
        with self.assertRaisesRegex(RuntimeError, "exactly"):
            build_apk.verify_apk_contents(self.apk, "arm64-v8a")
        with zipfile.ZipFile(self.apk, "w") as archive:
            archive.writestr("lib/arm64-v8a/libtestactivity.so", elf("arm64-v8a"))
        with self.assertRaisesRegex(RuntimeError, "AndroidManifest"):
            build_apk.verify_apk_contents(self.apk, "arm64-v8a")

    def test_native_commands_pin_api_level_and_use_locked_dependencies(self):
        for output, verb in [(None, "check"), (self.root, "build")]:
            command = build_apk.native_command(environment.DEFAULT_TARGET, ["sdk_os_build"], output)
            self.assertIn(verb, command)
            self.assertEqual(command[command.index("-P") + 1], "31")
            self.assertNotIn("-p", command)
            self.assertIn("--locked", command)
            self.assertIn("--no-default-features", command)
            self.assertEqual(command[-2:], ["--features", "sdk_os_build"])

    def test_alignment_and_signature_checks_use_native_executables(self):
        write_apk(self.apk, "arm64-v8a")
        with patch.object(environment, "build_tool", return_value=Path("zipalign.exe")), \
                patch.object(environment, "require_file", side_effect=lambda path: path), \
                patch.object(tools, "java_tool", return_value="java.exe"), patch.object(tools, "execute") as execute:
            build_apk.validate_apk(self.apk, "arm64-v8a", self.root)
        first, second = [call.args for call in execute.call_args_list]
        self.assertEqual(first[1:6], ("-c", "-P", "16", "-v", "4"))
        self.assertEqual(second[:2], ("java.exe", "-jar"))
        self.assertTrue(str(second[2]).endswith("lib/apksigner.jar"))
        self.assertEqual(second[3:5], ("verify", "--verbose"))

    def test_package_uses_isolated_native_input_and_publishes_only_after_validation(self):
        calls = []

        def execute(*command, **kwargs):
            calls.append(command)
            if command[:2] == ("cargo", "ndk"):
                output = Path(command[command.index("-o") + 1])
                library = output / "arm64-v8a/libtestactivity.so"
                library.parent.mkdir()
                library.write_bytes(elf("arm64-v8a"))
            else:
                write_apk(self.root / "app/build/outputs/apk/debug/app-debug.apk", "arm64-v8a")
            return ""

        with patch.object(tools, "PROJECT", self.root), patch.object(tools, "java_tool", return_value="java"), \
                patch.object(environment, "check_environment"), patch.object(tools, "execute", side_effect=execute), \
                patch.object(build_apk, "validate_apk") as validate, contextlib.redirect_stdout(io.StringIO()):
            artifact = build_apk.package(self.root / "sdk", environment.DEFAULT_TARGET, [])
        self.assertTrue(artifact.is_file())
        self.assertEqual(len(calls), 2)
        validate.assert_called_once()
        native_input = Path(calls[0][calls[0].index("-o") + 1])
        self.assertIn(f"-PabosJniLibs={native_input}", calls[1])
        self.assertFalse(native_input.exists())
        self.assertFalse(any("adb" in str(arg) for call in calls for arg in call))

    def test_failed_native_build_never_invokes_gradle(self):
        with patch.object(tools, "PROJECT", self.root), patch.object(environment, "check_environment"), \
                patch.object(tools, "execute", side_effect=RuntimeError("link failed")) as execute:
            with self.assertRaisesRegex(RuntimeError, "link failed"):
                build_apk.package(self.root, environment.DEFAULT_TARGET, [])
        self.assertEqual(execute.call_count, 1)
        self.assertEqual(list((self.root / "build").iterdir()), [])
