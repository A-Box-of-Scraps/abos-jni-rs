#!/usr/bin/env python3
"""Build and test the Android application on an explicitly selected device."""

import argparse
import os
from pathlib import Path
import subprocess
import time
import uuid


PROJECT = Path(__file__).resolve().parents[1]
APPLICATION = "io.github.jni_rs.jbindgen.testactivity"
TARGETS = {
    "arm64-v8a": "aarch64-linux-android",
    "armeabi-v7a": "armv7-linux-androideabi",
    "x86_64": "x86_64-linux-android",
    "x86": "i686-linux-android",
}


def run(*args):
    return subprocess.run(args, cwd=PROJECT, check=True, text=True, capture_output=True, timeout=600).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", default=os.environ.get("ANDROID_TEST_SERIAL"))
    parser.add_argument("--features", default="")
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    if not args.serial:
        parser.error("select a device or running emulator using --serial or ANDROID_TEST_SERIAL")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if not sdk:
        parser.error("set ANDROID_HOME or ANDROID_SDK_ROOT")
    adb = str(Path(sdk) / "platform-tools" / ("adb.exe" if os.name == "nt" else "adb"))
    device = [adb, "-s", args.serial]
    abi = run(*device, "shell", "getprop", "ro.product.cpu.abi").strip()
    if abi not in TARGETS:
        parser.error(f"unsupported device ABI: {abi}")
    build = ["cargo", "ndk", "-t", TARGETS[abi], "-o", "app/src/main/jniLibs", "build"]
    if args.features:
        build.extend(["--features", args.features])
    print(run(*build))
    wrapper = PROJECT / ("gradlew.bat" if os.name == "nt" else "gradlew")
    print(run(str(wrapper), "--no-daemon", ":app:assembleDebug"))
    apk = PROJECT / "app/build/outputs/apk/debug/app-debug.apk"
    print(run(*device, "install", "-r", str(apk)))
    run(*device, "shell", "am", "force-stop", APPLICATION)
    run_id = uuid.uuid4().hex
    print(run(*device, "shell", "am", "start", "-W", "-n", f"{APPLICATION}/.TestActivity", "--es", "abos.test.run-id", run_id))
    deadline = time.monotonic() + args.timeout
    while time.monotonic() < deadline:
        log = run(*device, "logcat", "-d", "-s", "TestActivity:I")
        if f"ABOS_TEST_FAILED:{run_id}" in log:
            raise RuntimeError(log)
        if f"ABOS_TEST_PASSED:{run_id}" in log:
            print("Android JNI smoke tests passed.")
            return
        time.sleep(1)
    raise RuntimeError("Android tests did not report success before the timeout")


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        raise SystemExit(f"Command failed: {error.cmd}\n{error.stdout}\n{error.stderr}") from error
