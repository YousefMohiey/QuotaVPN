# QuotaVPN for Android

The phone app: a Tauri v2 web UI over the shared Rust core, with a Kotlin `VpnService` running the sing-box engine (libbox) for the actual tunnel.

## Layout

- `tauri-app/` - the app: phone UI in `ui/`, the Rust bridge and the shared core behind it
- `tauri-plugin-qctunnel/` - Kotlin `VpnService`, libbox engine, Quick Settings tile, per-app picker (`android/libs/libbox.aar` is the prebuilt sing-box library)
- `tauri-cli-npm/` - pinned Tauri CLI used by the build scripts

## Build

From the repo root:

```bash
bash tools/build-apk.sh
```

That sets the toolchain env, syncs the shared assets, builds, zipaligns and signs with the release keystore, then verifies the signature. Output: `android/QuotaVPN-mobile-signed.apk`.

It also runs `tools/android-icons.py` before Gradle. `tauri android init` writes the Tauri template icon into the generated mipmaps, so that step restores the QuotaVPN launcher icon every build.

Needs Rust stable with the Android targets, an Android SDK with platform 36 and NDK 28.2, and a JDK 17 or newer (JDK 23 is what this host builds with). The script picks the newest installed build-tools, so 35 and 36 both work. Go and gomobile are only needed when regenerating `libbox.aar`.

For a local x86_64 emulator, keep the release APK untouched:

```bash
TARGET=x86_64 OUT_NAME=QuotaVPN-mobile-x86_64.apk bash tools/build-apk.sh
```

## Updates

The app checks GitHub Releases for the signed APK and opens the installer (Settings, Updates, one confirmation tap). It walks the release list for the newest release that actually carries `QuotaVPN-mobile-signed.apk`, so a Windows-only release cannot announce an update the phone cannot install.
