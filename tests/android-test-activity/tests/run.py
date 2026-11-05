#!/usr/bin/env python3
"""Build and test the Android application on an explicitly selected device."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import time
import uuid


PROJECT = Path(__file__).resolve().parents[1]
APPLICATION = "io.github.jni_rs.jbindgen.testactivity"
EVENT_PREFIX = "ABOS_TEST_EVENT:"
SMOKE_TESTS = {"jni_callback", "jni_string", "jni_integer"}
SDK_TESTS = {
    "sdk_bluetooth": "sdk_bluetooth_resolution",
    "sdk_content_intent": "sdk_content_intent",
    "sdk_net_uri": "sdk_net_uri",
    "sdk_os_binder": "sdk_os_binder_resolution",
    "sdk_os_build": "sdk_os_build",
    "sdk_util_time_utils": "sdk_util_time_utils",
}
TARGETS = {
    "arm64-v8a": "aarch64-linux-android",
    "armeabi-v7a": "armv7-linux-androideabi",
    "x86_64": "x86_64-linux-android",
    "x86": "i686-linux-android",
}


def run(*args):
    return subprocess.run(args, cwd=PROJECT, check=True, text=True, capture_output=True, timeout=600).stdout


def selected_features(args, parser):
    if args.features is not None:
        features = args.features.split(",")
        if any(feature not in SDK_TESTS for feature in features) or len(set(features)) != len(features):
            parser.error("--features must be a comma-separated list of unique SDK feature names")
        return sorted(features)
    return sorted(SDK_TESTS) if args.suite == "full" else []


def read_report(log, run_id, expected):
    """Accept only a complete, internally consistent report for this exact run."""
    results = {}
    summary = None
    for line in log.splitlines():
        if EVENT_PREFIX not in line:
            continue
        try:
            event = json.loads(line.partition(EVENT_PREFIX)[2])
        except json.JSONDecodeError:
            # Old or truncated log entries have no trustworthy run identity.
            # A current run still needs every named result and a valid summary.
            continue
        if not isinstance(event, dict) or event.get("run_id") != run_id:
            continue
        if event.get("status") == "failed":
            raise RuntimeError(f"Android test failed: {event}")
        if event.get("type") == "result":
            name = event.get("name")
            if not isinstance(name, str) or name not in expected or name in results or event.get("status") != "passed":
                raise RuntimeError(f"Unexpected Android test result: {event}")
            results[name] = event
        elif event.get("type") == "summary":
            if summary is not None:
                raise RuntimeError("Duplicate Android test summary")
            summary = event
        else:
            raise RuntimeError(f"Unknown Android test event: {event}")
    if summary is None:
        return None
    counts = {"selected": len(expected), "executed": len(expected), "passed": len(expected), "failed": 0, "skipped": 0}
    if summary.get("status") != "passed" or set(results) != expected:
        raise RuntimeError(f"Incomplete Android test report: {summary}")
    if any(type(summary.get(key)) is not int or summary[key] != value for key, value in counts.items()):
        raise RuntimeError(f"Incorrect Android test counts: {summary}")
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", default=os.environ.get("ANDROID_TEST_SERIAL"))
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument("--suite", choices=["smoke", "full"], default="full")
    selection.add_argument("--features", help="comma-separated SDK feature names; always includes JNI smoke tests")
    parser.add_argument("--compile-only", action="store_true", help="check Rust bindings without installing an APK")
    parser.add_argument("--target", choices=sorted(TARGETS.values()), default="aarch64-linux-android",
                        help="Rust target for --compile-only; device runs detect the ABI")
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    features = selected_features(args, parser)
    if not args.compile_only and not args.serial:
        parser.error("select a device or running emulator using --serial or ANDROID_TEST_SERIAL")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if not sdk:
        parser.error("set ANDROID_HOME or ANDROID_SDK_ROOT")
    feature_args = ["--no-default-features"]
    if features:
        feature_args.extend(["--features", ",".join(features)])
    if args.compile_only:
        print(run("cargo", "ndk", "-t", args.target, "-p", "31", "check", *feature_args))
        return
    adb = str(Path(sdk) / "platform-tools" / ("adb.exe" if os.name == "nt" else "adb"))
    device = [adb, "-s", args.serial]
    api = run(*device, "shell", "getprop", "ro.build.version.sdk").strip()
    minimum_api = 35 if features else 31
    if not api.isdecimal() or int(api) < minimum_api:
        parser.error(f"selected tests require device API {minimum_api} or newer, found {api!r}")
    abi = run(*device, "shell", "getprop", "ro.product.cpu.abi").strip()
    if abi not in TARGETS:
        parser.error(f"unsupported device ABI: {abi}")
    # Match the native library's minimum API to the APK's minSdk.
    print(run("cargo", "ndk", "-t", TARGETS[abi], "-p", "31", "-o", "app/src/main/jniLibs", "build", *feature_args))
    wrapper = PROJECT / ("gradlew.bat" if os.name == "nt" else "gradlew")
    print(run(str(wrapper), "--no-daemon", ":app:assembleDebug"))
    apk = PROJECT / "app/build/outputs/apk/debug/app-debug.apk"
    print(run(*device, "install", "-r", str(apk)))
    run(*device, "shell", "am", "force-stop", APPLICATION)
    run_id = uuid.uuid4().hex
    print(run(*device, "shell", "am", "start", "-W", "-n", f"{APPLICATION}/.TestActivity", "--es", "abos.test.run-id", run_id))
    expected = SMOKE_TESTS | {SDK_TESTS[feature] for feature in features}
    print(f"Selected {len(expected)} tests; SDK features: {', '.join(features) or 'none (smoke only)'}")
    deadline = time.monotonic() + args.timeout
    while time.monotonic() < deadline:
        log = run(*device, "logcat", "-d", "-s", "TestActivity:I")
        report = read_report(log, run_id, expected)
        if report is not None:
            print(f"Android JNI tests passed: {report['executed']} executed, {report['failed']} failed, {report['skipped']} skipped.")
            return
        time.sleep(1)
    raise RuntimeError("Android tests did not report success before the timeout")


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        raise SystemExit(f"Command failed: {error.cmd}\n{error.stdout}\n{error.stderr}") from error
