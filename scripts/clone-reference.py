#!/usr/bin/env python3
"""Refresh disposable, pinned upstream source snapshots."""

import argparse
import os
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

REVISIONS = {
    "javac": "8dcbc94481d5bdf003895957dd010486e95bf20e",
    "jbindgen": "b331bcc2c24ff03fbd64cc5ee833c9bc21aa30c1",
    "jni-rs": "dbfb17647988b81ff37b897ce93cc85ee4c84863",
    "jni-sys": "809b4c4939a2a581b84aa17018a5033b71c6d435",
}


def retry_readonly(function, path, error):
    """Git object files can be read-only, which blocks deletion on Windows."""
    if not isinstance(error, PermissionError):
        raise error
    os.chmod(path, os.stat(path).st_mode | stat.S_IWUSR)
    function(path)


def remove_path(path):
    # Remove directory links themselves, never the source they point to.
    if path.is_symlink():
        path.unlink()
    elif path.is_junction():
        path.rmdir()
    elif path.is_dir():
        shutil.rmtree(path, onexc=retry_readonly)
    elif path.exists():
        path.unlink()


def clone_reference(name, reference_root):
    revision = REVISIONS[name]
    if reference_root.is_symlink() or reference_root.is_junction():
        raise OSError(f"Refusing to use linked reference directory: {reference_root}")
    reference_root.mkdir(parents=True, exist_ok=True)
    temporary = Path(tempfile.mkdtemp(prefix=f".{name}.", dir=reference_root))
    staging = temporary / name
    try:
        # Prepare the replacement before touching the old copy.
        subprocess.run(
            [
                "git", "clone", "--quiet", "--depth", "5", "--single-branch",
                "--no-tags",
                f"https://github.com/jni-rs/{name}.git", str(staging),
            ],
            check=True,
        )
        git = ["git", "-C", str(staging)]
        available = subprocess.run(
            [*git, "cat-file", "-e", f"{revision}^{{commit}}"],
            stderr=subprocess.DEVNULL,
            check=False,
        )
        # The pinned commit may be outside the default branch's shallow history.
        if available.returncode != 0:
            subprocess.run([*git, "fetch", "--quiet", "--depth", "5", "origin", revision], check=True)
        subprocess.run([*git, "reset", "--quiet", "--hard", revision], check=True)
        destination = reference_root / name
        remove_path(destination)
        staging.rename(destination)
    finally:
        remove_path(temporary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repository", choices=sorted(REVISIONS))
    args = parser.parse_args()
    reference_root = Path(__file__).resolve().parent.parent / ".references"
    try:
        clone_reference(args.repository, reference_root)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"Unable to refresh {args.repository}: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
