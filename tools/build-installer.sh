#!/usr/bin/env bash
# Build the Windows installer.
#
# The host default toolchain is stable-x86_64-pc-windows-gnu, and the build
# script embeds the icon and manifest through embed-resource, which shells
# out to windres. windres is NOT part of the rustup GNU toolchain, so the
# PATH must carry the mingw binutils that live in C:/Tools/mingw_extract
# or tauri-winres panics with NotAttempted("windres"). RC is set as well so
# the lookup cannot miss. The updater key signs the artifact; the password
# comes from its file, never from this script.
set -e
export PATH="/c/Tools/mingw_extract/mingw64/bin:$PATH"
export RC="C:/Tools/mingw_extract/mingw64/bin/windres.exe"
export TAURI_SIGNING_PRIVATE_KEY="C:/Tools/qc-updater.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat C:/Tools/qc-updater-pass.txt)"
cd "$(dirname "$0")/.." || exit 1
cd desktop

# Code signing: set QC_SIGN_THUMBPRINT to the thumbprint of the code signing
# certificate as it appears in the Windows certificate store (SimplySign and
# every cloud HSM client present the certificate there). The bundler then
# signs the app binary and the installer. QC_SIGN_TIMESTAMP overrides the
# timestamp authority. With the variable unset the build is unchanged.
CONFIG_ARGS=()
if [ -n "${QC_SIGN_THUMBPRINT:-}" ]; then
  TS="${QC_SIGN_TIMESTAMP:-http://timestamp.digicert.com}"
  TMPCFG="$(pwd)/sign-config.json"
  cat > "$TMPCFG" <<JSON
{
  "bundle": {
    "windows": {
      "certificateThumbprint": "${QC_SIGN_THUMBPRINT}",
      "digestAlgorithm": "sha256",
      "timestampUrl": "${TS}"
    }
  }
}
JSON
  CONFIG_ARGS=(--config "$(cygpath -w "$TMPCFG" 2>/dev/null || echo "$TMPCFG")")
  echo "signing with certificate ${QC_SIGN_THUMBPRINT}"
else
  echo "building unsigned (set QC_SIGN_THUMBPRINT to sign)"
fi

npx --yes @tauri-apps/cli@2 build "${CONFIG_ARGS[@]}" "$@"
