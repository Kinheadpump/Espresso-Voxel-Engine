# Recent voxel engine research (2024–2026): what Espresso can adopt

> Snapshot, 2026-11-03. The R-01..R-13 candidates below are merged with the engine survey's proposals and ranked in `docs/BACKLOG.md` (old-id table at its end). This page keeps the evidence and the original order; the backlog holds the current ranking.

Survey date: 2026-11-03. Five topic reports, each with sources, fit, expected benefit, effort and the measurement that decides it:

| Report | Topic |
|---|---|
| [meshing.md](meshing.md) | Meshing, quad formats, texturing, mesh shaders |
| [gpu-culling.md](gpu-culling.md) | GPU-driven culling, Hi-Z, cave/visibility graphs, occlusion metrics |
| [raytracing-sparse.md](raytracing-sparse.md) | Ray marching, sparse trees/DAGs, GI built on them |
| [lod.md](lod.md) | LOD and far terrain (Cubyz, Voxy, Distant Horizons source read) |
| [storage-gen-streaming.md](storage-gen-streaming.md) | Persistence, compression, generation, streaming, light propagation |

This extends `docs/research/voxel-techniques.md` and the engine survey behind `docs/BACKLOG.md` (PR #2); sources covered there were skipped. Nothing here changes engine code, and nothing starts before the hardware baseline (roadmap item 1).

Labels follow the benchmarking docs: **[S]** read on a page or in source fetched this session (URL given), **[U]** background knowledge or unverified. Numbers are **quoted** (from the source), **computed** or **estimate**. No number in these reports was measured on Espresso.

## Headline finding

There is little 2024–2026 *academic* work aimed at editable blocky worlds. The papers that exist (SVDAG editing, transform-aware DAGs, radiance cascades, Split RC, SGGX-H LOD) target static scans, triangle scenes or offline rendering and are rejected or watched below. The useful new material is engineering: Minecraft-ecosystem renderers (Sodium 0.9, Voxy, Distant Horizons, Meshelium), Cubyz, Hytale's published storage and worldgen notes, and a few Rust crates. Espresso's binary mesher (~35 µs per surface chunk, measured) is already in the range of the best published numbers, so the gains are in culling, LOD, AO and streaming, not raw meshing speed.

## Ranked shortlist across topics

R-ids are separate from the P-ids in `docs/BACKLOG.md`; where an item sharpens a proposal or roadmap entry, the column says so.

| # | Candidate | Source (year) | Relates to | Effort | Verdict | Decided by |
|---|---|---|---|---|---|---|
| R-01 | Occlusion counter ladder + `cave`/`mountain`/`fly-fast` scenarios | Bevy docs, Hitchman, own design | Roadmap 3, 9, 11; P-02 | S | Adopt-candidate | Prerequisite: every culling item below is judged with it |
| R-02 | Compute face-direction + frustum cull, atomic-compacted draws | Voxy `cmdgen` (2025–26) | ADR-0010, roadmap 9 | M | Adopt-candidate | quads submitted, terrain-pass GPU ms, image identical with cull off |
| R-03 | Voxel visibility graph (cave culling + angle clamping, async) | Hitchman (2025), Sodium PR #2811 and 0.9 | Alternative/complement to roadmap 11 and P-02 | M | Prototype | +5–25% sections culled, ~0.5 ms / 8k sections (quoted) vs our `vis_graph_ms` |
| R-04 | Cubyz-style LOD: same 32³ chunk, voxel size doubles per level, one ring per level, finer side builds border faces against the coarser neighbour (no skirts) | Cubyz source (2026) | ADR-0011, P-04 | M–L | Adopt-candidate | max radius at fixed frame/VRAM budget, seam screenshot test |
| R-05 | LOD downsample = pick one real child block (surface-biased, parity tie-break) | Cubyz, Voxy `Mipper` (2026) | R-04 | S | Adopt-candidate | silhouette screenshots per level |
| R-06 | AO-safe binary greedy merge + 2-bit corner AO in free quad bits | block-mesh-bgm 0.2.4 (2026) | P-03, roadmap 15 | M | Adopt-candidate | quad count, mesh µs (quoted 0–15% cost), vs SSAO GPU ms |
| R-07 | Edited-chunk-only saves, zstd/lz4 on palette blobs, region files | Hytale storage notes (2026), zstd/lz4_flex benches | P-08, roadmap 14 | M | Adopt-candidate | bytes/chunk, load vs regenerate ms, content hash round-trip |
| R-08 | Streaming rules: forward-frustum priority, teleport boost, slot-epoch staleness check, retire-frame frees | Paper, concinnity-engine | ADR-0005, roadmap 7–8, P-05 | M | Adopt-candidate | time-to-fill-view, wasted-job %, p99 frame ms while flying |
| R-09 | Coarse density grid + interpolation + masks for 3D noise; generator writes straight into palette storage | Hytale worldgen doc (2026), Noisium | ADR-0009, P-09 | S–M | Adopt-candidate once 3D noise exists | gen µs/chunk on a cave seed |
| R-10 | Texture array with repeat sampler instead of atlas | Exile (2018) | roadmap 17 | S | Adopt-candidate | no bleed at mip levels; terrain-pass GPU ms unchanged |
| R-11 | 1-bit occupancy volume from mesher columns, bit-DDA AO/shadow rays | VoxelRT (2024–25), Teardown | P-10 | M | Prototype after SSAO exists | GPU ms vs SSAO at equal quality, occupancy MiB |
| R-12 | Voxy hierarchical GPU LOD traversal (screen-area split, Hi-Z on nodes) | Voxy source (2026) | after R-04 | L | Prototype later | GPU ms, nodes visited, CPU request-queue latency |
| R-13 | Starlight-style light propagation (inc/dec deques, direction masks) | Starlight, voxel-light (Rust) | roadmap 16 | M | Prototype when lighting starts | light µs/chunk vs 174 µs torch baseline (quoted) |

**Watch:** wgpu mesh shaders (still `EXPERIMENTAL_MESH_SHADER` in wgpu 30, Vulkan-only in naga; Meshelium quotes 3.1–5.3× vs vanilla Minecraft, not vs an MDI renderer), 4-byte quads (geometry is 7.7 MiB measured in the default scenario, so the saving is small unless the terrain pass proves bandwidth-bound; also collides with AO bits), Brixelizer GI (~1.7–2 ms quoted, 110–500 MB), sparse 64-trees, screen-space radiance cascades, 0fps geomorphed LOD, Distant Horizons column heightfields as a >10 km tier, SIMD noise via fastnoise2 (C++ toolchain dependency).

**Reject:** GPU compute meshing, geometry-shader quad expansion, transform-aware/editable SVDAGs, Split Radiance Cascades (8.6–11.5 ms quoted), wgpu hardware ray query (experimental, BLAS rebuild per edit), GPU world generation (readback spikes), lossy VDB brick compression, Transvoxel (marching cubes only), SGGX-H, HROC.

## Suggested order once the baseline is in

1. **R-01** first: the new scenarios and counters are what make R-02, R-03, P-02 and Hi-Z comparable. It is benchmark infrastructure, so it fits the current "measure before features" priority.
2. **R-08** as the design of roadmap item 7, then **R-02** with roadmap item 9.
3. Pick between **R-03**, P-02 (occluder quads) and two-phase Hi-Z with R-01's `cave` and `mountain` numbers.
4. **R-04 + R-05** as the concrete ADR-0011 prototype; it reuses `ChunkWindow` and the mesher unchanged except for border faces.
5. Looks: **R-06** vs SSAO, then **R-11** only if it beats SSAO on Noah's GPU.

## New benchmark work these items need

- Scenarios: `cave`, `mountain`, `fly-fast` (straight flight + teleport), `save-load`.
- Metrics: `chunks_frustum`, `chunks_graph`, `chunks_hiz`, `quads_submitted`, cull-pass GPU ms, `vis_graph_ms`, time-to-fill-view, wasted-job %, `save_bytes`, `load_ms`, occupancy MiB.
- Correctness: render with culling off and on and require identical images (catches over-culling holes); a random-solid-faces torture world for the visibility graph.

## Gaps

Not reachable or not found: reddit (blocked), zeux.io two-phase write-up, Meshelium TECHNICAL/PERFORMANCE docs, Douglas Dwyer's engine, HashDAG follow-ups, Voxy/Distant Horizons write-ups with frame-time numbers (both were read as source code instead), Morton-vs-linear layout benchmarks, visibility-buffer rendering for voxels, and any 2024–2026 paper on blocky-terrain LOD. Voxy and Nvidium internals partly come from auto-generated DeepWiki summaries, marked as secondary in the reports.
