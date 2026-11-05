"""Cross-build the JNI library and validate an APK without a device."""
from pathlib import Path
import shutil
import struct
import tempfile
import zipfile

import android_env as environment
import android_tools as tools


# ELF class and e_machine for Android's supported ABIs.
ELF_TARGETS = {"arm64-v8a": (2, 183), "armeabi-v7a": (1, 40), "x86_64": (2, 62), "x86": (1, 3)}


def feature_arguments(features):
    arguments = ["--locked", "--no-default-features"]
    if features:
        arguments.extend(["--features", ",".join(features)])
    return arguments


def native_command(target, features, output=None):
    command = ["cargo", "ndk", "-t", target, "-P", str(environment.MIN_API)]
    if output is not None:
        command.extend(["-o", str(output), "build"])
    else:
        command.append("check")
    return [*command, *feature_arguments(features)]


def verify_apk_contents(apk, abi):
    expected = f"lib/{abi}/libtestactivity.so"
    with zipfile.ZipFile(apk) as archive:
        names = archive.namelist()
        native = [name for name in names if name.startswith("lib/") and name.endswith(".so")]
        if native != [expected]:
            raise RuntimeError(f"APK must contain exactly {expected}; found {native}")
        for name in ["AndroidManifest.xml", "classes.dex"]:
            if names.count(name) != 1 or archive.getinfo(name).file_size == 0:
                raise RuntimeError(f"APK is missing a valid {name}")
        with archive.open(expected) as library:
            header = library.read(64)
        elf_class, machine = ELF_TARGETS[abi]
        if (len(header) < 52 or header[:4] != b"\x7fELF" or header[4] != elf_class or header[5] != 1
                or struct.unpack_from("<HH", header, 16) != (3, machine)):
            raise RuntimeError(f"APK native library is not an Android {abi} ELF shared library")


def validate_apk(apk, abi, sdk):
    verify_apk_contents(apk, abi)
    tools.execute(environment.build_tool(sdk, "zipalign"), "-c", "-P", "16", "-v", "4", apk)
    signer = environment.require_file(sdk / f"build-tools/{environment.BUILD_TOOLS}/lib/apksigner.jar")
    tools.execute(tools.java_tool(), "-jar", signer, "verify", "--verbose", apk)


def package(sdk, target, features):
    environment.check_environment(sdk, target)
    abi = next(name for name, triple in environment.TARGETS.items() if triple == target)
    build = tools.PROJECT / "build"
    build.mkdir(parents=True, exist_ok=True)
    # An isolated jniLibs input prevents old ABIs or libraries from another suite
    # being accidentally packaged. Gradle's normal outputs remain incremental.
    with tempfile.TemporaryDirectory(prefix="jni-", dir=build) as temporary:
        libraries = Path(temporary)
        print(tools.execute(*native_command(target, features, libraries), capture=False, timeout=1800))
        environment.require_file(libraries / abi / "libtestactivity.so")
        print(tools.execute(*tools.gradle_command(f"-PabosJniLibs={libraries}", ":app:assembleDebug"), capture=False, timeout=1800))
        apk = environment.require_file(tools.PROJECT / "app/build/outputs/apk/debug/app-debug.apk")
        validate_apk(apk, abi, sdk)
        destination = build / "test-apks" / abi / "app-debug.apk"
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(apk, destination)
    print(f"Validated APK ({abi}): {destination}")
    return destination
