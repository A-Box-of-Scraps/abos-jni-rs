#!/usr/bin/env python3
"""Install or validate the pinned Android build environment without Android Studio."""

import argparse
import os
import platform
import re
import sys
import tempfile
from pathlib import Path

import android_tools as tools

API = 35
MIN_API = 31
BUILD_TOOLS = "36.0.0"
NDK = "28.2.13676358"
PLATFORM_TOOLS = "37.0.1"
DEFAULT_TARGET = "aarch64-linux-android"
TARGETS = {
    "arm64-v8a": "aarch64-linux-android",
    "armeabi-v7a": "armv7-linux-androideabi",
    "x86_64": "x86_64-linux-android",
    "x86": "i686-linux-android",
}
PACKAGES = {
    f"platforms/android-{API}": "2.0.0",
    f"build-tools/{BUILD_TOOLS}": BUILD_TOOLS,
    f"ndk/{NDK}": NDK,
    "platform-tools": PLATFORM_TOOLS,
}
TERMS = "https://developer.android.com/studio/terms"


def supported_host(system=None, machine=None):
    system = system or platform.system()
    machine = (machine or platform.machine()).lower()
    x64 = machine in {"x86_64", "amd64"}
    arm64 = machine in {"arm64", "aarch64"}
    if system == "Linux" and x64:
        return "linux-x86_64"
    if system == "Darwin" and (x64 or arm64):
        # The NDK uses this directory name on both Intel and Apple Silicon Macs.
        return "darwin-x86_64"
    if system == "Windows" and x64:
        return "windows-x86_64"
    raise RuntimeError(
        f"Unsupported Android build host: {system}/{machine}. "
        "Supported: Linux x86-64, macOS Intel/Apple Silicon, Windows x86-64."
    )


def sdk_root(explicit=None):
    if explicit:
        return Path(explicit).expanduser().resolve()
    home = os.environ.get("ANDROID_HOME")
    legacy = os.environ.get("ANDROID_SDK_ROOT")
    if home and legacy and Path(home).resolve() != Path(legacy).resolve():
        raise RuntimeError("ANDROID_HOME and ANDROID_SDK_ROOT disagree; set one SDK or use --sdk-root.")
    configured_root = home or legacy
    if configured_root:
        return Path(configured_root).expanduser().resolve()
    if platform.system() == "Windows":
        local_app_data = os.environ.get("LOCALAPPDATA")
        base = Path(local_app_data) if local_app_data else Path.home() / "AppData/Local"
    elif platform.system() == "Darwin":
        base = Path.home() / "Library/Application Support"
    else:
        data_home = os.environ.get("XDG_DATA_HOME")
        base = Path(data_home) if data_home else Path.home() / ".local/share"
    return (base / "abos/android-sdk").resolve()


def configure(sdk):
    # These changes affect only this process and its children, never shell profiles.
    os.environ["ANDROID_HOME"] = str(sdk)
    os.environ["ANDROID_SDK_ROOT"] = str(sdk)
    os.environ["ANDROID_NDK_HOME"] = str(sdk / "ndk" / NDK)
    os.environ["ANDROID_NDK_ROOT"] = str(sdk / "ndk" / NDK)


def installed_revision(directory):
    properties = directory / "source.properties"
    if not properties.is_file():
        return None
    for line in properties.read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        if separator and key.strip() == "Pkg.Revision":
            return value.strip()
    return None


def revision_tuple(value):
    parts = [int(part) for part in value.split(".")]
    while len(parts) > 1 and parts[-1] == 0:
        parts.pop()
    return tuple(parts)


def missing_packages(sdk):
    missing = []
    for package, version in PACKAGES.items():
        actual = installed_revision(sdk / package)
        if actual is None:
            missing.append(f"{package}@{version}")
        elif revision_tuple(actual) != revision_tuple(version):
            raise RuntimeError(
                f"{sdk / package} has revision {actual}; expected {version}. "
                "Use a separate --sdk-root to avoid changing a shared SDK."
            )
    return missing


def require_file(path):
    if not path.is_file():
        raise RuntimeError(f"Missing Android tool/file: {path}. Run mise run setup:android.")
    return path


def build_tool(sdk, name):
    return require_file(sdk / "build-tools" / BUILD_TOOLS / tools.executable_name(name))


def adb(sdk):
    return require_file(sdk / "platform-tools" / tools.executable_name("adb"))


def check_host(probe_linker=False):
    supported_host()
    for name in ["cargo", "rustc", "rustup", "cargo-ndk"]:
        tools.executable(name)
    tools.java_tool("javac")
    tools.execute(tools.java_tool(), "-version")
    if probe_linker:
        # rustc selects the correct host linker (including MSVC). Do not guess a
        # compiler name or silently install privileged OS packages.
        with tempfile.TemporaryDirectory(prefix="abos-host-check-") as temporary:
            source = Path(temporary) / "main.rs"
            source.write_text("fn main() {}\n", encoding="utf-8")
            try:
                tools.execute("rustc", str(source), "-o", str(Path(temporary) / "probe"))
            except RuntimeError as error:
                raise RuntimeError(
                    "Host Rust linking failed. Install the OS C/C++ toolchain "
                    "(MSVC Build Tools on Windows, Xcode Command Line Tools on macOS, "
                    f"or a C compiler/linker on Linux).\n{error}"
                ) from error


def check_environment(sdk, target=DEFAULT_TARGET, probe_linker=False):
    check_host(probe_linker)
    missing = missing_packages(sdk)
    if missing:
        raise RuntimeError(f"Missing SDK packages: {', '.join(missing)}. Run mise run setup:android.")
    require_file(sdk / f"platforms/android-{API}/android.jar")
    require_file(sdk / f"build-tools/{BUILD_TOOLS}/lib/apksigner.jar")
    for name in ["zipalign", "aapt2"]:
        build_tool(sdk, name)
    host = supported_host()
    require_file(sdk / f"ndk/{NDK}/toolchains/llvm/prebuilt/{host}/bin" / tools.executable_name("clang"))
    adb(sdk)
    if target:
        installed = tools.execute("rustup", "target", "list", "--installed").splitlines()
        if target not in installed:
            raise RuntimeError(f"Missing Rust target {target}; run mise run setup:android -- --target {target}.")
    local = tools.PROJECT / "local.properties"
    if local.is_file() and re.search(r"^\s*sdk\.dir\s*[=:]", local.read_text(encoding="utf-8"), re.MULTILINE):
        raise RuntimeError(f"Remove sdk.dir from {local}; this runner supplies the selected SDK through ANDROID_HOME.")


def confirm_licenses(accepted):
    if accepted:
        return
    print(f"Installing Android SDK packages requires accepting the SDK license: {TERMS}")
    if not sys.stdin.isatty():
        raise RuntimeError(
            "Run setup interactively to review the license, or pass --accept-licenses "
            "only after you have reviewed and accepted it."
        )
    if input("After reviewing the license, type 'accept' to install: ").strip() != "accept":
        raise RuntimeError("SDK installation cancelled; license not accepted.")


def setup(sdk, targets, accepted=False):
    check_host(probe_linker=True)
    missing = missing_packages(sdk)
    if missing:
        confirm_licenses(accepted)
        tools.executable("android")
        sdk.mkdir(parents=True, exist_ok=True)
        for package in missing:
            # Pin revisions, disable CLI telemetry, and leave unrelated packages alone.
            tools.execute(
                "android",
                "--no-metrics",
                f"--sdk={sdk}",
                "sdk",
                "--ignore-outdated-xmls",
                "install",
                package,
                capture=False,
                timeout=1800,
            )
    for target in targets:
        tools.execute("rustup", "target", "add", target, capture=False)
        check_environment(sdk, target)
    print(f"Android environment ready: {sdk}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["setup", "check"])
    parser.add_argument("--sdk-root", help="SDK directory; otherwise uses ANDROID_HOME or a user-owned default")
    parser.add_argument("--target", action="append", choices=sorted(TARGETS.values()))
    parser.add_argument(
        "--accept-licenses", action="store_true", help="affirm that you reviewed and accepted the Android SDK license"
    )
    args = parser.parse_args()
    sdk = sdk_root(args.sdk_root)
    configure(sdk)
    targets = args.target or [DEFAULT_TARGET]
    if args.action == "setup":
        setup(sdk, targets, args.accept_licenses)
    else:
        for target in targets:
            check_environment(sdk, target, probe_linker=True)
        print(f"Android environment ready: {sdk}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, ValueError) as error:
        raise SystemExit(str(error)) from error
