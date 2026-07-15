#!/usr/bin/env python3
from pathlib import Path

from PIL import Image, ImageOps


ROOT = Path(__file__).resolve().parents[1]
SOURCE_PATH = ROOT / "src-tauri" / "icons" / "sleepy-cat-logo-full.png"
PNG_PATH = ROOT / "src-tauri" / "icons" / "menubar-template.png"
RGBA_PATH = ROOT / "src-tauri" / "icons" / "menubar-template.rgba"
SIZE = 22
MARK_SIZE = (21, 13)
FOREGROUND_THRESHOLD = 176
ALPHA_THRESHOLD = 176


def main() -> None:
    source = Image.open(SOURCE_PATH).convert("RGB")
    luminance = ImageOps.grayscale(source)
    mask = luminance.point(
        lambda value: 255 if value >= FOREGROUND_THRESHOLD else 0,
        mode="L",
    )
    bounds = mask.getbbox()
    if bounds is None:
        raise ValueError(f"No foreground found in {SOURCE_PATH}")

    mark = mask.crop(bounds).resize(MARK_SIZE, Image.Resampling.LANCZOS)
    mark = mark.point(
        lambda alpha: 255 if alpha >= ALPHA_THRESHOLD else 0,
        mode="L",
    )

    image = Image.new("RGBA", (SIZE, SIZE), (255, 255, 255, 0))
    x = (SIZE - mark.width) // 2
    y = (SIZE - mark.height) // 2
    image.paste((255, 255, 255, 255), (x, y), mark)

    PNG_PATH.parent.mkdir(parents=True, exist_ok=True)
    image.save(PNG_PATH)
    RGBA_PATH.write_bytes(image.tobytes())


if __name__ == "__main__":
    main()
