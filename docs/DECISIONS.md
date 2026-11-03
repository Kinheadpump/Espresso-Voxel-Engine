# Architecture decision records

Every design choice with alternatives, as append-only ADRs. Status is Proposed (needs the stated measurement), Accepted, Rejected, or Superseded by ADR-n. Format and editing rules: `docs/DOC-GUIDELINES.md` §5. Older ADRs predate that format and stay as written.

## ADR-0001 New workspace instead of continuing Decaf or Lemon (Accepted, 2026-11-03)

Both prior engines' cores (storage layout, neighbour handling, world index, mesher) need replacing, and those are the pieces everything else hangs off. Lemon's renderer ideas are good, but its code is tied to the 64 KiB pool, the prediction mesher and a 918-line config. A fresh Cargo workspace lets us port ideas deliberately, keeps both old repos intact as references, and gives a clean git log for performance tracking. Recommended home: a new repo `Kinheadpump/Espresso-Voxel-Engine` (needs Noah's approval).

## ADR-0002 Workspace split core / world / mesh / render / app (Accepted)

CPU crates stay free of wgpu/winit so benches compile in seconds and run headless. The render crate knows nothing about terrain generation. The app crate wires them together.

## ADR-0003 32³ chunks, padded 34³ mesher input, u64 columns (Accepted, revisit with measurement)

32³ keeps chunk-level culling granular and the padded column fits a `u64`. The alternative from the research is 62³ data padded to 64³ (cgerikj / TanTanDev), which halves per-chunk overhead (draws, cull work, upload count) at the cost of coarser culling and remeshing more on edits. `CHUNK_SIZE` is a single constant with `PADDED_SIZE ≤ 64` asserted, so the experiment is cheap. Decide with: mesh µs per voxel, draws visible, and frame time at the same view distance.

## ADR-0004 8-byte quads with vertex pulling (Accepted; 4-byte variant Proposed)

`u64` quads leave 15 free bits for AO/light and a 16-bit block id. Lemon's 4-byte format fits 32³ chunks with ≤128 block types and halves vertex-fetch bandwidth. Decide with: GPU time of the opaque pass on a real GPU at radius 32 for both formats.

## ADR-0005 Job system: rayon + main-thread priority heap (Proposed)

Research (`docs/research/rust-tooling.md`): rayon has no priorities or cancellation. Plan: dedicated `ThreadPool` (cores − 1), main thread keeps a distance-sorted heap and submits at most ~2× worker count jobs per frame with `spawn_fifo`, results over crossbeam, stale results dropped by a per-slot generation counter. Replace only if profiling shows rayon overhead.

## ADR-0006 Mesh only with real neighbours (Accepted)

`gather_padded` returns `MissingNeighbour` instead of guessing. A chunk is meshed once all 6 face neighbours are resident; edits remesh the chunk and any neighbour whose border changed. This removes Lemon's `boundary_planes` cost (5–10× the mesh itself) and its bug class.

## ADR-0007 Palette storage with uniform shortcut (Accepted)

See `docs/ARCHITECTURE.md` for numbers. Open follow-ups: palette compaction after many edits, and a `u8` fast path if decode shows in profiles (decode is 17–24 µs per surface chunk today).

## ADR-0008 Ring-buffer chunk window instead of a hash map (Accepted)

O(1) neighbour lookup without hashing, trivial eviction on camera movement. A hash map (foldhash, packed `u64` key via `ChunkPos::packed_key`) is reserved for sparse sets such as persisted edits.

## ADR-0009 Noise: fastnoise-lite now, SIMD later (Accepted for now)

Bench (`docs/research/rust-tooling.md`): fastnoise-lite was the fastest scalar crate (44 ns per 2D sample with 4 octaves); simdnoise (SIMD) was ~3× faster but is unmaintained. Terrain is heightmap-first (1024 2D samples per column chunk); 3D noise for caves will be sampled on a coarse grid and interpolated. A hand-written `wide::f32x8` fBm is the next step if generation dominates the profile.

## ADR-0010 GPU-driven culling path (Proposed)

Port Lemon's compute cull (frustum + per-direction back-face rejection + Hi-Z), extended to two-phase occlusion, writing indirect args and a count for `multi_draw_indirect_count`. Fallback without the count feature: same compute pass writes zero-instance draws, drawn with plain `multi_draw_indirect`. Decide on the Hi-Z phase only once overdraw shows in GPU timings.

## ADR-0011 Far terrain / LOD (Proposed, research)

Candidates: meshed LOD rings with downsampled chunks (Distant Horizons style), or ray-marching a coarse brick map for the far field. Scope added 2026-11-03 by ADR-0016: far rings are generated directly at coarse spacing, and a horizon tier (heightmap clipmap or impostors) beyond the last ring is part of the comparison. Aokana (2025) reports 10¹⁰ voxels in ~6 ms but is read-only. Decide after the near field is GPU-driven and measured, with a prototype of each at the same view distance.

## ADR-0012 Benchmark methodology (Accepted, 2026-11-03)

One deterministic, offscreen scenario per scale (`small`, `default`, `far`), with a fixed seed, fixed camera orbit and fixed resolution. Warm-up is discarded, there are at least 3 repeats, and the CPU waits for the GPU every frame, so frame numbers are serialised per-frame cost. Each run stores its environment and a hash of the mesh output. `bench-compare` compares medians of per-repeat medians against a noise band of max(relative spread, 2%), and refuses debug builds or mismatched scenarios. Rationale: Decaf and Lemon recorded numbers that could not be reproduced or compared. A windowed present-mode bench is a later addition for frame pacing; it does not replace this.

## ADR-0013 Repository history (Accepted, 2026-11-03)

The new repo starts from one bootstrap commit. Decaf and Lemon keep their history in their own repos, and `docs/HISTORY.md` and the audits cite their commits by hash. Fair comparisons against legacy code use copies pinned to named commits in `tools/legacy-compare`.

## ADR-0014 Block textures: 16 px texture array with world-space UVs (Accepted, 2026-11-03)

Concept in `docs/TEXTURES.md`. One `Rgba8UnormSrgb` 2D array (16 × 16, 5 mips, `Repeat`), layer looked up per vertex from a `block_faces` storage buffer, one sample per fragment, texel-snapped linear + anisotropic filtering, per-voxel rotation from an ALU hash. `PackedQuad` is unchanged. Alternatives: 32 px array (same pipeline, 4× texels), flat colour + ALU noise (no fetches). Atlas rejected: greedy quads need per-block repeat, which an atlas cannot do without breaking mips. Art direction chosen by Noah on 2026-11-03: crisp 16 px. Accepted on the measurement: textured adds +8.6% / +7.8% (`default`, two runs) and +2.7% (`far`) to the GPU terrain pass on the RTX 2060, inside the +10% budget (measured, `docs/PERFORMANCE.md`). Windowed and screenshot modes render textured; `vx bench` defaults to `flat` until the sprint plan switches gates to textured (`docs/SPRINTS.md` rule 4), and `bench-compare` refuses mixed shading unless `--allow-shading-change` is passed.

## ADR-0015 wgpu indirect-draw validation off in release (Accepted, 2026-11-03)

Context: wgpu 30 keeps `InstanceFlags::VALIDATION_INDIRECT_CALL` on in release builds. With it, `multi_draw_indirect` does per-draw CPU work in wgpu-core (`DrawBatcher::add` per draw) plus a validation dispatch, so frame CPU record grew ~30 ns per draw (computed) although we record one call (backlog B-02).
Decision: release builds create the instance with the flag off (`IndirectValidation::for_build`); debug builds keep it on. With it off, D3D12 is excluded from the backends, because wgpu then drops `first_vertex`/`first_instance` from the vertex builtins on D3D12 and the terrain shader depends on both. `TerrainRenderer::upload_static` builds every indirect argument in range by construction and checks it with a `debug_assert`. `vx bench` and `vx --screenshot` take `--indirect-validation on|off` to measure the switch.
Alternatives: keep validation (costs 0.28 ms of CPU per frame at `default` and 1.09 ms at `far`, computed from measured record medians); emit fewer draws first (B-06 does that too, but the per-draw cost would remain); keep D3D12 with validation on (possible later via a per-backend instance if a Windows machine lacks Vulkan).
Decided by (measured, `noah-msi-rtx2060`, `docs/PERFORMANCE.md`): frame CPU record −81% at `default` (two runs) and −94% at `far`, FASTER; GPU terrain pass within noise everywhere; screenshots byte-identical with the flag on and off on the RTX 2060 (Vulkan) and lavapipe.

## ADR-0016 Unbounded world: coordinate ranges and precision (Accepted, 2026-11-03)

Context: Noah set the top-level goal of an infinite world with nearly infinite render distance (2026-11-03). Today the terrain shader builds absolute f32 world positions (`chunk_origin + local`) and the camera is an absolute f32 `Vec3`. f32 spacing is 1/128 block at 65,536 blocks and 1/8 block at 1,048,576 blocks (computed), so vertices and the camera visibly jitter long before the world ends. Terrain noise takes block coordinates as f32 and multiplies by the frequency, which loses resolution between neighbouring blocks around a million blocks out (estimate).
Decision:
- **Block and chunk coordinates stay integer.** `i32` blocks give ±2.1 × 10⁹ blocks (computed). `ChunkPos::packed_key` uses 21 bits per axis, ±1,048,576 chunks = ±33.5 M blocks at 32³ (computed); that is the playable range until a key change is needed, and the world is generated, streamed and stored without any other bound. `ChunkWindow` addresses by `rem_euclid` and is unbounded already.
- **Camera-relative rendering** (roadmap 18, moved to S3): the camera is a chunk position plus an f32 local offset; chunk origins go to the GPU relative to the camera chunk; the view matrix only translates by the local offset. f32 then only ever holds distances within the view, never absolute positions.
- **Noise precision:** sample noise from the integer cell plus an f32 fraction (or fastnoise-lite's `f64` feature, chosen by gungraun instruction counts), so generation is identical in quality at any distance.
- **Far field:** LOD levels beyond the near rings are generated directly from noise at coarse spacing (never by downsampling level 0), and a horizon tier beyond the last voxel ring carries the silhouette to the far plane. Mechanism chosen in ADR-0011.
Alternatives: f64 positions on the GPU (not portable in WGSL, slow where supported); periodic origin rebasing of absolute f32 positions (works, but every system must handle the rebase event; chunk + local makes it implicit); a bounded world (rejected by the goal).
Re-check of S2 decisions under this goal: 32³ chunks hold (LOD keeps the chunk count per ring constant, and 32³ won generation, meshing and memory at `default` and `far`, measured, `docs/PERFORMANCE.md`); SSAO with the 6-neighbour rule holds (vertex AO would add ~67% quads at every LOD level, computed by the AO probe, and screen-space cost does not grow with reach).
