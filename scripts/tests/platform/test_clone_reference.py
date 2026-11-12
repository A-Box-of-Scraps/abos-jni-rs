import importlib.util
import stat
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

REPOSITORY = "javac"
SOURCE_FILE = "source"
ORIGINAL_CONTENT = "original"
REPLACEMENT_CONTENT = "replacement"

SCRIPT = Path(__file__).resolve().parents[2] / "clone_reference.py"
SPEC = importlib.util.spec_from_file_location("clone_reference", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise ImportError(f"Unable to load script: {SCRIPT}")
clone = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(clone)


class CloneReferenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name) / "references"
        self.destination = self.root / REPOSITORY
        self.destination.mkdir(parents=True)
        self.marker = self.destination / "local-edit"
        self.marker.write_text(ORIGINAL_CONTENT)
        self.missing_revision = False
        self.fail_command = None
        self.git = self.enterContext(patch.object(clone.subprocess, "run", side_effect=self.fake_git))

    def fake_git(self, args, **kwargs):
        command = args[1] if args[1] == "clone" else args[3]
        if command == self.fail_command:
            raise subprocess.CalledProcessError(1, args)
        if command == "clone":
            destination = Path(args[-1])
            destination.mkdir()
            (destination / SOURCE_FILE).write_text(REPLACEMENT_CONTENT)
        status = int(command == "cat-file" and self.missing_revision)
        return subprocess.CompletedProcess(args, status)

    def test_replaces_old_copy_and_pins_commit(self):
        clone.clone_reference(REPOSITORY, self.root)
        self.assertFalse(self.marker.exists())
        self.assertEqual((self.destination / SOURCE_FILE).read_text(), REPLACEMENT_CONTENT)
        commands = [call.args[0] for call in self.git.call_args_list]
        self.assertIn("--single-branch", commands[0])
        self.assertNotIn("--branch", commands[0])
        self.assertEqual(commands[-1][-4:], ["reset", "--quiet", "--hard", clone.REVISIONS[REPOSITORY]])
        self.assertEqual(list(self.root.iterdir()), [self.destination])

    def test_clones_each_registered_repository(self):
        for name, revision in clone.REVISIONS.items():
            with self.subTest(repository=name):
                self.git.reset_mock()
                clone.clone_reference(name, self.root)
                commands = [call.args[0] for call in self.git.call_args_list]
                self.assertEqual(commands[0][-2], f"https://github.com/jni-rs/{name}.git")
                self.assertEqual(commands[-1][-1], revision)
                self.assertRegex(revision, r"^[0-9a-f]{40}$")
                self.assertEqual((self.root / name / SOURCE_FILE).read_text(), REPLACEMENT_CONTENT)

    def test_clone_tasks_cover_all_repositories(self):
        config = tomllib.loads((SCRIPT.parent.parent / "mise.toml").read_text())
        tasks = config["tasks"]
        self.assertEqual(list(tasks), sorted(tasks))
        self.assertEqual(tasks["clone:all"]["depends"], [f"clone:{name}" for name in sorted(clone.REVISIONS)])
        for name in clone.REVISIONS:
            self.assertEqual(tasks[f"clone:{name}"]["run"], f"python scripts/clone_reference.py {name}")

    def test_fetches_revision_outside_shallow_history(self):
        self.missing_revision = True
        clone.clone_reference(REPOSITORY, self.root)
        commands = [call.args[0] for call in self.git.call_args_list]
        self.assertEqual(commands[2][3:], ["fetch", "--quiet", "--depth", "5", "origin", clone.REVISIONS[REPOSITORY]])

    def test_git_failures_preserve_old_copy_and_clean_staging(self):
        self.missing_revision = True
        for command in ["clone", "fetch", "reset"]:
            with self.subTest(command=command):
                self.fail_command = command
                with self.assertRaises(subprocess.CalledProcessError):
                    clone.clone_reference(REPOSITORY, self.root)
                self.assertEqual(self.marker.read_text(), ORIGINAL_CONTENT)
                self.assertEqual(list(self.root.iterdir()), [self.destination])

    def test_rejects_linked_reference_roots(self):
        for link_check in ["is_symlink", "is_junction"]:
            with (
                self.subTest(link_check=link_check),
                patch.object(Path, link_check, return_value=True),
                self.assertRaises(OSError),
            ):
                clone.clone_reference(REPOSITORY, self.root)
        self.git.assert_not_called()

    def test_retries_readonly_file_deletion(self):
        delete = Mock()
        with patch.object(clone.os, "chmod") as chmod:
            clone.retry_readonly(delete, self.marker, PermissionError())
        self.assertTrue(chmod.call_args.args[1] & stat.S_IWUSR)
        delete.assert_called_once_with(self.marker)

    def test_does_not_hide_other_deletion_errors(self):
        with self.assertRaises(OSError):
            clone.retry_readonly(Mock(), self.marker, OSError("disk failure"))


if __name__ == "__main__":
    unittest.main()
