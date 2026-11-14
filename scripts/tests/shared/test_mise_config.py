import shlex
import sys
import tomllib
import unittest
from pathlib import Path
from unittest.mock import patch

CONFIG = Path(__file__).resolve().parents[3] / "mise.toml"


def task_dependencies(tasks, root):
    dependencies = set()
    pending = [root]
    while pending:
        name = pending.pop()
        if name not in dependencies:
            dependencies.add(name)
            pending.extend(tasks[name].get("depends", []))
    return dependencies


def discovered_modules(suite):
    modules = set()
    for test in suite:
        if isinstance(test, unittest.TestSuite):
            modules.update(discovered_modules(test))
        else:
            module = sys.modules[type(test).__module__]
            filename = module.__file__
            if filename is None:
                raise AssertionError(f"Test module {module.__name__} has no source file")
            modules.add(Path(filename).resolve())
    return modules


class MiseConfigTests(unittest.TestCase):
    def test_python_script_filenames_use_snake_case(self):
        for path in (CONFIG.parent / "scripts").rglob("*.py"):
            with self.subTest(path=path.relative_to(CONFIG.parent)):
                self.assertRegex(path.name, r"^[a-z_][a-z0-9_]*\.py$")

    def test_tools_and_tasks_are_alphabetically_ordered(self):
        config = tomllib.loads(CONFIG.read_text())
        for section in ["tools", "tasks"]:
            with self.subTest(section=section):
                keys = list(config[section])
                self.assertEqual(keys, sorted(keys))

    def test_check_remains_the_full_validation_gate(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        self.assertCountEqual(tasks["check"]["depends"], ["fmt:check", "lint", "check:duplicates", "test"])

    def test_platform_checks_keep_native_validation_without_shared_tasks(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        self.assertCountEqual(tasks["check:platform"]["depends"], ["lint:rust", "test:platform"])
        self.assertCountEqual(tasks["lint"]["depends"], ["lint:python", "lint:rust"])
        self.assertCountEqual(
            tasks["lint:rust"]["depends"],
            [
                "lint:abos-jbindgen:runtime",
                "lint:abos-jni-sys:systest",
                "lint:abos-jvmti-sys:systest",
                "lint:docs",
                "lint:tools:windows-bindings",
            ],
        )
        self.assertEqual(
            tasks["lint:rust"]["run"],
            [
                "cargo clippy --locked --workspace --all-targets --all-features -q -- -D warnings",
                "cargo dylint --all --workspace --fail-on-no-libraries -q -- --all-targets --all-features --locked -q",
            ],
        )
        self.assertEqual(tasks["lint:rust"]["env"], {"DYLINT_RUSTFLAGS": "-D warnings"})
        dependencies = task_dependencies(tasks, "check:platform")
        self.assertTrue(set(tasks["test:platform"]["depends"]).issubset(dependencies))
        self.assertTrue({"fmt:check", "check:duplicates", "lint:python", "test:shared"}.isdisjoint(dependencies))

    def test_global_tests_include_platform_and_shared_suites(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        self.assertCountEqual(tasks["test"]["depends"], ["test:platform", "test:shared"])
        self.assertCountEqual(
            tasks["test:platform"]["depends"],
            [
                "test:abos-jni:examples",
                "test:abos-jni-sys:systest",
                "test:abos-jvmti-sys:systest",
                "test:android-runner",
                "test:scripts:platform",
                "test:workspace",
            ],
        )
        dependencies = task_dependencies(tasks, "test")
        self.assertTrue(set(tasks["test:platform"]["depends"]).issubset(dependencies))
        self.assertIn("test:shared", dependencies)

    def test_discovered_modules_reject_modules_without_source_files(self):
        with (
            patch.object(sys.modules[__name__], "__file__", None),
            self.assertRaisesRegex(AssertionError, "has no source file"),
        ):
            discovered_modules(unittest.TestSuite([self]))

    def test_script_groups_discover_every_module_exactly_once(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        groups = ["test:scripts:platform", "test:shared"]
        self.assertCountEqual(tasks["test:scripts"]["depends"], groups)
        modules = []
        for name in groups:
            with self.subTest(task=name):
                self.assertNotIn("dir", tasks[name])
                group = name.rsplit(":", 1)[1]
                directories = []
                for command in tasks[name]["run"]:
                    args = shlex.split(command)
                    self.assertEqual(args[:6], ["python", "-B", "-m", "unittest", "discover", "--start-directory"])
                    self.assertEqual(args[7:], ["--top-level-directory", ".", "--quiet"])
                    directory = args[6]
                    directories.append(directory)
                    loader = unittest.TestLoader()
                    suite = loader.discover(str(CONFIG.parent / directory), top_level_dir=str(CONFIG.parent))
                    self.assertEqual(loader.errors, [])
                    self.assertGreater(suite.countTestCases(), 0)
                    modules.extend(discovered_modules(suite))
                self.assertCountEqual(directories, [f"scripts/ci/tests/{group}", f"scripts/tests/{group}"])
        self.assertCountEqual(modules, list((CONFIG.parent / "scripts").rglob("test_*.py")))

    def test_cross_platform_workflow_runs_shared_checks_only_on_linux_amd64(self):
        workflow = (CONFIG.parent / ".github/workflows/cross-platform.yml").read_text()
        self.assertIn(
            "run: mise run ${{ matrix.target == 'linux-amd64' && 'check' || 'check:platform' }}",
            workflow,
        )

    def test_dylint_uses_all_three_rules_from_a_pinned_git_revision(self):
        manifest = tomllib.loads((CONFIG.parent / "Cargo.toml").read_text())
        libraries = manifest["workspace"]["metadata"]["dylint"]["libraries"]
        self.assertEqual(len(libraries), 1)
        library = libraries[0]
        self.assertEqual(library["git"], "https://github.com/A-Box-of-Scraps/abos-dylint-rules")
        self.assertRegex(library["rev"], r"^[0-9a-f]{40}$")
        self.assertCountEqual(library["pattern"], ["explicit_binding_types", "file_length", "tests_outside_src"])
        self.assertNotIn("path", library)

    def test_workspace_tests_cover_all_features_default_jni_and_ignored_jvm(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        self.assertEqual(
            tasks["test:workspace"]["run"],
            [
                "cargo test --locked --workspace --all-features --no-fail-fast -q",
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
        self.assertEqual(tasks["profile:abos-jvmti"]["run"], "python scripts/profile_abos_jni.py --package abos-jvmti")

    def test_jni_dynamic_benchmark_is_opt_in_and_requires_a_jvm(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        self.assertEqual(
            tasks["bench:abos-jni:dynamic"]["run"],
            "cargo bench --locked --package abos-jni --features invocation --bench dynamic_calls",
        )
        self.assertNotIn("dynamic_calls", tasks["bench:abos-jni"]["run"])
        manifest = tomllib.loads((CONFIG.parent / "crates/abos-jni/Cargo.toml").read_text())
        benchmark = next(bench for bench in manifest["bench"] if bench["name"] == "dynamic_calls")
        self.assertFalse(benchmark["harness"])
        self.assertEqual(benchmark["required-features"], ["invocation"])

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

    def test_publish_tasks_cover_workspace_crates_in_dependency_order(self):
        tasks = tomllib.loads(CONFIG.read_text())["tasks"]
        manifests = [tomllib.loads(path.read_text()) for path in (CONFIG.parent / "crates").glob("*/Cargo.toml")]
        crates = {manifest["package"]["name"]: manifest for manifest in manifests}
        commands = tasks["publish"]["run"]
        order = []
        for command in commands:
            args = shlex.split(command)
            self.assertEqual(args[:2], ["mise", "run"])
            self.assertEqual(len(args), 3)
            self.assertTrue(args[2].startswith("publish:"))
            order.append(args[2].removeprefix("publish:"))
        self.assertCountEqual(order, crates)
        self.assertCountEqual(
            [name for name in tasks if name.startswith("publish:")],
            [f"publish:{name}" for name in crates],
        )
        for name, manifest in crates.items():
            with self.subTest(crate=name):
                task = tasks[f"publish:{name}"]
                self.assertEqual(task["run"], f"cargo publish --locked --package {name}")
                self.assertNotIn("depends", task)
                tables = [manifest, *manifest.get("target", {}).values()]
                for table in tables:
                    for kind in ("dependencies", "build-dependencies", "dev-dependencies"):
                        for alias, dependency in table.get(kind, {}).items():
                            dependency_name = (
                                dependency.get("package", alias) if isinstance(dependency, dict) else alias
                            )
                            if dependency_name in crates:
                                self.assertLess(order.index(dependency_name), order.index(name))

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
        self.assertIn("lint:rust", tasks["lint"]["depends"])
        self.assertIn(name, tasks["lint:rust"]["depends"])
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
