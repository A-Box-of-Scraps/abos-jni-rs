"""Publish newer workspace versions and journal only confirmed release changelogs."""

import argparse
import datetime
import hashlib
import io
import json
import math
import os
import re
import subprocess
import tarfile
import time
import urllib.error
import urllib.request
from email.utils import parsedate_to_datetime
from graphlib import TopologicalSorter
from pathlib import Path

MAX_PUBLISH_ATTEMPTS = 10
PUBLISH_RETRY_SECONDS = 60

SEMVER = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)


def version_key(version):
    """SemVer precedence ignores build metadata and places releases after prereleases."""
    match = SEMVER.fullmatch(version)
    if match is None:
        raise ValueError(f"Invalid SemVer: {version}")
    major, minor, patch, prerelease, _ = match.groups()
    identifiers = []
    for identifier in prerelease.split(".") if prerelease else []:
        numeric = identifier.isascii() and identifier.isdigit()
        if numeric and len(identifier) > 1 and identifier.startswith("0"):
            raise ValueError(f"Invalid numeric prerelease identifier: {version}")
        identifiers.append((0, int(identifier)) if numeric else (1, identifier))
    return int(major), int(minor), int(patch), prerelease is None, tuple(identifiers)


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "workspace-publisher", "Cache-Control": "no-cache"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read()


def registry_versions(name):
    name = name.lower()
    if len(name) < 3:
        prefix = str(len(name))
    elif len(name) == 3:
        prefix = f"3/{name[0]}"
    else:
        prefix = f"{name[:2]}/{name[2:4]}"
    try:
        body = fetch(f"https://index.crates.io/{prefix}/{name}")
    except urllib.error.HTTPError as error:
        if error.code == 404:
            error.close()
            return []
        raise
    versions = [json.loads(line) for line in body.splitlines() if line.strip()]
    if not versions:
        raise ValueError(f"Empty registry response for {name}")
    return versions


def wait_for_version(name, version):
    for attempt in range(18):
        for entry in registry_versions(name):
            if entry["vers"] == version:
                return entry
        if attempt < 17:
            time.sleep(10)
    raise RuntimeError(f"{name} {version} is not visible in the registry; rerun to reconcile before continuing")


def release_changelog(text, version, date):
    """Keep Unreleased, including when it is empty, and preserve older sections."""
    if re.search(rf"^## \[{re.escape(version)}\](?:\s|$)", text, re.MULTILINE):
        raise ValueError(f"Changelog already contains version {version}")
    headings = list(re.finditer(r"^## \[Unreleased\][ \t]*$", text, re.MULTILINE))
    if len(headings) != 1:
        raise ValueError("Changelog must contain exactly one ## [Unreleased] heading")
    heading = headings[0]
    remaining = text[heading.end() :]
    next_section = re.search(r"^## ", remaining, re.MULTILINE)
    split = next_section.start() if next_section else len(remaining)
    changes = remaining[:split].strip()
    older = remaining[split:].rstrip()
    result = f"{text[: heading.start()]}## [Unreleased]\n\n## [{version}] - {date}\n"
    if changes:
        result += f"\n{changes}\n"
    if older:
        result += f"\n{older}\n"
    return result


def published_changelog(name, entry):
    version = entry["vers"]
    archive = fetch(f"https://static.crates.io/crates/{name}/{name}-{version}.crate")
    if hashlib.sha256(archive).hexdigest() != entry["cksum"]:
        raise ValueError(f"Registry checksum mismatch for {name} {version}")
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
        # Read one member, never extract paths from a remote archive onto disk.
        try:
            member = bundle.extractfile(f"{name}-{version}/CHANGELOG.md")
        except KeyError:
            return None
        if member is None:
            return None
        return member.read().decode("utf-8")


def recover_changelog(text, name, entry):
    """Recover a previous upload without attributing newer Unreleased changes to it."""
    version = entry["vers"]
    if re.search(rf"^## \[{re.escape(version)}\](?:\s|$)", text, re.MULTILINE):
        return None
    published = published_changelog(name, entry)
    if published is None:
        return None
    heading = re.search(rf"^## \[{re.escape(version)}\] - (\d{{4}}-\d{{2}}-\d{{2}})$", published, re.MULTILINE)
    # Older releases made outside this workflow may not contain finalized notes.
    if heading is None:
        return None
    if release_changelog(text, version, heading[1]) != published:
        raise ValueError(f"Reconcile {name} {version} CHANGELOG.md manually: published and local changes differ")
    return published


def workspace_packages():
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(result.stdout)
    return [package for package in metadata["packages"] if package["id"] in metadata["workspace_members"]]


def publication_order(packages):
    publishable = {
        package["name"]: package
        for package in packages
        if package["publish"] is None or "crates-io" in package["publish"]
    }
    by_path = {str(Path(package["manifest_path"]).parent): package["name"] for package in packages}
    graph = {}
    for name, package in publishable.items():
        # Include build, dev and target-specific dependencies: packaging resolves them too.
        graph[name] = {
            by_path[dependency["path"]]
            for dependency in package["dependencies"]
            if dependency.get("path") in by_path and by_path[dependency["path"]] in publishable
        }
    return [publishable[name] for name in TopologicalSorter(graph).static_order()]


def save_release(state, path, text):
    state.mkdir(parents=True, exist_ok=True)
    journal = state / "releases.json"
    releases = json.loads(journal.read_text(encoding="utf-8")) if journal.exists() else {}
    releases[path.as_posix()] = text
    temporary = journal.with_suffix(".tmp")
    temporary.write_text(json.dumps(releases, indent=2) + "\n", encoding="utf-8")
    temporary.replace(journal)


def publish_retry_delay(output):
    match = re.search(r"try again after ([^\r\n]+? GMT)\b", output, re.IGNORECASE)
    if match is not None:
        try:
            retry_at = parsedate_to_datetime(match[1])
            remaining = (retry_at - datetime.datetime.now(datetime.UTC)).total_seconds()
            return max(PUBLISH_RETRY_SECONDS, math.ceil(remaining))
        except (TypeError, ValueError, OverflowError):
            pass
    return PUBLISH_RETRY_SECONDS


def publish_with_retry(name, version):
    for attempt in range(1, MAX_PUBLISH_ATTEMPTS + 1):
        result = subprocess.run(
            ["cargo", "publish", "--locked", "--registry", "crates-io", "--package", name, "--allow-dirty"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        output = result.stdout or ""
        print(output, end="", flush=True)
        if result.returncode == 0 or not re.search(r"429 Too Many Requests|status 429\b", output, re.IGNORECASE):
            return result
        if attempt == MAX_PUBLISH_ATTEMPTS:
            raise RuntimeError(f"{name} {version}: crates.io still rate-limited publishing after {attempt} attempts")
        delay = publish_retry_delay(output)
        print(
            f"{name} {version}: crates.io rate limit; waiting {delay}s before attempt "
            f"{attempt + 1}/{MAX_PUBLISH_ATTEMPTS}",
            flush=True,
        )
        time.sleep(delay)
        # Cargo may have uploaded successfully before encountering a rate-limited index request.
        # Let the caller verify the published archive instead of attempting a duplicate upload.
        if any(entry["vers"] == version for entry in registry_versions(name)):
            return result
    raise RuntimeError("Publishing requires at least one attempt")


def publish_package(package, state):
    name, version = package["name"], package["version"]
    path = (Path(package["manifest_path"]).parent / "CHANGELOG.md").relative_to(Path.cwd())
    subprocess.run(["git", "ls-files", "--error-unmatch", "--", str(path)], check=True, capture_output=True)
    text = path.read_text(encoding="utf-8")
    versions = registry_versions(name)
    latest = max((version_key(entry["vers"]) for entry in versions), default=None)
    local = version_key(version)
    if latest is not None and local <= latest:
        print(f"Skipping {name} {version}: not newer than the registry", flush=True)
        exact = next((entry for entry in versions if entry["vers"] == version), None)
        if exact is not None and local == latest:
            recovered = recover_changelog(text, name, exact)
            if recovered is not None:
                save_release(state, path, recovered)
        return

    date = datetime.datetime.now(datetime.UTC).date().isoformat()
    prepared = release_changelog(text, version, date)
    print(f"Publishing {name} {version}", flush=True)
    path.write_text(prepared, encoding="utf-8")
    try:
        result = publish_with_retry(name, version)
        if result.returncode == 0:
            # Cargo confirmed the upload, even if our subsequent index request fails.
            save_release(state, path, prepared)
        entry = wait_for_version(name, version)
        if result.returncode != 0 and published_changelog(name, entry) != prepared:
            raise RuntimeError(f"{name} {version} was published with different release notes")
        save_release(state, path, prepared)
    finally:
        # Keep only one intentional dirty file during packaging. The journal is committed later.
        path.write_text(text, encoding="utf-8")


def publish(state):
    if not os.environ.get("CARGO_REGISTRY_TOKEN"):
        raise ValueError("Set the CARGO_REGISTRY_TOKEN GitHub Actions secret before publishing")
    if subprocess.run(["git", "status", "--porcelain"], check=True, capture_output=True, text=True).stdout:
        raise ValueError("Publishing requires a clean checkout")
    for package in publication_order(workspace_packages()):
        publish_package(package, state)


def commit(state):
    journal = state / "releases.json"
    if not journal.exists():
        print("No confirmed releases to commit")
        return
    releases = json.loads(journal.read_text(encoding="utf-8"))
    for filename, text in releases.items():
        path = Path(filename)
        if path.is_absolute() or ".." in path.parts or path.name != "CHANGELOG.md":
            raise ValueError(f"Invalid changelog path: {filename}")
        path.write_text(text, encoding="utf-8")
    subprocess.run(["git", "add", "--", *releases], check=True)
    diff = subprocess.run(["git", "diff", "--cached", "--binary"], check=True, capture_output=True).stdout
    (state / "release.patch").write_bytes(diff)
    if diff:
        subprocess.run(
            [
                "git",
                "-c",
                "user.name=github-actions[bot]",
                "-c",
                "user.email=41898282+github-actions[bot]@users.noreply.github.com",
                "commit",
                "-m",
                "Next cycle",
            ],
            check=True,
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["publish", "commit"])
    args = parser.parse_args()
    state = Path(os.environ["RELEASE_STATE_DIR"])
    if args.command == "publish":
        publish(state)
    else:
        commit(state)


if __name__ == "__main__":
    main()
