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
# The host default is MSVC. Only a GNU build needs MinGW's binutils and windres
# on PATH; forcing them on an MSVC build makes embed-resource hand an MSVC
# resource to windres, which fails with "package.metadata does not exist".
case " $* " in
  *" windows-gnu "*)
    export PATH="/c/Tools/mingw_extract/mingw64/bin:$PATH"
    export RC="C:/Tools/mingw_extract/mingw64/bin/windres.exe"
    ;;
esac
export TAURI_SIGNING_PRIVATE_KEY="C:/Tools/qc-updater.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat C:/Tools/qc-updater-pass.txt)"
cd "$(dirname "$0")/.." || exit 1

# The layer-2 adapter is part of the product now. A build without the driver
# payload would install and then fail every layer-2 space, so refuse it here.
TAP_SYS="desktop/src-tauri/tap/amd64/tap0901.sys"
TAP_INF="desktop/src-tauri/tap/amd64/OemVista.inf"
TAP_CAT="desktop/src-tauri/tap/amd64/tap0901.cat"
TAP_DEV="desktop/src-tauri/tap/amd64/devcon.exe"
for f in "$TAP_SYS" "$TAP_INF" "$TAP_CAT" "$TAP_DEV" "desktop/src-tauri/tap/setup.ps1" "desktop/src-tauri/tap/remove.ps1"; do
  if [ ! -f "$f" ]; then
    echo "missing layer-2 payload: $f" >&2
    exit 1
  fi
done
cd desktop

# Code signing: set QC_SIGN_THUMBPRINT to the thumbprint of the code signing
# certificate as it appears in the Windows certificate store (SimplySign and
# every cloud HSM client present the certificate there). The bundler then
# signs the app binary and the installer. QC_SIGN_TIMESTAMP overrides the
# timestamp authority. With the variable unset the build looks for the local
# YousefMohiey certificate, which is the publisher this project ships under.
CONFIG_ARGS=()
if [ -z "${QC_SIGN_THUMBPRINT:-}" ]; then
  QC_SIGN_THUMBPRINT="$(powershell -NoProfile -Command "(Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Where-Object { \$_.Subject -eq 'CN=YousefMohiey' -and \$_.NotAfter -gt (Get-Date) } | Sort-Object NotAfter -Descending | Select-Object -First 1).Thumbprint" | tr -d '\r' | xargs)"
fi
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
  echo "building unsigned (no YousefMohiey certificate and QC_SIGN_THUMBPRINT is unset)"
fi

npx --yes @tauri-apps/cli@2 build "${CONFIG_ARGS[@]}" "$@"

# The bundler signs the app binary and the installer. The payload DLLs and the
# bundled LAN engine have to be signed here, and the installer is signed again
# once they are in place. A test session can leave the engine running, and
# signtool cannot sign a locked file, so ask it to stop first. Only runs when a
# certificate is set.
if [ -n "${QC_SIGN_THUMBPRINT:-}" ]; then
  VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
  SETUP="$(pwd)/src-tauri/target/release/bundle/nsis/QuotaVPN_${VERSION}_x64-setup.exe"
  ENGINE="$(pwd)/src-tauri/target/release/openrad/QuotaSpace.exe"
  if [ -x "$ENGINE" ]; then
    "$ENGINE" stop >/dev/null 2>&1 || true
    sleep 1
  fi
  powershell -NoProfile -ExecutionPolicy Bypass -File "$(pwd)/../tools/sign-release.ps1" \
    -Installer "$SETUP" -Thumbprint "$QC_SIGN_THUMBPRINT" -IncludeSigned
  # The updater signature is over the final installer bytes. The bundler wrote
  # one before this last Authenticode pass; rewrite it so the feed and the
  # installer agree.
  npx --yes @tauri-apps/cli@2 signer sign \
    -f "$TAURI_SIGNING_PRIVATE_KEY" -p "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" "$SETUP"
fi
