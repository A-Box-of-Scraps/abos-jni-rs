import contextlib
import importlib.util
import io
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "seed-fuzz-corpus.py"
SPEC = importlib.util.spec_from_file_location("seed_fuzz_corpus", SCRIPT)
seeder = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(seeder)


class SeedFuzzCorpusTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="fuzz seeds ")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.fuzz = self.root / "fuzz"
        self.sources = {
            "cafebabe_parse_class": b"\xca\xfe\xba\xbe",
            "descriptors": b"(I)V",
            "jni_mutf8": b"\xc0\x80",
            "jni_string_roundtrip": b"hello\x00world",
        }
        for source, content in self.sources.items():
            directory = self.fuzz / "seeds" / source
            directory.mkdir(parents=True)
            (directory / "seed with spaces").write_bytes(content)

    def test_copies_binary_seeds_to_all_targets(self):
        seeder.seed_corpus(self.fuzz)
        expected = {
            "cafebabe_parse_class": "cafebabe_parse_class",
            "jbindgen_descriptors": "descriptors",
            "jbindgen_parse_class": "cafebabe_parse_class",
            "jni_mutf8": "jni_mutf8",
            "jni_signatures": "descriptors",
            "jni_string_roundtrip": "jni_string_roundtrip",
        }
        self.assertEqual({path.name for path in (self.fuzz / "corpus").iterdir()}, set(expected))
        for target, source in expected.items():
            with self.subTest(target=target):
                corpus = self.fuzz / "corpus" / target
                self.assertEqual(list(corpus.iterdir()), [corpus / "seed with spaces"])
                self.assertEqual((corpus / "seed with spaces").read_bytes(), self.sources[source])

    def test_repeated_runs_preserve_existing_files_and_add_new_seeds(self):
        seeder.seed_corpus(self.fuzz)
        corpus = self.fuzz / "corpus" / "jni_mutf8"
        (corpus / "seed with spaces").write_bytes(b"local edit")
        (corpus / "generated").write_bytes(b"fuzzer output")
        (self.fuzz / "seeds" / "jni_mutf8" / "new").write_bytes(b"new seed")
        seeder.seed_corpus(self.fuzz)
        self.assertEqual((corpus / "seed with spaces").read_bytes(), b"local edit")
        self.assertEqual((corpus / "generated").read_bytes(), b"fuzzer output")
        self.assertEqual((corpus / "new").read_bytes(), b"new seed")

    def test_empty_seed_directory_is_allowed_and_subdirectories_are_skipped(self):
        source = self.fuzz / "seeds" / "jni_mutf8"
        (source / "seed with spaces").unlink()
        (source / "nested").mkdir()
        seeder.seed_corpus(self.fuzz)
        self.assertEqual(list((self.fuzz / "corpus" / "jni_mutf8").iterdir()), [])

    def test_missing_seed_directory_is_an_error(self):
        shutil.rmtree(self.fuzz / "seeds" / "jni_mutf8")
        with self.assertRaises(FileNotFoundError):
            seeder.seed_corpus(self.fuzz)

    def test_script_resolves_paths_independently_of_working_directory(self):
        script = self.root / "scripts" / SCRIPT.name
        script.parent.mkdir()
        shutil.copyfile(SCRIPT, script)
        result = subprocess.run(
            [sys.executable, "-B", str(script)], cwd=script.parent,
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            (self.fuzz / "corpus" / "jni_mutf8" / "seed with spaces").read_bytes(),
            self.sources["jni_mutf8"],
        )

    def test_main_reports_filesystem_errors(self):
        output = io.StringIO()
        with patch.object(seeder, "seed_corpus", side_effect=PermissionError("corpus")):
            with contextlib.redirect_stderr(output):
                self.assertEqual(seeder.main(), 1)
        self.assertIn("Unable to seed fuzz corpus:", output.getvalue())

    def test_mise_task_uses_script_from_project_root(self):
        config = tomllib.loads((SCRIPT.parent.parent / "mise.toml").read_text())
        self.assertEqual(config["tasks"]["fuzz:seed"], {"run": "python scripts/seed-fuzz-corpus.py"})


if __name__ == "__main__":
    unittest.main()
