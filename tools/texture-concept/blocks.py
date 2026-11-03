"""Engine block textures, revision 2: clean 16 px pixel art (docs/TEXTURES.md).

Rules that fix revision 1's grain: no per-texel random values; every feature
is a cluster of at least 2 texels or a deliberate 1 px line; 3-4 close shades
per material with one dominant base shade; contrast between neighbours stays
low so greedy-merged walls read as surfaces, not static.

Deterministic. Usage: python3 blocks.py OUT_DIR [--preview FILE]
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

from texgen import fbm, tileable_voronoi

SIZE = 16


def hexes(*colors):
    return np.array([[int(c[i:i + 2], 16) for i in (1, 3, 5)] for c in colors], dtype=np.float32)


PALETTE = {
    "grass": hexes("#47712f", "#558538", "#639541", "#79a94e"),
    "grass_edge": hexes("#3a5e29"),
    "dirt": hexes("#57422f", "#654c36", "#73583f", "#8a7055"),
    "stone": hexes("#616469", "#73767b", "#7e8185", "#8c8e91"),
    "sand": hexes("#c9b48a", "#d3bf94", "#dccaa0", "#e5d5ad"),
    "snow": hexes("#cfd8e3", "#dde4ec", "#e9eef3", "#f4f6f8"),
}


def rng_for(name):
    return np.random.default_rng(sum(ord(c) * 31**i for i, c in enumerate(name)) % 2**32)


def wrap_put(canvas, y, x, value):
    canvas[y % SIZE, x % SIZE] = value


def soft_regions(rng, levels, cells=2, octaves=2):
    """Low-frequency tileable field cut into `levels` bands: large flat areas."""
    field = fbm(rng, SIZE, cells, octaves)
    edges = np.quantile(field, np.linspace(0, 1, levels + 1)[1:-1])
    return np.digitize(field, edges)


def grass_top(rng):
    shade = soft_regions(rng, 3, cells=4)
    for _ in range(12):
        y, x = rng.integers(0, SIZE, 2)
        wrap_put(shade, y, x, 3)
        wrap_put(shade, y + 1, x, 2)
    for _ in range(3):
        y, x = rng.integers(0, SIZE, 2)
        wrap_put(shade, y, x, 0)
        wrap_put(shade, y, x + 1, 0)
    return PALETTE["grass"][shade]


def dirt(rng):
    shade = 1 + soft_regions(rng, 2, cells=4)
    placed = 0
    while placed < 8:
        y, x = rng.integers(0, SIZE, 2)
        width = 1 + rng.integers(0, 2)
        for dx in range(width):
            wrap_put(shade, y, x + dx, 3)
            wrap_put(shade, y + 1, x + dx, 0)
        placed += 1
    return PALETTE["dirt"][shade]


def stone(rng):
    _, _, owner = tileable_voronoi(rng, SIZE, 7)
    plate_shade = 1 + (rng.random(owner.max() + 1) < 0.45)
    shade = plate_shade[owner]
    seam = (owner != np.roll(owner, -1, axis=1)) | (owner != np.roll(owner, -1, axis=0))
    bevel = (np.roll(seam, 1, axis=0) | np.roll(seam, 1, axis=1)) & ~seam
    shade = np.where(bevel, 3, shade)
    return PALETTE["stone"][np.where(seam, 0, shade)]


def sand(rng):
    shade = np.full((SIZE, SIZE), 1)
    shade[soft_regions(rng, 2, cells=2, octaves=1) == 1] = 2
    for row in (3, 11):
        phase = rng.integers(0, SIZE)
        for x in range(SIZE):
            wobble = int(round(np.sin((x + phase) * 2 * np.pi / SIZE)))
            wrap_put(shade, row + wobble, x, 3)
    for _ in range(5):
        y, x = rng.integers(0, SIZE, 2)
        wrap_put(shade, y, x, 0)
    return PALETTE["sand"][shade]


def snow(rng):
    shade = 2 + soft_regions(rng, 2, cells=4, octaves=1)
    for _ in range(5):
        y, x = rng.integers(0, SIZE, 2)
        wrap_put(shade, y, x, 1)
        wrap_put(shade, y, x + 1, 1)
    return PALETTE["snow"][shade]


def overhang_depths(rng, base, low, high):
    depth = np.full(SIZE, base, dtype=int)
    for x in range(SIZE):
        step = rng.choice([-1, 0, 0, 1])
        depth[x] = np.clip((depth[x - 1] if x else base) + step, low, high)
    drips = rng.choice(SIZE, 3, replace=False)
    depth[drips] = np.minimum(depth[drips] + 2, high + 2)
    return depth


def capped_side(rng, top_texture, edge_color, base, low, high):
    side = dirt(rng)
    depth = overhang_depths(rng, base, low, high)
    rows = np.arange(SIZE)[:, None]
    cap = rows < depth[None, :]
    edge = rows == depth[None, :] - 1
    side = np.where(cap[..., None], top_texture, side)
    return np.where(edge[..., None], edge_color, side)


def grass_side(rng):
    return capped_side(rng, grass_top(rng), PALETTE["grass_edge"][0], 4, 3, 6)


def snow_side(rng):
    return capped_side(rng, snow(rng), PALETTE["snow"][0], 3, 2, 4)


GENERATORS = {
    "stone": stone,
    "dirt": dirt,
    "grass_top": grass_top,
    "grass_side": grass_side,
    "sand": sand,
    "snow": snow,
    "snow_side": snow_side,
}


def generate():
    return {name: np.clip(gen(rng_for(name)), 0, 255).astype(np.uint8) for name, gen in GENERATORS.items()}


def graininess(pixels):
    """Mean luminance step between neighbouring texels (wrapping), 0-255 scale."""
    lum = pixels.astype(float) @ [0.2126, 0.7152, 0.0722]
    return (np.abs(lum - np.roll(lum, 1, 0)).mean() + np.abs(lum - np.roll(lum, 1, 1)).mean()) / 2


def preview(textures, path, scale=8, repeat=2):
    tiles = [np.tile(p, (repeat, repeat, 1)) for p in textures.values()]
    side = SIZE * repeat * scale
    sheet = Image.new("RGB", (len(tiles) * (side + 8) + 8, side + 16), (28, 30, 34))
    for i, tile in enumerate(tiles):
        sheet.paste(Image.fromarray(tile).resize((side, side), Image.NEAREST), (8 + i * (side + 8), 8))
    sheet.save(path)


if __name__ == "__main__":
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    textures = generate()
    for name, pixels in textures.items():
        Image.fromarray(pixels).save(out / f"{name}.png", optimize=True)
        print(f"{name:12s} graininess {graininess(pixels):5.1f}")
    if "--preview" in sys.argv:
        preview(textures, sys.argv[sys.argv.index("--preview") + 1])
