"""Procedural block textures for the Espresso art concept (docs/TEXTURES.md).

Deterministic: same seed, same pixels.
Usage: python3 texgen.py OUT_DIR            # crisp16/ and crisp32/ concept sets
       python3 texgen.py OUT_DIR SIZE       # one flat set, e.g. ../../assets/blocks 16
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def srgb(hex_colors):
    return np.array([[int(h[i:i + 2], 16) for i in (1, 3, 5)] for h in hex_colors], dtype=np.float32)


# Hue-shifted ramps: shadows lean cool/blue, highlights lean warm.
RAMPS = {
    "stone": srgb(["#3d4048", "#52555c", "#676a6f", "#7c7e80", "#929390", "#a8a8a2"]),
    "dirt": srgb(["#3b2a22", "#4e3727", "#62452e", "#765535", "#8a6a45", "#a08259"]),
    "grass": srgb(["#2c4a2a", "#3a612e", "#4c7a33", "#61913a", "#7aa845", "#97bd57"]),
    "sand": srgb(["#a68f68", "#b9a275", "#c9b483", "#d6c392", "#e2d1a3", "#ede0b8"]),
    "snow": srgb(["#a9b5c9", "#bfc9d9", "#d2dae6", "#e2e8ef", "#eef2f5", "#fbfcfc"]),
    "gravel": srgb(["#403c3d", "#575152", "#6e6865", "#86807a", "#9d978f", "#b6b0a6"]),
    "bark": srgb(["#2e2219", "#3f2e20", "#523c28", "#664b31", "#7a5c3d"]),
    "wood": srgb(["#6a4a2c", "#7f5a36", "#946c42", "#a87f50", "#bb9462", "#cfa974"]),
    "leaves": srgb(["#1f3a22", "#2a4d2a", "#376234", "#467840", "#588e4b"]),
    "water": srgb(["#1e4a6e", "#24597f", "#2c6990", "#3779a0", "#4a8db1"]),
}


def _stable(name, seed):
    value = seed * 1_000_003
    for char in name:
        value = (value * 131 + ord(char)) % (2**32)
    return value


def tileable_value_noise(rng, size, cells):
    lattice = rng.random((cells, cells))
    coords = np.arange(size) * cells / size
    i0 = np.floor(coords).astype(int)
    t = coords - i0
    t = t * t * (3 - 2 * t)
    i1 = (i0 + 1) % cells
    ty, tx = np.meshgrid(t, t, indexing="ij")
    a = lattice[np.ix_(i0, i0)]
    b = lattice[np.ix_(i0, i1)]
    c = lattice[np.ix_(i1, i0)]
    d = lattice[np.ix_(i1, i1)]
    return (a * (1 - tx) + b * tx) * (1 - ty) + (c * (1 - tx) + d * tx) * ty


def fbm(rng, size, base_cells, octaves=3):
    total = np.zeros((size, size))
    amplitude, norm = 1.0, 0.0
    cells = base_cells
    for _ in range(octaves):
        total += amplitude * tileable_value_noise(rng, size, min(cells, size))
        norm += amplitude
        amplitude *= 0.5
        cells *= 2
    return total / norm


def tileable_voronoi(rng, size, points):
    seeds = rng.random((points, 2)) * size
    ys, xs = np.mgrid[0:size, 0:size] + 0.5
    best = np.full((size, size), np.inf)
    second = np.full((size, size), np.inf)
    owner = np.zeros((size, size), dtype=int)
    for index, (sy, sx) in enumerate(seeds):
        dy = np.abs(ys - sy)
        dx = np.abs(xs - sx)
        dy = np.minimum(dy, size - dy)
        dx = np.minimum(dx, size - dx)
        dist = np.sqrt(dx * dx + dy * dy)
        closer = dist < best
        second = np.where(closer, best, np.minimum(second, dist))
        owner = np.where(closer, index, owner)
        best = np.where(closer, dist, best)
    return best, second, owner


def quantize(field, ramp, dither_rng=None, dither=0.0, levels=None):
    levels = len(ramp) if levels is None else levels
    f = (field - field.min()) / max(field.max() - field.min(), 1e-6)
    if dither_rng is not None:
        f = f + (dither_rng.random(f.shape) - 0.5) * dither
    idx = np.clip((f * levels).astype(int), 0, levels - 1)
    return idx


def paint(idx, ramp):
    return ramp[np.clip(idx, 0, len(ramp) - 1)]


def stone(rng, s):
    near, second, owner = tileable_voronoi(rng, s, max(7, s // 2))
    cell_shade = rng.random(owner.max() + 1)[owner]
    field = 0.55 * cell_shade + 0.45 * fbm(rng, s, 4)
    idx = quantize(field, RAMPS["stone"], rng, 0.25, levels=5) + 1
    crack = (second - near) < (0.45 if s <= 16 else 0.9)
    idx = np.where(crack, 0, idx)
    return paint(idx, RAMPS["stone"])


def dirt(rng, s):
    idx = quantize(fbm(rng, s, 4), RAMPS["dirt"], rng, 0.55, levels=4) + 1
    pebbles = rng.random((s, s)) < 0.06
    idx = np.where(pebbles, 5, idx)
    pits = rng.random((s, s)) < 0.05
    return paint(np.where(pits, 0, idx), RAMPS["dirt"])


def grass_top(rng, s):
    field = 0.6 * fbm(rng, s, 4) + 0.4 * rng.random((s, s))
    idx = quantize(field, RAMPS["grass"], levels=5)
    tips = rng.random((s, s)) < 0.07
    return paint(np.where(tips, 5, idx), RAMPS["grass"])


def grass_side(rng, s):
    base = dirt(rng, s)
    top = grass_top(rng, s)
    unit = s / 16
    walk = np.cumsum(rng.integers(-1, 2, s))
    walk = walk - np.linspace(0, walk[-1], s)
    drips = (rng.random(s) < 0.2) * rng.integers(1, 4, s)
    fringe = np.round((3 + 0.6 * walk + drips) * unit).astype(int)
    fringe = np.clip(fringe, int(2 * unit), int(7 * unit))
    rows = np.arange(s)[:, None]
    mask = rows < fringe[None, :]
    shadow = rows == fringe[None, :]
    out = np.where(mask[..., None], top * 0.92, base)
    out = np.where(shadow[..., None], base * 0.72, out)
    return out


def sand(rng, s):
    ys = np.mgrid[0:s, 0:s][0]
    ripple = 0.5 + 0.5 * np.sin((ys + 1.5 * fbm(rng, s, 2) * s / 4) * 2 * np.pi * 3 / s)
    field = 0.35 * ripple + 0.65 * rng.random((s, s))
    return paint(quantize(field, RAMPS["sand"], levels=6), RAMPS["sand"])


def snow(rng, s):
    field = 0.7 * fbm(rng, s, 3) + 0.3 * rng.random((s, s))
    idx = quantize(field, RAMPS["snow"], levels=4) + 2
    sparkle = rng.random((s, s)) < 0.02
    return paint(np.where(sparkle, 5, idx), RAMPS["snow"])


def snow_side(rng, s):
    base = dirt(rng, s)
    top = snow(rng, s)
    fringe = (s * 0.22 + rng.integers(0, max(2, s // 8), s)).astype(int)
    rows = np.arange(s)[:, None]
    out = np.where((rows < fringe[None, :])[..., None], top, base)
    return np.where((rows == fringe[None, :])[..., None], base * 0.75, out)


def gravel(rng, s):
    near, second, owner = tileable_voronoi(rng, s, max(8, (s * s) // 18))
    shade = rng.random(owner.max() + 1)[owner]
    rim = (second - near) < 0.8
    idx = quantize(shade - near * 0.05, RAMPS["gravel"], levels=5) + 1
    return paint(np.where(rim, 0, idx), RAMPS["gravel"])


def log_side(rng, s):
    xs = np.mgrid[0:s, 0:s][1]
    grooves = 0.5 + 0.5 * np.sin(xs * 2 * np.pi * 4 / s + fbm(rng, s, 2) * 3)
    field = 0.6 * grooves + 0.4 * rng.random((s, s))
    return paint(quantize(field, RAMPS["bark"], levels=5), RAMPS["bark"])


def log_top(rng, s):
    ys, xs = np.mgrid[0:s, 0:s] + 0.5
    r = np.hypot(xs - s / 2, ys - s / 2)
    rings = 0.5 + 0.5 * np.sin(r * 2 * np.pi * 3.2 / s * 2 + fbm(rng, s, 2))
    out = paint(quantize(rings, RAMPS["wood"], levels=5) + 1, RAMPS["wood"])
    bark = (np.minimum(np.minimum(xs, s - xs), np.minimum(ys, s - ys)) < s / 16 + 0.5)
    return np.where(bark[..., None], log_side(rng, s), out)


def planks(rng, s):
    ys, xs = np.mgrid[0:s, 0:s]
    board = ys // (s // 4)
    grain = fbm(rng, s, 2) * 0.4 + 0.6 * (0.5 + 0.5 * np.sin(xs * 0.9 + board * 2.1))
    idx = quantize(grain, RAMPS["wood"], levels=4) + 1
    seam = (ys % (s // 4)) == (s // 4 - 1)
    offset = (board % 2) * (s // 2)
    joint = ((xs + offset) % s) == 0
    return paint(np.where(seam | joint, 0, idx), RAMPS["wood"])


def leaves(rng, s):
    near, second, owner = tileable_voronoi(rng, s, max(10, (s * s) // 14))
    field = 0.6 * rng.random(owner.max() + 1)[owner] - 0.08 * near + 0.3 * fbm(rng, s, 4)
    idx = quantize(field, RAMPS["leaves"], levels=5)
    holes = rng.random((s, s)) < 0.10
    return paint(np.where(holes, 0, idx), RAMPS["leaves"])


def water(rng, s):
    field = 0.7 * fbm(rng, s, 3) + 0.3 * rng.random((s, s))
    return paint(quantize(field, RAMPS["water"], levels=5), RAMPS["water"])


GENERATORS = {
    "stone": stone, "dirt": dirt, "grass_top": grass_top, "grass_side": grass_side,
    "sand": sand, "snow": snow, "snow_side": snow_side, "gravel": gravel,
    "log_side": log_side, "log_top": log_top, "planks": planks, "leaves": leaves, "water": water,
}


def generate(size, seed=7):
    return {name: np.clip(gen(np.random.default_rng(_stable(name, seed)), size), 0, 255).astype(np.uint8)
            for name, gen in GENERATORS.items()}


if __name__ == "__main__":
    out = Path(sys.argv[1])
    if len(sys.argv) > 2:
        out.mkdir(parents=True, exist_ok=True)
        for name, pixels in generate(int(sys.argv[2])).items():
            Image.fromarray(pixels).save(out / f"{name}.png", optimize=True)
        sys.exit(0)
    for size in (16, 32):
        folder = out / f"crisp{size}"
        folder.mkdir(parents=True, exist_ok=True)
        for name, pixels in generate(size).items():
            Image.fromarray(pixels).save(folder / f"{name}.png")
