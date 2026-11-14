#!/usr/bin/env python3
"""Build and test the Android application on an explicitly selected device."""

import argparse
import json
import os
import time
import uuid
from pathlib import Path

import android_env as environment
import android_tools as tools
import build_apk

PROJECT = Path(__file__).resolve().parents[1]
APPLICATION = "io.github.jni_rs.jbindgen.testactivity"
EVENT_PREFIX = "ABOS_TEST_EVENT:"
SMOKE_TESTS = {"jni_callback", "jni_string", "jni_integer", "jni_array_map_iterator"}
SDK_TESTS = {
    "sdk_bluetooth": "sdk_bluetooth_resolution",
    "sdk_content_intent": "sdk_content_intent",
    "sdk_net_uri": "sdk_net_uri",
    "sdk_os_binder": "sdk_os_binder_resolution",
    "sdk_os_build": "sdk_os_build",
    "sdk_util_time_utils": "sdk_util_time_utils",
}
TARGETS = environment.TARGETS


def run(*args):
    return tools.execute(*args)


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
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--compile-only", action="store_true", help="check Rust bindings without linking or packaging")
    mode.add_argument("--package-only", action="store_true", help="cross-build and validate an APK without a device")
    parser.add_argument("--sdk-root", help="SDK directory; otherwise uses ANDROID_HOME or a user-owned default")
    parser.add_argument(
        "--target",
        choices=sorted(TARGETS.values()),
        default=environment.DEFAULT_TARGET,
        help="Rust target for no-device modes; device runs detect the ABI",
    )
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    features = selected_features(args, parser)
    if not (args.compile_only or args.package_only) and not args.serial:
        parser.error("select a device or running emulator using --serial or ANDROID_TEST_SERIAL")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    sdk = environment.sdk_root(args.sdk_root)
    environment.configure(sdk)
    if args.compile_only:
        environment.check_environment(sdk, args.target)
        print(run(*build_apk.native_command(args.target, features)))
        return
    if args.package_only:
        build_apk.package(sdk, args.target, features)
        return
    adb = str(environment.adb(sdk))
    device = [adb, "-s", args.serial]
    api = run(*device, "shell", "getprop", "ro.build.version.sdk").strip()
    minimum_api = 35 if features else 31
    if not api.isdecimal() or int(api) < minimum_api:
        parser.error(f"selected tests require device API {minimum_api} or newer, found {api!r}")
    abi = run(*device, "shell", "getprop", "ro.product.cpu.abi").strip()
    if abi not in TARGETS:
        parser.error(f"unsupported device ABI: {abi}")
    apk = build_apk.package(sdk, TARGETS[abi], features)
    print(run(*device, "install", "-r", str(apk)))
    run(*device, "shell", "am", "force-stop", APPLICATION)
    run_id = uuid.uuid4().hex
    print(
        run(
            *device,
            "shell",
            "am",
            "start",
            "-W",
            "-n",
            f"{APPLICATION}/.TestActivity",
            "--es",
            "abos.test.run-id",
            run_id,
        )
    )
    expected = SMOKE_TESTS | {SDK_TESTS[feature] for feature in features}
    print(f"Selected {len(expected)} tests; SDK features: {', '.join(features) or 'none (smoke only)'}")
    deadline = time.monotonic() + args.timeout
    while time.monotonic() < deadline:
        log = run(*device, "logcat", "-d", "-s", "TestActivity:I")
        report = read_report(log, run_id, expected)
        if report is not None:
            print(
                f"Android JNI tests passed: {report['executed']} executed, {report['failed']} failed, {report['skipped']} skipped."
            )
            return
        time.sleep(1)
    raise RuntimeError("Android tests did not report success before the timeout")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError) as error:
        raise SystemExit(str(error)) from error
