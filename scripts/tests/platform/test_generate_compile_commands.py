import importlib.util
import json
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / "generate_compile_commands.py"
SPEC = importlib.util.spec_from_file_location("generate_compile_commands", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise ImportError(f"Unable to load script: {SCRIPT}")
generator = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(generator)


class CompileCommandsTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / "project with spaces"
        self.root.mkdir()
        self.manifest = self.root / "tests" / generator.PROBES[0] / "Cargo.toml"
        self.out = self.root / "build output" / "out"
        self.out.mkdir(parents=True)
        self.commands = [
            {
                "directory": str(self.manifest.parent),
                "file": str(self.manifest.parent / "probe.c"),
                "arguments": ["cc", "-I/path with spaces/include", "-I" + str(self.out), "-c", "probe.c"],
            }
        ]
        self.database = self.out / "compile_commands.json"
        self.database.write_text(json.dumps(self.commands), encoding="utf-8")
        self.messages = [
            {"reason": "compiler-artifact"},
            {"reason": "build-script-executed", "out_dir": str(self.root / "unrelated")},
            {"reason": "build-script-executed", "out_dir": str(self.out)},
            {"reason": "build-finished", "success": True},
        ]

    def cargo_result(self):
        return subprocess.CompletedProcess([], 0, stdout="\n".join(json.dumps(message) for message in self.messages))

    def test_reads_active_output_directory_including_cached_builds(self):
        with patch.object(generator.subprocess, "run", return_value=self.cargo_result()) as run:
            self.assertEqual(generator.build_commands(self.manifest), self.commands)
        run.assert_called_once_with(
            [
                "cargo",
                "build",
                "--locked",
                "--manifest-path",
                str(self.manifest),
                "--message-format=json-render-diagnostics",
            ],
            cwd=self.manifest.parent,
            stdout=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            check=True,
        )

    def test_ignores_non_json_output_from_build_tools(self):
        result = self.cargo_result()
        result.stdout = "build tool output\n\n" + result.stdout
        with patch.object(generator.subprocess, "run", return_value=result):
            self.assertEqual(generator.build_commands(self.manifest), self.commands)

    def test_preserves_windows_compiler_arguments(self):
        self.commands[0]["arguments"] = [
            r"C:\Program Files\LLVM\bin\clang-cl.exe",
            r"/IC:\Program Files\Java\jdk\include\win32",
            "/std:c11",
            "-c",
            "probe.c",
        ]
        self.database.write_text(json.dumps(self.commands), encoding="utf-8")
        with patch.object(generator.subprocess, "run", return_value=self.cargo_result()):
            self.assertEqual(generator.build_commands(self.manifest), self.commands)

    def test_missing_database_is_an_error(self):
        self.database.unlink()
        with (
            patch.object(generator.subprocess, "run", return_value=self.cargo_result()),
            self.assertRaisesRegex(RuntimeError, "No compilation commands"),
        ):
            generator.build_commands(self.manifest)

    def test_merges_both_probes(self):
        with patch.object(generator, "build_commands", return_value=self.commands) as build:
            self.assertEqual(generator.generate(self.root), 2)
        self.assertEqual(
            [call.args[0] for call in build.call_args_list],
            [self.root / "tests" / name / "Cargo.toml" for name in generator.PROBES],
        )
        result = json.loads((self.root / "compile_commands.json").read_text(encoding="utf-8"))
        self.assertEqual(result, self.commands * 2)

    def test_build_failure_preserves_previous_database(self):
        destination = self.root / "compile_commands.json"
        destination.write_text("previous database", encoding="utf-8")
        failure = subprocess.CalledProcessError(1, ["cargo", "build"])
        with (
            patch.object(generator, "build_commands", side_effect=[self.commands, failure]),
            self.assertRaises(subprocess.CalledProcessError),
        ):
            generator.generate(self.root)
        self.assertEqual(destination.read_text(encoding="utf-8"), "previous database")

    def test_generation_task_and_ignore_rule(self):
        root = SCRIPT.parent.parent
        tasks = tomllib.loads((root / "mise.toml").read_text())["tasks"]
        self.assertEqual(tasks["generate:compile-commands"]["run"], "python scripts/generate_compile_commands.py")
        self.assertIn("/compile_commands.json", (root / ".gitignore").read_text().splitlines())


if __name__ == "__main__":
    unittest.main()
