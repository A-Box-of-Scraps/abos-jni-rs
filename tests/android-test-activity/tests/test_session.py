"""Run the Android-independent session logic against a fake native interface."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import run as runner


class SessionTests(unittest.TestCase):
    def test_java_session(self):
        package = Path("io/github/jni_rs/jbindgen/testactivity")
        sources = runner.PROJECT / "app/src"
        home = os.environ.get("JAVA_HOME")
        suffix = ".exe" if os.name == "nt" else ""

        def tool(name):
            return str(Path(home) / "bin" / (name + suffix)) if home else name

        with tempfile.TemporaryDirectory() as output:
            subprocess.run([
                tool("javac"), "--release", "17", "-d", output,
                str(sources / "main/java" / package / "TestResult.java"),
                str(sources / "main/java" / package / "TestSession.java"),
                str(sources / "test/java" / package / "TestSessionTest.java"),
            ], check=True, capture_output=True, text=True, timeout=60)
            result = subprocess.run([
                tool("java"), "-cp", output, "io.github.jni_rs.jbindgen.testactivity.TestSessionTest",
            ], capture_output=True, text=True, timeout=60)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
