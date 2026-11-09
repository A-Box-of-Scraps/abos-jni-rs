import shlex
import tomllib
import unittest
from pathlib import Path

CONFIG = Path(__file__).resolve().parents[2] / "mise.toml"


class MiseConfigTests(unittest.TestCase):
    def test_tools_and_tasks_are_alphabetically_ordered(self):
        config = tomllib.loads(CONFIG.read_text())
        for section in ["tools", "tasks"]:
            with self.subTest(section=section):
                keys = list(config[section])
                self.assertEqual(keys, sorted(keys))

    def test_check_is_the_single_non_mutating_validation_gate(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        self.assertNotIn("check:ci", tasks)
        self.assertCountEqual(tasks["check"]["depends"], ["fmt:check", "lint", "check:duplicates", "test"])

    def test_global_tests_include_examples_and_standalone_suites(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        self.assertCountEqual(
            tasks["test"]["depends"],
            [
                "test:abos-jni:examples",
                "test:abos-jni-sys:systest",
                "test:abos-jvmti-sys:systest",
                "test:android-runner",
                "test:lints",
                "test:scripts",
                "test:workspace",
            ],
        )

    def test_workspace_tests_cover_all_features_default_jni_and_ignored_jvm(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        self.assertEqual(
            tasks["test:workspace"]["run"],
            [
                "cargo test --locked --workspace --all-features -q",
                tasks["test:abos-jni"]["run"],
                tasks["test:abos-jni-sys:jvm"]["run"],
            ],
        )

    def test_python_quality_tasks_are_wired_into_global_tasks(self):
        config = tomllib.loads(CONFIG.read_text())
        self.assertIn("ruff", config["tools"])
        tasks = config["tasks"]
        for parent, child in [
            ("fmt", "fmt:python"),
            ("fmt:check", "fmt:python:check"),
            ("lint", "lint:python"),
            ("lint:fix", "lint:python:fix"),
        ]:
            with self.subTest(task=parent):
                self.assertIn(child, tasks[parent]["depends"])
        self.assertEqual(tasks["fmt:python"]["run"], ["ruff check --select I --fix .", "ruff format ."])
        self.assertEqual(tasks["fmt:python:check"]["run"], ["ruff check --select I .", "ruff format --check ."])
        self.assertEqual(tasks["lint:python"]["run"], "ruff check .")
        self.assertEqual(tasks["lint:python:fix"]["run"], "ruff check --fix .")

    def test_jvmti_benchmarks_match_jni_setup(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        for suffix in ("", ":rust"):
            with self.subTest(suffix=suffix):
                self.assertEqual(
                    tasks[f"bench:abos-jvmti{suffix}"]["run"],
                    tasks[f"bench:abos-jni{suffix}"]["run"].replace("abos-jni", "abos-jvmti"),
                )
        self.assertEqual(tasks["profile:abos-jvmti"]["tools"], tasks["profile:abos-jni"]["tools"])
        self.assertEqual(tasks["profile:abos-jvmti"]["run"], "python scripts/profile-abos-jni.py --package abos-jvmti")

    def test_jvmti_lifecycle_benchmark_is_opt_in_and_requires_a_jvm(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        command = tasks["bench:abos-jvmti:lifecycle"]["run"]
        self.assertEqual(
            command,
            "cargo bench --locked --package abos-jvmti --features invocation --bench environment_lifecycle",
        )
        self.assertNotIn("environment_lifecycle", tasks["bench:abos-jvmti"]["run"])
        manifest = tomllib.loads((CONFIG.parent / "crates/abos-jvmti/Cargo.toml").read_text())
        benchmark = next(bench for bench in manifest["bench"] if bench["name"] == "environment_lifecycle")
        self.assertFalse(benchmark["harness"])
        self.assertEqual(benchmark["required-features"], ["invocation"])

    def test_task_dependencies_exist(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        for name, task in tasks.items():
            for dependency in task.get("depends", []):
                with self.subTest(task=name, dependency=dependency):
                    self.assertIn(dependency, tasks)

    def test_cargo_commands_use_locked_dependencies(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        commands = {"bench", "clippy", "doc", "dylint", "run", "test"}
        for name, task in tasks.items():
            runs = task.get("run", [])
            run_commands = [runs] if isinstance(runs, str) else runs
            for command in run_commands:
                args = shlex.split(command)
                if len(args) < 2 or args[0] != "cargo" or args[1] not in commands:
                    continue
                if args[1] == "dylint":
                    cargo_args = args[args.index("--") + 1 :]
                else:
                    cargo_args = args[2 : args.index("--")] if "--" in args else args[2:]
                with self.subTest(task=name, command=command):
                    self.assertIn("--locked", cargo_args)

    def test_lint_includes_the_standalone_windows_bindings_generator(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        name = "lint:tools:windows-bindings"
        self.assertIn(name, tasks["lint"]["depends"])
        args = shlex.split(tasks[name]["run"])
        self.assertEqual(args[:2], ["cargo", "clippy"])
        self.assertEqual(args[args.index("--manifest-path") + 1], "tools/abos-jni-gen-windows-bindings/Cargo.toml")
        self.assertIn("--all-targets", args)
        self.assertIn("--all-features", args)
        self.assertEqual(args[-3:], ["--", "-D", "warnings"])

    def test_individual_fuzz_smoke_tasks_only_build_the_selected_target(self):
        config = tomllib.loads(CONFIG.read_text())
        tasks = config["tasks"]
        manifest = CONFIG.parent / "fuzz" / "Cargo.toml"
        targets = {target["name"] for target in tomllib.loads(manifest.read_text())["bin"]}
        smoke_tasks = tasks["fuzz:smoke:all"]["depends"]
        selected_targets = set()
        for name in smoke_tasks:
            with self.subTest(task=name):
                self.assertCountEqual(tasks[name]["depends"], ["fuzz:seed"])
                self.assertEqual(tasks[name]["dir"], "fuzz")
                args = shlex.split(tasks[name]["run"])
                self.assertEqual(args[:3], ["cargo", "fuzz", "run"])
                selected_targets.add(args[3])
        self.assertEqual(selected_targets, targets)


if __name__ == "__main__":
    unittest.main()
