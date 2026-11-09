"""Native process launchers shared by Android setup and tests."""

import os
import platform
import shutil
import subprocess
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[1]


def executable(name):
    path = shutil.which(name)
    if path is None:
        raise RuntimeError(f"Missing {name}; run through mise and install the required host toolchain.")
    if Path(path).suffix.lower() in {".bat", ".cmd"}:
        raise RuntimeError(f"Use a native executable for {name}, not a batch shim: {path}. Run with mise exec.")
    return path


def executable_name(name, system=None):
    return name + (".exe" if (system or platform.system()) == "Windows" else "")


def java_tool(name="java"):
    home = os.environ.get("JAVA_HOME")
    if home:
        path = Path(home) / "bin" / executable_name(name)
        if not path.is_file():
            raise RuntimeError(f"JAVA_HOME does not contain {path.name}: {path}")
        return str(path)
    return executable(name)


def execute(*args, capture=True, timeout=600):
    # Never delegate argument parsing to cmd.exe or a shell. Gradle and apksigner
    # use their Java entry points instead of the platform's batch launchers.
    command = [str(arg) for arg in args]
    command[0] = executable(command[0])
    try:
        result = subprocess.run(
            command,
            cwd=PROJECT,
            check=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=capture,
            timeout=timeout,
        )
    except subprocess.CalledProcessError as error:
        raise RuntimeError(
            f"Command failed ({error.returncode}): {command}\n{error.stdout or ''}\n{error.stderr or ''}"
        ) from error
    except subprocess.TimeoutExpired as error:
        raise RuntimeError(f"Command timed out after {timeout}s: {command}") from error
    return result.stdout or ""


def gradle_command(*args):
    wrapper = PROJECT / "gradle/wrapper/gradle-wrapper.jar"
    return [
        java_tool(),
        "-Xmx64m",
        "-Xms64m",
        "-classpath",
        str(wrapper),
        "org.gradle.wrapper.GradleWrapperMain",
        "--no-daemon",
        *args,
    ]
