# Proposals backlog

Ideas from external engines, each with the expected benefit and the measurement that decides it. Nothing here is accepted. A proposal becomes work by getting a roadmap slot and, if it is a design choice, an ADR. Nothing here starts before the hardware baseline (roadmap item 1) exists.

Sources and the comparison with our code: `docs/research/engine-survey-2026-11.md`. Number labels as in `docs/BENCHMARKING.md`: **measured** (this repo, `docs/PERFORMANCE.md`), **quoted** (a source), **computed** (arithmetic), **estimate** (a guess to be replaced by a measurement).

Size: S = under a day, M = a few days, L = rewrites a subsystem.

## Ranked summary

| # | Proposal | Source | Size | Expected benefit | Decided by |
|---|---|---|---|---|---|
| P-01 | Decode straight into the padded mesher input | 0fps (iteration cost) | S–M | Up to ~40% less CPU per meshed chunk (estimate) | divan `gather+mesh` bench, region mesh ms |
| P-02 | Occluder quads + small software depth buffer | procworld | M | Fewer draws in mountains and caves, no GPU readback | draws issued and GPU ms, cave and mountain bench paths |
| P-03 | 26-neighbour gather, then vertex AO in free quad bits | 0fps AO | M | Biggest visual gain per ms of any lighting option (estimate) | quad count, mesh µs, opaque-pass GPU ms vs SSAO |
| P-04 | LOD clipmap: one ring buffer per LOD level | Voxel Tools | L | View distance several times larger at the same VRAM (estimate) | max radius at fixed frame and VRAM budget |
| P-05 | Upload time budget + suballocated buffers | Voxel Tools | M | No frame spikes while streaming | frame p99 while flying at sprint speed |
| P-06 | Attribute channels with the storage shape | John Lin | M | Light and metadata cost zero in uniform chunks | voxel + light heap MiB at radius 32 |
| P-07 | Batched edit API | Voxel Tools | M | One remesh per chunk per frame however many voxels change | edits/s, edit-to-visible latency |
| P-08 | Persistence: palette words + LZ4 | Voxel Tools | M | Small saves, fast reload | bytes per chunk, load µs per chunk |
| P-09 | Interval range analysis for 3D noise | Voxel Tools | S–M | Keep the uniform shortcut when caves arrive | gen µs per chunk on cave terrain |
| P-10 | 1-bit occupancy on the GPU for ray-marched shadows/AO/GI | IOLITE, Teardown | L | Path-traced look without a second scene representation | GPU ms vs SSAO + cascaded shadows at equal quality |
| P-11 | T-junction seam fix by quad dilation | blackflux, cgerikj | S | No sky-coloured pixel leaks | pixel-diff test on headless screenshots |

## P-01 Decode straight into the padded mesher input

**Now (measured):** per meshed surface chunk we spend 17–24 µs decoding the palette into a dense 32³ scratch, then 29–35 µs copying it into the 34³ padded array and reading six border slabs through `ChunkVoxels::get` (one palette lookup per voxel, 6 × 1024 calls). The mesher itself takes 33–39 µs. So data movement costs more than meshing.

**Change:**
1. `ChunkVoxels::decode_into_padded`: unpack rows straight into the padded layout. Removes the dense scratch and a 64 KiB copy.
2. Border slabs: `Uniform` neighbour → `fill`. `Packed` neighbour → unpack whole x-rows (the ±Y and ±Z slabs are contiguous rows; ±X slabs are strided single values).
3. Later, if the mesher's column build shows in profiles: build the three opaque bit-column arrays during decode, using a per-palette opacity bitmask, so `build_opaque_columns` stops scanning 39,304 voxels.

**Expected:** combined gather + decode drops from ~46–59 µs to roughly 15–25 µs (estimate), so meshed chunks/s on the worker pool rises by up to ~40% (estimate).

**Measure:** new divan bench `gather_and_mesh_surface_chunk` (before/after on the same machine and session), and region mesh ms at radius 32 in the bench mode. The mesher-vs-reference test must stay green.

## P-02 Occluder quads with a small software depth buffer

**Idea (quoted, procworld):** per chunk, find a few large quads that lie fully inside solid voxels. Each frame, rasterise the nearest ones into a 64×64 depth buffer on the CPU and drop chunks whose screen rect is fully behind it.

**Fit with Espresso:** the mesher already builds opaque bit-columns for every chunk, so maximal solid rectangles per axis come almost free during meshing (a by-product stored next to the mesh, a few quads per chunk). It avoids the GPU-readback latency that made Decaf disable its Hi-Z, works on every render tier (no `MULTI_DRAW_INDIRECT_COUNT` needed), and can feed the same indirect buffer.

**Trade-off vs ADR-0010 (GPU Hi-Z):** Hi-Z is exact to pixel level but needs compute, two-phase logic and the count feature for best results. Occluder quads are conservative and coarse, cost CPU per frame, but are simple. They are not exclusive: the CPU pass can pre-cull before the GPU pass.

**Measure:** add `cave` and `mountain` camera paths to the bench mode. Record draws issued, chunks culled, CPU µs for the occlusion pass, and opaque-pass GPU ms, with the pass on and off. Keep it only if GPU ms falls by more than the CPU cost added.

## P-03 26-neighbour gather, then vertex AO in the free quad bits

**Idea (quoted, 0fps):** per-vertex AO from two side voxels and one corner voxel, 4 levels (2 bits) per vertex, merge greedy faces only when all four AO values match, flip the quad diagonal when `a00 + a11 > a01 + a10`.

**Finding:** our padded gather only fills the 6 face slabs; the 12 edge rows and 8 corner voxels of the 34³ array stay air. Correct AO at chunk borders needs them. This is a change to ADR-0006: a chunk can only be meshed once its 26 neighbours exist, which delays meshing at the streaming edge by one more ring.

**Bits (computed):** 4 vertices × 2 bits = 8 of the 15 free bits in `PackedQuad` (ADR-0004). No format growth.

**Cost to measure:** equal-AO merging splits quads; the 0fps article gives no number, so measure quad count and mesh µs on the surface and checkerboard benches. Compare against SSAO on the same frame, which costs GPU time but zero mesh splitting.

**Decide with:** total quads at radius 32 (+X%), mesh µs per chunk, opaque-pass GPU ms, and a screenshot pair. Pick vertex AO if the quad increase is under ~30% (estimate threshold) and SSAO costs more GPU ms than the extra quads.

## P-04 LOD clipmap: one ring buffer per LOD level

**Idea (quoted, Voxel Tools):** each LOD level doubles block size; streaming by concentric boxes (`Clipbox`).

**Fit with Espresso:** our `ChunkWindow` is already a ring buffer with `pos.rem_euclid(extent)` slots. A clipmap is N such windows with the same extent, level `n` covering 2ⁿ× the distance. LOD `n` chunk voxels come from 2×2×2 downsampling of level `n−1` (pick the most common non-air block), or straight from the generator at coarse resolution, which is cheaper for far terrain that is never edited. The 32³ mesher is reused unchanged per level; quads get scaled by a per-chunk LOD shift in `chunk_origins`. Cracks between levels: skirts (one-voxel border walls) rather than Transvoxel, since our terrain is blocky.

**Size:** large. Touches `vx-world` (window per level, downsampler), app streaming, render (per-chunk scale). It supersedes the two candidates in ADR-0011 only after a prototype comparison.

**Expected (computed):** with 4 levels at the same chunk count per level, view distance is 8× level 0 for roughly 4× the chunks of level 0 alone, since far levels are mostly uniform. Real VRAM and frame cost need measurement.

**Measure:** maximum radius reached at a fixed frame-time and VRAM budget, LOD pop visibility in a flight capture, chunks/s generated for far levels.

## P-05 Upload time budget and suballocated buffers

**Idea (quoted, Voxel Tools):** cap main-thread upload work per frame (default ~8 ms), and avoid per-mesh buffer allocation/deallocation, which spikes on Vulkan.

**Fit:** this is roadmap item 4 (`offset-allocator`, byte budget per frame). The new point is the budget in time, not only bytes, and freeing through the allocator only, never `wgpu::Buffer` drops per chunk. No new design needed; this confirms the existing plan.

**Measure:** frame p99 and max while flying at sprint speed for 60 s, upload ms per frame, fragmentation (free bytes in the largest free block) after the flight.

## P-06 Attribute channels with the same storage shape

**Idea (quoted, John Lin):** voxel attributes are separate, named channels so per-voxel data can be added "only where we want it".

**Narrow adoption:** make the `Uniform | Packed { palette, bit-width words }` shape generic over the element type and keep one channel per attribute: blocks (`BlockId`), light (4-bit sky + 4-bit block, roadmap item 12), later block state. Sky-lit air and dark underground stone are uniform, so most chunks pay nothing for light.

**Rejected part:** Lin's general allocator/tagging/converter framework. A runtime-typed attribute system is exactly the kind of feature-first infrastructure that stalled Decaf and Lemon, and the essay itself has no numbers or rendering design.

**Measure:** heap MiB for blocks + light at radius 32, light propagation µs per chunk, and decode µs unchanged for blocks.

## P-07 Batched edit API

**Idea (quoted, Voxel Tools):** a single voxel set is expensive end to end; edits go through bulk shape operations.

**Change:** edits are queued as shapes (`Box`, `Sphere`, `Paste`) per frame, applied once per touched chunk, then the chunk and neighbours whose borders changed get one remesh each (ADR-0006). A `Uniform` chunk fully covered by a box stays `Uniform`.

**Measure:** voxels edited per second, edit-to-visible latency in frames, remeshes per edit.

## P-08 Persistence: palette words plus LZ4

**Idea (quoted, Voxel Tools):** per-channel header with a uniform flag, then the whole block LZ4 (default) or Zstandard.

**Fit:** our `PackedVoxels` is already compact; write palette + bit width + words, `Uniform` as one value, and compress per region file. Covers roadmap item 10.

**Measure:** bytes per chunk on disk (compare LZ4 vs zstd level 3), save and load µs per chunk, and total save size after a scripted edit session.

## P-09 Interval range analysis for 3D noise

**Now:** `TerrainGenerator::generate` already returns `Uniform` air or stone from the heightmap min/max without filling the chunk.

**Idea (quoted, Voxel Tools):** when 3D noise (caves, overhangs) lands, bound the noise over each 16³ sub-block by interval arithmetic and skip sub-blocks that are provably all solid or all air.

**Measure:** gen µs per chunk and uniform-chunk ratio on a cave seed, with and without range analysis. Only relevant once ADR-0009's coarse-grid 3D noise exists.

## P-10 1-bit occupancy on the GPU for ray-marched shadows, AO or GI (large rewrite, research)

**Idea:** IOLITE ("powered by path tracing", quoted) and Teardown (ray-marched shadows and AO from a coarse volume, see `docs/research/voxel-techniques.md`) get their look from tracing rays through voxel occupancy instead of shadow maps and SSAO.

**Fit:** the mesher already builds 1-bit occupancy per chunk. Uploading 32³ bits per non-uniform chunk is 4 KiB, ~42 MiB for the 10,858 meshed chunks at radius 32 (computed), plus a per-chunk index for uniform chunks. Rasterised primary visibility stays; a half-resolution compute pass marches short rays (AO, contact shadows) or sun rays through the occupancy with a two-level DDA (chunk index, then bits).

**Tension with our rules:** CLAUDE.md prefers screen-space approximations. This proposal only proceeds if it beats SSAO + cascaded shadows on GPU ms at comparable quality, measured on Noah's GPU.

**Measure:** GPU ms of the trace pass at half resolution, VRAM for occupancy, and a screenshot comparison against SSAO + CSM at the same frame budget.

## P-11 T-junction seams

**Known issue:** a faint seam was seen in one headless frame (`docs/ROADMAP.md`).

**Sources:** blackflux shows T-junction-free meshing (monotone) costs more triangles and still fails between neighbouring faces in 3D (quoted). The cgerikj mesher dilates quads by about one pixel in the vertex shader (quoted in `docs/research/voxel-techniques.md`).

**Change:** dilate quad corners by a small clip-space epsilon in `terrain.wgsl`. No mesher change.

**Measure:** a headless test that renders a fixed view and counts sky-coloured pixels inside terrain silhouettes, before and after; opaque-pass GPU ms unchanged.

## Not adopted

- **Noa, Rosebud, Hytopia, GDevelop, Unity assets:** browser/JS or no-code platforms with no transferable internals.
- **Luanti-style Lua modding:** out of scope for an engine-performance project.
- **Octrees or SVOs as primary storage:** 0fps measured ~9× slower random reads than flat arrays (quoted); our palette chunks already compress better for editable terrain.
- **RLE / interval trees as primary storage:** great for iteration (quoted) but the bitmask mesher needs per-row bits; RLE stays a candidate for disk only (P-08 alternative).
- **John Lin's generic format/converter framework:** see P-06.
- **Smaller-than-block voxels ("chisels & bits"):** would need a second mesher and storage path; revisit only as a game-design ask.
