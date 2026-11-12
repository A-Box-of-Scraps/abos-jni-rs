"""Check that workflow commands still resolve after CI modules move."""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]


class EntrypointTests(unittest.TestCase):
    def test_ci_modules_expose_help_without_running_workflow_actions(self):
        for module in [
            "scripts.ci.downloads",
            "scripts.ci.publish",
            "scripts.ci.cross_platform.prepare",
            "scripts.ci.cross_platform.archive",
        ]:
            with self.subTest(module=module):
                result = subprocess.run(
                    [sys.executable, "-B", "-m", module, "--help"],
                    cwd=ROOT,
                    capture_output=True,
                    text=True,
                    timeout=30,
                    check=False,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn("usage:", result.stdout)

    def test_prepare_entrypoint_writes_job_outputs_without_querying_history_for_manual_runs(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "github-output"
            result = subprocess.run(
                [sys.executable, "-B", "-m", "scripts.ci.cross_platform.prepare"],
                cwd=ROOT,
                env={
                    **os.environ,
                    "CI_TARGET": "windows-amd64",
                    "GITHUB_EVENT_NAME": "workflow_dispatch",
                    "GITHUB_OUTPUT": str(output),
                },
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            self.assertRegex(values["sha"], r"^[0-9a-f]{40}$")
            self.assertEqual(values["run-native"], "true")
            self.assertEqual(values["run-android"], "false")
            self.assertEqual(
                json.loads(values["matrix"]),
                {"include": [{"target": "windows-amd64", "runner": "windows-2025"}]},
            )

    def test_workflows_use_the_package_entrypoints(self):
        commands = {
            "cross-platform": [
                "python -m scripts.ci.cross_platform.prepare",
                "python -m scripts.ci.cross_platform.archive",
            ],
            "downloads": ["python -m scripts.ci.downloads"],
            "publish": ["python -m scripts.ci.publish publish", "python -m scripts.ci.publish commit"],
        }
        for workflow, entrypoints in commands.items():
            with self.subTest(workflow=workflow):
                source = (ROOT / ".github/workflows" / f"{workflow}.yml").read_text()
                for command in entrypoints:
                    self.assertIn(command, source)
                self.assertNotIn("python scripts/ci-", source)


if __name__ == "__main__":
    unittest.main()
