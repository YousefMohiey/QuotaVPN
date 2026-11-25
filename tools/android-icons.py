#!/usr/bin/env python3
"""Write the Android launcher icons from the tracked QuotaVPN icon.

`tauri android init` generates the project with the Tauri template icon (the
cyan/yellow circles) in gen/android/app/src/main/res/mipmap-*. That folder is
ignored by git, so the wrong icon survives into the APK unless it is replaced
after init and before Gradle. The build script calls this file every time.

Source of truth: android/tauri-app/src-tauri/icons/icon.png (the QuotaVPN tile).
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "android/tauri-app/src-tauri/icons/icon.png"
RES = ROOT / "android/tauri-app/src-tauri/gen/android/app/src/main/res"

# Legacy launcher bitmaps, one per density bucket.
SIZES = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
# Adaptive foreground canvas is 108dp; the art sits inside the safe circle.
FOREGROUND = {"mdpi": 108, "hdpi": 162, "xhdpi": 216, "xxhdpi": 324, "xxxhdpi": 432}


def circle_mask(size: int, scale: int = 4) -> Image.Image:
    big = size * scale
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, big - 1, big - 1), fill=255)
    return mask.resize((size, size), Image.LANCZOS)


def main() -> int:
    if not SRC.is_file():
        raise SystemExit(f"icon source missing: {SRC}")
    if not RES.is_dir():
        raise SystemExit(f"Android res folder missing: {RES} (run tauri android init)")

    icon = Image.open(SRC).convert("RGBA")

    for bucket, size in SIZES.items():
        folder = RES / f"mipmap-{bucket}"
        folder.mkdir(parents=True, exist_ok=True)
        square = icon.resize((size, size), Image.LANCZOS)
        square.save(folder / "ic_launcher.png", "PNG")

        rounded = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        rounded.paste(square, (0, 0), circle_mask(size))
        rounded.save(folder / "ic_launcher_round.png", "PNG")

        canvas = FOREGROUND[bucket]
        art = icon.resize((round(canvas * 0.72), round(canvas * 0.72)), Image.LANCZOS)
        fg = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
        fg.alpha_composite(art, ((canvas - art.width) // 2, (canvas - art.height) // 2))
        fg.save(folder / "ic_launcher_foreground.png", "PNG")

        print(f"icon {bucket}: {size}px launcher, {canvas}px foreground")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
