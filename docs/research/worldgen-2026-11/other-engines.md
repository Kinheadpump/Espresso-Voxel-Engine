# Worldgen survey: how other engines generate large worlds

> Snapshot, 2026-11-03.

Question: how do other voxel engines and games generate large or infinite worlds, especially far LOD generated directly from the generator, and what staged architecture should Espresso adopt under ADR-0016 (deterministic, chunk-local, LOD-direct, >= 64 km horizon)?

Tags: **[S]** seen this session (page fetched or source cloned; URL or repo given). **[U]** unverified background knowledge or own reasoning. Numbers are labelled quoted / computed / estimate. Sources were shallow clones of the repos named; file paths below are relative to each repo root.

## Summary

| Candidate | Source | Fit for Espresso | Cost / complexity | LOD-friendliness | Verdict |
|---|---|---|---|---|---|
| Cubyz map fragments + per-LOD chunks | github.com/PixelGuys/Cubyz `src/server/terrain/` [S] | Closest match: 2D maps at every LOD, blocky, 32^3 chunks | 2D passes cheap; caves/structures gated by LOD | Direct generation at voxelSize 1..32; octave truncation by construction | **Adopt-candidate** (architecture) |
| Cubyz hash-seeded midpoint noise (`FractalNoise`) | same [S] | Coarse grid at 2^L is an exact prefix of the fine grid | Scalar, grid-based, not SIMD | Perfect: LOD L = skip the finest L refinement rounds | **Prototype** (idea, not the code) |
| Cubyz blue-noise structure tile + per-chunk structure lists | same [S] | Cross-chunk features without neighbour access | One table lookup per point | Falls back to per-column hash chance above LOD 2 | **Adopt-candidate** |
| Luanti v7 (2D base + sparse 3D mountain/ridge) | github.com/luanti-org/luanti `src/mapgen/mapgen_v7.cpp` [S] | Shows the "2D height, 3D only above it" split | 2D noises per mapchunk; 3D noise only where flags and Y range ask | None (no LOD) | **Watch** |
| Veloren worldsim + ColumnGen | gitlab.com/veloren/veloren `world/src/` [S] | Finite 1024x1024-chunk map, erosion needs global state | Erosion is global; column sample is cheap | Not designed for LOD | **Reject** (sim); **Adopt-candidate** (jittered structure grid) |
| Hytale density graph | docs.hytalemodding.dev, hytale.com [S] | Node graph with `YSampled`, `Cache`, 2D/3D split | Quoted 14 ms to 5 ms terrain stage with Y sampling | Not documented | **Prototype** (YSampled idea) |
| Distant Horizons rough generator | gitlab/DH repo clone `DhRoughSurfaceGenerator.java` [S] | Direct LOD from a density function: sparse column samples + marching | 1 of 4 columns sampled, 8-block march step | Yes, by design | **Watch** (shows the cost of 3D-density LOD) |
| FastNoise2 node graph | github.com/Auburn/FastNoise2 [S] | Composable SIMD generators, batch grid API with step size | SIMD batch; per-sample call is slower | Step parameter gives coarse grids; no octave truncation knob | **Prototype** (design only, we already cover the crate) |
| Vintage Story | wiki.vintagestory.at [S, partial] | Column generation with neighbour-region dependency | Not measured | None | **Watch** |
| No Man's Sky | GDC 2017 notes [S, third-party notes] | Local-only generation, noise-shaped voxels, 36 m chunks | Not stated | Not stated | **Watch** |
| Terra, Voxy, Rust LOD crates | [U] | Not read this session | n/a | n/a | **Watch** (gap) |

## 1. Cubyz

Repo: https://github.com/PixelGuys/Cubyz, shallow clone, files read: `src/server/terrain/{terrain.zig, SurfaceMap.zig, ClimateMap.zig, CaveMap.zig, StructureMap.zig, mapgen/MapGenV1.zig, noise/FractalNoise.zig, noise/BlueNoise.zig, chunkgen/TerrainGenerator.zig, cavegen/FractalCaveGenerator.zig, structuremapgen/SimpleStructureGen.zig, climategen/NoiseBasedVoronoi.zig}`, `src/settings.zig`. [S]

### Stage layout [S]

`TerrainGenerationProfile` (`terrain.zig`) holds the ordered stages: `ClimateMapGenerator`, `MapGenerator` (surface), `CaveBiomeGenerator[]`, `CaveGenerator[]`, `StructureMapGenerator[]`, and priority-sorted `BlockGenerator[]` (terrain, ore, crystal, structure). Chunks are 32^3 (`chunkShift = 5` in `src/chunk.zig`). `highestSupportedLod = 5` (`src/settings.zig`), so voxel sizes 1, 2, 4, 8, 16, 32 blocks. Every cache key and every fragment carries `voxelSize`.

### Per column vs per voxel [S]

- **Climate map** (`ClimateMapFragment`): one `BiomeSample` per 32x32-block cell (`biomeShift = 5`), independent of LOD. Fragment is 2^13 blocks wide. A Voronoi-like tree (`NoiseBasedVoronoi.zig`, `GenerationStructure`) fills it deterministically from the world seed and fragment position.
- **Surface map** (`MapFragment`): 256x256 samples (`mapShift = 8`) at the fragment's `voxelSize`. Holds `heightMap: i32`, `biomeMap: *Biome`, `caveBiomeOffsetMap: i16`, plus `minHeight`/`maxHeight`. At LOD 5 one fragment covers 256 x 32 = 8192 blocks (computed). Fragment position must be aligned to `voxelSize` (assert in `MapFragmentPosition.init`).
- **Cave map**: per chunk-sized fragment, per column a 64-bit mask of solid voxels (`heightData & (1 << (z >> voxelSizeShift))` in `TerrainGenerator.zig`). One u64 per column means a 32-voxel column is a bitmask.
- **Block generation** (`TerrainGenerator.generate`): loops x, y; reads the column mask; walks z through biome slices; writes block runs with `updateBlockColumnInGeneration(x, y, zMin, zMax, block)`. Surface layers come from `biome.structure.addSubTerranian` with slope from four neighbours.

### Biome blending [S]

`MapGenV1.generateMapFragment`: biome centres sit on a hex grid in climate space (`getNearestNeighborsInHexGrid`, 3 nearest). Coordinates are first warped by two sparse fractal offset maps (`offsetScale = biomeSize*16`). Barycentric weights of the 3 hex corners are mapped through the biome's `interpolation` mode (`none` = winner takes all, `linear`, `square` = weights squared and renormalised) times `interpolationWeight`. Height, roughness, hills and mountains are blended with these weights; then height = blended base + roughMap * roughness + hillMap * hills + mountainMap * mountains. The biome ID of the nearest of the 3 corners is stored. So blending is weighted interpolation of per-biome noise amplitudes, not of block types. Cost per column: 3 climate lookups, about 20 flops, 4 noise-map reads.

### Direct LOD generation [S]

- `MapGenV1` passes `map.pos.voxelSize` to every noise function; the same 256x256 array just samples a coarser world grid. No downsampling of finer fragments.
- `noise/FractalNoise.zig` is a hash-seeded midpoint-displacement field: corner values come from `initSeed2D(worldSeed*(scale*maxResolution|1), (x*maxResolution, y*maxResolution))`, refinement rounds halve `res` and add `nextFloatSigned(seed)*res/scale/2`. Every grid point's seed depends only on its absolute position and its round, so a grid at spacing `voxelSize` is exactly the prefix of the full grid with the finest rounds skipped. `generateSparseFractalTerrain(..., maxResolution)` stops at `res = maxResolution`. This is octave truncation with an exactly preserved coarse silhouette, and it is chunk-local (the 2^n+1 grid cell is regenerated from corners; no neighbour data).
- `TerrainGenerator.generate`: for `voxelSize >= 8`, it reads surface heights on the chunk plus a 1-sample border and, if the chunk is wholly above or below, calls `fillUniform(stone|air)`. Equivalent to our uniform-chunk shortcut, driven by a 2D min/max.
- Caves: `FractalCaveGenerator.generate` returns immediately if `voxelSize > 2`. Cave spheres and branches are evaluated by iterating all candidate seed points within `range = 8*chunkSize = 256` blocks (`initSeed3D(worldSeed, wx, wy, wz)` per 32-block cell), which is the bounded-padding pattern for worm-like features.
- Structures: `SimpleStructureGen.generate`: for `voxelSize <= 4` uses a blue-noise point set; above that it iterates each column at LOD spacing, scales chance by `1 - (1-p/16)^(voxelSize^2)` and accepts that positions are no longer exact ("density matters more", comment in source).
- Ordinary distance: lod0.5Distance setting default 200 blocks (`settings.zig`, quoted). Far distance is capped by LOD 5 (32-block voxels), not by a horizon tier.

### Cross-chunk features [S]

- `noise/BlueNoise.zig` pre-computes a 128x128 pattern of points (`sizeShift = 7`, comment says it was 10 before loading time suffered) with min-distance relaxation; `getRegionData(wx, wy, w, h)` returns the points of any world rectangle by tiling the pattern. Structure points therefore cost one table scan per region, no hashing per cell.
- `StructureMapFragment` (128 blocks per axis, `size = 1 << 7`) stores, per 32^3 chunk, a list of structure records `(model, seed, wx, wy, wz)`. `addStructure(structure, min, max)` appends the record to every chunk its bounding box touches (margin 16 horizontal, 32 vertical in `SimpleStructureGen`). A chunk generates its structures from its own list with the structure's own seed, so there is no neighbour access at block-placement time. The padding is explicit and bounded: structure radius <= margin.
- Biome per column for placement comes from `CaveBiomeMapView`.

### Fit and numbers

1. Chunk-local and deterministic for an infinite world: yes. Coordinates are `i32` (wrap arithmetic `+%`), so Cubyz itself notes a TODO for the world border (`FractalCaveGenerator.zig` comment). Espresso needs i64 cells or i32 cell + f32 offset.
2. Coarse spacing without silhouette change: yes for height and biome maps (prefix property above). Caves and structures are dropped at LOD > 2 and > 4 respectively; this changes the interior and trees, not the silhouette.
3. Cost: no timings published in the files read; none claimed. Gap.

What to measure for Espresso: (a) time of a 256x256 height map at LOD 0, 3, 5; (b) fraction of chunks that hit the uniform shortcut at LOD >= 3 using min/max from the 2D map.

## 2. Luanti (Minetest) mapgen v7

Repo: https://github.com/luanti-org/luanti, `src/mapgen/mapgen_v7.cpp`, `mg_biome.cpp`, `mg_decoration.cpp`. [S]

- Noise defaults (quoted from `mapgen_v7.cpp`, `NoiseParams(offset, scale, spread, seed, octaves, persist, lacunarity)`): terrain_base (4, 70, spread 600, 5 oct, 0.6), terrain_alt (4, 25, 600, 5, 0.6), terrain_persist (0.6, 0.1, 2000, 3), height_select (-8, 16, 500, 6, 0.7), mount_height (256, 112, 1000, 3), mountain (-0.6, 1, 250/350/250, 5 oct 3D), ridge (3D, spread 100), cave1/cave2 (spread 61/67, 3D, 3 oct).
- Per column (2D, once per mapchunk): persist, base, alt, height_select, mount_height, ridge_uwater, filler_depth. `baseTerrainLevelFromMap` blends base and alt terrain by `height_select` and persistence scales the octaves.
- Per voxel (3D, only when flagged and in Y range): mountain noise `noise_mountain->noiseMap3D(...)`, ridge (river carving) noise, cave noises. In `generateTerrain`: if `y <= surface_y` the voxel is stone without any 3D lookup; only above the 2D surface does it test mountain terrain against a density that is lowered with height. The 3D noise array is still computed for the whole chunk (`noiseMap3D` call is unconditional when mountains are on), so the saving is skipping the test, not the noise.
- Biomes (`mg_biome.cpp`, `calcBiomeFromNoise`): two 2D fBm fields (heat, humidity) plus two blend fields (`np_heat_blend`, `np_humidity_blend`) added to them; the biome is the nearest biome point in (heat, humidity) space, distance divided by `weight`. Vertical blending: a PCG rng seeded from `pos.Y + (heat+humidity)*0.9` randomly picks the closest or the blend candidate above its `max_pos.Y`. There is no spatial interpolation; boundaries are hard but noisy (dither).
- Decorations (`Decoration::placeDeco`): `PcgRandom ps(blockseed + 53)`, area split into `sidelen` cells, per-cell count from a 2D noise (`NoiseFractal2D`) at the cell centre; positions drawn from the block seed. Cross-chunk trees rely on the mapchunk emerging with a margin, i.e. the mapgen writes into a VoxelManip larger than the chunk (not read in detail, [U]).
- Fit: 2D-first split is right; the lack of LOD and mapchunk-wide 3D arrays are not. Chunk-local: yes (seed + position only). LOD: not supported. Cost per voxel of the mountain test: 1 compare vs the 2D surface; 3D noise cost as computed by the library. Verdict: Watch, nothing to port beyond the split.

## 3. Veloren

Repo: https://gitlab.com/veloren/veloren, `world/src/sim/erosion.rs` (2,683 lines), `sim/mod.rs`, `column.rs`, `lib.rs`, `util/structure.rs`, `layer/tree.rs`. [S]

- What is finite and why: `WorldSim` holds a grid of `SimChunk` (chaos, alt, basement, water_alt, downhill, flux, temp, humidity, rockiness, tree_density, forest_kind, river, ...). `DEFAULT_WORLD_CHUNKS_LG` = 2^10 x 2^10 chunks; `TERRAIN_CHUNK_BLOCKS_LG = 5` (32 blocks), so 32,768 x 32,768 blocks (computed). Erosion (`do_erosion`, stream-power law with `n_steps = 100 * erosion_quality` iterations over all chunks, uses `downhill`, `uphill` and a sorted-by-height pass for drainage) requires the whole grid: flow accumulation and lakes depend on global topology. Civ simulation (`civ/`, `sim2.rs`) also runs on the whole map. These cannot be chunk-local.
- Column sampling (`ColumnGen::get`): bilinear/Catmull-Rom interpolation (`get_interpolated`, 4x4 stencil of `SimChunk`) of the coarse 32-block grid for temp, humidity, chaos, alt, plus per-column noise (`hill_nz`, marble octaves at wavelength 3, 12, 48 blocks). Coarse 2D data plus cheap fine noise: this is the reusable pattern. The interpolation is `Catmull-Rom` cubic (`cubic` closure in `get_interpolated`).
- Structures: `util/structure.rs` `StructureGen2d { freq, spread, x_field, y_field, seed_field }`: cell centre `index*freq + freq/2`, offset by `RandomField(pos) % (2*spread) - spread` per axis, seed from a third field. `layer/tree.rs` queries it through `StructureGenCache` per column; a tree is a pure function of (cell, world seed). A column checks the neighbouring cells within the maximum tree radius. This is a jittered grid with bounded padding, trivially chunk-local.
- Fit: sim = Reject (global state, finite, contradicts ADR-0016). Jittered structure grid = Adopt-candidate. Chunk-local: only the column and structure parts. LOD: none; 2D sim data could feed horizon impostors but is finite.
- Cost: not measured here.

## 4. Hytale (worldgen v2 docs)

Sources: https://hytale.com/news/2026/1/the-future-of-world-generation, https://hytalemodding.dev/en/docs/official-documentation/worldgen/technical-hytale-generator/density, `.../positions-provider`. [S]

- Terrain is a signed density graph (positive = solid). Built from a base height plus 2D/3D simplex terms and a Y curve; nodes: `SimplexNoise2D` (auto-cached per x/z column), `SimplexNoise3D`, `Cache` (capacity, "a safe value is 3"), `YSampled`, `GradientWarp` (internal 2D cache, "relatively expensive"), `Multiplier` (skips remaining inputs after one returns 0; page quotes about 40% gain for one example), `DistanceToBiomeEdge`, `BaseHeight`.
- `YSampled`: "interpolating between spread-out samples along the Y-axis". Quoted terrain-stage timings from the page: none 14 ms, SampleDistance 4 → 8 ms, 8 → 6 ms, 16 → 5 ms (quoted, machine and scene not given). Page notes the 2D parts see no speed-up and upstream warping cancels the gain. This is the same idea as Minecraft's NoiseChunk 4x8x4 cell interpolation already covered in the repo docs.
- Biomes are tiles, terrain shape per biome, blended near borders by `DistanceToBiomeEdge`-style terms; the news post says creators can shape terrain "near their edges". Details of the blend are not published on the pages fetched.
- Props use positions providers: `SquareGrid2d/3d`, `TriangularGrid2d` (1-block spacing), then `Jitter2d/3d` (Magnitude, Seed), `Occurrence` (keep chance = density, seeded), `FieldFunction` (keep where density between Min and Max), `Cache` with `SectionSize` (start 32) and `CacheSize` (start 50). Points are pure functions of position and seed, so chunk-local. Page does not state the maximum prop radius or padding.
- LOD: no statement found. Verdict: Prototype the YSampled and "skip when mask is 0" ideas.

## 5. Distant Horizons (Minecraft LOD mod)

Repo clone: `dh/` (GitLab distant-horizons-team). Files: `common/.../worldGeneration/DhRoughSurfaceGenerator.java`, `DhChunkGenerator.java`. [S]

- The "rough surface" generator builds LOD data straight from the vanilla `DensityFunction` (`randomState.router().finalDensity()`), without generating real chunks.
- It samples heights for one in four columns (`x += 2, z += 2`) and interpolates the rest, then runs a spike cleanup for water pockets (`smoothUnderwaterSpikes`).
- `findSurfaceHeightMarching` steps down by `MARCH_STEP = 8` blocks and sanity-checks depths {4, 8, 16} and heights {4, 8, 16, 32} to reject floating islands. Source comment records three variants with counts and wall times on 24 cores, 128 render distance, a Terralith world: marching 15.9 million / 37 s, binary search 3.3 million / 23 s, hybrid 5.3 million / 27 s (quoted from source comment; unit of the "million" is not stated, probably density evaluations; wall time includes other work).
- Lesson: 3D density at LOD costs one density evaluation per march step per sampled column; binary search over the surface is cheaper but wrong under overhangs. An analytic 2D height (Cubyz style) avoids both.
- Other DH generation modes (surface only, features, full) exist in the API [U]; not read.
- Voxy: only known as a mod that voxelizes already generated chunks into an octree [U]; not read.

## 6. FastNoise2 node graph

Repo: https://github.com/Auburn/FastNoise2 (clone; wiki page "Node Editor" fetched). [S]

- Generators are composable nodes (`Fractal` wraps a source; `Domain Warp*`, `Cellular`, `Blends`, `Remap`, `Terrace`, `SeedOffset`, `Cache`). The graph serialises to an encoded string (`NewFromEncodedNodeTree`), so terrain definitions are data.
- Batch API: `GenUniformGrid2D/3D(out, xStart, yStart, w, h, xStep, yStep, seed)` and `GenPositionArray2D/3D`; both return `OutputMinMax` (`Generator.h`), the measured min/max of the batch. Header says batch calls are faster per sample than `GenSingle*` because SIMD lanes are filled.
- `GeneratorCache` (`Modifiers.h`): "if the same input coordinates and seed are requested again, the cached value is returned". It is a single-entry memo for repeated positions, not a spatial cache.
- Step parameter gives coarse grids (LOD by stride) but the fractal evaluates all octaves at the coarse point, so there is no octave truncation: a coarse sample at spacing 2^L still includes octaves smaller than 2^L, which alias. To keep a stable silhouette, our generator needs its own octave cutoff per LOD.
- Already covered in earlier research: SIMD speed-up quoted 5–8×. New here: the encoded-tree-as-data design and `OutputMinMax` for conservative uniform-chunk tests (range from last batch, not a proof bound).
- Verdict: Prototype the design (data-driven graph compiled to SoA batch passes), not the dependency.

## 7. Vintage Story, No Man's Sky, Terra

- Vintage Story: https://wiki.vintagestory.at/index.php/Modding:WorldGen_Configuration and `...WorldGen_Concept`. [S] Chunk 32^3, map region 16x16 chunk columns = 512x512 blocks (computed from the page). Each region/map chunk/column is generated once, stored. Terrain pass needs no neighbours (strata, caves, block layers); TerrainFeatures, Vegetation need neighbouring columns; a column needs its map region plus all 8 neighbour regions. Landforms: `terrainOctaves` fixed at 10 amplitudes (0–1), `terrainOctaveThresholds` act as subtractors, `terrainYKeyPositions/Thresholds` define a vertical density profile (0 = always solid, 1 = always air); sea level at 0.43 of world height. Landform selection, blending and climate maps are not on the pages fetched. Not infinite-LOD; neighbour-region dependency is the opposite of our constraint.
- No Man's Sky: GDC 2017 talk (https://www.gdcvault.com/play/1024265/Continuous-World-Generation-in-No). [S abstract] plus third-party notes (https://blather-rinse-repeat.blogspot.com/2018/11/gdc-2017-part-ii-main-conference-notes.html) [S]: 1 m voxels, 6 bytes each, 36x36x36 m chunks, generation at runtime on worker threads (CPU on PC, compute on PS4), a linear density bias added to 3D noise, Perlin worms for caves, erosion approximated procedurally because generation must be local. Notes say nothing about uber noise or LOD; "uber noise" as a named technique is [U].
- Terra (https://github.com/PolyhedralDev/Terra): landing page only [S]; config-driven pipeline of biome providers and samplers is [U].

## 8. Common patterns across engines

| Question | Cubyz [S] | Luanti v7 [S] | Veloren [S] | Hytale [S] | DH [S] |
|---|---|---|---|---|---|
| 2D per column | height, biome, cave-biome offset | base/alt/persist/height_select | sim alt, temp, humidity, tree density (interpolated) | `SimplexNoise2D` cached per column | height from 1/4 of columns |
| 3D per voxel | cave bitmask only (LOD <= 2) | mountain, ridge, caves | caves layer | `SimplexNoise3D`, Y curves | marched density |
| Biome blend | hex-grid barycentric weights of per-biome amplitudes | nearest point in heat/humidity + blend noise | interpolated climate fields | edge-distance terms | none (reads vanilla biome) |
| Feature placement | blue-noise tile + per-chunk lists | block-seeded cells | jittered grid, hash per cell | grid + Jitter + Occurrence | none |
| Far LOD | direct, voxelSize <= 32 | no | no | not documented | direct, approximate |

## 9. Recommended architecture for Espresso

Goal: staged generator where each stage is a pure function of (seed, integer coordinates, LOD) and caches live at the coarsest level that can serve them. This is a design proposal [U]; no numbers measured.

### Stages

1. **S0 climate grid (2D, coarse, LOD independent).** One cell per 32–64 blocks: temperature, humidity, continentalness, erosion-like "ruggedness". Value-noise lattice with hash `h(seed, cell_x: i64, cell_z: i64)`, cubic interpolation (Veloren pattern). Cached per 256x256-cell region (Cubyz `ClimateMapFragment` pattern). Biome weights: 3 nearest hex-grid corners with barycentric weights (Cubyz), blending amplitudes (base, roughness, hills, mountains), not blocks; block palette picks nearest corner.
2. **S1 height map (2D, per LOD).** `HeightTile { lod: u8, origin: (i64, i64), h: [f32; 256*256] }`, one per (tile, LOD); aligned to `2^lod`. Octave `k` has wavelength `W0 >> k`; at LOD L skip octaves with wavelength < 2 * 2^L * blocks_per_voxel (the lattice-prefix rule that Cubyz gets for free). Store also `min`/`max` per 32x32 sub-block (computed during the fBm pass, free) for conservative uniform-chunk tests. Same fn feeds the >= 64 km horizon tier at spacing 64–256 blocks (octave count shrinks to 2–4). Ridged mountain term is multiplied by a mask so it can be skipped (Hytale `Multiplier` trick) where the mask is 0.
3. **S2 3D density (only where needed).** For a chunk with `min < chunk_top` and `max > chunk_bottom` (from S1 min/max), evaluate `density(x,y,z) = (h(x,z) - y)*k + cave_noise(x,y,z)*mask(y_depth)` on a coarse cell grid (4x8x4 style, already documented) and write directly to palette. Overhangs and caves are enabled only for LOD <= 2 (Cubyz gates caves at voxelSize <= 2); above that, columns are solid up to `h`. Optional `YSampled`-style interpolation for the cave term only.
4. **S3 surface rules (per column, after height).** Biome block layers by depth below `h`; slope from 4 neighbours in the height tile (Cubyz `slope = min(maxUp, maxDown)`), so no neighbour chunk access: the height tile already contains the +1 border.
5. **S4 decoration with bounded padding.** Per feature type `f` with max radius `R_f`: jittered grid cell size `C_f >= R_f`, jitter `< C_f/2`, hash `h(seed, f, cell)` gives position, kind, seed (Veloren `StructureGen2d`). A chunk checks the cells whose centre lies within `R_f` of the chunk (at most 3x3 cells when `C_f >= R_f`), reads `h(x,z)` from S1 for the base Y, and stamps only the part inside the chunk. No neighbour chunk is read. For LOD > 4 replace trees by per-column hash with probability `1-(1-p)^(voxel^2)` (Cubyz) or drop them; far tier gets tint from biome only.

### Data layout (SIMD)

- Height tile and climate tile as SoA `Vec<f32>` rows, x contiguous, aligned 32 B, so `wide::f32x8` runs along x. One noise pass per field over the whole tile (FastNoise2 `GenUniformGrid` style); per-field arrays, not an array of structs.
- 3D density cell grid `[f32; 9*5*9]` per chunk (for 32^3 with cells of 4x8x4 blocks, computed: 8+1, 4+1, 8+1 corners); trilinear expand in registers into the palette writer.
- Column masks: `u32` solid bitmask per column per chunk (Cubyz uses u64 for 64 voxels; we have 32) lets caves and surface rules work with `trailing_zeros` run finding and feed the binary greedy mesher's column bitmasks directly.
- Positions: `i64` cell index + `f32` fractional part; hash on `(i64, i64)` so precision holds at 64 km and beyond.
- Cache: LRU keyed by `(tile_x, tile_z, lod)` for S0/S1; S2/S4 are not cached beyond the chunk's lifetime.

### What to measure first (Decided-by candidates)

1. S1 tile at LOD 0 vs LOD 3 vs LOD 6: ms per tile scalar vs `f32x8`; confirm octave truncation keeps silhouette (compare level-0 downsample vs LOD-L direct, max height error in blocks).
2. Fraction of chunks resolved by the S1 min/max shortcut at distances 1–64 km.
3. S4: cost per chunk of 3x3 cell check, trees per chunk, wrong-tree rate across chunk borders (must be 0).
4. Whether S2 density at coarse cells beats a pure heightfield at LOD <= 2 on frame time, not just worldgen time.

## Gaps

- Cubyz: no performance numbers in the source; did not run it. Did not read `CaveBiomeMap.zig` biome-layer logic, `RandomlyWeightedFractalNoise`, `PerlinNoise.generateSmoothNoise` in detail, or the LOD streaming code in `world.zig`.
- Luanti: decoration margin and schematic overflow handling not read.
- Veloren: did not read the `StructureGenCache` neighbour-cell loop or `layer/cave.rs`.
- Hytale: no LOD or chunk-border documentation found; biome blend maths not published on pages fetched; Graphs System and Optimization pages not read.
- Vintage Story: landform blending, climate map and the Terrain noise pass not documented on pages fetched; `vssurvivalmod` source not cloned.
- No Man's Sky: only third-party notes and the abstract; talk video not seen. "Uber noise" is not verified.
- Terra, Voxy, Rust infinite-world/LOD crates: not found or not read. One web search for Rust LOD projects returned nothing relevant.
- DH other generation modes and Voxy: not read.
- FastNoise2: did not read the Node Graph Architecture guide or SIMD internals; no performance figures re-measured.
