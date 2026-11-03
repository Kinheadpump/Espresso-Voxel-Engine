# Improvement backlog

The ranked list of unscheduled ideas for Espresso, merged on 2026-11-03 from two sources: the engine survey (former `docs/PROPOSALS.md`, ids P-01..P-11, sources in `docs/research/engine-survey-2026-11.md`) and the 2024–2026 research survey (ids R-01..R-13, sources in `docs/research/recent-2024-2026/`). Duplicates are merged into one `B-nn` item that lists every origin id. Nothing here is accepted: an item becomes work by getting a roadmap slot in `docs/ROADMAP.md` and, if it is a design choice, an ADR in `docs/DECISIONS.md`.

Format and labels follow `docs/DOC-GUIDELINES.md`: numbers are **measured**, **quoted**, **computed** or **estimate**; effort is S (under a day), M (a few days) or L (rewrites a subsystem).

## Ranking

Ranked by expected gain on the core goal (view distance and frame time) per unit of effort, with prerequisites first. Since 2026-11-03 the ranking can lean on the first hardware baseline (`noah-msi-rtx2060`, details in `docs/PERFORMANCE.md`). What it shows, all measured, `default` scenario unless noted:

- The GPU is far from busy: terrain pass ~0.52 ms, frame wall ~1.25–1.30 ms. In `far` the terrain pass is ~1.94–1.98 ms.
- Frame CPU encode grows with draw count although the renderer issues one `multi_draw_indirect` call: 171–182 µs (`small`, 2,506 draws), 480–497 µs (`default`, 9,492 draws), 1.34–1.74 ms (`far`, 36,275 draws). That is about 40–70 ns per draw (computed). Explained and fixed, see B-02.
- World generation (~260–267 ms) costs about 2.5× meshing (~103–110 ms) on 12 threads, so generation, not meshing, dominates region build time.

| Rank | Id | Item | Origin | Effort | Expected benefit (label) | Decided by |
|---|---|---|---|---|---|---|
| 1 | B-01 | Occlusion counters and `cave` / `mountain` / `fly-fast` scenarios | R-01 | S | None by itself; makes B-05, B-06 and Hi-Z comparable | Prerequisite; correctness = culling-off image identity |
| 2 | B-02 | ~~Find why frame CPU encode scales with draw count~~ Done: wgpu indirect validation, off in release (ADR-0015) | own finding from the baseline | S | −1.1 ms CPU per frame at `far` (computed from measured medians) | encode µs vs draws across `small`/`default`/`far` |
| 3 | B-03 | Decode straight into the padded mesher input | P-01 | S–M | Up to ~40% more meshed chunks/s (estimate) | divan `gather_and_mesh_surface_chunk`, meshing ms |
| 4 | B-04 | Streaming rules: forward-frustum priority, teleport boost, slot-epoch staleness, retire-frame frees | R-08 | M | Fixes pop-in order and wasted jobs (quoted, qualitative) | time-to-fill-view, wasted-job %, frame p99 while flying |
| 5 | B-05 | Upload time budget and suballocated buffers | P-05 | M | No frame spikes while streaming (quoted, qualitative) | frame p99 and max while flying, upload ms per frame |
| 6 | B-18 | Integer-lattice noise kernel: power-of-two wavelengths, own hash, SoA column batches, identical bits on scalar/SSE2/AVX2 | worldgen survey 2026-11 | M | Precision at any distance (ADR-0016) and the prefix property LOD needs; 2D height ≤ 20 ns per column (target) | `vx bench` world generation within noise of v1 or faster; cross-width hash test |
| 7 | B-06 | Compute face-direction + frustum cull, atomic-compacted draws | R-02 | M | Up to ~2× fewer vertex invocations for far chunks (estimate) | quads submitted, terrain-pass GPU ms, identical image |
| 8 | B-07 | Occlusion culling: pick one of visibility graph, occluder quads, two-phase Hi-Z | R-03, P-02, roadmap 11 | M each | +5–25% sections culled for the graph (quoted) | B-01 counters in `cave`/`mountain`, net GPU ms |
| 9 | B-20 | LOD-direct sampling: per-octave range bounds and octave truncation with a tail bound | worldgen survey 2026-11 | M | Far LOD within ~0.2–0.35 voxel of level 0 (computed); 2D cost at LOD ≥ 3 ≤ 60% of level 0 (target) | tail-bound test, uniform-chunk ratio, gen µs per far chunk |
| 10 | B-08 | Cubyz-style LOD: one ring per level, real-child downsample, border faces instead of skirts | P-04, R-04, R-05 | M–L | View distance 2^L × level 0 per extra level (computed) | max radius at fixed frame and VRAM budget, seam test |
| 11 | B-19 | Climate fields with spline terrain shaping and folded ridges (Minecraft 1.18 style, Tectonic scale) | worldgen survey 2026-11 | M | Continents, mountain ranges, plains from 2D work only | v2 world generation ≤ 1.25× v1; screenshots |
| 12 | B-09 | Vertex AO: 26-neighbour gather, AO-safe greedy merge, 2-bit corner AO in free quad bits | P-03, R-06 | M | AO at zero extra bytes; mesher +0–15% (quoted) | quad count, mesh µs, opaque GPU ms vs SSAO |
| 13 | B-10 | T-junction seam fix by quad dilation | P-11 | S | No sky-coloured pixel leaks | pixel-count test on headless screenshots |
| 14 | B-25 | Surface rules, biome lookup table, water at sea level | worldgen survey 2026-11 | S–M | Biomes and coasts at per-column cost | v2 ≤ 1.35× v1; screenshots |
| 15 | B-11 | 3D noise: coarse density grid + interpolation + masks, interval range analysis, direct palette writes | P-09, R-09 | S–M | 4–8× fewer noise evaluations per chunk (estimate) | gen µs per chunk on a cave seed |
| 16 | B-21 | Erosion filter (Johansen, Phacelle) on damped fBm with analytic gradient | worldgen survey 2026-11 | M | Eroded valleys and gullies without simulation; ~3.3 kflop per column at 5 octaves (estimate) | ns per column, v2 ≤ 1.6× v1 with B-22; screenshots |
| 17 | B-22 | Rivers: folded-ridge bands first, then a jittered-grid river graph | worldgen survey 2026-11 | M–L | Continuous rivers at ~1 kflop per column (estimate) | no-uphill test on a 16 km square, seam test |
| 18 | B-23 | Overhangs: 3D jaggedness band on the 4×8×4 lattice | worldgen survey 2026-11 | S–M | Cliffs and arches at ≤ 405 lattice points per surface chunk (computed) | gen µs per surface chunk, uniform ratio |
| 19 | B-12 | Persistence: edited chunks only, palette words + zstd/lz4, region files | P-08, R-07 | M | 65 KB → 5.7 KB per cave chunk in Stardance (quoted) | bytes per chunk, load vs regenerate ms, hash round-trip |
| 20 | B-13 | Batched edit API | P-07 | M | One remesh per chunk per frame however many voxels change | edits/s, edit-to-visible latency |
| 21 | B-24 | Hash-cell decoration: trees and structures with per-chunk feature lists | worldgen survey 2026-11 | M | Cross-chunk features with no neighbour access | zero cut-off trees over 10⁴ chunks; ≤ 10% of world generation |
| 22 | B-14 | Attribute channels with the storage shape | P-06 | M | Light and metadata cost zero in uniform chunks | voxel + light heap MiB at radius 32 |
| 23 | B-15 | Starlight-style light propagation | R-13 | M | ~2× faster than vanilla Minecraft 1.20 (quoted) | light µs per chunk vs 174 µs torch baseline (quoted) |
| 24 | B-16 | 1-bit occupancy volume for ray-marched AO, shadows, GI | P-10, R-11 | L | Off-screen-correct AO without a second scene format (estimate) | GPU ms vs SSAO + CSM at equal quality |
| 25 | B-17 | Voxy hierarchical GPU LOD traversal | R-12 | L | Not quoted | GPU ms, nodes visited, request-queue latency |

Taken up elsewhere: **R-10** (texture array with repeat sampler instead of an atlas) is the basis of the texture concept, `docs/TEXTURES.md` and ADR-0014 (PR #4).

Why this order:

1. **B-01** is benchmark infrastructure. It fits the "measure before features" rule and every culling item is judged with it.
2. **B-02** is the only cost the baseline shows that nobody has explained. It is cheap to investigate and may be the largest CPU frame cost at `far`.
3. **B-03** is a small, CPU-only change measurable today with divan.
4. **B-04** and **B-05** are the design of roadmap items 7 and 8 (streaming core).
5. **B-06** is the cheapest step of ADR-0010 (roadmap 9). **B-07** follows only when B-01 shows real occlusion in `cave` and `mountain`.
6. **B-08** is the concrete ADR-0011 prototype.
7. Looks (**B-09**, **B-10**) and world features (**B-11** to **B-15**) wait for streaming. **B-16** and **B-17** are research-sized.
8. World generation (added 2026-11-03, `docs/WORLDGEN.md`): **B-18** ranks high because ADR-0016 needs its precision fix anyway and every later stage relies on it. **B-20** sits just before B-08 because LOD rings need it. **B-19**, **B-25**, **B-21**, **B-22**, **B-23** and **B-24** are CPU-only and run as a parallel track behind `--terrain v2`, so they never block the GPU sprints.

---

## B-01 Occlusion counters and new scenarios

**Origin:** R-01 (`docs/research/recent-2024-2026/gpu-culling.md` §8). Relates to roadmap items 3, 9, 11.

**Status (2026-11-03):** scenarios and the CPU/pixel ladder are done (roadmap 1, `docs/BENCHMARKING.md` § Culling ladder). Still open: GPU-atomic counters for the graph and Hi-Z rungs and the culling-off image-identity check; they come with B-06 and B-07.

**Change:**
- Scenarios: `cave` (camera inside a hill), `mountain` (ridge in front of a wide valley), `fly-fast` (straight flight plus a teleport), later `save-load`.
- Counter ladder per frame, written by GPU atomics and read back 2–3 frames late through a staging buffer so the main thread never stalls: `chunks_resident` → `chunks_frustum` → `chunks_graph` → `chunks_hiz` → `draws_emitted`, plus `quads_submitted`. Report ratios to `quads_total`.
- Net benefit: always report cull-pass GPU ms next to the terrain-pass ms saved.
- Correctness: render with culling off and on and require identical images, on orbit frames, the `cave` scenario and a random-solid-faces torture world.

**Expected:** no speed-up by itself.

**Effort:** S.

**Decided by:** prerequisite, not a trade-off. The current `far` orbit is above ground and probably shows little occlusion (estimate).

## B-02 Frame CPU encode scales with draw count

**Origin:** own finding from the `noah-msi-rtx2060` baseline (2026-11-03).

**Status (2026-11-03): found and fixed.** Cause: wgpu 30 keeps `VALIDATION_INDIRECT_CALL` on in release, and `multi_draw_indirect` then runs `DrawBatcher::add` once per draw on the CPU (wgpu-core `command/render.rs`, read in source [S]). With it off, frame CPU record drops 81% at `default` and 94% at `far` with the GPU terrain pass within noise (measured, `docs/PERFORMANCE.md`). Decision in ADR-0015. Per-draw slope with validation on: ~30 ns per draw (computed from record 147 µs at 2,506 draws and 1,166 µs at 36,275 draws).

**Now (measured, RTX 2060, Vulkan, median of repeat medians, run 1 / run 2):**

| Scenario | Draws | Frame CPU encode | Frame GPU terrain pass |
|---|---|---|---|
| `small` | 2,506 | 171 / 182 µs | 170 / 169 µs |
| `default` | 9,492 | 497 / 480 µs | 522 / 520 µs |
| `far` | 36,275 | 1,740 / 1,341 µs | 1,943 / 1,976 µs |

`TerrainRenderer` records exactly one `multi_draw_indirect` call (`crates/vx-render/src/terrain.rs`) and builds the indirect buffer once on upload. CPU encode should therefore be flat in draw count, but it grows almost linearly, ~40–70 ns per draw (computed).

**Hypotheses [U]:** wgpu's indirect-draw validation or per-draw bookkeeping in wgpu-core for `multi_draw_indirect`; or the time is spent in `queue.submit`, which the encode metric includes (`docs/BENCHMARKING.md`).

**Change:** add a Tracy zone or `profiling` scope around encode and submit separately (roadmap 5), sweep draw count at a fixed scene, and check the wgpu 30 source for per-draw work in `multi_draw_indirect`. If it is validation, measure with it disabled in release (if wgpu allows) and record the trade-off in an ADR.

**Expected:** unknown. Upper bound is the whole encode cost, 1.3–1.7 ms per frame at `far` (measured). B-06 also shrinks it by emitting fewer draws.

**Effort:** S to find, unknown to fix.

**Decided by:** frame CPU encode µs at `small`, `default`, `far` before and after, `bench-compare` verdict outside noise.

## B-03 Decode straight into the padded mesher input

**Origin:** P-01. Source: 0fps on iteration cost (`docs/research/engine-survey-2026-11.md`).

**Now (measured, cloud VM, divan, single thread):** per meshed surface chunk, 17–24 µs decoding the palette into a dense 32³ scratch, then 29–35 µs copying it into the 34³ padded array and reading six border slabs through `ChunkVoxels::get` (one palette lookup per voxel, 6 × 1024 calls). The mesher itself takes 33–39 µs. Data movement costs more than meshing.

**Change:**
1. `ChunkVoxels::decode_into_padded`: unpack rows straight into the padded layout. Removes the dense scratch and a 64 KiB copy.
2. Border slabs: `Uniform` neighbour → `fill`. `Packed` neighbour → unpack whole x-rows (the ±Y and ±Z slabs are contiguous rows; ±X slabs are strided single values).
3. Later, if the mesher's column build shows in profiles: build the three opaque bit-column arrays during decode, using a per-palette opacity bitmask, so `build_opaque_columns` stops scanning 39,304 voxels.

**Expected (estimate):** combined gather + decode drops from ~46–59 µs to roughly 15–25 µs, so meshed chunks/s on the worker pool rises by up to ~40%.

**Effort:** S–M.

**Decided by:** new divan bench `gather_and_mesh_surface_chunk` (before and after, same machine and session) and meshing ms in `vx bench` at `far`. The mesher-vs-reference test must stay green.

## B-04 Streaming rules

**Origin:** R-08 (`storage-gen-streaming.md` §4: Paper/Moonrise priority commit, concinnity streaming system). Design input for roadmap item 7 and ADR-0005; overlaps B-05.

**Change:** priority by forward-frustum zones with delayed far loads, a priority boost after teleports, a slot-epoch check that drops stale job results (matches ADR-0005's generation counter), and freeing GPU ranges only after the frame that last used them retires.

**Expected (quoted, qualitative):** correct pop-in order and less wasted work. No numbers in the sources.

**Effort:** M.

**Decided by:** time-to-fill-view, wasted-job %, p99 frame ms while flying (`fly-fast` from B-01).

## B-05 Upload time budget and suballocated buffers

**Origin:** P-05 (Voxel Tools). Confirms roadmap item 8.

**Idea (quoted, Voxel Tools):** cap main-thread upload work per frame (default ~8 ms) and avoid per-mesh buffer allocation and deallocation, which spikes on Vulkan.

**Fit:** roadmap item 8 already plans `offset-allocator` with a byte budget per frame. The new points are a budget in time as well as bytes, and freeing only through the allocator, never through `wgpu::Buffer` drops per chunk.

**Now (measured, RTX 2060):** whole-region upload is 13.9–15.2 ms at `default` and 59–66 ms at `far`, with 33–50% repeat spread, so upload is the noisiest metric on that machine.

**Effort:** M.

**Decided by:** frame p99 and max while flying at sprint speed for 60 s, upload ms per frame, fragmentation (free bytes in the largest free block) after the flight.

## B-06 Compute face-direction and frustum cull

**Origin:** R-02 (`gpu-culling.md` §1, Voxy `cmdgen` via DeepWiki, secondary [S]). First step of ADR-0010, roadmap item 9.

**Change:** one compute thread per chunk tests the chunk AABB against the frustum and the camera position against each face direction (+X faces are visible only if `camera.x > chunk.min.x`, six compares, no matrix maths), skips empty directions and appends draws with `atomicAdd`. Draw with `multi_draw_indirect_count` where available; otherwise the same pass writes zero-instance draws for plain `multi_draw_indirect`.

**Expected (estimate):** about half the direction ranges of every chunk face away from the camera, so vertex invocations drop by up to ~2× for far chunks. Whether that saves GPU ms depends on the vertex stage being the bottleneck. It also cuts the draw count that B-02 found to cost CPU.

**Effort:** M.

**Decided by:** `quads_submitted`, terrain-pass GPU ms p50/p99 on a real GPU, and an image identical to culling off.

## B-07 Occlusion culling: one of three candidates

**Origin:** R-03, P-02, roadmap item 11 (ADR-0010's Hi-Z phase). The three are alternatives (or a CPU pre-pass plus a GPU pass); pick with B-01's numbers.

**a. Voxel visibility graph (R-03).** Sources: Hitchman, "Voxel Grid Visibility" (2025-11-03), Sodium PR #2811 and 0.9 async graph culling [S]. Per chunk, store which of the 15 face pairs connect through air (one flood fill per chunk at mesh time); BFS from the camera chunk, outward only, through allowed pairs, with angle clamping. Quoted: Sodium #2811 cut 4,875 → 4,370 sections (~11%) and BFS time −13%; Hitchman ~60 ns per BFS step, ~0.5 ms for 8,000 sections, an extra 5–25% culled. At radius 32 the BFS visits ≤ 22k chunks, ~1.3 ms on a worker (estimate, linear extrapolation). Helps only with enclosed air; edits invalidate connectivity bits. Runs on the CPU, so it is testable on the software adapter.

**b. Occluder quads with a small software depth buffer (P-02).** Source: procworld (quoted). Per chunk, find a few large quads fully inside solid voxels; each frame rasterise the nearest into a 64×64 CPU depth buffer and drop chunks whose screen rect is fully behind it. The mesher already builds opaque bit-columns, so maximal solid rectangles per axis come almost free as a by-product. No GPU readback (the latency that made Decaf disable its Hi-Z), works on every render tier, can pre-cull before a GPU pass. Conservative and coarse, costs CPU per frame.

**c. Two-phase Hi-Z (ADR-0010, `gpu-culling.md` §2).** Draw last frame's visible set, build the HZB, test the rest, draw the newly visible. wgpu has no min-reduction sampler, so the HZB is a two-pass R32Float compute downsample as in Bevy [S]. With reverse-Z the farthest depth per 2×2 is the min [U]. Bevy flags its version experimental and "can be slower on small scenes" (quoted). Exact to pixel level; needs compute and two-phase logic.

**Effort:** M each.

**Decided by:** draws issued, chunks culled, CPU µs (a, b) or cull-pass GPU ms (c), and terrain-pass GPU ms in `cave` and `mountain`, with the pass on and off. Keep a candidate only if net frame ms falls in at least one scenario and regresses in none.

## B-08 LOD: Cubyz-style rings with real-child downsampling

**Origin:** P-04 (Voxel Tools clipmap), R-04 and R-05 (Cubyz and Voxy source read, `lod.md` §1, §3). Concrete prototype for ADR-0011, roadmap item 12.

**Design:**
- Chunks stay 32³; level L has voxel size 2^L, up to 6 levels (Cubyz, [S]). One ring buffer per level, the same `ChunkWindow` with `pos.rem_euclid(extent)` slots.
- The 32³ mesher is reused per level; quads are scaled by a per-chunk LOD shift (in the per-draw origin buffer, costs nothing per quad) or 3 of the free quad bits.
- Downsample (R-05): replace each 2×2×2 group by one real child block, never an average: non-air over air, opaque over transparent, then highest exposed-face count, with a position-parity tie-break (Cubyz) and a remap table for leaves and plants. Voxy picks max opacity with a top-corner tie-break [S]. The occupancy half can be a bit-or over `u64` columns (estimate: nearly free). Far terrain that is never edited can come straight from the generator at coarse resolution instead.
- Seams: no skirts. The finer chunk builds its border faces against the coarser neighbour's voxels; the coarse side emits nothing extra (Cubyz [S]; neither Cubyz nor Voxy uses skirts, grep-verified). This replaces P-04's skirt plan; skirts stay the fallback if hairline gaps show.
- Hand-off: a coarse node is hidden only when all 8 finer children are meshed (Cubyz bitmask), which avoids holes while streaming.

**Expected (computed):** each extra level doubles reach at the same chunks per level; 4 levels reach 8× level 0's distance for roughly 4× its chunks, fewer in practice because far levels are mostly `Uniform`. With radius 16 and 5 extra levels Cubyz-style reach is ~16k blocks (estimate in `lod.md`). VRAM and frame cost need measurement.

**Effort:** M–L (downsample S, rings M, border-face variant M, scale S, hand-off S). Supersedes the two candidates in ADR-0011 only after a prototype comparison.

**Decided by:** maximum radius at a fixed frame-time and VRAM budget, `lod_build_ms`, quads and heap MiB per level, a seam screenshot test, LOD pop visibility in a flight capture.

## B-09 Vertex AO

**Origin:** P-03 (0fps AO), R-06 (block-mesh-bgm 0.2.4, `meshing.md` §1). Roadmap item 15; the texture concept assumes it lands.

**Technique (quoted, 0fps):** per-vertex AO from two side voxels and one corner voxel, 4 levels (2 bits) per vertex; merge greedy faces only when all four AO values match; flip the quad diagonal when `a00 + a11 > a01 + a10`. block-mesh-bgm does the AO-safe merge inside a binary greedy mesher, quoted at 0–15% extra mesher cost.

**Finding:** our padded gather fills only the 6 face slabs; the 12 edge rows and 8 corner voxels of the 34³ array stay air. Correct AO at chunk borders needs them. That changes ADR-0006: a chunk can be meshed only once its 26 neighbours exist, which delays meshing at the streaming edge by one more ring.

**Bits (computed):** 4 vertices × 2 bits = 8 of the 15 free bits in `PackedQuad` (ADR-0004). No format growth, but it collides with a future 4-byte quad.

**Effort:** M.

**Decided by:** total quads at radius 32, mesh µs per chunk (surface and checkerboard benches), opaque-pass GPU ms, and a screenshot pair, against SSAO on the same frame (GPU cost, no mesh splitting). Pick vertex AO if the quad increase is under ~30% (estimate threshold) and SSAO costs more GPU ms than the extra quads.

## B-10 T-junction seams

**Origin:** P-11. Known issue in `docs/ROADMAP.md`: a faint sub-pixel seam in one headless frame.

**Sources (quoted):** blackflux shows T-junction-free (monotone) meshing costs more triangles and still fails between neighbouring faces in 3D. The cgerikj mesher dilates quads by about one pixel in the vertex shader (`docs/research/voxel-techniques.md`).

**Change:** dilate quad corners by a small clip-space epsilon in `terrain.wgsl`. No mesher change.

**Effort:** S. Do it only once the seam is confirmed on the RTX 2060.

**Decided by:** a headless test that renders a fixed view and counts sky-coloured pixels inside terrain silhouettes, before and after; opaque-pass GPU ms unchanged.

## B-11 3D noise for caves and overhangs

2026-11-03: concrete cave formulas (cheese, spaghetti, noodle) and the 4×8×4 lattice cost are in `docs/research/worldgen-2026-11/minecraft.md` §3 and §9; B-11 is step 6 of `docs/WORLDGEN.md`, generated in terrain v2 only and off at LOD ≥ 2. Overhangs moved to B-23.

**Origin:** P-09 (Voxel Tools range analysis), R-09 (Hytale worldgen notes 2026, Minecraft `NoiseChunk`, Noisium). Builds on ADR-0009.

**Now:** `TerrainGenerator::generate` returns `Uniform` air or stone from the heightmap min/max without filling the chunk. Terrain generate is 140–149 µs per surface chunk (measured, cloud VM, divan), and region generation is ~2.5× meshing on the RTX 2060 machine (measured).

**Change:**
1. Sample 3D density on a coarse grid and interpolate trilinearly, with masks that skip regions known to be solid or air (R-09).
2. Bound the noise over each 16³ sub-block with interval arithmetic and skip sub-blocks that are provably all solid or all air (P-09).
3. Write straight into palette storage instead of per-voxel `set` calls (Noisium, qualitative).

**Expected (estimate):** 4–8× fewer noise evaluations per chunk; keeps the uniform shortcut working once caves exist.

**Effort:** S–M. Only relevant once 3D noise exists.

**Decided by:** gen µs per chunk and uniform-chunk ratio on a cave seed, with and without each step.

## B-12 Persistence

**Origin:** P-08 (Voxel Tools), R-07 (Hytale storage notes 2026, Stardance devlog, zstd and lz4_flex benches; `storage-gen-streaming.md` §1). Roadmap item 14.

**Change:** save only edited chunks (unedited ones regenerate deterministically). Per chunk: palette, bit width and words, `Uniform` as one value, then zstd or lz4 per blob (Voxel Tools defaults to LZ4; Hytale uses zstd level 3, quoted), grouped in region files.

**Expected (quoted):** Stardance went from 65 KB to 5.7 KB for a cave chunk.

**Effort:** M.

**Decided by:** bytes per chunk on disk (lz4 vs zstd-3), save and load µs per chunk, load vs regenerate ms, content hash identical after a round-trip, total save size after a scripted edit session.

## B-13 Batched edit API

**Origin:** P-07 (Voxel Tools: a single voxel set is expensive end to end, so edits go through bulk shape operations, quoted).

**Change:** queue edits as shapes (`Box`, `Sphere`, `Paste`) per frame, apply once per touched chunk, then remesh the chunk and each neighbour whose border changed once (ADR-0006). A `Uniform` chunk fully covered by a box stays `Uniform`.

**Effort:** M.

**Decided by:** voxels edited per second, edit-to-visible latency in frames, remeshes per edit.

## B-14 Attribute channels with the storage shape

**Origin:** P-06 (John Lin: attributes as separate named channels, quoted).

**Change:** make the `Uniform | Packed { palette, bit-width words }` shape generic over the element type, one channel per attribute: blocks (`BlockId`), light (4-bit sky + 4-bit block, roadmap 16), later block state. Sky-lit air and dark underground stone are uniform, so most chunks pay nothing for light.

**Rejected part:** Lin's general allocator, tagging and converter framework. A runtime-typed attribute system is the kind of feature-first infrastructure that stalled Decaf and Lemon, and the essay has no numbers or rendering design.

**Effort:** M.

**Decided by:** heap MiB for blocks + light at radius 32, light propagation µs per chunk, decode µs unchanged for blocks.

## B-15 Light propagation

**Origin:** R-13 (Starlight, voxel-light in Rust, Pulsar 2026; `storage-gen-streaming.md` §5). Roadmap item 16; stores into B-14's light channel.

**Change:** level-based propagation with separate increase and decrease deques and per-entry direction masks, light data tied to the chunk.

**Expected (quoted):** ~2× faster than vanilla Minecraft 1.20; 0.1–1 ms per propagation.

**Effort:** M. Prototype when lighting starts.

**Decided by:** light µs per chunk against the 174 µs torch baseline (quoted).

## B-16 Occupancy volume for ray-marched AO, shadows or GI

**Origin:** P-10 (IOLITE, Teardown), R-11 (VoxelRT XBrickMap / MultiDDA 2024–25, `raytracing-sparse.md` §1).

**Idea:** IOLITE ("powered by path tracing", quoted) and Teardown (ray-marched shadows and AO from a coarse volume, `docs/research/voxel-techniques.md`) trace rays through voxel occupancy instead of using shadow maps and SSAO.

**Fit:** the mesher already builds 1-bit occupancy per chunk. Uploading 32³ bits per non-uniform chunk is 4 KiB, ~42 MiB for the 10,858 meshed chunks at radius 32 (computed), plus a per-chunk index for uniform chunks. Rasterised primary visibility stays; a half-resolution compute pass marches short rays (AO, contact shadows) or sun rays through the occupancy with a two-level DDA (chunk index, then bits).

**Tension with our rules:** `CLAUDE.md` prefers screen-space approximations. This proceeds only if it beats SSAO + cascaded shadows on GPU ms at comparable quality, measured on Noah's GPU, and only after SSAO exists.

**Effort:** L (P-10) or M for AO/shadow rays only (R-11).

**Decided by:** GPU ms of the trace pass at half resolution, occupancy MiB, and a screenshot comparison against SSAO + CSM at the same frame budget.

## B-17 Hierarchical GPU LOD traversal

**Origin:** R-12 (Voxy source, `lod.md` §2, `gpu-culling.md` §5).

**Technique [S]:** 16-byte nodes traversed breadth-first on the GPU, one dispatch per level; per node a frustum test, a Hi-Z test and a screen-area test (default split at 64² px, auto-tuned between 28 and 256 by FPS, quoted from code). Nodes without a mesh push a request to a GPU→CPU queue and fall back to their children.

**Fit:** only after B-08 works with distance rings. Take the screen-area rule and the node layout; the CPU request-queue readback is awkward in wgpu (async map, one-frame latency).

**Effort:** L.

**Decided by:** GPU ms, nodes visited and rendered per level, request-queue latency, with the camera in a valley and on a ridge.

## B-18 Integer-lattice noise kernel

**Origin:** worldgen survey 2026-11 (`docs/research/worldgen-2026-11/noise-performance-lod.md` §1, §2, §7). Implements ADR-0016's noise-precision rule. Step 1 of `docs/WORLDGEN.md`.

**Now:** fastnoise-lite takes block coordinates as `f32`; spacing is 0.0078 blocks at 64 km and 0.125 blocks at 2^20 blocks (computed). S2 samples heights once per chunk column (PR #17).

**Change:**
1. Own gradient and value noise on an integer lattice: power-of-two base wavelength, lacunarity 2, `cell = x >> s` and an exact `f32` fraction, integer hash with a mixed seed and integer offset per octave, no rotations.
2. SoA kernel over the 1,024 columns of a chunk column, plain lane-independent loops, no `mul_add`, no libm, no table gathers; `multiversion` dispatch for baseline and x86-64-v3.
3. Ships as terrain v2 with v1-equivalent settings behind `--terrain v1|v2`, plus `vx worldmap` for top-down review.

**Expected (target):** 2D 5-octave height ≤ 20 ns per column; identical quality at any distance.

**Effort:** M.

**Decided by:** `vx bench` world generation at `default`, v2 within noise of v1 or faster; content hash identical for scalar, SSE2 and AVX2 builds at coordinates up to ±2^24; far-origin test at 2^30 against an `f64` reference.

## B-19 Climate fields and spline terrain shaping

**Origin:** worldgen survey 2026-11 (`minecraft.md` §2, §6, `other-engines.md` §1, §9). Step 2 of `docs/WORLDGEN.md`.

**Now:** one fBm height, no notion of continents, plains or mountain ranges.

**Change:** continentalness, erosion and ridge noises on a coarse 2D lattice (32–64 blocks, interpolated); ridges folded into peaks and valleys (`1 − |3|r| − 2|` form, `minecraft.md` §2); cubic splines map them to surface offset, factor and jaggedness, with Tectonic-style scale knobs (continents 2× larger than vanilla). Temperature and humidity on the same lattice for B-25.

**Expected (estimate):** about 38 octave evaluations per column for the three spline inputs at vanilla settings (`minecraft.md` §1), fewer on the coarse lattice.

**Effort:** M.

**Decided by:** v2 world generation at `default` ≤ 1.25× v1 in the same session (target); uniform-chunk ratio ≥ v1's; `vx worldmap` and screenshots reviewed by Noah.

## B-20 LOD-direct sampling and range bounds

**Origin:** worldgen survey 2026-11 (`noise-performance-lod.md` §3, §4; `other-engines.md` §1 for Cubyz). Steps 2 and 4 of `docs/WORLDGEN.md`. Prerequisite for B-08's far rings (ADR-0016).

**Now:** the uniform shortcut uses the exact min/max of the chunk column's 1,024 heights; no LOD.

**Change:**
1. Per-octave range bound `Σ min(2·a_k, a_k·Lip·f_k·r)` to classify chunks and sub-blocks as air, solid or mixed before any 3D work.
2. LOD level L evaluates only octaves with wavelength ≥ 2 voxels (2^(L+1) blocks); every stage is written so octave k depends only on octaves below k.
3. The same height function at 64–256 block spacing feeds the horizon tier.

**Expected (computed):** far terrain within 0.19, 0.29 and 0.34 voxel of level 0 at LOD 3, 4 and 5 with today's settings; 2D cost per far chunk 20–60% lower than level 0.

**Effort:** M.

**Decided by:** unit test |H_L − H_0| ≤ tail(L) over 10⁷ samples; gen µs per far chunk at LOD ≥ 3 ≤ 60% of level 0 (target); `horizon` scenario in S5.

## B-21 Erosion filter

**Origin:** worldgen survey 2026-11 (`erosion-rivers-papers.md` §1.1, §1.2, §1.4). Step 5 of `docs/WORLDGEN.md`.

**Now:** plain fBm; slopes have no gullies or drainage look.

**Change:** base height as derivative-damped fBm (IQ) or swiss turbulence, which yield the analytic gradient; then Johansen's erosion filter with Phacelle noise on height and gradient, implemented clean-room from the published description (ADR-0017; the reference code is MPL-2.0). Filter octaves truncate per LOD like B-20.

**Expected (estimate):** 16 taps per filter octave (one `exp`, one `sin_cos` each), about 3.3 kflop per column at 5 octaves.

**Effort:** M.

**Decided by:** ns per column scalar and `f32x8` (divan); v2 world generation ≤ 1.6× v1 together with B-22 (target); chunk-seam test; screenshots reviewed by Noah.

## B-22 Rivers

**Origin:** worldgen survey 2026-11 (`erosion-rivers-papers.md` §2, `minecraft.md` §6). Step 5 of `docs/WORLDGEN.md`.

**Now:** no rivers.

**Change:** first, Tectonic-style rivers where the folded ridge value falls in a narrow band, carved after erosion. If the bands read as wrong (rivers that do not flow anywhere), prototype a jittered-grid river graph: one candidate node per cell per level, edges to the lowest neighbour within a bounded 3×3 neighbourhood, width by level, coarse levels dropped per LOD.

**Expected (estimate):** about 1 kflop per column for the graph; the band variant costs a compare after B-19's ridge noise.

**Effort:** M (bands S, graph M–L).

**Decided by:** no uphill step along any river centreline over a 16 km square; chunk-seam test; cost inside B-21's shared budget.

## B-23 Overhangs

**Origin:** worldgen survey 2026-11 (`minecraft.md` §2, density with jaggedness). Step 6 of `docs/WORLDGEN.md`.

**Now:** pure heightmap, no overhangs or arches.

**Change:** a 3D term scaled by B-19's jaggedness, evaluated on the absolute 4×8×4 block lattice only in chunks whose B-20 bounds straddle the surface, trilinear, straight into palette storage; off at LOD ≥ 2.

**Expected (computed):** at most 405 lattice points per surface chunk (9×5×9), against 32,768 voxels.

**Effort:** S–M (shares the lattice and writer with B-11).

**Decided by:** gen µs per surface chunk; uniform-chunk ratio; v2 with caves ≤ 2.0× v1 (target, shared with B-11).

## B-24 Hash-cell decoration

**Origin:** worldgen survey 2026-11 (`other-engines.md` §1, §3, §9: Cubyz structure lists, Veloren `StructureGen2d`). Step 7 of `docs/WORLDGEN.md`.

**Now:** no trees or structures.

**Change:** per feature type, a jittered grid with cell size ≥ the feature's radius; a hash of the cell gives position, kind and seed. A chunk visits the cells within reach, takes the base height from the cached 2D tile and stamps only its own voxels. Beyond LOD 2, a per-column hash probability replaces exact placement.

**Expected:** cross-chunk features with no neighbour access (qualitative).

**Effort:** M. Needs leaf and log block ids and textures.

**Decided by:** zero cut-off trees across chunk borders over 10⁴ chunks; decoration ≤ 10% of v2 world generation (target).

## B-25 Surface rules, biomes and water

**Origin:** worldgen survey 2026-11 (`minecraft.md` §4, `other-engines.md` §9). Step 3 of `docs/WORLDGEN.md`.

**Now:** grass, dirt, stone by depth only. `sand`, `snow` and `snow_side` textures exist (ADR-0014).

**Change:** biome from a precomputed lookup table over quantised climate values (about 9,800 entries, `minecraft.md` §10) instead of an RTree; top and filler blocks by biome, slope (from the height tile's border) and altitude; water below sea level. New block ids follow `docs/TEXTURES.md`.

**Expected:** biomes at per-column cost (qualitative).

**Effort:** S–M.

**Decided by:** v2 world generation ≤ 1.35× v1 (target); screenshots and `vx worldmap` reviewed by Noah.

---

## Watch

Revisit when the stated condition changes.

- **wgpu mesh shaders:** still `EXPERIMENTAL_MESH_SHADER` in wgpu 30, Naga supports it only on Vulkan. Meshelium quotes 1.5× (render distance 12) to 5.3× (64) against vanilla Minecraft, not against an MDI renderer. Revisit after B-06 if the terrain pass is vertex-bound.
- **4-byte quads** (ADR-0004, roadmap 10): geometry is 7.7 MiB in `default` (measured), so the saving is small unless the terrain pass proves bandwidth-bound; it also collides with B-09's AO bits.
- **Opacity-mask fast path** (binary-greedy-meshing 0.5.2): quoted 4× between its two modes (300 → 65 µs); our mesher is already ~35 µs.
- **SIMD noise via fastnoise2:** quoted ~5–8× vs FastNoiseLite (AVX2), but adds a C++/CMake dependency. Compare with a hand-written `wide::f32x8` fBm (ADR-0009, roadmap 13); generation already dominates region build on the RTX 2060 (measured).
- **Brixelizer-style GI:** ~1.7 ms at 1080p (RX 7600 XT) to ~2 ms at 4K (7900 XTX), 110–500 MB (quoted).
- **Sparse 64-trees:** 182.6 Mrays/s on an iGPU, about half ESVO's memory (quoted). A second representation.
- **Screen-space radiance cascades:** ~2 / 6 / 26 ms at low / medium / high (quoted).
- **0fps geomorphed POP-buffer LOD:** crack-free without skirts, no numbers; keeps full detail data in memory. Fallback if B-08's border faces show artefacts.
- **Aquifers and ore veins** (Minecraft 1.18): per-voxel neighbour search below sea level (`docs/research/worldgen-2026-11/minecraft.md` §3). Revisit after B-11 caves exist.
- **Baked macro atlas from stream-power erosion or Dendry** (Cordonnier 2016, Tzathas 2024, Gaillard 2019): global per tile, needs border handling and an offline step (`erosion-rivers-papers.md` §2–3). Only if B-21 + B-22 fail the visual review.
- **Terrain Diffusion** (arXiv 2512.08309, 2025): windowed and seed-consistent, but GPU inference with no published latency or weight licence.
- **GPU compute for the horizon clipmap:** data stays on the GPU, so no readback; not bit-equal to the CPU, so never used for block contents (`noise-performance-lod.md` §6).
- **C2ME-style density compiler passes** (constant folding, branch elimination, cache elimination): apply as compile-time Rust structure and interval bounds if the v2 stage graph grows.
- **Distant Horizons column heightfields:** a possible tier beyond ~10 km; separate data model and mesher.

## Not adopted

- **Noa, Rosebud, Hytopia, GDevelop, Unity assets:** browser/JS or no-code platforms with no transferable internals.
- **Luanti-style Lua modding:** out of scope for an engine-performance project.
- **Octrees or SVOs as primary storage:** 0fps measured ~9× slower random reads than flat arrays (quoted); palette chunks already compress better for editable terrain.
- **RLE / interval trees as primary storage:** great for iteration (quoted), but the bitmask mesher needs per-row bits. RLE stays a disk-format alternative for B-12.
- **John Lin's generic format/converter framework:** see B-14.
- **Smaller-than-block voxels ("chisels & bits"):** would need a second mesher and storage path; revisit only as a game-design ask.
- **GPU compute meshing:** CPU meshing is ~35 µs per chunk and hidden by rayon; the claimed 50–200 µs per chunk is unverified.
- **Geometry-shader quad expansion:** wgpu has no geometry shaders; vertex pulling is the same idea.
- **Transform-aware or editable SVDAGs:** memory-only gains, static-scene oriented.
- **Split Radiance Cascades:** 8.6–11.5 ms on an RTX 3080 Laptop (quoted).
- **wgpu hardware ray query:** experimental, BLAS rebuild per edit. Revisit later.
- **GPU world generation:** readback spikes (quoted author complaint).
- **Lossy VDB brick compression:** lossy, for volume rendering, not faster than a dense texture on 4 of 5 data sets (quoted).
- **Transvoxel:** marching cubes only; the blocky equivalent is B-08's border faces.
- **Minecraft's generation-status pyramid and 3×3 feature neighbourhood:** breaks chunk-locality (ADR-0016, ADR-0017); B-24's hash cells replace it.
- **Runtime erosion simulation** (Mei 2007 GPU hydraulic, particle erosion): global grids or order-dependent droplets (`erosion-rivers-papers.md` §3.4).
- **simdnoise as the noise authority:** its scalar and SIMD outputs differ (quoted), which changes block heights per machine. `std::simd`: still nightly.
- **Lochner 2023 diffusion authoring:** tile-based, not seamless.
- **SGGX-H anisotropic LOD, HROC:** offline microgeometry and irregular-object scenes respectively; no fit for a regular grid.

## Old ids

| Old id | Now | Old id | Now |
|---|---|---|---|
| P-01 | B-03 | R-01 | B-01 |
| P-02 | B-07b | R-02 | B-06 |
| P-03 | B-09 | R-03 | B-07a |
| P-04 | B-08 | R-04, R-05 | B-08 |
| P-05 | B-05 | R-06 | B-09 |
| P-06 | B-14 | R-07 | B-12 |
| P-07 | B-13 | R-08 | B-04 |
| P-08 | B-12 | R-09 | B-11 |
| P-09 | B-11 | R-10 | `docs/TEXTURES.md`, ADR-0014 |
| P-10 | B-16 | R-11 | B-16 |
| P-11 | B-10 | R-12 | B-17 |
| | | R-13 | B-15 |
