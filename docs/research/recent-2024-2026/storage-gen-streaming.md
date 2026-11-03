# Storage, compression, generation, streaming: research for Espresso

> Snapshot, 2026-11-03. Tags [S] and [U] and the number labels as in `docs/DOC-GUIDELINES.md` §4. Ranked candidates from this report are in `docs/BACKLOG.md`.

Date: 2026-11-03. Tags: [S] verified from a page I fetched (URL given), [U] unverified / background knowledge.
Honest note up front: this topic area has little 2024-2026 academic work that fits a blocky 32^3 palette engine. Most usable material is engineering practice from Minecraft-ecosystem projects, a few READMEs with real numbers, and canonical library benchmarks. Several searches returned nothing usable (see end).

## Ranked summary

| # | Candidate | Source/year | Fit | Benefit | Effort | Verdict |
|---|---|---|---|---|---|---|
| 1 | Persist only edited chunks (deterministic regen) + zstd/lz4 on palette blobs | Hytale storage (2026), Stardance devlog, zstd/lz4 benchmarks | good (ChunkWindow, palette storage, content hash) | Stardance: 65 KB -> 5.7 KB cave chunk (quoted); Hytale uses zstd-3 per blob | M | Adopt-candidate |
| 2 | Coarse-grid density sampling + trilinear interpolation + masking in the generator | Hytale worldgen optimisation doc (2026); Minecraft NoiseChunk | good (generator, divan gen bench) | estimate: 4x-8x fewer noise evals per chunk | S-M | Adopt-candidate (measure first) |
| 3 | Generator writes directly into palette storage (no per-voxel set bookkeeping) | Noisium mod (2024-25) | good (Packed/Uniform storage) | qualitative only | S | Adopt-candidate |
| 4 | Streaming priority: forward-frustum zones, delayed far loads, stale-job epoch check, retire-frame frees | Paper/Moonrise priority commit; concinnity streaming system | good (rayon priority heap, ring window, offset-allocator) | qualitative; fixes pop-in order and wasted work | M | Adopt-candidate |
| 5 | Starlight-style level-based light propagation (inc/dec deques, direction masks, light tied to chunk) | Spottedleaf gist / Starlight (2023), voxel-light, Pulsar (2026) | partial now, good when lighting lands | quoted: ~2x faster than vanilla 1.20; 0.1-1 ms propagation | M | Prototype when lighting starts |
| 6 | SIMD noise: fastnoise2 (C++ via Rust binding) vs simdnoise | FastNoise2 README, fastnoise2-rs, rust-simd-noise | partial (C++/CMake dependency) | quoted ~5-8x vs FastNoiseLite (AVX2, 2019 CPU) | S | Prototype (A/B vs own scalar noise) |
| 7 | GPU world generation with async readback | jedjoud10/VoxelTerrainGenerator, DH issue #937 | poor | readback spikes (quoted author complaint) | L | Reject for now |
| 8 | Lossy brick/VDB GPU volume compression | Zellmann/Wald et al., arXiv 2504.04564 (2025) | poor (lossy, volume rendering, not editable blocks) | not faster than dense texture on 4 of 5 sets | L | Reject |

## 1. Edited-chunk-only persistence + fast block compression

- Source: Hytale World Storage write-up (player.games, 2026) https://www.player.games/learn/hytale/hytale-world-storage-persistence-serialization ; Stardance devlog https://stardance.hackclub.com/projects/37737 ; zstd README benchmark https://github.com/facebook/zstd ; lz4_flex README https://android.googlesource.com/platform/external/rust/android-crates-io/+show/837c8013/crates/lz4_flex/README.md ; Linear mod https://github.com/memesgmm/Linear/
- What it does: [S] Hytale stores 32x32-column region files with 1024 blob slots and 4096-byte segments, zstd level 3 per blob, per-slot locks so chunks in one region write in parallel, a dirty poll every 0.5 s, loads fall back to worldgen only when nothing is stored. [S] The Stardance voxel project saves only edited chunks as diffs and reports a cave chunk shrinking 65 KB -> 5.7 KB (quoted, self-reported). Because Espresso generation is deterministic (content hash exists), the save needs only chunks whose edit flag is set; everything else regenerates. The payload is the existing palette + packed indices, then lz4 or zstd.
- Numbers [S, zstd README, i7-9700K 4.9 GHz, Silesia, lzbench]: zstd 1.5.7 -1 ratio 2.896, 510 MB/s compress, 1550 MB/s decompress; lz4 1.10.0 ratio 2.101, 675 / 3850 MB/s; zlib -1 2.743, 105 / 390. [S] lz4_flex (Ryzen 7 5900HX, rustc 1.69): safe decode 2338-4540 MiB/s, unsafe 2734-5512 MiB/s, depending on data. Palette-packed voxel data is not Silesia, so ratios must be measured on our own chunks.
- Fit: good. Touches palette storage serialisation, a new `world::save` module, ChunkWindow load path (load-else-generate). Caveat: Linear's claim "50-60% smaller saves" by compressing whole regions with zstd is [S] on the README but unverified there (no methodology; README says the code is AI-generated). Treat as a hint that region-level or dictionary compression beats per-chunk, not as a number.
- Effort: M (format, versioning, async IO on worker pool, edit tracking).
- Measure: new bench scenario `save-load`: chunks/s write+read, bytes per chunk (p50/p99), load-vs-generate ms per chunk (decides whether loading from disk even beats regenerating). Metrics to add: `save_bytes`, `load_ms`. Content hash must be identical after round trip.
- Decision test worth running first [U]: if gen ms/chunk is small, regenerating is cheaper than reading disk, so only edited chunks should be stored (that is the point). Compare zstd-1 vs lz4_flex on edited-chunk payloads; pick lz4 if decode time dominates load, zstd if file size matters.
- Verdict: Adopt-candidate. Small, deterministic, easy to verify with the content hash.

## 2. Coarse density grid + interpolation + masking in the generator

- Source: Hytale modding docs, worldgen optimisation page https://hytalemodding.dev/en/docs/official-documentation/worldgen/worldgen-tutorial/optimization-wip (2026). Minecraft NoiseChunk has a NoiseInterpolator class [S, mappings.dev listing exists in search results but I did not fetch the page]; the 4x8x4 cell size is [U].
- What it does: [S] Hytale docs recommend Y sample distance 8 "for the biggest performance gain", masking regions above the terrain max and below ground with constants (-1 / 1) so expensive density is skipped, preferring 2D over 3D domain warping, using cheap Mix over Sum/Min/Max, and caching. Core idea: evaluate 3D noise on a coarse lattice (e.g. every 4 in xz, 8 in y) and trilinearly interpolate; skip whole cells when a conservative bound says solid or air; per-column heightmap noise in 2D.
- Fit: good for the generator (divan gen microbench already exists). Output must stay deterministic, so the content hash will change once (accept as a new baseline, note in ADR).
- Benefit: estimate only [U]: 3D evals drop by cell volume ratio (4*8*4 = 128 voxels per lattice cell sample vs 1 per voxel before interpolation), plus most chunks above/below the surface need no 3D noise at all. Real gain depends on how much of gen time is noise; measure.
- Effort: S-M.
- Measure: existing `gen ms` (divan generator + `vx bench` world gen), content hash change recorded; add visual diff check of terrain shape (cell artifacts on overhangs).
- Verdict: Adopt-candidate, after a profile shows noise dominates gen time.

## 3. Generator writes straight into palette storage

- Source: Noisium https://modrinth.com/mod/noisium (2024-25).
- What it does: [S] Optimises `NoiseChunkGenerator#populateNoise` by writing block states directly into palette storage, skipping set-block bookkeeping that is useless during generation. Vanilla-identical output. Speedup quoted only as "a few seconds to a few dozen seconds faster depending on the amount of chunks" (qualitative).
- Fit: good. Espresso's Packed{palette, 1/2/4/8/16-bit} can be built in one pass: fill a flat u16 scratch array per chunk (thread-local, reused), detect Uniform early (all equal) and otherwise build palette and pack once, rather than calling a checked `set` per voxel that may re-grow the palette bit width. [U] This also avoids allocation churn; an unrelated Unity-style devlog (https://itch.io/devlog/619399/devlog-6-super-dooper-speedy-chunk-generation.amp) [S] confirms per-chunk kilobyte allocations were a real GC cost there, no numbers.
- Effort: S.
- Measure: divan generator bench, alloc count per chunk (add a counting allocator metric if missing), `voxel heap MiB` must not change.
- Verdict: Adopt-candidate. If current generator already does bulk build, drop it.

## 4. Streaming priority, stale-job handling, safe frees

- Sources: Paper priority commit https://ayakael.net/mirrors/papermc/commit/64666dc8b605f0f2c2ac309378b1e973eb0e4956 ; concinnity-engine streaming system https://docs.rs/concinnity-engine/0.19.16/src/concinnity_engine/gfx/streaming_system/mod.rs.html ; Stardance devlog (refcount on unload).
- What it does: [S] Paper's priority rules: priority zones in front of the player (circular zone radius 6, then forward frustum wedges at 3/5/7 chunks ahead with decreasing priority), a one-time high priority radius at teleport/login (priority 28), and delayed load of far chunks with front chunks delayed less than back chunks, high-priority recheck at most every 20 ticks. [S] concinnity: re-score by camera distance every frame, evict the item the camera moved away from when over cap, mesh eviction sets a retire frame = current frame + frames in flight so freed buffer space is not reused until the GPU finishes, and a memory-pressure valve (freeze dispatch, then shrink budgets, with hysteresis). That file has no explicit cancellation; it blocks work instead. [S] Stardance: unloading a chunk while a job still read it was the main race; fixed by refcount.
- Fit: good for the planned rayon priority heap, the ring ChunkWindow (slot generation counter = free staleness check) and offset-allocator suballocation (retire-frame free list is exactly what the allocator needs). [U] rayon cannot cancel a running task, so cancel at job start and between stages (gen -> mesh) by comparing the job's window-slot epoch with the current one; discard the result on mismatch.
- Benefit: no quoted numbers. Expected [U]: fewer wasted gen/mesh jobs while flying fast, front-first fill-in, no GPU use-after-free.
- Effort: M.
- Measure: new scenario `fly-fast` (orbit already deterministic; add a straight-line fast flight and a teleport). New metrics: time-to-fill-view (ms until all chunks within R are resident), wasted jobs (% results discarded), p99 frame ms during streaming, main-thread upload ms per frame.
- Verdict: Adopt-candidate (roadmap item already; use these rules as the design baseline).

## 5. Starlight-style light propagation

- Sources: Spottedleaf's explanation gist https://gist.github.com/Spottedleaf/6cc1acdd03a9b7ac34699bf5e8f1b85c ; Starlight README https://github.com/PaperMC/Starlight (archived 2024-03-08, last support 1.20); Pulsar https://github.com/Sumire-Labs/Pulsar (2026); voxel-light https://docs.rs/crate/voxel-light/latest/source/README.md
- What it does: [S] Propagate light levels, not updates; separate increase and decrease queues; Starlight adds (a) a flag for special-shape blocks halving block reads (author: 2x), (b) per-entry direction sets to skip useless directions such as the one the light came from, (c) a plain deque instead of a linked hash set. Light sections are tied to chunk objects (stateless engine), which allows lighting to run in parallel with generation. [S] Author states vanilla 1.20 is still about 2x slower than Starlight in propagation tests, tests take sub-millisecond to 1-2 ms at full world height, and chunk lighting is far below 1% of chunk generation time. [S] Pulsar (async, 1.12.2): fresh-chunk gen of 10,404 chunks 56.461 s vanilla vs 48.831 s Pulsar (1.16x total), single light edits 0.083-1.541 ms median (Ryzen AI Max+ 395, Lightbench). Pulsar's 550x is for one skylight workload and the page says it does not mean 550x FPS. [S] voxel-light (generic Rust BFS with ahash visited map, Ryzen 9 7900): level-14 torch propagation 174 us, removal 432 us, full place+remove 585 us, ~11.5k voxels touched. That is a naive baseline Espresso should beat by using dense 4-bit arrays and no hash map.
- Fit: partial today (no lighting yet), good later. Light storage as two nibble arrays per 32^3 chunk (16 KiB for both), Uniform-light compression for all-dark/all-lit sections, a 34^3 padded read for the mesher that already exists. Cross-chunk propagation needs neighbour access across ring-buffer slots; edits trigger relight on the worker pool. Starlight's design is Java, CPU, queue-based; not SIMD. A bitmask/SIMD variant is [U] unproven, no source found.
- Effort: M (L with sky light and cross-chunk edge cases).
- Measure: new metrics `light ms per chunk` (initial sky+block), `light update us per edit` (place/remove torch, open/close roof), light heap MiB; extend the mesher bench to include light-sampled AO if baked into quads.
- Verdict: Prototype when lighting is scheduled. Do not start before measuring that lighting is wanted; Decaf/Lemon history says no features before measurement.

## 6. SIMD noise: fastnoise2 vs simdnoise vs own

- Sources: FastNoise2 https://github.com/Auburn/FastNoise2 ; Rust binding https://github.com/Lemonzyy/fastnoise2-rs (crate fastnoise2 0.4.0 per https://docs.rs/crate/fastnoise2/latest/source) ; simdnoise https://github.com/verpeteren/rust-simd-noise
- What it does: [S] FastNoise2: node graph with intermediates kept in SIMD registers; SSE2/SSE4.1/AVX2/AVX512/NEON/WASM; runtime dispatch. Quoted (author, Intel 7820X 4.9 GHz, millions of points/s, 3D): Perlin 261.10 vs FastNoiseLite 47.93; Simplex 268.44 vs 36.83; Value 494.49 vs 64.13; Cellular 52.43 vs 12.49. 2D Perlin 624.27 vs 92.83. [S] simdnoise 4.x (i7-6700, single thread, Criterion): 3D 128^3 cell noise 1400 ms scalar -> 47 ms AVX2; 2D 4K fBm 3 octaves 888 -> 108 ms; supports fill into caller-owned buffers; maintainer is Peter Reijnders, release date not visible on the page.
- Fit: partial. fastnoise2-rs needs CMake and a C++17 compiler unless FASTNOISE2_LIB_DIR points to a prebuilt static lib [S]. That adds a C++ toolchain to the build and conflicts with a pure-Rust ethos; an ADR is needed if adopted. The README for the binding only showed `gen_uniform_grid_2d`; I could not confirm a 3D grid API from the fetched pages [U]. simdnoise is pure Rust with runtime dispatch but I have no measurement on its maintenance health.
- Benefit: quoted numbers are vs FastNoiseLite on a 2019 CPU, not vs a SIMD-aware hand-written Rust noise; true gain over Espresso's current generator is unknown. Combined with #2 (coarse grid) the noise cost may already be small.
- Effort: S for a throwaway A/B (fill a 9x5x9 coarse grid per chunk).
- Measure: divan generator microbench; `gen ms`, content hash will change (different noise function), so ship behind a bench-only switch first.
- Verdict: Prototype only if profiling shows noise > ~50% of gen time after #2.

## 7. GPU world generation (Reject for now)

- Sources: https://github.com/jedjoud10/VoxelTerrainGenerator ; Distant Horizons feature request https://gitlab.com/distant-horizons-team/distant-horizons/-/work_items/937
- [S] jedjoud10 generates voxels on GPU with async CPU readback and lists the readback as a known problem: slow, with frame-time spikes when more than one request per frame; no benchmark numbers on the page. [S] The DH item is a feature request without replies; obstacle named is mod compatibility.
- Fit: poor. Espresso needs chunk data on the CPU (mesher, edits, save), so readback is on the critical path; CPU rayon generation already runs off the main thread. A GPU-resident-only path (generate straight into the geometry buffer) would be a rewrite and makes edits and determinism hard. Software-adapter dev machines also cannot measure it.
- Verdict: Reject until CPU gen is shown to be the bottleneck for the radius-32 fill (22,445 chunks).

## 8. Lossy brick/VDB GPU compression (Reject)

- Source: Zellmann, Jaros, Amstutz, Wald, "GPU Volume Rendering with Hierarchical Compression Using VDB", arXiv 2504.04564 (2025; year inferred from references) https://arxiv.org/pdf/2504.04564
- [S] Lossy fixed-rate compression with OpenVDB, converted to NanoVDB, sampled in a path tracer; 32^3 bricks ranked by distance from the background value and activated to hit a target rate (1:8 to 1:1). Quoted FPS dense vs VDB at 1:8: 75.9 vs 55.8, 49.2 vs 42.1, 25.4 vs 24.2, 20.0 vs 18.2; only the galaxy set wins (18.6 vs 34.8) because it fits one GPU instead of three. Noise data: under 10% overhead.
- Fit: poor. Medical/scientific scalar volumes, lossy, no editing, ray sampling. Espresso renders meshed quads; palette storage is already lossless and tiny (58.6 MiB for 22,445 chunks). Only takeaway: 32^3 bricks ranked by emptiness is the same idea as Uniform chunks.
- Verdict: Reject.

## Searched but nothing usable

- Morton/Z-order vs linear voxel layout benchmarks on modern CPUs: no benchmark found. Only a zarray crate listing and unrelated hits. Gap; would need an own microbench (meshing 34^3 pad is bitmask-based, so Morton probably does not help; [U]).
- GPU-friendly random-access voxel compression 2024-2026 (brick palettes decodable in shaders): search returned only medical-volume papers (KIT/EG "Random Access Segmentation Volume Compression", CGF 2025, https://diglib.eg.org/handle/10.1111/cgf70116; not fetched) and VDB. Nothing for block worlds.
- C2ME: fetched README (https://github.com/RelativityMC/C2ME-fabric) has no numbers or algorithm description; module names (`c2me-opts-scheduling`, `c2me-opts-chunkio`, `c2me-rewrites-chunk-serializer`) only. Moonrise/Generator Accelerator pages: no numbers.
- Starlight pages give graphs only (images), no extractable timings; the Spottedleaf gist is the usable source. Starlight has been archived since 2024-03.
- Minecraft 26.1: Paper news page [S] mentions only region-file-compression allowing gzip and dimension folder layout; no new chunk-storage or lighting algorithm.
- SIMD/bitmask light propagation or GPU compute flood fill for voxel lighting: no source found (0fps voxel lighting is out of scope, already covered).
- Direct lz4_flex vs zstd comparison on voxel data: none found; only generic corpora (above).
- Rayon-specific chunk job cancellation write-ups: none found; the Paper commit titled "Cancel pending chunk loads when they are no longer needed" appeared in results but I did not fetch it [U].
