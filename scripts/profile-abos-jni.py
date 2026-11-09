#!/usr/bin/env python3
"""Record a JNI or JVMTI Criterion benchmark as a CPU flamegraph using perf, blondie, or xctrace."""

import argparse
import os
import platform
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def positive_seconds(value):
    seconds = int(value)
    if seconds <= 0:
        raise argparse.ArgumentTypeError("duration must be a positive number of seconds")
    return seconds


def backend_options(env):
    system = platform.system()
    if system == "Linux":
        if shutil.which(env.get("PERF", "perf")) is None:
            raise RuntimeError("perf is required. Install your distribution's perf package.")
        # cycles:u filters samples, but callchain collection also needs to exclude the kernel.
        # This keeps recording compatible with perf_event_paranoid=2.
        return ["--cmd", "record -e cycles:u -F 997 --call-graph dwarf,64000 --user-callchains -g -o perf.data"]
    if system == "Windows":
        # In flamegraph 0.6.15, a failed DTrace spawn selects bundled blondie.
        # An empty executable name ensures an installed DTrace cannot take precedence.
        env["DTRACE"] = ""
        return []
    if system == "Darwin":
        if not env.get("XCTRACE"):
            if shutil.which("xcrun") is None:
                raise RuntimeError("xctrace requires Xcode and its selected developer tools (xcrun).")
            result = subprocess.run(["xcrun", "--find", "xctrace"], capture_output=True, text=True, check=False)
            if result.returncode or not result.stdout.strip():
                raise RuntimeError("xctrace was not found. Install Xcode and select its developer directory.")
            env["XCTRACE"] = result.stdout.strip()
        return []
    raise RuntimeError(f"Unsupported profiling platform: {system}. Supported platforms: Linux, Windows, macOS.")


def profile(root, benchmark_filter, seconds, package="abos-jni"):
    env = os.environ.copy()
    env["CARGO_PROFILE_BENCH_DEBUG"] = "true"
    options = backend_options(env)
    output = root / "target" / "profiling"
    output.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "flamegraph",
        "--manifest-path",
        str(root / "Cargo.toml"),
        "--package",
        package,
        "--features",
        "invocation",
        "--bench",
        "api_calls",
        "--output",
        str(output / f"{package}.svg"),
        *options,
        "--",
        "--bench",
        benchmark_filter,
        "--profile-time",
        str(seconds),
    ]
    # xctrace and blondie use fixed filenames in the working directory.
    # A separate directory per run avoids stale traces and concurrent raw-file collisions.
    recording = tempfile.mkdtemp(prefix="recording-", dir=output)
    return subprocess.run(command, cwd=recording, env=env, check=False).returncode


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", choices=["abos-jni", "abos-jvmti"], default="abos-jni")
    parser.add_argument(
        "filter",
        nargs="?",
        default=os.environ.get("usage_filter"),
        help="Criterion benchmark filter",
    )
    parser.add_argument(
        "--seconds",
        type=positive_seconds,
        default=os.environ.get("usage_seconds", "15"),
        help="profiling duration per matching benchmark",
    )
    args = parser.parse_args(argv)
    if args.filter is None:
        args.filter = "^jvmti/phase/wrapper$" if args.package == "abos-jvmti" else "^jni_call_static_abs_method_safe$"
    root = Path(__file__).resolve().parent.parent
    try:
        return profile(root, args.filter, args.seconds, package=args.package)
    except (OSError, RuntimeError) as error:
        interface = "JVMTI" if args.package == "abos-jvmti" else "JNI"
        print(f"Unable to profile {interface} benchmark: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
