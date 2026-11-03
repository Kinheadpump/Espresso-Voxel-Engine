"""CPU ray-cast mock-up of one terrain view in each candidate style.

Mirrors terrain.wgsl's shading (direction shade, exponential fog) so the only
variable is the surface texture. Usage: python3 mockup.py OUT_DIR
"""
import sys
from pathlib import Path

import numpy as np
from PIL import Image

from texgen import fbm, generate

AIR, STONE, DIRT, GRASS, SAND, SNOW, GRAVEL, LOG, LEAVES, WATER = range(10)
SIZE_XZ, SIZE_Y = 112, 72
SEA = 17
DIRECTION_SHADE = np.array([0.80, 0.80, 1.00, 0.50, 0.65, 0.65])
SKY = np.array([0.34, 0.54, 0.83])
FOG_DENSITY = 0.0035
AO_CURVE = np.array([0.45, 0.66, 0.84, 1.0])
FLAT_COLORS = np.array([
    [1.0, 0.0, 1.0], [0.50, 0.50, 0.52], [0.47, 0.33, 0.22], [0.33, 0.62, 0.24],
    [0.86, 0.80, 0.55], [0.95, 0.96, 0.98], [0.52, 0.49, 0.46], [0.36, 0.26, 0.17],
    [0.24, 0.42, 0.22], [0.17, 0.39, 0.58],
])
FACE_TEXTURES = {
    STONE: ("stone", "stone", "stone"), DIRT: ("dirt", "dirt", "dirt"),
    GRASS: ("grass_top", "grass_side", "dirt"), SAND: ("sand", "sand", "sand"),
    SNOW: ("snow", "snow_side", "dirt"), GRAVEL: ("gravel", "gravel", "gravel"),
    LOG: ("log_top", "log_side", "log_top"), LEAVES: ("leaves", "leaves", "leaves"),
    WATER: ("water", "water", "water"),
}
ROTATABLE = {STONE, DIRT, GRASS, SAND, SNOW, GRAVEL, LEAVES, WATER}


def build_world():
    rng = np.random.default_rng(3)
    heights = (SEA + (fbm(rng, SIZE_XZ, 3, 4) - 0.40) * 90).astype(int)
    world = np.zeros((SIZE_XZ, SIZE_Y, SIZE_XZ), dtype=np.uint8)
    gravel_patch = fbm(rng, SIZE_XZ, 6, 2) > 0.62
    for x in range(SIZE_XZ):
        for z in range(SIZE_XZ):
            h = int(np.clip(heights[x, z], 4, SIZE_Y - 10))
            world[x, :h - 3, z] = STONE
            world[x, max(h - 3, 0):h, z] = DIRT
            if h <= SEA + 1:
                world[x, max(h - 3, 0):h + 1, z] = GRAVEL if gravel_patch[x, z] else SAND
            elif h >= SEA + 21:
                world[x, h, z] = SNOW
            elif h >= SEA + 15:
                world[x, h - 2:h + 1, z] = STONE
            else:
                world[x, h, z] = GRASS
            if h < SEA:
                world[x, h + 1:SEA + 1, z] = WATER
    for _ in range(26):
        x, z = rng.integers(4, SIZE_XZ - 4, 2)
        h = heights[x, z]
        if not (SEA + 2 <= h < SEA + 13):
            continue
        world[x - 2:x + 3, h + 4:h + 6, z - 2:z + 3] = LEAVES
        world[x - 1:x + 2, h + 6:h + 8, z - 1:z + 2] = LEAVES
        world[x, h + 1:h + 6, z] = LOG
    return world


def camera_rays(width, height, eye, target, fov_deg=60):
    forward = target - eye
    forward /= np.linalg.norm(forward)
    right = np.cross(forward, [0, 1, 0])
    right /= np.linalg.norm(right)
    up = np.cross(right, forward)
    tan_half = np.tan(np.radians(fov_deg) / 2)
    xs = (np.arange(width) + 0.5) / width * 2 - 1
    ys = 1 - (np.arange(height) + 0.5) / height * 2
    px, py = np.meshgrid(xs * tan_half * width / height, ys * tan_half)
    dirs = forward + px[..., None] * right + py[..., None] * up
    return (dirs / np.linalg.norm(dirs, axis=-1, keepdims=True)).reshape(-1, 3)


def trace(world, eye, dirs, max_steps=420):
    count = len(dirs)
    cell = np.repeat(np.floor(eye).astype(int)[None], count, 0)
    step = np.sign(dirs).astype(int)
    with np.errstate(divide="ignore"):
        delta = np.abs(1.0 / dirs)
        boundary = np.where(step > 0, cell + 1 - eye, eye - cell)
        t_max = np.where(dirs != 0, boundary * delta, np.inf)
    hit_block = np.zeros(count, dtype=np.uint8)
    hit_axis = np.zeros(count, dtype=int)
    hit_t = np.full(count, np.inf)
    active = np.ones(count, dtype=bool)
    bounds = np.array(world.shape)
    for _ in range(max_steps):
        idx = np.nonzero(active)[0]
        if idx.size == 0:
            break
        axis = np.argmin(t_max[idx], axis=1)
        t_here = t_max[idx, axis]
        cell[idx, axis] += step[idx, axis]
        t_max[idx, axis] += delta[idx, axis]
        c = cell[idx]
        inside = np.all((c >= 0) & (c < bounds), axis=1)
        escaped = ~inside & (((c[:, 1] >= bounds[1]) & (step[idx, 1] >= 0)) | np.any((c < 0) | (c >= bounds), axis=1))
        blocks = np.zeros(idx.size, dtype=np.uint8)
        blocks[inside] = world[c[inside, 0], c[inside, 1], c[inside, 2]]
        hit = blocks != 0
        hit_block[idx[hit]] = blocks[hit]
        hit_axis[idx[hit]] = axis[hit]
        hit_t[idx[hit]] = t_here[hit]
        active[idx[hit | escaped]] = False
    return hit_block, hit_axis, hit_t, cell


def face_direction(axis, dirs):
    positive_face = np.take_along_axis(dirs, axis[:, None], 1)[:, 0] < 0
    return axis * 2 + np.where(positive_face, 0, 1)


def surface_uv(point, cell, direction):
    local = point - cell
    axis = direction // 2
    u = np.where(axis == 0, local[:, 2], local[:, 0])
    v = np.where(axis == 1, local[:, 2], 1 - local[:, 1])
    return np.clip(u, 0, 0.9999), np.clip(v, 0, 0.9999)


def voxel_hash(cell):
    h = (cell[:, 0] * 73856093) ^ (cell[:, 1] * 19349663) ^ (cell[:, 2] * 83492791)
    h = (h ^ (h >> 13)) * 0x5BD1E995
    return (h ^ (h >> 15)) & 0xFFFF


def textured_albedo(blocks, direction, u, v, cell, textures, rotate):
    size = next(iter(textures.values())).shape[0]
    albedo = np.zeros((len(blocks), 3))
    turns = voxel_hash(cell) & 3
    for block, (top, side, bottom) in FACE_TEXTURES.items():
        for name, face_mask in ((top, direction == 2), (bottom, direction == 3), (side, direction // 2 != 1)):
            sel = (blocks == block) & face_mask
            if not sel.any():
                continue
            uu, vv = u[sel], v[sel]
            if rotate and block in ROTATABLE and name in (top, bottom) and direction[sel].min() >= 2:
                t = turns[sel]
                uu, vv = (np.select([t == 0, t == 1, t == 2], [uu, 1 - vv, 1 - uu], vv),
                          np.select([t == 0, t == 1, t == 2], [vv, uu, 1 - vv], 1 - uu))
            px = (np.clip(uu, 0, 0.9999) * size).astype(int)
            py = (np.clip(vv, 0, 0.9999) * size).astype(int)
            albedo[sel] = textures[name][py, px] / 255.0
    return albedo


def flat_albedo(blocks, cell, point):
    tint = 1 + ((voxel_hash(cell) & 255) / 255.0 - 0.5) * 0.10
    macro = 1 + 0.06 * np.sin(point[:, 0] * 0.11) * np.sin(point[:, 2] * 0.13)
    return FLAT_COLORS[blocks] * (tint * macro)[:, None]


def solid(world, cells):
    bounds = np.array(world.shape)
    inside = np.all((cells >= 0) & (cells < bounds), axis=1)
    out = np.zeros(len(cells), dtype=bool)
    c = cells[inside]
    out[inside] = world[c[:, 0], c[:, 1], c[:, 2]] != 0
    return out


def vertex_ao(world, cell, direction, point):
    axis = direction // 2
    normal = np.zeros_like(cell)
    normal[np.arange(len(cell)), axis] = np.where(direction % 2 == 0, 1, -1)
    outside = cell + normal
    tangent_a = (axis + 1) % 3
    tangent_b = (axis + 2) % 3
    rows = np.arange(len(cell))
    local = point - cell
    fa = np.clip(local[rows, tangent_a], 0, 1)
    fb = np.clip(local[rows, tangent_b], 0, 1)
    result = np.zeros(len(cell))
    for sa in (-1, 1):
        for sb in (-1, 1):
            ea = np.zeros_like(cell)
            eb = np.zeros_like(cell)
            ea[rows, tangent_a] = sa
            eb[rows, tangent_b] = sb
            side1 = solid(world, outside + ea)
            side2 = solid(world, outside + eb)
            corner = solid(world, outside + ea + eb)
            level = np.where(side1 & side2, 0, 3 - side1.astype(int) - side2 - corner)
            weight = (fa if sa > 0 else 1 - fa) * (fb if sb > 0 else 1 - fb)
            result += AO_CURVE[level] * weight
    return result


def shade(albedo_srgb, direction, distance, occlusion=None):
    linear = albedo_srgb ** 2.2
    lit = linear * DIRECTION_SHADE[direction][:, None]
    if occlusion is not None:
        lit = lit * occlusion[:, None]
    fog = 1 - np.exp(-distance * FOG_DENSITY)
    return lit * (1 - fog[:, None]) + (SKY ** 2.2) * fog[:, None]


VIEWS = {
    "overview": (np.array([6.5, 58.0, 8.5]), np.array([60.0, 24.0, 64.0])),
    "closeup": (np.array([30.5, 31.0, 47.5]), np.array([46.0, 24.0, 58.0])),
}


def render(world, style, textures, width, height, supersample=2, with_ao=False, view="overview"):
    eye, target = VIEWS[view]
    w, h = width * supersample, height * supersample
    dirs = camera_rays(w, h, eye, target)
    blocks, axis, t, cell = trace(world, eye, dirs)
    hit = blocks != 0
    color = np.tile(SKY ** 2.2, (len(dirs), 1))
    point = eye + dirs[hit] * t[hit, None]
    direction = face_direction(axis[hit], dirs[hit])
    if style == "flat":
        albedo = flat_albedo(blocks[hit], cell[hit], point)
    else:
        u, v = surface_uv(point, cell[hit], direction)
        albedo = textured_albedo(blocks[hit], direction, u, v, cell[hit], textures, rotate=True)
    occlusion = vertex_ao(world, cell[hit], direction, point) if with_ao else None
    color[hit] = shade(albedo, direction, t[hit], occlusion)
    image = color.reshape(h, w, 3).reshape(height, supersample, width, supersample, 3).mean((1, 3))
    return Image.fromarray((np.clip(image, 0, 1) ** (1 / 2.2) * 255).astype(np.uint8))


if __name__ == "__main__":
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    world = build_world()
    width, height = (int(a) for a in (sys.argv[2:4] if len(sys.argv) > 3 else (640, 360)))
    sets = {"crisp16": generate(16), "crisp32": generate(32), "flat": None}
    for style, textures in sets.items():
        for with_ao in (False, True):
            name = f"mockup-{style}{'-ao' if with_ao else ''}.png"
            render(world, style, textures, width, height, with_ao=with_ao).save(out / name)
        render(world, style, textures, width, height, with_ao=True, view="closeup").save(
            out / f"closeup-{style}-ao.png")
        print(style)
