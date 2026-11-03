"""Swatch sheet: each texture at 16 and 32 px, tiled 2x2 and magnified."""
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

from texgen import GENERATORS, generate

TILE_PX = 128


def tiled(pixels, repeat=2):
    return np.tile(pixels, (repeat, repeat, 1))


def sheet(out_path):
    names = list(GENERATORS)
    sets = [("16 px", generate(16)), ("32 px", generate(32))]
    label_h = 18
    cols = len(names)
    width = cols * (TILE_PX + 8) + 70
    height = len(sets) * (TILE_PX + label_h + 8) + 10
    img = Image.new("RGB", (width, height), (24, 26, 30))
    draw = ImageDraw.Draw(img)
    for row, (label, textures) in enumerate(sets):
        y = 10 + row * (TILE_PX + label_h + 8)
        draw.text((8, y + TILE_PX // 2), label, fill=(220, 220, 220))
        for col, name in enumerate(names):
            x = 70 + col * (TILE_PX + 8)
            tile = Image.fromarray(tiled(textures[name])).resize((TILE_PX, TILE_PX), Image.NEAREST)
            img.paste(tile, (x, y))
            if row == 0:
                pass
            draw.text((x, y + TILE_PX + 3), name, fill=(170, 170, 170))
    img.save(out_path)


if __name__ == "__main__":
    sheet(Path(sys.argv[1]))
