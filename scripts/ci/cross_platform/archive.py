"""Archive available native or Android debug outputs for CI artifacts."""

import argparse
import io
import json
import os
import tarfile
from pathlib import Path

from .common import TARGETS, write_output

DEBUG_DIRECTORY = "target/debug"
ANDROID_PROJECT = "tests/android-test-activity"
ANDROID_DIRECTORIES = (
    f"{ANDROID_PROJECT}/target/aarch64-linux-android/debug",
    f"{ANDROID_PROJECT}/target/x86_64-linux-android/debug",
    f"{ANDROID_PROJECT}/build/test-apks",
    f"{ANDROID_PROJECT}/build/ci-logs",
)


def archive_debug(root, destination, target, sha):
    """Preserve symbols, libraries, tests and permissions, but not incremental caches."""
    if target not in TARGETS:
        raise ValueError(f"Unknown artifact target: {target!r}")
    if len(sha) != 40 or any(character not in "0123456789abcdef" for character in sha):
        raise ValueError(f"Invalid commit SHA: {sha!r}")
    directories = ANDROID_DIRECTORIES if target == "android" else (DEBUG_DIRECTORY,)
    available = [directory for directory in directories if (root / directory).is_dir()]
    if not available:
        return None
    destination.mkdir(parents=True, exist_ok=True)
    archive = destination / f"debug-{target}-{sha}.tar.gz"
    metadata = json.dumps({"target": target, "sha": sha, "profile": "debug"}, indent=2).encode("utf-8")
    with tarfile.open(archive, "w:gz", compresslevel=1) as bundle:
        info = tarfile.TarInfo("build-info.json")
        info.size = len(metadata)
        bundle.addfile(info, io.BytesIO(metadata))
        for directory in available:
            bundle.add(root / directory, arcname=directory, filter=exclude_incremental)
    return archive


def exclude_incremental(member):
    if "incremental" in Path(member.name).parts:
        return None
    return member


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    archive = archive_debug(
        Path.cwd(),
        Path(os.environ["RUNNER_TEMP"]) / "cross-platform-artifacts",
        os.environ["CI_TARGET"],
        os.environ["CI_SHA"],
    )
    if archive is not None:
        write_output("path", str(archive))
        print(f"Archived debug outputs: {archive}")
    else:
        print(f"No debug outputs to archive for {os.environ['CI_TARGET']}.")


if __name__ == "__main__":
    main()
