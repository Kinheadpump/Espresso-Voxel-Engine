# Minecraft Java world generation (1.18+) and the mods around it

> Snapshot, 2026-11-03.

**Question.** Which parts of the Minecraft 1.18+ generator (noise router, spline terrain shaping, multi-noise biomes, noise caves, aquifers) should Espresso copy, given an infinite world, chunk-local deterministic generation, far LOD generated directly from noise, and a CPU budget of about 0.5 ms core time per 32^3 chunk?

**Tags.** **[S]** seen this session (URL or cloned path given). **[U]** unverified: background knowledge or own reasoning. Numbers are labelled quoted / computed / estimate (DOC-GUIDELINES section 4).

**Sources actually read this session.**
- Vanilla data (JSON worldgen files, `misode/mcmeta` branch `data`, fetched 2026-11-03): `https://raw.githubusercontent.com/misode/mcmeta/data/data/minecraft/worldgen/{noise_settings/overworld.json, density_function/overworld/*.json, noise/*.json}`. This snapshot uses the newest data format (named `upscale`, `find_top_surface`, `chunk_surface_level`). The exact game version string was not checked.
- Shallow clones read this session (paths below as `clones/<name>/…` are relative to each repo root): `clones/C2ME-fabric` = https://github.com/RelativityMC/C2ME-fabric, `clones/noisium` = https://github.com/Steveplays28/noisium, `clones/tectonic` = https://github.com/Apollounknowndev/tectonic (contains 21.1, 26.1 and 26.2 source sets), `clones/dh` = https://gitlab.com/distant-horizons-team/distant-horizons.
- `minecraft.wiki` returned `client_error` for every page (Noise_router, Density_function, World_generation, Climate). All wiki-derived knowledge below is **[U]**.

## 1. Summary table

| Candidate | Source | Fit for Espresso | Cost (estimate) | LOD-friendly | Verdict |
|---|---|---|---|---|---|
| A. Spline shaping: continents, erosion, ridges to offset / factor / jaggedness | vanilla JSON [S] | High: 2D, column-local, replaces the single fBm | 3 splines per column, about 38 octave evals per column | Yes, 2D, any spacing | **Adopt-candidate** |
| B. Folded ridges (peaks and valleys), ridge-driven rivers | vanilla JSON [S], Tectonic [S] | High, 3 flops after one noise | free after the ridge noise | Yes | **Adopt-candidate** |
| C. `depth` gradient + `4*quarter_negative((depth+jag)*factor) + 3D noise`, squeeze, slides, 4x8x4 trilinear | vanilla JSON [S] | Medium: overhangs and arches, but 3D cost | 405 grid corners per chunk, about 20k octave evals | Only through a 2D height bound | **Prototype**, surface band only |
| D. Cheese / spaghetti 2D+3D / noodle / entrances / pillars | vanilla JSON [S] | Medium: caves are the main 3D cost | 7–25 octave evals per corner below the surface | Skip at LOD > 1 | **Prototype** cheese + spaghetti only; **Reject** noodle for now |
| E. Aquifers + ore veins | vanilla JSON (aquifer fields) [S], algorithm [U] | Low now: per-voxel randomised neighbour search | per air voxel below sea level | No | **Watch** |
| F. Multi-noise biome source (6 parameters, RTree, quart resolution) | [U] | Medium: needed once biomes exist | 64 columns x 5 noises + 512 lookups per chunk | Yes (2D part) | **Prototype** with a lookup table, **Reject** RTree |
| G. Generation-status pyramid, 3x3 feature neighbourhood | [U] | Low: conflicts with ADR-0016 chunk-locality | n/a | No | **Reject**; use hash-cell placement instead |
| H. DH "rough surface" sampler: march 8 blocks down, bisect, 1 in 4 columns | `clones/dh` [S] | High as the far-LOD fallback | about 6–12 density evals per sampled column | Yes (it is the LOD generator) | **Adopt-candidate** for the horizon tier, if density is expensive |
| I. C2ME density-function compiler (AST to bytecode) | `clones/C2ME-fabric` [S] | Idea only: we compile at build time in Rust | n/a | n/a | **Watch** (copy the optimisation passes, not the runtime compiler) |
| J. Tectonic-style parameter retuning (scale knobs, larger continents) | `clones/tectonic` [S] | High: knobs on A | free | Yes | **Adopt-candidate** |

## 2. The vanilla noise router [S unless noted]

### 2.1 Router fields

`noise_settings/overworld.json` (2026-11-03) [S] declares `"noise": {"min_y": -64, "height": 384}`, `sea_level: 63`, and a `noise_router` with eight named entries: `continents`, `erosion`, `ridges`, `temperature`, `vegetation`, `depth`, `final_density`, `chunk_surface_level`. Aquifer functions (`barrier`, `fluid_level_floodedness`, `fluid_level_spread`, `lava`, `exclusion`, `surface_level`) are a separate `aquifers` block. Older versions listed `barrier`, `fluid_level_*`, `lava`, `vein_*`, `initial_density_without_jaggedness` inside the router [U]; the ore-vein block was not found in this snapshot (gap).

### 2.2 The 2D climate noises

```
continents = cache( noise(continentalness, shift_x, shift_z, xz_scale 0.25, y_scale 0) )
erosion    = cache( noise(erosion,         ...,               xz_scale 0.25, y_scale 0) )
ridges     = cache( noise(ridge,           ...,               xz_scale 0.25, y_scale 0) )
```

Noise parameters (`worldgen/noise/*.json`) [S]:

| Noise | first octave | amplitudes | non-zero octaves | base wavelength (computed) | finest wavelength (computed) |
|---|---|---|---|---|---|
| continentalness | -9 | 1,1,2,2,2,1,1,1,1 | 9 | 2^9 / 0.25 = 2048 blocks | 2048 / 2^8 = 8 blocks |
| erosion | -9 | 1,1,0,1,1 | 4 | 2048 blocks | 2048 / 2^4 = 128 blocks |
| ridge | -7 | 1,2,1,0,0,0 | 3 | 512 blocks | 512 / 2^2 = 128 blocks |
| temperature | -10 | 1.5,0,1,0,0,0 | 2 | 4096 blocks | 1024 blocks |
| vegetation | -8 | 1,1,0,0,0,0 | 2 | 1024 blocks | 512 blocks |

Wavelength = 2^|first octave| / xz_scale (computed; Perlin lattice period, not a hard feature size). Octave `i` has frequency `2^(first + i)` and amplitude `base_amplitude * amplitude_modifier[i] * 2^-i`-style persistence [U]. Consequence: the climate noises are very smooth. Continents, the finest one, has a minimum feature of about 8 blocks, the rest 128 blocks or more. A 4-block cell grid never aliases them.

`shift_x`/`shift_z` is a domain warp, a small 3-octave noise added to the coordinates before the lookup [U for the octave count].

### 2.3 Folded ridges: peaks and valleys

`ridges_folded = (|(|ridges| - 2/3)| - 1/3) * -3` [S] (exact constants `-0.6666667`, `-0.33333334`, `-3.0`). It maps the signed ridge noise to a value in [-1, 1] that is 1 at the peaks (|r| = 2/3 is a peak, |r| in {1/3...} gives zero crossings, r = 0 and |r| = 1 give valleys). The same expression is the "Peaks/Valleys" debug function in the noise settings [S]. Cost: 3 abs/add/mul ops on an existing sample.

### 2.4 Offset, factor, jaggedness: nested cubic splines

Three splines of the three coordinates `continents`, `erosion`, and `ridges` / `ridges_folded`. Counted `coordinate` nodes [S]: offset 53, factor 49, jaggedness 16. Each is a `CubicSpline`: sorted `(location, value | child spline, derivative)` points, evaluated by finding the segment by location and doing cubic Hermite interpolation between value and derivative at the two ends, recursing into child splines [U for the evaluation detail; node format seen]. Structure:

```
offset  = -0.50375 + spline_continents(
   -1.1:0.044, -1.02:-0.2222, -0.51:-0.2222, -0.44:-0.12, -0.18:-0.12,
   -0.16: spline_erosion( -0.85: spline_ridges_folded(-1: -0.0888, 1: 0.69) , ... ), ... )
factor  = spline_continents(-0.19: 3.95, -0.15: spline_erosion(-0.6: spline_ridges(...6.3..6.25), -0.5: ...2.67), ... )
jaggedness = spline_continents(-0.11: 0, 0.03: spline_erosion(-1: spline_ridges_folded(..., 1: spline_ridges(-0.01: 0.63, 0.01: 0.3)), ...), ...)
```

Each is wrapped as `cache(lerp(blend_alpha, blend_offset | 10.0 | 0.0, <spline>))`. `blend_alpha` is 1 when no old chunks are blended, so for a fresh world the lerp is a no-op (the new-vs-old-version border blender, irrelevant to Espresso).

Intuition (consistent with the values above): the continents noise picks ocean / coast / inland; within inland, erosion low means mountains, high means flat plains; ridges and folded ridges then raise peaks and cut valleys. Offset is a vertical shift in density units, factor sets how quickly density changes with height, jaggedness adds a high-frequency 2D noise term.

Cost per column: 3 splines with a nesting depth of 3. Each level is a binary search over 2–8 points plus a Hermite evaluation, about 30–60 flops per level (estimate). No memory fetch except the point tables, which stay in cache. Measure: spline eval ns per column.

### 2.5 `depth` and the density equation

```
depth          = y_gradient(y: -64 -> 1.5, 320 -> -1.5) + offset         [S]
jagged_term    = jaggedness * half_negative( noise(jagged, xz_scale 1500, y_scale 0) )   [S]
sloped_cheese  = 4 * quarter_negative( (depth + jagged_term) * factor ) + base_3d_noise   [S]
base_3d_noise  = old_blended_noise(xz_scale 0.25, y_scale 0.125, xz_factor 80, y_factor 160, smear 8)   [S]
```

`quarter_negative(x) = x<0 ? 0.25x : x`, `half_negative(x) = x<0 ? 0.5x : x` [U], `squeeze(x) = c/2 - c^3/24` with `c = clamp(x,-1,1)` [U]. Solid where density > 0.

Surface height without 3D noise (computed): `depth = 0` gives `1.5 - (y + 64)/128 + offset = 0`, so `y_surface = 128 * offset + 128`. With offset = -0.50375 + spline, offset = -0.5 gives y = 64, which matches sea level 63. A 2D field gives the whole heightmap before any 3D noise is touched. `factor` only sets the steepness, so high factor means a sharper, flatter surface and less room for the 3D noise to make overhangs.

`jagged` has `first_octave = -16`, 16 octaves, `xz_scale = 1500` [S]: base wavelength about 2^16 / 1500 = 44 blocks (computed), finest octave about 0.001 blocks. Only the first 5–6 octaves are meaningful for blocks. Vanilla evaluates all 16 [U]; Espresso can truncate this to 6 with no visible change (estimate; verify by diffing heightmaps).

`old_blended_noise` is the legacy 1.12-style terrain noise: three Perlin stacks, 16 lower-limit, 16 upper-limit and 8 main-selector octaves, blended with `lerp(clamp(main/10+1)/2, lower, upper)` [U]. When the selector is fully on one side, one 16-octave stack is skipped [U]. This is the single most expensive term per 3D sample: 24–40 octave evaluations.

### 2.6 Slides and final_density

```
final_density = min( squeeze( 0.64 * interp(                              [S]
                   blend_density( lerp(y: -64..-40 -> 0..1,  0.1171875,         // bottom slide
                                  lerp(y: 240..256 -> 1..0, -0.078125,          // top slide
                                       range_choice(sloped_cheese, <1.5625:
                                            min(sloped_cheese, entrances*5),
                                          else: caves(...)) )))) ,  noodle )
               + beardifier
```

The `interp` wrapper (`upscale` in this data format) has `x.cell_size 4, y.cell_size 8, z.cell_size 4, mode interpolate_linear` [S]. Slides: below y = -40 density is pulled to +0.117 (solid bedrock zone), above y = 240 pulled to -0.078 (air). Effective terrain ceiling is y = 256 even though the world is 384 tall.

### 2.7 Cell grid, caching and evaluation

Vanilla `NoiseChunk` (cache wrappers named in the JSON [S]; behaviour [U]):
- `interpolated` / `upscale` with cell 4x8x4: the expensive function is evaluated only at cell corners and trilinearly interpolated. For one 16x16 chunk column slice the interpolator keeps two Z-slices of corner values and advances in X, so each corner is computed once.
- `flat_cache`: a 2D value cached at the quart (4x4) grid for the whole chunk plus padding. `cache_2d`: last (x,z) result. `cache_once`: result per evaluation context. `cache_all_in_cell`: per cell. In the new data these appear as `cache` or inlined.
- Results written per cell: `onSampledCellCorners`, then the 4x8x4 sub-voxels read interpolated density and a material rule picks the block (Noisium's page [S] says it bypasses `setBlockState` for direct palette writes, about 20–30 % faster `populateNoise` per its own docs, quoted from `clones/noisium/docs/NoiseChunkGenerator_populateNoise_optimisation.md`).

Noise-cell count per 32^3 Espresso chunk (computed): vanilla corners are `(32/4 + 1) x (32/8 + 1) x (32/4 + 1) = 9 x 5 x 9 = 405`, or 12.4 per 1000 voxels. A 16x16x384 vanilla column has `5 x 49 x 5 = 1225` corners for 98,304 voxels, the same density.

## 3. Caves, aquifers, ore veins

All from `density_function/overworld/caves/*.json` and `final_density.json` [S].

**Cheese caves** (big caverns), inside the deep-solid branch (`sloped_cheese >= 1.5625`):
```
cheese_term = 4 * square(noise(cave_layer, y_scale 8))
            + clamp(noise(cave_cheese, y_scale 0.667) + 0.27, -1, 1)
            + clamp(sloped_cheese * -0.64 + 1.5, 0, 0.5)
```
`cave_cheese`: first octave -8, 9 slots with 7 non-zero [S]. The third term makes caves less likely near the surface (where `sloped_cheese` is small). The `cave_layer` square with y_scale 8 produces thin horizontal sheets (stratified voids).

**Spaghetti 3D** is part of `entrances`:
```
entrances = cache(min( noise(cave_entrance, xz 0.75, y 0.5) + 0.37 + y_gradient(-10: 0.3, 30: 0),
                       spaghetti_roughness + clamp( max(|A|, |B|) + (thickness*-0.0115 - 0.0765), -1, 1) ))
A, B = interval_select(spaghetti_3d_rarity, thresholds -0.5, 0, 0.5, 4 scales 1.33, 1, 0.667, 0.5) applied to spaghetti_3d_1 / _2
```
Two tubes, `|A|` and `|B|`, cross where both are near zero, which carves a thin 1D curve in 3D (the zero set of two noises). `max(|A|,|B|) < threshold` is the tube. The rarity noise picks the scale per region (thin dense vs thick sparse tunnels). Entrances are those tunnels plus a vertical gradient that reaches the surface (values add 0.3 at y = -10 and 0 at y = 30).

**Spaghetti 2D**: the same trick in 2D: `max(|selected spaghetti_2d noise| + thickness_mod*0.083, cube(|elevation_noise*8 + y_gradient(-64: 8, 320: -40)| + thickness_mod))`. Horizontal sheets of tunnels that follow a slowly varying elevation.

**Noodle caves**: four noises (`noodle`, `noodle_thickness`, `noodle_ridge_a`, `noodle_ridge_b`) all wrapped in `upscale(cell 4x8x4)` and gated to y in [-60, 321): if `noodle >= 0` then `-(0.075 + 0.025*thickness) + 1.5*max(|ridge_a|, |ridge_b|)` (very thin tunnels, radius about 1 block), otherwise a constant 64 (no cave).

**Pillars**: `pillar*2 + (-pillar_rareness - 1)` times `cube(pillar_thickness*0.55 + 0.55)`; values below 0.03 are replaced by -1e6; the final `max(...)` forces solid columns through caves in rare regions.

**Combination**: `max( min( min(cheese_term, entrances), spaghetti_2d + roughness ), pillars_filtered )`: any cave noise being low carves, pillars force solid.

**Legacy carvers** (pre-1.18 and still present for ravines/canyons and the Nether until later versions [U]): sweep a random-walk worm and carve spheres/ellipsoids; they need neighbour chunks because a worm starts up to 8 chunks away [U]. This is chunk-local only when re-simulated for every chunk in range, which vanilla does (each chunk re-runs the carver for neighbours within 8 chunks, carving only its own area) [U]. Noise caves replaced them in 1.18 because they are a pure function of (x,y,z) [U].

**Aquifers** [algorithm U, fields S]: the world is cut into a grid of 16 (x) x 12 (y) x 16 (z) cells; every grid cell has one pseudo-random centre derived from a hash of the cell index. For a voxel with density <= 0 the sampler finds the three nearest centres, reads a fluid level for each (from the preliminary surface level at that column: the data defines `surface_level = overworld/preliminary_surface_level`, plus `fluid_level_floodedness` y_scale 0.67, `fluid_level_spread` y_scale 0.714, `lava` and `barrier` y_scale 0.5 noises), and uses the distance difference between the nearest two as a pressure term to place barrier stone between different fluid regions. The result decides water, lava or air per voxel. `exclusion` (`min(-0.225 - erosion, max(depth - 0.9, 0))`) switches aquifers off in certain terrain [S]. Chunk-local: yes (pure function of position), but it needs the preliminary surface level for neighbouring columns, which vanilla provides through `chunk_surface_level = upscale(preliminary_surface_level, x/z cell 16, y cell 1)` [S].

**preliminary_surface_level** [S]: `find_top_surface(cell_height 8, lower_bound -64, upper_bound = clamp(-128*(0.2734375/factor - offset) + 128, -40, 320))` over a density that drops `base_3d_noise` and the caves, `clamp(4*quarter_negative((depth)*factor) - 0.703125, -64, 64)` with the same slides. So vanilla itself scans for the surface in a 2D-only function with an analytic upper bound from (factor, offset). That is the design Espresso's far tier needs.

**Ore veins**: the vein noises (`vein_toggle`, `vein_ridged`, `vein_gap`) put copper above y = 0 and iron below, with a ridged-noise mask and a gap noise that thins ore inside veins [U]. Not found in this data snapshot (gap).

## 4. Multi-noise biome source [all U, wiki not reachable]

- Six parameters per sample: temperature, humidity (the `vegetation` noise), continentalness, erosion, weirdness (the `ridges` noise), and depth (computed from y and offset, which is why underground biomes like lush caves and deep dark exist), plus a per-biome `offset` scalar.
- Each biome is a list of `ParameterPoint`s, each parameter an interval [min, max]. The overworld preset hard-codes hundreds of points through `OverworldBiomeBuilder` (temperature 5 bands, humidity 5 bands, continentalness 7 bands, erosion 7 bands, weirdness 8 bands).
- Lookup: nearest point by a 7-D squared "fitness" distance (distance to the interval, 0 inside it). `Climate.ParameterList` stores an R-tree over the 7-D boxes and a per-thread "last result" hint; this is a nearest-box search with branch-and-bound.
- Resolution: biomes are sampled at quart (4x4x4 block) cells, using the centre of the quart. A 32^3 chunk has 8^3 = 512 quart cells, with the 2D parameters shared along y.
- Surface rules (`surface_rule` in the noise settings, applied after terrain): a decision tree over biome, y, a stone-depth counter from the top, a surface `noise` and water level, giving grass/dirt/sand/gravel/terracotta bands. Pure function of (x, y, z, biome, depth below surface) [U].

Chunk-local: yes. LOD: the 2D parameter fields are smooth, so far LOD can evaluate them at any spacing. Cost: 64 columns x (continents 9 + erosion 4 + ridges 3 + temperature 2 + vegetation 2 + shifts about 6 = 26 octave evals), about 1.7k per chunk (estimate), plus 512 table lookups.

## 5. Why vanilla generation is chunk-local, and what is not

**Chunk-local by construction.** Terrain, caves (noise), aquifers (position-hashed centres), biomes and surface rules are pure functions of (seed, x, y, z) plus the preliminary-surface lookup. Reading any block needs no neighbour. This is the property ADR-0016 requires.

**Not chunk-local** (vanilla statuses, [U]): `empty, structure_starts, structure_references, biomes, noise, surface, carvers, features, initialize_light, light, spawn, full`.
- **Structures**: a start is decided per chunk by a hash plus placement rules (grid spacing/separation or ring placement), and the piece layout is built from that. Other chunks need only a reference to the start within a search radius (structure references reaches up to 8 chunks); each chunk then draws only the pieces that fall into it. This is 2D, hash-based and deterministic.
- **Features** (trees, ores, lakes, flowers): run in `features` status, with decoration positions inside a 16x16 region offset by +8 blocks from the chunk origin so a feature can extend into neighbours. Vanilla therefore requires the 3x3 chunk neighbourhood to be at least at the carvers status before a chunk can run its features, and features write into neighbours (a 3x3 chunk neighbourhood, i.e. a radius-1 dependency pyramid) [U]. This also makes vanilla results nondeterministic across runs (Mojang bug MC-55596, cited in the C2ME README [S] `clones/C2ME-fabric/README.md`, "non-determinism of vanilla world generation").
- **Light**: a spatial flood-fill across chunk borders, which is why the status pyramid has light statuses at all.

Espresso answer (reasoning, not vanilla): decoration needs a deterministic cell-hash design (each 2D jittered cell decides one feature, and a chunk iterates over the cells within the max feature radius and keeps only its own voxels). That keeps generation chunk-local with bounded padding, with no pyramid. This is Terrain-side work for a later research doc.

## 6. Datapack terrain mods

**Tectonic** [S, `clones/tectonic`]. A datapack plus mod for 21.1 / 26.1 / 26.2, built on Lithostitched [S: `lithostitched/worldgen_modifier/*.json`]. It does not change the architecture; it replaces the density functions with configurable ones:
- New noise parameters (`data/tectonic/worldgen/noise/parameter/*.json`) [S]: `continentalness first_octave -10, amplitudes [1.75,1,2,3,2,2,1,1,1]` (vanilla -9), `erosion first_octave -10, amplitudes [2,1.75,1.5,1.5,1.3,1,1,1,1]` (vanilla -9 with 4 non-zero octaves), `ridge first_octave -8, amplitudes [1,2,1]` (vanilla -7). One octave lower first octave doubles the wavelength: continents base period 4096 blocks instead of 2048 (computed).
- `tectonic:config_noise` density function: `noise((x + shift_x) * scale, 0, (z + shift_z) * scale) * multiplier + offset`, where `scale`, `multiplier`, `offset` come from the user config `NoiseState` [S, `ConfigNoise.java`]. It reduces to `shifted_noise_2d` for the engine's own optimiser.
- Vertical controls: `vertical_scale`, `elevation_boost`, `max_y` (must be a multiple of 16, below 256), `ocean_depth` -0.15 and `deep_ocean_depth` -0.45 defaults [S]. Offset: `vertical_scale = elevation_boost * square(continents_offset_spline) + vertical_scale`, i.e. high continental values get extra relief.
- Selectors: `continent_selector`, `island_selector` (two island noises `island_a`, `island_b`), separate offset splines for continents and islands, merged and clamped by `config_clamp`; `flat_cache(cache_2d(...))` wraps the 2D result [S, `terrain_spline/offset/final.json`].
- Rivers: ridges folded to `[0, 0.2)` mark a river valley; underground rivers are `range_choice` on `y in [32, 72)`, `continent/ridges_folded in [0, 0.2)`, `raw_continents >= -0.17`, `erosion_folded in [0, 0.37)` [S, `underground_river/parameters.json`]. A river is a band around the zero set of the folded ridge noise, no flow simulation.
- Cave toggles: `cave/cheese_enabled`, `noodle_enabled`, `spaghetti_enabled` constants [S file names only].

**Terralith (Stardust Labs), William Wyther's Overhauled Overworld, Lithostitched** [U, not fetched]. Terralith is a closed-source datapack with about 100 added biomes and edits to the multi-noise parameter list and surface rules; per DH's source comment it is a test case for "Terralith world" LOD timing [S below]. Lithostitched provides non-destructive worldgen modifiers so several packs can patch the same noise settings (Tectonic uses it [S]). Neither changes the 6-parameter model.

Take-away [reasoning]: every one of these mods keeps the same structure (2D climate noises, spline to offset, 3D noise on top) and only changes noise scales, spline points and constants. That is evidence the architecture is a stable base; the tunable part is data.

## 7. Distant Horizons: LODs from world generation [S, `clones/dh`]

- Files in `common/.../wrappers/worldGeneration/`: `DhChunkGenerator.java`, `DhInternalServerGenerator.java`, `DhRoughSurfaceGenerator.java`, `RoughSurfaceDensitySampler.java` and `step/{StepStructureStart, StepStructureReference, StepBiomes, StepNoise, StepSurface, StepFeatures, StepTerrain}.java`, a reimplementation of the vanilla status pyramid.
- Modes seen in code: `PRE_EXISTING_ONLY` (only already-saved chunks), `INTERNAL_SERVER` (the server generates and saves the chunk, DH builds the LOD from it), plus the generators that run the vanilla steps up to a chosen target step (`EDhApiWorldGenerationStep`) and the new rough-surface generator. DH calls the vanilla pipeline (Fabric/Forge world generation steps), then converts finished chunks to its own LOD data points. Detail level L uses data columns of 2^L blocks, built by sampling, not by downsampling cached full chunks.
- **Rough surface generator** (`DhRoughSurfaceGenerator.generateSurface`) is the relevant approximation:
  1. Evaluates `final_density` directly (`RoughSurfaceDensitySampler`, a thread-local cached copy of the density function), no cell grid, no caves-stage chunk.
  2. Only every second column in x and z is sampled ("1 in 4 columns"); the others are interpolated from neighbours (`interpHeightFromAdjacentValues`).
  3. `findSurfaceHeightMarching`: step down from the world top by `MARCH_STEP = 8` until `density > 0`, then bisect between the last non-solid and first solid y (`binaryFindSurfaceHeight`), about log2(8) = 3 more samples.
  4. Alternatives implemented but disabled in the code: plain binary search over the whole height (fast, wrong with overhangs and caverns) and binary search plus `sanityCheckSurface`, which probes solid at depths 4/8/16 below and non-solid at heights 4/8/16/32 above.
  5. `smoothUnderwaterSpikes` removes water pockets (`MAX_UNDERWATER_HEIGHT_DEVIATION = 8`); the block is picked per biome from a most-common-block map.
  - Source comment (quoted, no machine detail except "24 cores", "128 render distance", "terralith world", "MC 26.2"): marching "15.9 million // 37 sec", binary search "3.3 million // 23 sec", hybrid "5.3 million // 27 sec". The unit of "million" is not stated; it is probably the number of density samples. Quoted, not verified.
- DH is not a model of "generate far LOD directly at 2^L block voxels with the same silhouette": it runs the full density function at the original scale for each sampled column, so its LOD detail does not get cheaper as it gets coarser, apart from the 1-in-4 column thinning.

**Voxy** [U, not fetched]: a LOD renderer that ingests chunks the client has loaded (and pairs with DH-style generation); it does not define terrain. Not relevant to generation.

## 8. Performance: C2ME, Noisium, Lithium, FerriteCore

- **C2ME `c2me-opts-dfc`** [S file inspection, no numbers read]: a density-function compiler. Pipeline in `common/ast/`: `McToAst` turns the vanilla `DensityFunction` tree into an AST, optimisation passes `FoldConstants`, `TreeNormalization`, `BranchElimination`, `CacheElimination` (removes cache wrappers that a flat cache does not need), then `gen/jvm/BytecodeGen` emits JVM bytecode ("compiles density functions ... to JVM bytecode to increase performance by allowing JVM JIT to better optimize", config text). `gen/opencl/OpenCLCEmitter` is an OpenCL C emitter (GPU path, experimental, `c2me-opts-accel-opencl`). It also supports splines (`SplineSupport.java`), emits per-region `fillArray` style batch evaluation (`ICoordinatesFilling`) [U]. Other modules (names only [S]): `c2me-opts-worldgen-biome-cache`, `c2me-opts-math` / `c2me-opts-natives-math` (native math), `c2me-threading-lighting`, `c2me-rewrites-chunk-system`, `c2me-opts-scheduling`. README: it keeps vanilla parity and uses multiple cores for chunk gen and IO.
  Takeaway for Espresso: we do not need a runtime compiler. The same passes (constant folding, dead branch removal when a range makes a `range_choice` constant, cache elimination) are what a hand-written Rust function plus interval analysis would do. Interval/range analysis (already documented in the Voxel Tools notes) matches the `BranchElimination` idea.
- **Noisium** [S, `clones/noisium/README.md` and docs]: direct palette writes in `populateNoise` (documented as 20–30 % speedup of that function, quoted from the repo's doc), faster biome population, faster block-state sampling, 1:1 parity. The Modrinth page gives "a few seconds to a few dozen seconds faster" with no per-chunk figure. Already covered by the existing "direct palette writes" note.
- **Lithium, FerriteCore** [U]: Lithium optimises general game logic (collision, entity, block lookup) and some worldgen internals; FerriteCore reduces memory (block state objects). No terrain algorithm to copy.
- **Per-chunk generation time**: no published figure was fetched. Do not use a number from memory. Commonly quoted values are ms per 16x16x384 chunk single-threaded for vanilla, but I have no source in this session (gap).

## 9. Cost estimate for a vanilla-style density per 32^3 chunk (computed from the data above)

Octave evaluation counts per sample (non-zero octaves read from the JSON [S]; blended noise octaves [U]):

| Term | Octaves | Where evaluated |
|---|---|---|
| shift_x + shift_z | about 6 | per column, 2D |
| continents / erosion / ridges | 9 / 4 / 3 | per column, 2D |
| jagged (all 16 octaves) | 16 (6 if truncated) | per column, 2D |
| base_3d_noise (blended) | 24–40 | per corner, 3D |
| entrances (entrance, roughness x2, rarity, 2 selected tubes, thickness) | about 7 | per corner, 3D, almost everywhere |
| deep-solid cave stage (layer 1, cheese 7, spaghetti_2d 3 in 3D, pillars 3) | about 14 | per corner where `sloped_cheese >= 1.5625` |
| noodle (4 noises) | 4 | per corner for y > -60 |

- 2D per column about 38 octave evals; 81 corners (9 x 9) per chunk column gives about 3.1k octave evals, once per vertical column of chunks and not once per chunk. At 4-block cells we could also compute the 2D part on a 4-block grid and bilinearly interpolate, which vanilla does not do.
- 3D per corner: 35–65 octave evals (computed as 24–40 + 7 + 0–14 + 4). 405 corners gives 14k–26k octave evals per chunk, about 20k typical.
- Cost per Perlin octave (estimate, to be replaced by a measurement): about 8 ns scalar, about 1.5 ns effective with `wide::f32x8` or fastnoise2, from the quoted 5–8x SIMD gain in existing docs. Total: about 160 µs scalar and 30–40 µs SIMD per fully-3D chunk, plus interpolation (32,768 voxels, trilinear in 3 lerp stages, estimate 20–40 µs SIMD) and material/surface rules.
- Budget reference (computed): current generation 260 ms wall x 12 threads / 6,125 chunks is at most 0.51 ms core time per chunk (upper bound, assumes all 12 threads busy for the whole 260 ms).
- Conclusion: full vanilla 3D at every corner is affordable only for chunks that straddle the terrain surface band. Vanilla spends the same effort for air above the mountains and solid rock 200 blocks down. Two cuts: (1) chunk column 2D pass gives `y_surface = 128 * offset + 128` (computed) and a bound `±δ` from the maximum of `|base_3d_noise|` divided by the local slope `4 * factor / 128`, so chunks entirely above `y_surface + δ` are air and chunks entirely below `y_surface - δ - cave_depth` are solid with no 3D evaluation; (2) caves only for chunks below the surface by more than the entrance blend (entrances fade out between y = -10 and 30 in absolute y, but the cheese term already fades with `sloped_cheese`).

## 10. Candidate sections

### A. Spline shaping of climate noises (Adopt-candidate)
- **What**: three 2D noises (continents, erosion, ridges) feed nested cubic splines that return offset, factor, jaggedness; `y_surface = 128*offset + 128`.
- **Algorithm to implement**: a flat spline node table (`struct Node { loc: f32, value_or_child: f32, child: u16, deriv: f32 }`), evaluate depth-first with binary search over locations; Hermite between two points. Points can be taken directly from the vanilla JSON above (offset 53 nodes, factor 49, jaggedness 16 [S]); our own splines are fine since only the shape matters.
- **Chunk-local / deterministic**: yes, pure function of (seed, x, z); precise far from the origin if the integer cell and f32 fraction are used for the noise lookup.
- **Far LOD without changing the silhouette**: the finest continents octave has wavelength about 8 blocks, erosion and ridges at least 128 blocks (computed above). At spacing 2^L >= 16 blocks, drop continentalness octaves with wavelength < 2^(L+1) blocks; the discarded amplitude is a small share of the total (the finest octaves carry the least amplitude), so the splines see almost the same input. Drop jagged octaves the same way. Spline outputs are nonlinear, so check the error on a test map before shipping.
- **Cost**: about 38 octave evals plus 3 splines per column (estimate), about 3k octave evals for the 81 corners; on a 4-block grid and interpolated it is much less.
- **Measure**: ns per column for the 2D pass at 4-block and 1-block spacing; heightmap difference between full and truncated octaves at LOD 3–6.

### B. Folded ridges and valleys (Adopt-candidate)
`peaks_valleys = -3 * (|(|r| - 2/3|| - 1/3)` [S]. It provides: sharp ridge lines (peaks) and rivers along the zero crossings of `r` (Tectonic: river where the folded value is in [0, 0.2) [S]). One extra noise (the ridge, 3 non-zero octaves) and a few flops. Chunk-local: yes. LOD: yes (finest ridge feature 128 blocks, [computed]). Measure: erosion of ridge lines (distance field) at the truncated octave count.

### C. Density equation, squeeze, slides, trilinear cells (Prototype, surface band only)
- **Algorithm**: `density(y) = 4*qneg((depth(y) + jag) * factor) + n3(x,y,z)`; bottom slide and top slide as lerps; `squeeze` before caves; evaluate on a 4x8x4 corner grid and trilinearly interpolate (the vanilla scheme [S]); solid if > 0.
- **Chunk-local**: yes (padding of one extra cell layer, so corners are shared with neighbours by recomputation, not by communication).
- **LOD**: the 3D noise term is the problem. At spacing >= 8 blocks, a 3D octave with a wavelength below the cell size aliases. Use a 2D bound for LOD (see H) and enable the 3D term only for LOD 0–1.
- **Cost**: 405 corners x (24–40 blended octaves) = 10k–16k octave evals per chunk (computed) for the blended noise alone; heavy compared with a 2D heightmap. Replace `old_blended_noise` with 3 or 4 octaves of Perlin (the blended noise exists for legacy parity; we do not need parity). That brings it to about 405 x 4 = 1.6k octave evals per chunk (computed).
- **Measure**: ns per chunk for 2D-only, 2D + 4-octave 3D noise in a band of ±16 blocks around the surface, versus full.

### D. Caves (Prototype: cheese + spaghetti; Reject noodle for now)
- **Cheese**: `n_cheese + 0.27 + 4*layer^2 + clamp(1.5 - 0.64*sloped, 0, 0.5) < 0` carves. **Spaghetti**: `max(|A|, |B|) < thickness` with `A`, `B` two independent noises; carves a 1D tunnel network. One octave per tube is enough for the shape (the noise params have one amplitude slot [S]).
- **Chunk-local**: yes. **LOD**: caves are invisible at LOD >= 2, skip them. **Cost**: spaghetti about 7 octaves per corner (computed from the entrance function) and cheese 7–9 octaves, each about 20 % of the blended-noise cost; gate by depth below surface (the vanilla branch is `sloped_cheese >= 1.5625`, i.e. deep solid, to restrict the cheese stage [S]).
- Noodle: 4 more noises for tubes about 1 block wide, mostly a visual detail; costs 4 octaves per corner everywhere above y = -60. Reject until a measurement shows it matters.
- **Measure**: chunk time with and without caves at depth bands; fraction of chunks that need the cave stage.

### E. Aquifers and ore veins (Watch)
Per-voxel nearest-centre search over a 16x12x16 hashed grid with three noises for levels; chunk-local and deterministic but a per-air-voxel cost. Not needed until water and lava below sea level matter. The 2D `preliminary_surface_level` (find top surface with analytic upper bound) is the reusable piece. Ore veins: a ridged-noise mask [U]; Espresso's cheaper option is a hashed-cell blob placement.

### F. Biome source (Prototype with LUT)
Replace the 7-D R-tree with a 3-D or 4-D lookup table built at start-up: quantise temperature (5), humidity (5), continentalness (7), erosion (7), weirdness (8) bands, cached in an `[u8; N]` index (5 x 5 x 7 x 7 x 8 = 9,800 entries, computed); lookup is one multiply-add index, no memory scan. Depth only for underground biomes. Quart-resolution sampling (4^3 blocks) matches the 4-block cell grid. LOD: biomes at 2D, any spacing. Measure: biome pass per chunk (64 columns) in µs.

### G. Status pyramid and 3x3 neighbourhood (Reject)
It is what vanilla needs because features and carvers write across chunk borders. Espresso should instead use cell-hashed placement with bounded padding (section 5).

### H. DH rough-surface sampler (Adopt-candidate for the horizon tier)
For the heightmap/impostor tier: per sampled column, scan down from the top in steps of 8 blocks and bisect (about 48 + 3 density samples worst case for a 384-block world, computed; terrain with a good starting bound needs far fewer). Espresso can start from the analytic 2D bound `y_surface + δ` (section 9), which turns the scan into about 3–6 samples with the 3D term (computed from `δ / 8`). Chunk-local: yes. Silhouette: it finds the actual top-most solid point, so overhangs come out right, at the cost of more samples; DH's own comment says plain binary search is fastest but wrong with overhangs [S].
**Measure**: density samples per column with bound + march vs. 2D-only.

### I. C2ME density-function compiler (Watch)
Concept only. In Rust, write the router as a `const` function tree and let LLVM fold constants; keep range-based branch elimination as an explicit interval pass. Cost: a build-time effort; value depends on how many cave variants exist.

### J. Tectonic-style knobs (Adopt-candidate)
Expose `scale`, `multiplier`, `offset` per noise and `vertical_scale`, `ocean_depth` as a `WorldgenParams` struct. Larger continents is one first-octave step (period x2, computed). Cheap and gives tuning space without code changes. Chunk-local: yes.

## 11. Gaps

- minecraft.wiki was unreachable (`client_error` for four pages), so the density-function type definitions (`squeeze`, `quarter_negative`, `interval_select`, caches), `BlendedNoise` octave counts, `shift` noise parameters, the aquifer algorithm, `Climate.ParameterList` / R-tree and the generation-status list are **[U]**. They should be checked against the 26.x deobfuscated source before implementing.
- Ore-vein functions were not found in the fetched snapshot; the game version string of the data was not checked.
- No published per-chunk generation timing, no C2ME or Noisium benchmark number, and no DH timing beyond the quoted source comment was read.
- Terralith, William Wyther's, Lithostitched and Voxy were not fetched; claims about them are [U] (Tectonic's use of Lithostitched is [S]).
- The Kniberg talk ("Reinventing Minecraft world generation", 2022) was not fetched; nothing in this doc depends on it.
- The octave-eval cost per Perlin sample (about 8 ns scalar) and all section 9 totals are estimates. They need an `vx bench` run on the S2 SIMD noise before any is used in an ADR.
- Spline node counts were counted from `"coordinate"` occurrences in the JSON; the nesting depth and exact point sets were not otherwise audited.
