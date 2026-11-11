"""Generate a Shields endpoint badge for total workspace crate downloads."""

import argparse
import json
import time
import tomllib
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
USER_AGENT = "abos-jni-rs-downloads (https://github.com/A-Box-of-Scraps/abos-jni-rs)"


def crate_names(crates_directory):
    names = []
    for manifest in sorted(crates_directory.glob("*/Cargo.toml")):
        package = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]
        if package.get("publish") is not False:
            names.append(package["name"])
    if not names:
        raise ValueError("No publishable crates found")
    return names


def fetch_downloads(name):
    url = f"https://crates.io/api/v1/crates/{urllib.parse.quote(name, safe='')}"
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:
        downloads = json.load(response)["crate"]["downloads"]
    if type(downloads) is not int or downloads < 0:
        raise ValueError(f"Invalid download count for {name}: {downloads!r}")
    return downloads


def build_badge(crates_directory):
    total = 0
    for index, name in enumerate(crate_names(crates_directory)):
        # Stay below crates.io's crawler limit of one request per second.
        if index:
            time.sleep(1)
        downloads = fetch_downloads(name)
        print(f"{name}: {downloads:,}")
        total += downloads
    return {
        "schemaVersion": 1,
        "label": "downloads",
        "message": f"{total:,}",
        "color": "00599C",
        "labelColor": "555555",
        "style": "for-the-badge",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="Path to the Pages downloads.json file")
    args = parser.parse_args()
    badge = build_badge(ROOT / "crates")
    # Do not replace a valid badge with partial totals if any request fails.
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(badge, indent=2) + "\n", encoding="utf-8")
    print(f"Total downloads: {badge['message']}")


if __name__ == "__main__":
    main()
