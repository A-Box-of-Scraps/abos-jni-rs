"""Select CI targets, skip unchanged daily runs, and archive debug outputs."""

import argparse
import io
import json
import os
import subprocess
import tarfile
import urllib.parse
import urllib.request
from pathlib import Path

WORKFLOW_FILE = "cross-platform.yml"
WORKFLOW_PATH = f".github/workflows/{WORKFLOW_FILE}"
ITEMS_PER_PAGE = 100
DEBUG_DIRECTORY = "target/debug"
ANDROID_PROJECT = "tests/android-test-activity"
ANDROID_DIRECTORIES = (
    f"{ANDROID_PROJECT}/target/aarch64-linux-android/debug",
    f"{ANDROID_PROJECT}/target/x86_64-linux-android/debug",
    f"{ANDROID_PROJECT}/build/test-apks",
    f"{ANDROID_PROJECT}/build/ci-logs",
)

NATIVE_RUNNERS = {
    "linux-amd64": "ubuntu-24.04",
    "linux-arm64": "ubuntu-24.04-arm",
    "windows-amd64": "windows-2025",
    "windows-arm64": "windows-11-arm",
    "macos-amd64": "macos-15-intel",
    "macos-arm64": "macos-15",
}
TARGETS = (*NATIVE_RUNNERS, "android")


def select_matrix(target):
    """Select native runners; Android is handled by a separate job."""
    if target != "all" and target not in TARGETS:
        raise ValueError(f"Unknown target {target!r}; expected all or one of {', '.join(TARGETS)}")
    return {
        "include": [
            {"target": name, "runner": runner}
            for name, runner in NATIVE_RUNNERS.items()
            if target == "all" or target == name
        ]
    }


def github_items(path, key, **filters):
    """Read all pages of a GitHub Actions collection, failing visibly on API errors."""
    api = os.environ.get("GITHUB_API_URL", "https://api.github.com")
    repository = os.environ["GITHUB_REPOSITORY"]
    url = f"{api}/repos/{repository}/actions/{path}"
    headers = {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {os.environ['GH_TOKEN']}",
        "X-GitHub-Api-Version": "2022-11-28",
    }
    page = 1
    while True:
        query = urllib.parse.urlencode({**filters, "per_page": ITEMS_PER_PAGE, "page": page})
        request = urllib.request.Request(f"{url}?{query}", headers=headers)
        with urllib.request.urlopen(request, timeout=30) as response:
            items = json.load(response)[key]
        yield from items
        if len(items) < ITEMS_PER_PAGE:
            return
        page += 1


def has_successful_scheduled_build(sha):
    """Require a completed matrix, not just a green workflow with skipped jobs."""
    run_number = int(os.environ["GITHUB_RUN_NUMBER"])
    runs = github_items(
        f"workflows/{WORKFLOW_FILE}/runs",
        "workflow_runs",
        event="schedule",
        branch=os.environ["GITHUB_REF_NAME"],
        head_sha=sha,
        status="success",
    )
    # Keep these names aligned with the native and Android jobs in cross-platform.yml.
    expected_jobs = {f"Check and build ({target})" for target in TARGETS}
    for run in runs:
        if (
            run["run_number"] >= run_number
            or run["head_sha"] != sha
            or run["status"] != "completed"
            or run["conclusion"] != "success"
        ):
            continue
        jobs = github_items(f"runs/{run['id']}/jobs", "jobs", filter="latest")
        successful_jobs = {
            job["name"] for job in jobs if job["status"] == "completed" and job["conclusion"] == "success"
        }
        if expected_jobs <= successful_jobs:
            return True
    return False


def should_run(sha):
    # Explicit reruns and manual/reusable calls are never suppressed by daily history.
    if os.environ["GITHUB_EVENT_NAME"] != "schedule" or int(os.environ.get("GITHUB_RUN_ATTEMPT", "1")) > 1:
        return True
    workflow = os.environ["GITHUB_WORKFLOW_REF"].split("@", 1)[0]
    if workflow != f"{os.environ['GITHUB_REPOSITORY']}/{WORKFLOW_PATH}":
        return True
    return not has_successful_scheduled_build(sha)


def write_output(name, value):
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"{name}={value}\n")


def prepare():
    target = os.environ.get("CI_TARGET", "all")
    matrix = select_matrix(target)
    sha = subprocess.run(["git", "rev-parse", "HEAD"], check=True, capture_output=True, text=True).stdout.strip()
    run = should_run(sha)
    write_output("matrix", json.dumps(matrix, separators=(",", ":")))
    write_output("sha", sha)
    write_output("run-native", str(run and bool(matrix["include"])).lower())
    write_output("run-android", str(run and target in {"all", "android"}).lower())
    print(f"{'Checking' if run else 'Skipping unchanged scheduled commit'} {sha}")


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
    parser.add_argument("command", choices=["prepare", "archive"])
    args = parser.parse_args()
    if args.command == "prepare":
        prepare()
    else:
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
