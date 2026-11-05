#!/usr/bin/env python3
"""Reject Rust source files longer than 600 lines."""

import os
from pathlib import Path
import subprocess
import sys


MAX_LINES = 600


def check_file_lengths(root):
    # Keep fd's ignore rules so generated and upstream sources are not checked.
    result = subprocess.run(
        ["fd", "--type", "f", "-0", "-e", "rs", "-E", "target"],
        cwd=root,
        capture_output=True,
        check=True,
    )
    failed = False
    for name in sorted(result.stdout.split(b"\0")):
        if not name:
            continue
        path = Path(os.fsdecode(name))
        with (root / path).open("rb") as source:
            # Count an unterminated final line too.
            lines = sum(1 for _ in source)
        if lines > MAX_LINES:
            print(f"{path}: {lines} lines (max {MAX_LINES})")
            failed = True
    return int(failed)


def main():
    root = Path(__file__).resolve().parent.parent
    try:
        return check_file_lengths(root)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"Unable to check Rust file lengths: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
