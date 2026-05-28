#!/usr/bin/env python3
"""Generate macOS menu bar tray icon (black template on transparent background)."""

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri/icons/tray-icon.png"


def main() -> None:
    size = 44
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    black = (0, 0, 0, 255)

    for index, y in enumerate([14, 22, 30]):
        x0 = 4 + index * 2
        draw.line((x0, y, 14, y), fill=black, width=2)

    cx, cy, radius = 28, 22, 11
    draw.ellipse(
        (cx - radius, cy - radius, cx + radius, cy + radius),
        outline=black,
        width=2,
    )
    draw.line((cx, cy, cx - 3, cy - 5), fill=black, width=2)
    draw.line((cx, cy, cx + 5, cy - 2), fill=black, width=2)
    draw.ellipse((cx - 1.5, cy - 1.5, cx + 1.5, cy + 1.5), fill=black)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    img.save(OUT)
    print(f"Wrote {OUT}")


if __name__ == "__main__":
    main()
