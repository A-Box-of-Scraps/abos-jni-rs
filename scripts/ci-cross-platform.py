"""Select native CI runners, skip unchanged daily runs, and archive debug outputs."""

import argparse
import io
import json
import os
import subprocess
import tarfile
import urllib.parse
import urllib.request
from pathlib import Path

RUNNERS = {
    "linux-amd64": "ubuntu-24.04",
    "linux-arm64": "ubuntu-24.04-arm",
    "windows-amd64": "windows-2025",
    "windows-arm64": "windows-11-arm",
    "macos-amd64": "macos-15-intel",
    "macos-arm64": "macos-15",
}


def select_matrix(target):
    if target != "all" and target not in RUNNERS:
        raise ValueError(f"Unknown target {target!r}; expected all or one of {', '.join(RUNNERS)}")
    return {
        "include": [
            {"target": name, "runner": runner} for name, runner in RUNNERS.items() if target == "all" or target == name
        ]
    }


def previous_scheduled_sha():
    """Find the preceding scheduled run, including failures and skipped matrices."""
    api = os.environ.get("GITHUB_API_URL", "https://api.github.com")
    repository = os.environ["GITHUB_REPOSITORY"]
    run_number = int(os.environ["GITHUB_RUN_NUMBER"])
    page = 1
    while True:
        query = urllib.parse.urlencode(
            {"event": "schedule", "branch": os.environ["GITHUB_REF_NAME"], "per_page": 100, "page": page}
        )
        request = urllib.request.Request(
            f"{api}/repos/{repository}/actions/workflows/cross-platform.yml/runs?{query}",
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {os.environ['GH_TOKEN']}",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        # API errors must fail visibly, not masquerade as an empty run history.
        with urllib.request.urlopen(request, timeout=30) as response:
            runs = json.load(response)["workflow_runs"]
        for run in runs:
            if run["run_number"] < run_number:
                return run["head_sha"]
        if len(runs) < 100:
            return None
        page += 1


def should_run(sha):
    # Explicit reruns and manual/reusable calls are never suppressed by daily history.
    if os.environ["GITHUB_EVENT_NAME"] != "schedule" or int(os.environ.get("GITHUB_RUN_ATTEMPT", "1")) > 1:
        return True
    workflow = os.environ["GITHUB_WORKFLOW_REF"].split("@", 1)[0]
    if workflow != f"{os.environ['GITHUB_REPOSITORY']}/.github/workflows/cross-platform.yml":
        return True
    return previous_scheduled_sha() != sha


def write_output(name, value):
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"{name}={value}\n")


def prepare():
    matrix = select_matrix(os.environ.get("CI_TARGET", "all"))
    sha = subprocess.run(["git", "rev-parse", "HEAD"], check=True, capture_output=True, text=True).stdout.strip()
    run = should_run(sha)
    write_output("matrix", json.dumps(matrix, separators=(",", ":")))
    write_output("sha", sha)
    write_output("should-run", str(run).lower())
    print(f"{'Checking' if run else 'Skipping unchanged scheduled commit'} {sha}")


def archive_debug(root, destination, target, sha):
    """Preserve symbols, libraries, tests and permissions, but not incremental caches."""
    if target not in RUNNERS:
        raise ValueError(f"Unknown artifact target: {target!r}")
    if len(sha) != 40 or any(character not in "0123456789abcdef" for character in sha):
        raise ValueError(f"Invalid commit SHA: {sha!r}")
    debug = root / "target" / "debug"
    if not debug.is_dir():
        return None
    destination.mkdir(parents=True, exist_ok=True)
    archive = destination / f"debug-{target}-{sha}.tar.gz"
    metadata = json.dumps({"target": target, "sha": sha, "profile": "debug"}, indent=2).encode("utf-8")
    with tarfile.open(archive, "w:gz", compresslevel=1) as bundle:
        info = tarfile.TarInfo("build-info.json")
        info.size = len(metadata)
        bundle.addfile(info, io.BytesIO(metadata))
        bundle.add(debug, arcname="target/debug", filter=exclude_incremental)
    return archive


def exclude_incremental(member):
    if member.name == "target/debug/incremental" or member.name.startswith("target/debug/incremental/"):
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
            print("No target/debug directory to archive.")


if __name__ == "__main__":
    main()
