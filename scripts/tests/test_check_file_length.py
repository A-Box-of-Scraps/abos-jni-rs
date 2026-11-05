import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "check-file-length.py"
SPEC = importlib.util.spec_from_file_location("check_file_length", SCRIPT)
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


class CheckFileLengthTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = io.StringIO()

    def write_source(self, name, lines, trailing_newline=True):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        content = b"\n".join([b"// source"] * lines)
        if lines and trailing_newline:
            content += b"\n"
        path.write_bytes(content)

    def check(self):
        with contextlib.redirect_stdout(self.output):
            return checker.check_file_lengths(self.root)

    def test_accepts_empty_and_at_limit_files(self):
        for lines in [0, 599, 600]:
            self.write_source(f"source-{lines}.rs", lines)
        self.assertEqual(self.check(), 0)
        self.assertEqual(self.output.getvalue(), "")

    def test_reports_all_oversized_files_including_unterminated_lines(self):
        self.write_source("a source.rs", 601)
        self.write_source("nested/b.rs", 602, trailing_newline=False)
        self.assertEqual(self.check(), 1)
        self.assertEqual(
            self.output.getvalue(),
            "a source.rs: 601 lines (max 600)\nnested/b.rs: 602 lines (max 600)\n",
        )

    def test_skips_targets_hidden_ignored_and_non_rust_files(self):
        (self.root / ".git").mkdir()
        (self.root / ".gitignore").write_text("ignored/\n")
        for name in ["target/a.rs", "nested/target/b.rs", ".references/c.rs", "ignored/d.rs", "notes.txt"]:
            self.write_source(name, 601)
        self.assertEqual(self.check(), 0)
        self.assertEqual(self.output.getvalue(), "")

    def test_accepts_no_rust_files(self):
        self.assertEqual(self.check(), 0)

    def test_main_reports_discovery_and_read_errors(self):
        for error in [FileNotFoundError("fd missing"), subprocess.CalledProcessError(1, "fd"), PermissionError("source")]:
            with self.subTest(error=error):
                with patch.object(checker, "check_file_lengths", side_effect=error):
                    with contextlib.redirect_stderr(self.output):
                        self.assertEqual(checker.main(), 1)
        self.assertIn("Unable to check Rust file lengths:", self.output.getvalue())


if __name__ == "__main__":
    unittest.main()
