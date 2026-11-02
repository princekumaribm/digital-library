"""Builds the app icon and in-app logo from assets/logo.png (needs Pillow: pip install pillow).

Usage: python scripts/make_icons.py   then   npx tauri icon build/app-icon.png -o src-tauri/icons
"""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "assets" / "logo.png"


def on_white_disc(logo: Image.Image) -> Image.Image:
    # The logo's inside is transparent; a white disc keeps dark text readable on dark backgrounds.
    left, top, right, bottom = logo.getchannel("A").getbbox()
    cx, cy = (left + right) / 2, (top + bottom) / 2
    radius = min(right - left, bottom - top) / 2 - 6
    disc = Image.new("RGBA", logo.size, (0, 0, 0, 0))
    ImageDraw.Draw(disc).ellipse((cx - radius, cy - radius, cx + radius, cy + radius), fill=(255, 255, 255, 255))
    return Image.alpha_composite(disc, logo)


def main() -> None:
    logo = on_white_disc(Image.open(SOURCE).convert("RGBA"))
    logo.resize((512, 512), Image.LANCZOS).save(ROOT / "ui" / "logo.png", optimize=True)

    # Leave a margin like other macOS/Windows icons instead of filling the whole tile.
    size, content = 1024, 900
    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    offset = (size - content) // 2
    icon.paste(logo.resize((content, content), Image.LANCZOS), (offset, offset))
    (ROOT / "build").mkdir(exist_ok=True)
    icon.save(ROOT / "build" / "app-icon.png")
    print("Wrote ui/logo.png and build/app-icon.png")


if __name__ == "__main__":
    main()
