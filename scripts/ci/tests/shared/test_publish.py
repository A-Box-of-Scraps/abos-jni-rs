import datetime
import hashlib
import io
import json
import os
import subprocess
import tarfile
import tempfile
import unittest
import urllib.error
from email.message import Message
from graphlib import CycleError
from pathlib import Path
from unittest.mock import patch

from scripts.ci import publish as ci

EMPTY = "# Changelog\n\n## [Unreleased]\n"
CHANGES = EMPTY + "\n### Added\n\n- New API.\n"
DATE = "2026-01-02"


def package(name, dependencies=(), publish=None, version="1.0.0", root=Path("/workspace")):
    return {
        "id": name,
        "name": name,
        "version": version,
        "manifest_path": str(root / name / "Cargo.toml"),
        "publish": publish,
        "dependencies": [{"name": dependency, "path": str(root / dependency)} for dependency in dependencies],
    }


class VersionTests(unittest.TestCase):
    def test_semver_precedence(self):
        ordered = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.2",
            "1.0.10",
            "2.0.0",
            "10.0.0",
        ]
        shuffled = list(reversed(ordered))
        self.assertEqual(sorted(shuffled, key=ci.version_key), ordered)
        self.assertEqual(ci.version_key("1.0.0+build.1"), ci.version_key("1.0.0+build.2"))
        self.assertEqual(ci.version_key("1.0.0-rc.1+build"), ci.version_key("1.0.0-rc.1"))

    def test_invalid_versions_fail_closed(self):
        for version in ["1.2", "01.2.3", "1.2.3-01", "1.2.3-a..b", "1.2.3-", "v1.2.3"]:
            with self.subTest(version=version), self.assertRaises(ValueError):
                ci.version_key(version)


class ChangelogTests(unittest.TestCase):
    def test_empty_unreleased_still_gets_release_heading(self):
        self.assertEqual(ci.release_changelog(EMPTY, "1.0.0", DATE), EMPTY + f"\n## [1.0.0] - {DATE}\n")

    def test_changes_move_and_history_is_preserved(self):
        older = "\n## [0.9.0] - 2025-12-01\n\n- Old change.\n"
        result = ci.release_changelog(CHANGES + older, "1.0.0", DATE)
        self.assertEqual(result, EMPTY + f"\n## [1.0.0] - {DATE}\n\n### Added\n\n- New API.\n" + older)

    def test_missing_duplicate_and_already_released_headings_fail(self):
        for text in ["# Changelog\n", EMPTY + "\n## [Unreleased]\n", EMPTY + "\n## [1.0.0] - 2026-01-01\n"]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                ci.release_changelog(text, "1.0.0", DATE)

    def test_retry_recovers_exact_published_notes_and_date(self):
        published = ci.release_changelog(CHANGES, "1.0.0", DATE)
        with patch.object(ci, "published_changelog", return_value=published):
            self.assertEqual(ci.recover_changelog(CHANGES, "demo", {"vers": "1.0.0"}), published)
            with self.assertRaisesRegex(ValueError, "manually"):
                ci.recover_changelog(CHANGES + "- Unpublished change.\n", "demo", {"vers": "1.0.0"})

    def test_existing_release_without_finalized_notes_is_skipped(self):
        for text in [None, EMPTY, CHANGES]:
            with self.subTest(text=text), patch.object(ci, "published_changelog", return_value=text):
                self.assertIsNone(ci.recover_changelog(CHANGES, "demo", {"vers": "1.0.0"}))

    def test_retry_leaves_already_committed_notes_untouched(self):
        published = ci.release_changelog(CHANGES, "1.0.0", DATE)
        with patch.object(ci, "published_changelog") as download:
            self.assertIsNone(ci.recover_changelog(published, "demo", {"vers": "1.0.0"}))
            download.assert_not_called()


class RegistryTests(unittest.TestCase):
    def test_sparse_index_paths_and_yanked_versions_are_retained(self):
        entry = {"vers": "1.0.0", "yanked": True}
        for name, expected in [("a", "1/a"), ("ab", "2/ab"), ("Abc", "3/a/abc"), ("demo", "de/mo/demo")]:
            with self.subTest(name=name), patch.object(ci, "fetch", return_value=json.dumps(entry).encode()) as fetch:
                self.assertEqual(ci.registry_versions(name), [entry])
                fetch.assert_called_once_with(f"https://index.crates.io/{expected}")

    def test_only_404_means_missing_crate(self):
        for code in [404, 403, 429, 500]:
            error = urllib.error.HTTPError("https://example.invalid", code, "error", Message(), None)
            self.addCleanup(error.close)
            with self.subTest(code=code), patch.object(ci, "fetch", side_effect=error):
                if code == 404:
                    self.assertEqual(ci.registry_versions("demo"), [])
                else:
                    with self.assertRaises(urllib.error.HTTPError):
                        ci.registry_versions("demo")

    def test_malformed_registry_response_fails(self):
        for body in [b"", b"not json"]:
            with self.subTest(body=body), patch.object(ci, "fetch", return_value=body), self.assertRaises(ValueError):
                ci.registry_versions("demo")

    def test_waits_until_version_is_visible(self):
        entry = {"vers": "1.0.0"}
        with (
            patch.object(ci, "registry_versions", side_effect=[[], [], [entry]]),
            patch.object(ci.time, "sleep") as sleep,
        ):
            self.assertEqual(ci.wait_for_version("demo", "1.0.0"), entry)
            self.assertEqual(sleep.call_count, 2)

    def test_registry_timeout_fails(self):
        with (
            patch.object(ci, "registry_versions", return_value=[]),
            patch.object(ci.time, "sleep"),
            self.assertRaisesRegex(RuntimeError, "not visible"),
        ):
            ci.wait_for_version("demo", "1.0.0")

    def test_reads_only_changelog_from_checksummed_archive(self):
        data = io.BytesIO()
        with tarfile.open(fileobj=data, mode="w:gz") as bundle:
            content = CHANGES.encode()
            member = tarfile.TarInfo("demo-1.0.0/CHANGELOG.md")
            member.size = len(content)
            bundle.addfile(member, io.BytesIO(content))
        archive = data.getvalue()
        entry = {"vers": "1.0.0", "cksum": hashlib.sha256(archive).hexdigest()}
        with patch.object(ci, "fetch", return_value=archive):
            self.assertEqual(ci.published_changelog("demo", entry), CHANGES)
            entry["cksum"] = "wrong"
            with self.assertRaisesRegex(ValueError, "checksum"):
                ci.published_changelog("demo", entry)


class OrderTests(unittest.TestCase):
    def test_dependencies_precede_dependents_and_private_crates_are_skipped(self):
        packages = [
            package("app", ["macros", "build", "dev"]),
            package("macros", ["core"]),
            package("core", publish=["crates-io"]),
            package("build"),
            package("dev"),
            package("private", publish=[]),
            package("other-registry", publish=["other"]),
        ]
        order = [item["name"] for item in ci.publication_order(packages)]
        self.assertEqual(set(order), {"app", "macros", "core", "build", "dev"})
        self.assertLess(order.index("core"), order.index("macros"))
        for dependency in ["macros", "build", "dev"]:
            self.assertLess(order.index(dependency), order.index("app"))

    def test_dependency_cycles_fail_before_uploading(self):
        with self.assertRaises(CycleError):
            ci.publication_order([package("a", ["b"]), package("b", ["a"])])

    def test_metadata_excludes_nonmembers(self):
        metadata = {"packages": [package("member"), package("other")], "workspace_members": ["member"]}
        result = subprocess.CompletedProcess([], 0, stdout=json.dumps(metadata))
        with patch.object(ci.subprocess, "run", return_value=result):
            self.assertEqual([item["name"] for item in ci.workspace_packages()], ["member"])


class PublishRetryTests(unittest.TestCase):
    def setUp(self):
        self.run_mock = self.enterContext(patch.object(ci.subprocess, "run"))
        self.sleep = self.enterContext(patch.object(ci.time, "sleep"))
        self.versions = self.enterContext(patch.object(ci, "registry_versions", return_value=[]))
        self.limited = subprocess.CompletedProcess([], 101, stdout="error: status 429 Too Many Requests\n")
        self.success = subprocess.CompletedProcess([], 0, stdout="Published demo\n")

    def test_rate_limit_retries_after_a_minute(self):
        self.run_mock.side_effect = [self.limited, self.success]
        self.assertEqual(ci.publish_with_retry("demo", "1.0.0"), self.success)
        self.sleep.assert_called_once_with(60)
        self.assertEqual(self.run_mock.call_count, 2)
        self.versions.assert_called_once_with("demo")

    def test_repeated_rate_limits_have_bounded_retries(self):
        self.run_mock.return_value = self.limited
        with self.assertRaisesRegex(RuntimeError, "rate-limited publishing after 10 attempts"):
            ci.publish_with_retry("demo", "1.0.0")
        self.assertEqual(self.run_mock.call_count, 10)
        self.assertEqual(self.sleep.call_count, 9)
        self.assertTrue(all(call.args == (60,) for call in self.sleep.call_args_list))

    def test_other_failures_are_not_retried(self):
        for output in ["error: status 403 Forbidden", "error: could not compile demo"]:
            result = subprocess.CompletedProcess([], 101, stdout=output)
            self.run_mock.return_value = result
            self.assertEqual(ci.publish_with_retry("demo", "1.0.0"), result)
        self.assertEqual(self.run_mock.call_count, 2)
        self.sleep.assert_not_called()

    def test_success_does_not_wait_or_retry(self):
        self.run_mock.return_value = self.success
        self.assertEqual(ci.publish_with_retry("demo", "1.0.0"), self.success)
        self.run_mock.assert_called_once()
        self.sleep.assert_not_called()
        self.versions.assert_not_called()

    def test_already_uploaded_version_is_not_uploaded_again(self):
        self.run_mock.return_value = self.limited
        self.versions.return_value = [{"vers": "1.0.0"}]
        self.assertEqual(ci.publish_with_retry("demo", "1.0.0"), self.limited)
        self.run_mock.assert_called_once()
        self.sleep.assert_called_once_with(60)

    def test_server_retry_date_is_honored(self):
        output = "Please try again after Wed, 11 Nov 2026 15:07:03 GMT and see the rate limits."
        now = datetime.datetime(2026, 11, 11, 15, 0, tzinfo=datetime.UTC)
        with patch.object(ci, "datetime") as clock:
            clock.datetime.now.return_value = now
            self.assertEqual(ci.publish_retry_delay(output), 423)
            self.limited.stdout = f"status 429: {output}"
            self.run_mock.side_effect = [self.limited, self.success]
            ci.publish_with_retry("demo", "1.0.0")
        self.sleep.assert_called_once_with(423)

    def test_missing_malformed_and_past_dates_wait_at_least_a_minute(self):
        for output in ["status 429", "try again after invalid GMT", "try again after Wed, 01 Jan 2020 00:00:00 GMT"]:
            with self.subTest(output=output):
                self.assertEqual(ci.publish_retry_delay(output), 60)


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(self.enterContext(tempfile.TemporaryDirectory())).resolve()
        self.state = Path(self.enterContext(tempfile.TemporaryDirectory()))
        previous = Path.cwd()
        self.addCleanup(os.chdir, previous)
        os.chdir(self.root)
        self.pkg = package("demo", root=self.root)
        self.path = self.root / "demo" / "CHANGELOG.md"
        self.path.parent.mkdir()
        self.path.write_text(CHANGES, encoding="utf-8")
        self.run_mock = self.enterContext(patch.object(ci.subprocess, "run"))
        self.run_mock.return_value = subprocess.CompletedProcess([], 0, stdout="")
        self.versions = self.enterContext(patch.object(ci, "registry_versions", return_value=[]))
        self.wait = self.enterContext(patch.object(ci, "wait_for_version", return_value={"vers": "1.0.0"}))

    def journal(self):
        return json.loads((self.state / "releases.json").read_text(encoding="utf-8"))

    def test_prepares_before_upload_and_restores_checkout(self):
        def run(command, **kwargs):
            if command[:2] == ["cargo", "publish"]:
                self.assertIn("## [1.0.0] - ", self.path.read_text())
                self.assertIn("--allow-dirty", command)
                self.assertIn("--locked", command)
                self.assertEqual(command[command.index("--registry") + 1], "crates-io")
            return subprocess.CompletedProcess(command, 0)

        self.run_mock.side_effect = run
        ci.publish_package(self.pkg, self.state)
        self.assertIn("## [1.0.0] - ", self.journal()["demo/CHANGELOG.md"])
        self.assertEqual(self.path.read_text(), CHANGES)

    def test_publishes_numerically_newer_version(self):
        self.pkg["version"] = "1.0.10"
        self.versions.return_value = [{"vers": "1.0.9", "yanked": True}]
        ci.publish_package(self.pkg, self.state)
        self.assertIn("## [1.0.10] - ", self.journal()["demo/CHANGELOG.md"])

    def test_does_not_publish_older_or_equal_precedence(self):
        for version in ["1.0.1", "1.0.0+build.1"]:
            with self.subTest(version=version):
                self.versions.return_value = [{"vers": version}]
                ci.publish_package(self.pkg, self.state)
                self.assertFalse((self.state / "releases.json").exists())
        self.wait.assert_not_called()
        self.assertTrue(all(call.args[0][0] == "git" for call in self.run_mock.call_args_list))

    def test_retry_recovers_without_uploading_again(self):
        self.versions.return_value = [{"vers": "1.0.0"}]
        released = ci.release_changelog(CHANGES, "1.0.0", DATE)
        with patch.object(ci, "published_changelog", return_value=released):
            ci.publish_package(self.pkg, self.state)
        self.assertEqual(self.journal()["demo/CHANGELOG.md"], released)
        self.wait.assert_not_called()

    def test_failed_upload_does_not_record_release(self):
        self.run_mock.return_value = subprocess.CompletedProcess([], 1)
        self.wait.side_effect = RuntimeError("not uploaded")
        with self.assertRaisesRegex(RuntimeError, "not uploaded"):
            ci.publish_package(self.pkg, self.state)
        self.assertFalse((self.state / "releases.json").exists())
        self.assertEqual(self.path.read_text(), CHANGES)

    def test_exhausted_rate_limit_retries_preserve_earlier_releases(self):
        ci.publish_package(self.pkg, self.state)
        earlier = self.journal()
        self.pkg["version"] = "1.0.1"
        self.run_mock.return_value = subprocess.CompletedProcess([], 101, stdout="status 429 Too Many Requests")
        self.wait.reset_mock()
        with patch.object(ci.time, "sleep"), self.assertRaisesRegex(RuntimeError, "rate-limited"):
            ci.publish_package(self.pkg, self.state)
        self.wait.assert_not_called()
        self.assertEqual(self.journal(), earlier)
        self.assertEqual(self.path.read_text(), CHANGES)

    def test_success_is_recorded_even_if_registry_poll_fails(self):
        self.wait.side_effect = RuntimeError("registry unavailable")
        with self.assertRaisesRegex(RuntimeError, "registry unavailable"):
            ci.publish_package(self.pkg, self.state)
        self.assertIn("demo/CHANGELOG.md", self.journal())
        self.assertEqual(self.path.read_text(), CHANGES)

    def test_nonzero_exit_after_upload_is_reconciled(self):
        self.run_mock.return_value = subprocess.CompletedProcess([], 1)
        with patch.object(ci, "published_changelog", side_effect=lambda *args: self.path.read_text()):
            ci.publish_package(self.pkg, self.state)
        self.assertIn("demo/CHANGELOG.md", self.journal())

    def test_conflicting_upload_is_not_journaled(self):
        self.run_mock.return_value = subprocess.CompletedProcess([], 1)
        with (
            patch.object(ci, "published_changelog", return_value="different notes"),
            self.assertRaisesRegex(RuntimeError, "different release notes"),
        ):
            ci.publish_package(self.pkg, self.state)
        self.assertFalse((self.state / "releases.json").exists())
        self.assertEqual(self.path.read_text(), CHANGES)

    def test_later_failure_preserves_earlier_release(self):
        ci.publish_package(self.pkg, self.state)
        earlier = self.journal()
        self.pkg["version"] = "1.0.1"
        self.run_mock.return_value = subprocess.CompletedProcess([], 1)
        self.wait.side_effect = RuntimeError("failed")
        with self.assertRaises(RuntimeError):
            ci.publish_package(self.pkg, self.state)
        self.assertEqual(self.journal(), earlier)

    def test_missing_secret_and_dirty_checkout_are_rejected(self):
        with patch.dict(os.environ, {"CARGO_REGISTRY_TOKEN": ""}), self.assertRaisesRegex(ValueError, "secret"):
            ci.publish(self.state)
        self.run_mock.return_value.stdout = " M demo/CHANGELOG.md"
        with (
            patch.dict(os.environ, {"CARGO_REGISTRY_TOKEN": "test-token"}),
            self.assertRaisesRegex(ValueError, "clean checkout"),
        ):
            ci.publish(self.state)


class CommitTests(unittest.TestCase):
    def test_only_confirmed_changelogs_are_committed_and_patch_is_saved(self):
        with tempfile.TemporaryDirectory() as directory, tempfile.TemporaryDirectory() as state_directory:
            root, state = Path(directory), Path(state_directory)
            previous = Path.cwd()
            try:
                os.chdir(root)
                subprocess.run(["git", "init", "--quiet"], check=True)
                subprocess.run(["git", "config", "user.name", "Test"], check=True)
                subprocess.run(["git", "config", "user.email", "test@example.invalid"], check=True)
                path = Path("CHANGELOG.md")
                path.write_text(CHANGES, encoding="utf-8")
                subprocess.run(["git", "add", "."], check=True)
                subprocess.run(["git", "commit", "--quiet", "-m", "Initial"], check=True)
                released = ci.release_changelog(CHANGES, "1.0.0", DATE)
                ci.save_release(state, path, released)
                ci.commit(state)
                self.assertEqual(path.read_text(), released)
                message = subprocess.run(
                    ["git", "log", "-1", "--format=%s"], check=True, capture_output=True, text=True
                ).stdout.strip()
                self.assertEqual(message, "Next cycle")
                self.assertIn(b"+## [1.0.0]", (state / "release.patch").read_bytes())
                ci.commit(state)
                self.assertEqual((state / "release.patch").read_bytes(), b"")
            finally:
                os.chdir(previous)

    def test_no_journal_means_no_commit(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(ci.subprocess, "run") as run:
            ci.commit(Path(directory))
            run.assert_not_called()

    def test_rejects_paths_outside_checkout(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            ci.save_release(state, Path("../CHANGELOG.md"), "unsafe")
            with self.assertRaisesRegex(ValueError, "Invalid changelog path"):
                ci.commit(state)


if __name__ == "__main__":
    unittest.main()
