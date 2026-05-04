#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
MANIFEST="$ROOT/demo/macroquad_sim/Cargo.toml"
ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Library/Android/sdk}}"
TARGET_SDK="$(awk '
  /^\[package\.metadata\.android\]/ { in_android=1; next }
  /^\[/ { if (in_android==1) in_android=0 }
  in_android && $1 ~ /^target_sdk_version/ {
    gsub(/[^0-9]/, "", $0);
    print $0;
    exit
  }
' "$MANIFEST")"
TARGET_SDK="${TARGET_SDK:-35}"
NDK_HOME="${NDK_HOME:-$ANDROID_SDK_ROOT/ndk/26.1.10909125}"
TOOLCHAIN_BIN="$NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin"
LLVM_LIB_BASE="$NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/lib/clang"
JAVA_SHIM_DIR="$ROOT/demo/macroquad_sim/.java8shim"
JAVA8_HOME="/Users/pingfanh/.sdkman/candidates/java/8.0.472-amzn"
BUILD_TOOLS_ROOT="$ANDROID_SDK_ROOT/build-tools"

find_sdkmanager() {
  local c1="$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager"
  local c2="$ANDROID_SDK_ROOT/tools/bin/sdkmanager"
  if [[ -x "$c1" ]]; then
    echo "$c1"
    return 0
  fi
  if [[ -x "$c2" ]]; then
    echo "$c2"
    return 0
  fi
  return 1
}

if ! cargo --list | rg -q "quad-apk"; then
  echo "[Mai2Chart] cargo-quad-apk not found. Installing..."
  cargo install cargo-quad-apk
fi

setup_java8_compat_shim() {
  if [[ -d "$JAVA8_HOME" ]]; then
    export JAVA_HOME="$JAVA8_HOME"
    export PATH="$JAVA_HOME/bin:$PATH"
    echo "[Mai2Chart] using Java8: $JAVA_HOME"
    return 0
  fi

  local java_bin javac_bin keytool_bin java_home rt_jar tmp_dir
  java_bin="$(command -v java || true)"
  javac_bin="$(command -v javac || true)"
  keytool_bin="$(command -v keytool || true)"
  if [[ -z "$java_bin" || -z "$javac_bin" || -z "$keytool_bin" ]]; then
    echo "[Mai2Chart] java/javac/keytool not found in PATH."
    return 1
  fi

  # If runtime already exposes rt.jar for old toolchain, no shim needed.
  if java -verbose -version 2>&1 | rg -q "rt\\.jar"; then
    return 0
  fi

  java_home="$(cd "$(dirname "$java_bin")/.." && pwd)"
  rt_jar="$JAVA_SHIM_DIR/rt.jar"
  mkdir -p "$JAVA_SHIM_DIR"

  if [[ ! -f "$rt_jar" ]]; then
    if [[ -x "$java_home/bin/jmod" && -x "$java_home/bin/jar" && -f "$java_home/jmods/java.base.jmod" ]]; then
      tmp_dir="$(mktemp -d)"
      "$java_home/bin/jmod" extract --dir "$tmp_dir" "$java_home/jmods/java.base.jmod" >/dev/null 2>&1 || true
      if [[ -d "$tmp_dir/classes" ]]; then
        (cd "$tmp_dir/classes" && "$java_home/bin/jar" cf "$rt_jar" .)
      fi
      rm -rf "$tmp_dir"
    fi
  fi

  if [[ ! -f "$rt_jar" ]]; then
    echo "[Mai2Chart] failed to create compatibility rt.jar"
    return 1
  fi

  cat > "$JAVA_SHIM_DIR/java" <<EOF
#!/usr/bin/env bash
if [[ "\${1:-}" == "-verbose" ]]; then
  echo "[Opened $rt_jar]"
fi
exec "$java_bin" "\$@"
EOF
  cat > "$JAVA_SHIM_DIR/javac" <<EOF
#!/usr/bin/env bash
exec "$javac_bin" "\$@"
EOF
  cat > "$JAVA_SHIM_DIR/keytool" <<EOF
#!/usr/bin/env bash
exec "$keytool_bin" "\$@"
EOF
  chmod +x "$JAVA_SHIM_DIR/java" "$JAVA_SHIM_DIR/javac" "$JAVA_SHIM_DIR/keytool"
  export PATH="$JAVA_SHIM_DIR:$PATH"
  echo "[Mai2Chart] enabled Java8-compat shim: $JAVA_SHIM_DIR"
}

echo "[Mai2Chart] ensure Android targets..."
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
echo "[Mai2Chart] target sdk from Cargo.toml: android-$TARGET_SDK"

if [[ -x "$TOOLCHAIN_BIN/llvm-ar" ]]; then
  for ar_name in \
    aarch64-linux-android-ar \
    armv7-linux-androideabi-ar \
    x86_64-linux-android-ar \
    i686-linux-android-ar
  do
    if [[ ! -e "$TOOLCHAIN_BIN/$ar_name" ]]; then
      ln -s llvm-ar "$TOOLCHAIN_BIN/$ar_name"
    fi
  done
fi

# cargo-quad-apk / older cargo-ndk helper may still expect legacy *-ld names.
if [[ -x "$TOOLCHAIN_BIN/ld.lld" ]]; then
  for ld_name in \
    aarch64-linux-android-ld \
    arm-linux-androideabi-ld \
    i686-linux-android-ld \
    x86_64-linux-android-ld
  do
    if [[ ! -e "$TOOLCHAIN_BIN/$ld_name" ]]; then
      ln -s ld.lld "$TOOLCHAIN_BIN/$ld_name"
    fi
  done
fi

# cargo-quad-apk may expect legacy binutils names that are absent in newer NDKs.
link_llvm_tool() {
  local legacy_name="$1"
  local llvm_name="$2"
  if [[ -x "$TOOLCHAIN_BIN/$llvm_name" && ! -e "$TOOLCHAIN_BIN/$legacy_name" ]]; then
    ln -s "$llvm_name" "$TOOLCHAIN_BIN/$legacy_name"
  fi
}

for triple in aarch64-linux-android arm-linux-androideabi i686-linux-android x86_64-linux-android; do
  link_llvm_tool "${triple}-readelf" "llvm-readelf"
  link_llvm_tool "${triple}-strip" "llvm-strip"
  link_llvm_tool "${triple}-objcopy" "llvm-objcopy"
  link_llvm_tool "${triple}-nm" "llvm-nm"
done

export NDK_HOME

# NDK r26 + legacy ld invocation may miss libunwind search path.
# Inject clang runtime lib dir so `-lunwind` resolves.
if [[ -d "$LLVM_LIB_BASE" ]]; then
  LIBUNWIND_DIR="$(find "$LLVM_LIB_BASE" -type d -path "*/lib/linux/aarch64" | head -n 1 || true)"
  if [[ -n "${LIBUNWIND_DIR:-}" ]]; then
    export RUSTFLAGS="${RUSTFLAGS:-} -Clink-arg=-L$LIBUNWIND_DIR"
    echo "[Mai2Chart] extra linker search path: $LIBUNWIND_DIR"
  fi
fi

if [[ ! -d "$ANDROID_SDK_ROOT/platforms/android-$TARGET_SDK" ]]; then
  echo "[Mai2Chart] Android platform $TARGET_SDK missing; trying to install..."
  if SDKMANAGER="$(find_sdkmanager)"; then
    yes | "$SDKMANAGER" --sdk_root="$ANDROID_SDK_ROOT" "platforms;android-$TARGET_SDK" "platform-tools" >/dev/null
    echo "[Mai2Chart] installed Android platform $TARGET_SDK."
  else
    echo "[Mai2Chart] sdkmanager not found."
    echo "Please install Android SDK command-line tools, then run:"
    echo "  sdkmanager --sdk_root=\"$ANDROID_SDK_ROOT\" \"platforms;android-$TARGET_SDK\" \"platform-tools\""
    exit 1
  fi
fi

setup_java8_compat_shim || true

ensure_dx_compat() {
  local latest_bt fallback_dx
  latest_bt="$(ls -1 "$BUILD_TOOLS_ROOT" 2>/dev/null | sort -V | tail -n 1 || true)"
  if [[ -z "$latest_bt" ]]; then
    return 0
  fi
  if [[ -x "$BUILD_TOOLS_ROOT/$latest_bt/dx" ]]; then
    return 0
  fi

  fallback_dx="$(find "$BUILD_TOOLS_ROOT" -maxdepth 2 -type f -name dx | sort -V | tail -n 1 || true)"
  if [[ -n "${fallback_dx:-}" ]]; then
    ln -sf "$fallback_dx" "$BUILD_TOOLS_ROOT/$latest_bt/dx"
    echo "[Mai2Chart] linked dx for build-tools/$latest_bt -> $fallback_dx"
  else
    echo "[Mai2Chart] warning: no legacy dx found in build-tools; build may fail."
  fi
}

ensure_dx_compat

echo "[Mai2Chart] building and running via cargo quad-apk (macroquad-compatible glue)..."
cargo quad-apk run --manifest-path "$MANIFEST"
