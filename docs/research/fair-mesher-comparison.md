# Fair mesher comparison: Decaf vs Lemon vs Espresso

Same input bytes, same machine, same build profile, single thread. Replaces earlier cross-input numbers.
Everything in "Measured" was produced by `tools/legacy-compare` (see reproduce command). Items labelled
**QUOTED** come from the audit docs and were NOT re-measured here.

## Method

- Standalone cargo project `tools/legacy-compare/` (own empty `[workspace]`, not part of the main workspace). Path deps on `vx-core`, `vx-world`, `vx-mesh`. Bench harness: divan 0.1.21.
- Profile: release, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"` (as main workspace). No `target-cpu=native` for any mesher (same flags for all three).
- Input: one padded 34^3 `[BlockId; PADDED_VOLUME]` per scene (interior 32^3 plus one voxel of real neighbour data per face). Identical bytes feed all three adapters.
- Timed region = the mesher call only. Conversion from the padded array into each legacy mesher's native layout is done once, outside the timed region (its cost is reported separately as context). Each mesher is warmed with one call before timing; output buffers are reused where the shipped code reuses them.
- Runs: 5 separate invocations of the bench binary, each pinned with `taskset -c 2`, each divan default 100 samples x 100 iters. Per invocation the divan **median** is taken; tables show the median of those 5 medians and (min-max) across invocations. The VM is shared and noisy; treat differences below ~10% as noise.
- Machine: Intel Xeon Processor @ 2.30GHz (VM, 4 vCPU, 1 thread/core, avx2/avx512f/bmi2 present, not used via target flags), Linux 6.18 (fc VM), rustc 1.97.0 (2d8144b78 2026-07-07).
- Legacy commits: Decaf `b436080eb26b6dfb5157695b49379379c33dec11`, Lemon `8bc10039d253f9e5bbe0d453a26e852b43bc446f`. Espresso mesher = `crates/vx-mesh` at the working tree on top of `3f813d689080e11055d0f9c55ec214da961dc5c6` (tree has uncommitted changes elsewhere; vx-mesh/vx-core/vx-world were not touched by this work).

## Scenes (identical bytes for all three)

| name | description |
|---|---|
| surface | `padded_surface_chunk()` copied from `crates/vx-mesh/benches/mesh.rs` (TerrainGenerator default settings, chunk (3,2,-5), real neighbours) |
| solid | interior 32^3 = STONE, 1-voxel border = air |
| random50 | whole padded volume, xorshift64 seed `0x9E3779B97F4A7C15`: if `roll & 1 == 0` then block `1 + (roll>>32) % 3`, else air (~50% fill, 3 block types) |
| checker | `(x+y+z) % 2 == 0` over the whole padded volume = STONE (same as the checkerboard in the existing bench) |
| deep | whole padded volume STONE, then 24 air pockets (cubes of side 2 or 3, xorshift64 seed `0x0123456789ABCDEF`, positions in padded 2..29) |

## Correctness / equivalent work

`cargo run --release` (the `legacy-compare` binary) expands every quad of every mesher into unit faces `(x, y, z, direction, block)` and compares the sorted multiset against `vx_mesh::mesh_culled_unit_faces` expanded the same way. Duplicates or gaps would make the multisets differ. Result (measured): **all three meshers cover exactly the same set of exposed faces, with the same block ids, in all 5 scenes.**

Face-culling rules are equivalent for these inputs: all non-air blocks are opaque and solid in the Decaf registry I built, Lemon treats any non-zero id as solid, Espresso `is_opaque() = id != 0`. A face is emitted iff the voxel is solid and the neighbour is air. Decaf's transparent/no-cull/tint paths still execute but never trigger (checked: 0 transparent-bucket quads).

Quad counts (measured, lower is better for GPU side):

| scene | unit faces | Espresso | Lemon | Decaf |
|---|---|---|---|---|
| surface | 1,616 | 640 | 645 | 645 |
| solid | 6,144 | 6 | 6 | 6 |
| random50 | 49,199 | 42,169 | 42,164 | 42,164 |
| checker | 98,304 | 98,304 | 98,304 | 98,304 |
| deep | 748 | 144 | 144 | 144 |

Merging quality is essentially identical (Lemon and Decaf produce the same counts, Espresso differs by a handful of quads due to scan order). Output sizes differ: Espresso `PackedQuad` 8 B, Lemon 4 B (direction implied by buffer), Decaf `PackedFace` 8 B (value + tint; direction/depth implied by slice vector).

## Results (measured): microseconds per chunk, median (min-max) across 5 invocations

| scene | Espresso binary greedy | Lemon (exact neighbours) | Decaf, no Instant | Decaf as shipped (with Instant) |
|---|---|---|---|---|
| surface | 38.6 (37.0-39.0) | 298 (296-315) | 906 (882-933) | 965 (919-979) |
| solid | 58.6 (57.1-60.0) | 64.3 (63.9-105) | 1,316 (1,279-1,351) | 1,305 (1,261-1,331) |
| random50 | 926 (909-953) | 1,664 (1,642-1,718) | 3,453 (3,417-3,604) | 3,573 (3,509-3,689) |
| checker | 836 (798-842) | 981 (958-1,002) | 1,633 (1,557-1,671) | 1,569 (1,530-1,663) |
| deep | 54.3 (51.6-55.0) | 280 (278-292) | 1,334 (1,331-1,342) | 1,309 (1,282-1,334) |

Speedup of Espresso (ratio of medians; >1 means Espresso faster):

| scene | vs Lemon | vs Decaf (no Instant) | vs Decaf (as shipped) | Lemon vs Decaf (no Instant) |
|---|---|---|---|---|
| surface | 7.7x | 23.5x | 25.0x | 3.0x |
| solid | 1.1x | 22.4x | 22.3x | 20.5x |
| random50 | 1.8x | 3.7x | 3.9x | 2.1x |
| checker | 1.2x | 2.0x | 1.9x | 1.7x |
| deep | 5.2x | 24.6x | 24.1x | 4.8x |

Reading the numbers:

- The `Instant::now()` pairs (384 calls per chunk) are not measurable here: with vs without differ by -4% to +7% in both directions, i.e. within noise (expected cost ~8 us at ~20 ns/call on this VM against 0.9-3.6 ms).
- Speedups are strongly scene dependent. On sparse, realistic terrain-like chunks (surface, deep) Espresso is 5-8x faster than Lemon and ~24x faster than Decaf. On dense high-entropy inputs (random50, checker) the work is dominated by emitting/merging ~42k-98k quads and the gap shrinks to 1.2-1.8x vs Lemon and 2-3.7x vs Decaf. The "solid" scene is the only one where Lemon is on par with Espresso (its early-out skips empty layers/directions, only 6 quads).
- Do not quote one headline multiple: "24x" is the best case (sparse chunks), "~1.2x" the worst (checkerboard).

## Adaptations (every change made to the legacy code)

Shared:
- Each legacy mesher is fed from the padded 34^3 array via an adapter in `src/adapt.rs`. Conversion happens outside the timed region.
- `#![allow(...)]` for dead code/clippy at the top of the copied modules. No algorithm edits.

Decaf (`src/decaf.rs`, from `src/engine/world/mesher.rs` at the commit above):
- Dependency types replaced by minimal local copies with identical layout/semantics: `BlockId(u16)`, `Voxel(u32)`, `FaceDir`, `Chunk` (`Arc<[Voxel; 32768]>` + `Arc<[ColumnBiomeTints; 1024]>`, same `voxel_index`), `LocalVoxelPos` (was a glam `UVec3` wrapper, now `[u32; 3]`, debug asserts dropped), `PackedFace`, `RenderBucket`, `ResolvedBlock`/`ResolvedBlockRegistry`, `ResolvedFaceTints`, `ResolvedFaceTextures`, `ChunkMeshSlices`.
- `BlockFlags` (bitflags crate) replaced by a plain `u16` newtype with `contains`; same bit values.
- Block registry: air + 5 solid opaque blocks, block `n` uses texture `n` on all faces, no tint (`ResolvedFaceTints::default()`). Real Decaf blocks like grass have per-face textures/tints; with them merging would split differently. This keeps merge criteria equivalent to "same block id", as in the other two meshers. The lookup `resolved_blocks.get_voxel()` per voxel is kept (it is part of the cost).
- `ChunkNeighborReader` trait replaced by `NeighborChunks { chunks: [Chunk; 6] }`. Each neighbour chunk is a full 32^3 `Chunk` with only the plane adjacent to the centre chunk populated, so the shipped `fill_slice_voxels` gather loop still runs unchanged.
- `ChunkMeshDirtyRegion` removed; `collect_all_slices` reproduces the `full()` branch (6 dirs x 32 depths). `rebuild_chunk_mesh_slices` takes a const generic `PROFILE`: `true` keeps the `Instant::now()`/`elapsed()` pairs and profile accumulation as shipped, `false` compiles them out (`PROFILE.then(Instant::now)`). The `MeshingBuildProfile` counters (`faces_emitted`, `slice_buffer_growths`) are kept in both.
- `debug_assert!`s dropped (release build, they would be compiled out anyway).
- Output is a reused `ChunkMeshSlices` (as in the shipped `rebuild_chunk_mesh_slices` path, 2x6x32 Vecs with retained capacity). `ChunkMeshSlices::flatten()` (copy to GPU-facing `ChunkMeshCpu`) is NOT timed; it is a separate shipped step that the audit measured as part of "meshing+flatten".
- Thread-local scratch kept.

Lemon (`src/lemon.rs`, from `src/engine/core/mesher.rs` + `Chunk` from `src/game/world/chunk.rs`):
- Code copied verbatim, including the German comments; only the `use` line and the test/diagnostic modules were removed, and `Chunk` (struct + `empty/set_block/get_block/index_from_pos`) pasted into the same module.
- Exact neighbours: `neighbors: [Option<&Chunk>; 6]` all `Some`, each a full `Chunk` with only the adjacent plane populated. The `neighbor_solid_at_world` closure is passed as `|_, _, _| false` and is never called. The generator's `boundary_planes` prediction is NOT used (see context below).
- `mesh_chunk` returns a freshly allocated `DirectionalMesh` (6 Vecs, each reserved exactly) on every call, as shipped. That allocation is inside the timed region; the drop happens outside divan's timing. Espresso and Decaf reuse buffers. This favours the other two slightly (small effect for sparse chunks, a few microseconds).

Espresso: unmodified `BinaryGreedyMesher::mesh`, reused `BinaryGreedyMesher` and `ChunkMesh`.

## Context numbers

Measured here: cost of building each legacy mesher's native input from the padded array (outside the timed region above; tooling overhead of this harness, not something the real engines pay in this form), us per chunk: Lemon 65-79, Decaf 430-470 (u32 voxels + Arc + 6 neighbour chunks).

| scene | padded to Lemon input | padded to Decaf input |
|---|---|---|
| surface | 64.8 | 432 |
| solid | 78.9 | 467 |
| random50 | 78.7 | 470 |
| checker | 78.6 | 462 |
| deep | 78.8 | 468 |

**QUOTED from `lemon-audit.md` (not re-measured; different machine/session, different terrain generator, not comparable to the table above):** in Lemon's shipped async path the neighbour data is predicted by `TerrainGenerator::boundary_planes`, which the audit measured at 1423 us for a surface chunk, 433 us for a deep chunk and 2442 us for an air chunk, versus 155 us (surface) / 224 us (deep) for `mesh_chunk` with trivial neighbours on the audit's terrain. In this report Lemon is given exact neighbours instead, so the Lemon figures above exclude that prediction cost entirely. Espresso needs `gather_padded` instead; that was not benchmarked here.

**QUOTED from `decaf-audit.md` (not re-measured):** ~1.22 ms avg per chunk for meshing+flatten on realistic generated terrain (hot loop 0.84 ms for chunk (0,0,0)). Consistent in magnitude with the 0.9-1.3 ms measured here for surface/solid/deep without flatten, but different inputs, so no ratio should be derived from it.

## Caveats

- One CPU model, shared noisy VM, 5 invocations pinned to one vCPU; the outlier max for Lemon `solid` (105 us vs 64 us typical) shows that single invocations can spike.
- Micro-benchmark with hot caches: same chunk meshed repeatedly. Real workloads have cold chunk data; the legacy meshers' bigger working sets (Decaf 128 KiB voxels + 6 neighbour chunks; Lemon 64 KiB) would likely suffer more than a measurement here shows. Not measured.
- The scenes are synthetic except `surface`; the terrain chunk is from Espresso's generator, not Decaf's or Lemon's, so "realistic" is only approximately true for them.
- Block variety is low (up to 3-4 ids). Decaf's per-cell `MaskCell` compare and Espresso's per-block plane lookup may scale differently with many block types.
- Decaf is measured without real texture/tint variety (see adaptations); with tints, merging would be less effective, not faster.
- Single-threaded only; the engines' threading models are not compared.
- Lemon alloc inside timing vs reused buffers elsewhere (see above).
- No `target-cpu=native`; Espresso's or Lemon's loops may auto-vectorise differently with it. Not measured.

## Reproduce

```
cd tools/legacy-compare
export CARGO_TARGET_DIR=target   # optional: any directory outside the main workspace target
cargo run --release --offline          # correctness: unit-face equivalence + quad counts (asserts)
cargo bench --offline --no-run
BIN=$(ls -t $CARGO_TARGET_DIR/release/deps/compare-* | grep -v '\.d$' | head -1)
for i in 1 2 3 4 5; do taskset -c 2 $BIN --bench; sleep 2; done   # take divan medians, aggregate across invocations
```

(`cargo bench --offline` alone also works for a single invocation.) Raw per-invocation outputs were aggregated with a throwaway script; the table values are the median and min-max of the divan "median" column over the 5 invocations.
