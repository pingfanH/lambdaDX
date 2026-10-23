#!/usr/bin/env python3
"""Build the standalone ARM64 Android player with its offline resources."""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import zipfile


ROOT = Path(__file__).resolve().parents[1]


def prepare_assets():
    destination = ROOT / "target/android-player-assets"
    staging = destination.with_name("android-player-assets.tmp")
    if staging.exists():
        shutil.rmtree(staging)
    staging.mkdir(parents=True)
    files = ["Arial.ttf", "generated_chart.json", "note_params.json", "pad.svg", "slide.svg"]
    for name in files:
        shutil.copy2(ROOT / "assets" / name, staging / name)
    for name in ["Sfx", "Skins", "ui", "player_ui", "notes", "effects", "extracted", "charts"]:
        shutil.copytree(
            ROOT / "assets" / name, staging / name,
            ignore=shutil.ignore_patterns(".DS_Store", "*.mp4", "*.fla", "*.swf", "*.cache"),
        )
    digest = hashlib.sha256()
    total = 0
    for path in sorted(staging.rglob("*")):
        if path.is_file():
            data = path.read_bytes()
            total += len(data)
            digest.update(path.relative_to(staging).as_posix().encode())
            digest.update(b"\0")
            digest.update(hashlib.sha256(data).digest())
    (staging / "android-assets.version").write_text(digest.hexdigest())
    if destination.exists():
        shutil.rmtree(destination)
    staging.rename(destination)
    print(f"Android resources: {total / 1024**2:.1f} MiB (videos excluded)", flush=True)


def build_environment():
    env = os.environ.copy()
    sdk = next((Path(env[key]) for key in ["ANDROID_SDK_HOME", "ANDROID_HOME", "ANDROID_SDK_ROOT"] if env.get(key)),
               Path.home() / "Library/Android/sdk")
    if not sdk.is_dir():
        raise RuntimeError("Set ANDROID_HOME to an installed Android SDK (platform 35 required).")
    # cargo-quad-apk 0.1.4 expects JDK 8's rt.jar for javac, while recent
    # Android build-tools ship D8 bytecode that requires JDK 11+. Keep the
    # packager's Java 8 environment and route only D8 through JDK 21 by using a
    # small SDK overlay. The real SDK and NDK remain untouched.
    sdk = prepare_sdk_overlay(sdk, env)
    env["ANDROID_HOME"] = str(sdk)
    env["ANDROID_SDK_HOME"] = str(sdk)
    env["ANDROID_SDK_ROOT"] = str(sdk)
    ndk = env.get("ANDROID_NDK_HOME") or env.get("ANDROID_NDK_ROOT")
    if not ndk:
        ndks = sorted((sdk / "ndk").glob("*"), key=lambda p: [int(n) for n in p.name.split(".")])
        if not ndks:
            raise RuntimeError("Install Android NDK and set ANDROID_NDK_HOME.")
        ndk = str(ndks[-1])
    env["ANDROID_NDK_HOME"] = ndk
    # cargo-quad-apk 0.1.4 compiles Java 7 bytecode using JDK 8's rt.jar.
    java = env.get("LDX_ANDROID_JAVA_HOME")
    if not java:
        java8 = sorted((Path.home() / ".sdkman/candidates/java").glob("8.*"))
        java = str(java8[-1]) if java8 else env.get("JAVA_HOME")
    if java:
        env["JAVA_HOME"] = java
        env["PATH"] = str(Path(java) / "bin") + os.pathsep + env.get("PATH", "")
    wrapper = ROOT / "tools/android/rustc-wrapper.py"
    wrapper.chmod(wrapper.stat().st_mode | 0o111)
    if env.get("RUSTC_WRAPPER") and env["RUSTC_WRAPPER"] != str(wrapper):
        env["LDX_ANDROID_RUSTC_WRAPPER"] = env["RUSTC_WRAPPER"]
    env["RUSTC_WRAPPER"] = str(wrapper)
    return env


def prepare_sdk_overlay(source: Path, env: dict[str, str]) -> Path:
    overlay = Path("/private/tmp/lambdadx-android-sdk")
    if overlay.exists():
        shutil.rmtree(overlay)
    (overlay / "build-tools" / "36.1.0").mkdir(parents=True)
    (overlay / "platforms").symlink_to(source / "platforms", target_is_directory=True)
    build_tools = source / "build-tools" / "36.1.0"
    overlay_tools = overlay / "build-tools" / "36.1.0"
    for path in build_tools.iterdir():
        if path.name != "d8":
            (overlay_tools / path.name).symlink_to(path)
    real_d8 = build_tools / "lib" / "d8.jar"
    d8_java = next((Path.home() / ".sdkman/candidates/java").glob("21.*"), None)
    if d8_java is None:
        raise RuntimeError("JDK 21 is required for the Android D8 tool.")
    d8_wrapper = overlay_tools / "d8"
    d8_wrapper.write_text(
        "#!/bin/sh\n"
        f'exec "{d8_java / "bin/java"}" -cp "{real_d8}" '
        "com.android.tools.r8.D8 \"$@\"\n"
    )
    d8_wrapper.chmod(0o755)
    return overlay


def package_apk_with_android_runtime(apk: Path, env: dict[str, str]) -> None:
    """Add the NDK's shared C++ runtime and sign the final aligned APK."""
    ndk = Path(env["ANDROID_NDK_HOME"])
    runtime = (
        ndk
        / "toolchains/llvm/prebuilt/darwin-x86_64/sysroot/usr/lib"
        / "aarch64-linux-android/libc++_shared.so"
    )
    if not runtime.is_file():
        raise RuntimeError(f"Android C++ runtime was not found: {runtime}")

    build_tools = Path(env["ANDROID_HOME"]) / "build-tools" / "36.1.0"
    zipalign = build_tools / "zipalign"
    apksigner = build_tools / "apksigner"
    keystore = Path.home() / ".android/debug.keystore"
    if not zipalign.is_file() or not apksigner.is_file():
        raise RuntimeError(f"Android packaging tools were not found under {build_tools}")
    if not keystore.is_file():
        raise RuntimeError(f"Android debug keystore was not found: {keystore}")

    with_runtime = apk.with_name(apk.stem + ".with-runtime.apk")
    aligned = apk.with_name(apk.stem + ".aligned.apk")
    signed = apk.with_name(apk.stem + ".signed.apk")
    for temporary in (with_runtime, aligned, signed):
        temporary.unlink(missing_ok=True)

    # cargo-quad-apk's unsigned output has no signature entries. Rebuild the
    # archive with the runtime, align it, and sign that exact final archive.
    with zipfile.ZipFile(apk, "r") as source, zipfile.ZipFile(
        with_runtime, "w", compression=zipfile.ZIP_DEFLATED
    ) as destination:
        names = set()
        for info in source.infolist():
            names.add(info.filename)
            destination.writestr(info, source.read(info.filename))
        if "lib/arm64-v8a/libc++_shared.so" not in names:
            destination.write(
                runtime,
                "lib/arm64-v8a/libc++_shared.so",
                compress_type=zipfile.ZIP_STORED,
            )

    subprocess.run([str(zipalign), "-f", "4", str(with_runtime), str(aligned)], check=True)
    subprocess.run(
        [
            str(apksigner),
            "sign",
            "--ks",
            str(keystore),
            "--ks-key-alias",
            "androiddebugkey",
            "--ks-pass",
            "pass:android",
            "--key-pass",
            "pass:android",
            "--out",
            str(signed),
            str(aligned),
        ],
        check=True,
    )
    shutil.move(signed, apk)
    for temporary in (with_runtime, aligned):
        temporary.unlink(missing_ok=True)


def verify_game_classification(apk: Path, env: dict[str, str]) -> None:
    aapt = Path(env["ANDROID_HOME"]) / "build-tools" / "36.1.0" / "aapt"
    result = subprocess.run(
        [str(aapt), "dump", "badging", str(apk)],
        env=env, check=True, capture_output=True, text=True,
    )
    if "application-isGame" not in result.stdout.splitlines():
        raise RuntimeError(
            "Final APK is not marked as an Android game; check "
            "package.metadata.android.application_attributes in Cargo.toml."
        )
    print("Android game classification: verified in the final APK", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target/android-player")
    parser.add_argument("--debug", action="store_true")
    args = parser.parse_args()
    env = build_environment()
    prepare_assets()
    command = ["cargo", "quad-apk", "--nosign", "build", "--locked", "--bin", "lambda_dx_player_ui",
               "--target", "aarch64-linux-android", "--target-dir", str(args.target_dir.resolve())]
    if not args.debug:
        command.append("--release")
    subprocess.run(command, cwd=ROOT, env=env, check=True)
    mode = "debug" if args.debug else "release"
    apk = args.target_dir.resolve() / "android-artifacts" / mode / "apk/lambda_dx_player_ui.apk"
    if not apk.is_file():
        raise RuntimeError(f"Build did not produce {apk}")
    package_apk_with_android_runtime(apk, env)
    verify_game_classification(apk, env)
    output = ROOT / "target/android/lambda_dx_player_ui.apk"
    output.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(apk, output)
    print(f"APK: {output} ({output.stat().st_size / 1024**2:.1f} MiB)", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Android build failed: {error}", file=sys.stderr)
        sys.exit(1)
