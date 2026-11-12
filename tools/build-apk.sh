#!/usr/bin/env bash
# Build, align and sign the QuotaVPN Android APK.
# Usage: bash tools/build-apk.sh
# Output: android/QuotaVPN-mobile-signed.apk (+ SIGN_VERIFY_OK)
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Toolchain discovery. Every value below is still an override; each falls back to a
# search rather than to one machine's layout. This script used to hardcode
# C:/Android/Sdk, a jdk-23 that is not installed on the build host, and a hermes/node
# directory, so a fresh machine needed four exported variables just to start.
for p in "/c/Tools/mingw_extract/mingw64/bin" "/c/Tools/dllshim" "/c/Tools/node" \
         "/c/Users/Administrator/AppData/Local/hermes/node" "$HOME/.cargo/bin"; do
  [ -d "$p" ] && PATH="$p:$PATH"
done
export PATH

if [ -z "${ANDROID_HOME:-}" ]; then
  for c in "C:/Android/Sdk" "C:/Android" "$HOME/Android/Sdk"; do
    [ -d "$c" ] && { ANDROID_HOME="$c"; break; }
  done
fi
export ANDROID_HOME="${ANDROID_HOME:?Android SDK not found; set ANDROID_HOME}"
export ANDROID_SDK_ROOT="$ANDROID_HOME"

# The NDK version this project pins, or the newest one installed.
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  NDK_DIR="$ANDROID_HOME/ndk/28.2.13676358"
  [ -d "$NDK_DIR" ] || NDK_DIR="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1)"
  ANDROID_NDK_HOME="$NDK_DIR"
fi
export ANDROID_NDK_HOME
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"

# Any installed JDK, newest by name: the Android Gradle plugin wants 17 or newer.
if [ -z "${JAVA_HOME:-}" ]; then
  for j in "/c/Program Files/Microsoft"/jdk-* "/c/Program Files/Java"/jdk-* \
           "/c/Program Files/Eclipse Adoptium"/jdk-*; do
    [ -d "$j" ] && JAVA_HOME="$(cygpath -m "$j")"
  done
fi
export JAVA_HOME="${JAVA_HOME:?JDK not found; set JAVA_HOME}"
BT="$ANDROID_HOME/build-tools/35.0.0"
TARGET=aarch64

export CC_aarch64_linux_android="$NDK/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android35-clang.cmd"
export AR_aarch64_linux_android="$NDK/toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-ar.exe"

# The APK packs gen/android/app/src/main/assets - keep it in step with ui/.
bash "$ROOT/tools/sync-mobile-assets.sh"

cd "$ROOT/android/tauri-app/src-tauri"
node ../../tauri-cli-npm/node_modules/@tauri-apps/cli/tauri.js android build -t "$TARGET" --apk

UNSIGNED="$ROOT/android/tauri-app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
[ -f "$UNSIGNED" ] || { echo "NO_UNSIGNED_APK"; exit 1; }

# Native .exe tools cannot read MSYS /c/... paths; hand them Windows paths.
UNSIGNED_W="$(cygpath -w "$UNSIGNED")"
ALIGNED_W="$(cygpath -w "$ROOT/android/app-aligned.apk")"
OUT_W="$(cygpath -w "$ROOT/android/QuotaVPN-mobile-signed.apk")"

. /c/Tools/qc-keystore-pass.txt
KS="C:/Tools/qc-release.keystore"
ALIAS="$("$JAVA_HOME/bin/keytool.exe" -list -keystore "$KS" -storepass "$KEYSTORE_PASS" | grep -i privatekeyentry | head -1 | cut -d, -f1)"

"$BT/zipalign.exe" -f -p 4 "$UNSIGNED_W" "$ALIGNED_W"
"$BT/apksigner.bat" sign --ks "$KS" --ks-pass "pass:$KEYSTORE_PASS" --ks-key-alias "$ALIAS" \
  --out "$OUT_W" "$ALIGNED_W"
rm -f "$ROOT/android/app-aligned.apk"

"$BT/apksigner.bat" verify "$OUT_W" && echo SIGN_VERIFY_OK
"$BT/aapt.exe" dump badging "$OUT_W" | head -1
ls -la "$ROOT/android/QuotaVPN-mobile-signed.apk"
echo APK_BUILD_DONE
