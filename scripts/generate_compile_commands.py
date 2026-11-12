#!/usr/bin/env python3
"""Generate a local clangd compilation database from the native ABI probe builds."""

import json
import subprocess
import sys
from pathlib import Path

DATABASE_NAME = "compile_commands.json"
PROBES = ("abos-jni-sys-systest", "abos-jvmti-sys-systest")


def build_commands(manifest):
    result = subprocess.run(
        [
            "cargo",
            "build",
            "--locked",
            "--manifest-path",
            str(manifest),
            "--message-format=json-render-diagnostics",
        ],
        cwd=manifest.parent,
        stdout=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        check=True,
    )
    commands = []
    for line in result.stdout.splitlines():
        if not line.startswith("{"):
            continue
        message = json.loads(line)
        if message.get("reason") != "build-script-executed":
            continue
        # Cargo reports the active OUT_DIR even when it reuses a cached build.
        database = Path(message["out_dir"]) / DATABASE_NAME
        if database.is_file():
            commands.extend(json.loads(database.read_text(encoding="utf-8")))
    if not commands:
        raise RuntimeError(f"No compilation commands produced by {manifest}")
    return commands


def generate(root):
    commands = []
    for probe in PROBES:
        commands.extend(build_commands(root / "tests" / probe / "Cargo.toml"))
    destination = root / DATABASE_NAME
    destination.write_text(json.dumps(commands, indent=2) + "\n", encoding="utf-8")
    return len(commands)


def main():
    root = Path(__file__).resolve().parent.parent
    try:
        count = generate(root)
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Unable to generate compilation database: {error}", file=sys.stderr)
        return 1
    print(f"Generated {DATABASE_NAME} with {count} C compilation commands.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
