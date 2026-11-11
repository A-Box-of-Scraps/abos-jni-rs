import importlib.util
import io
import json
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import call, patch

SCRIPT = Path(__file__).resolve().parents[1] / "ci-downloads.py"
SPEC = importlib.util.spec_from_file_location("ci_downloads", SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise ImportError(f"Unable to load script: {SCRIPT}")
ci = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ci)


class DownloadsTests(unittest.TestCase):
    def setUp(self):
        self.directory = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.crates = self.directory / "crates"
        self.crates.mkdir()

    def add_crate(self, directory, name, extra=""):
        crate = self.crates / directory
        crate.mkdir()
        (crate / "Cargo.toml").write_text(f'[package]\nname = "{name}"\n{extra}', encoding="utf-8")

    def test_discovers_package_names_and_skips_unpublished_crates(self):
        self.add_crate("b", "crate-b", "publish = true\n")
        self.add_crate("a", "crate-a")
        self.add_crate("private", "private", "publish = false\n")
        (self.crates / "not-a-crate").mkdir()
        self.assertEqual(ci.crate_names(self.crates), ["crate-a", "crate-b"])

    def test_empty_crates_directory_fails(self):
        with self.assertRaisesRegex(ValueError, "No publishable crates"):
            ci.crate_names(self.crates)

    def test_badge_sums_all_crates_and_throttles_requests(self):
        self.add_crate("a", "crate-a")
        self.add_crate("b", "crate-b")
        with (
            patch.object(ci, "fetch_downloads", side_effect=[200, 1000]) as fetch,
            patch.object(ci.time, "sleep") as sleep,
        ):
            badge = ci.build_badge(self.crates)
        self.assertEqual(fetch.call_args_list, [call("crate-a"), call("crate-b")])
        sleep.assert_called_once_with(1)
        self.assertEqual(
            badge,
            {
                "schemaVersion": 1,
                "label": "downloads",
                "message": "1,200",
                "color": "00599C",
                "labelColor": "555555",
                "style": "for-the-badge",
            },
        )

    def test_fetch_uses_total_downloads_and_identifies_the_client(self):
        response = io.BytesIO(json.dumps({"crate": {"downloads": 1234, "recent_downloads": 10}}).encode())
        with patch.object(ci.urllib.request, "urlopen", return_value=response) as urlopen:
            self.assertEqual(ci.fetch_downloads("crate-a"), 1234)
        request = urlopen.call_args.args[0]
        self.assertEqual(request.full_url, "https://crates.io/api/v1/crates/crate-a")
        self.assertEqual(request.get_header("User-agent"), ci.USER_AGENT)
        self.assertEqual(urlopen.call_args.kwargs["timeout"], 30)

    def test_invalid_counts_fail(self):
        for count in [-1, "100", 1.5, True, None]:
            with self.subTest(count=count):
                response = io.BytesIO(json.dumps({"crate": {"downloads": count}}).encode())
                with patch.object(ci.urllib.request, "urlopen", return_value=response), self.assertRaises(ValueError):
                    ci.fetch_downloads("crate-a")

    def test_api_failure_does_not_produce_a_partial_badge(self):
        self.add_crate("a", "crate-a")
        self.add_crate("b", "crate-b")
        error = self.enterContext(urllib.error.HTTPError("https://crates.io", 503, "Unavailable", None, io.BytesIO()))
        with (
            patch.object(ci, "fetch_downloads", side_effect=[10, error]),
            patch.object(ci.time, "sleep"),
            self.assertRaises(urllib.error.HTTPError),
        ):
            ci.build_badge(self.crates)

    def test_main_writes_endpoint_json_and_creates_directories(self):
        output = self.directory / "site" / "downloads.json"
        badge = {"schemaVersion": 1, "label": "downloads", "message": "0"}
        with (
            patch("sys.argv", [str(SCRIPT), str(output)]),
            patch.object(ci, "build_badge", return_value=badge) as build,
        ):
            ci.main()
        build.assert_called_once_with(ci.ROOT / "crates")
        self.assertEqual(json.loads(output.read_text(encoding="utf-8")), badge)

    def test_main_preserves_existing_badge_on_failure(self):
        output = self.directory / "downloads.json"
        output.write_text("previous badge", encoding="utf-8")
        with (
            patch("sys.argv", [str(SCRIPT), str(output)]),
            patch.object(ci, "build_badge", side_effect=ValueError("API failure")),
            self.assertRaises(ValueError),
        ):
            ci.main()
        self.assertEqual(output.read_text(encoding="utf-8"), "previous badge")


if __name__ == "__main__":
    unittest.main()
