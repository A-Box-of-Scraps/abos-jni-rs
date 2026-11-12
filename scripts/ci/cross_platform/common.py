"""Shared cross-platform targets and GitHub Actions output helpers."""

import os
from pathlib import Path

NATIVE_RUNNERS = {
    "linux-amd64": "ubuntu-24.04",
    "linux-arm64": "ubuntu-24.04-arm",
    "windows-amd64": "windows-2025",
    "windows-arm64": "windows-11-arm",
    "macos-amd64": "macos-15-intel",
    "macos-arm64": "macos-15",
}
TARGETS = (*NATIVE_RUNNERS, "android")


def write_output(name, value):
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"{name}={value}\n")
