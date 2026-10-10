import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).parent
FONT = HERE.parent / "fonts" / "JetBrainsMonoNerdFont-Regular.ttf"
GAS_STATION = "\U000f0298"
BACKGROUND = (0x0A, 0x0E, 0x14, 255)
BORDER = (0x2A, 0x3A, 0x52, 255)
ACCENT = (0x6C, 0xC4, 0xFF, 255)
SIZE = 512


def render():
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    draw.rounded_rectangle(
        (8, 8, SIZE - 9, SIZE - 9), radius=96, fill=BACKGROUND, outline=BORDER, width=12
    )
    font = ImageFont.truetype(str(FONT), 400)
    left, top, right, bottom = draw.textbbox((0, 0), GAS_STATION, font=font)
    x = (SIZE - (right - left)) / 2 - left
    y = (SIZE - (bottom - top)) / 2 - top
    draw.text((x, y), GAS_STATION, font=font, fill=ACCENT)
    return img


def main():
    img = render()
    img.save(HERE / "fuel-station.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    (HERE / "fuel-station-64.rgba").write_bytes(img.resize((64, 64), Image.LANCZOS).tobytes())


if __name__ == "__main__":
    sys.exit(main())
