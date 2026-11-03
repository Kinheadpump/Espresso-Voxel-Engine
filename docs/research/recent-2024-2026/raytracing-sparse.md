# Ray tracing / sparse structures / lighting built on them (Espresso)

> Snapshot, 2026-11-03. Tags [S] and [U] and the number labels as in `docs/DOC-GUIDELINES.md` §4. Ranked candidates from this report are in `docs/BACKLOG.md`.

Tags: [S] verified from a page fetched this session (URL given), [U] background knowledge / unverified. Numbers are "quoted" unless marked "estimate".

Judging rule: CLAUDE.md says screen-space approximations first; world-space tracing only if measurements say so. Espresso is a mesh-rasterised blocky world with a greedy mesher, so most ray-traced structures are a SECOND representation next to the mesh. Cost of keeping two representations is the main argument against almost everything below, except the cheap one (occupancy bitmask volume).

## Ranked summary

| # | Candidate | Source/year | Fit | Benefit | Effort | Verdict |
|---|---|---|---|---|---|---|
| 1 | 1-bit occupancy volume (derived from chunk palettes) for DDA-traced AO/shadow/contact rays | Teardown-style [U]; VoxelRT XBrickMap/MultiDDA 2024-25 [S] | good | est. replaces/augments SSAO with off-screen-correct AO; no quoted number | M | Prototype |
| 2 | Brixelizer-style cascaded sparse brick field + screen-probe GI as architecture reference | AMD GDC 2024 [S] | partial | quoted ~2 ms @4K (7900 XTX), ~1.7 ms @1080p (RX 7600 XT) | L | Watch (steal ideas) |
| 3 | Sparse 64-tree (4^3 branching, u64 child masks) | dubiousconst282 VoxelRT notes 2024-25; expenses/tree64 crate [S] | partial | quoted 182.6 Mrays/s Sponza 2k on an iGPU; ~half ESVO memory | L | Watch |
| 4 | Radiance cascades: screen-space + visibility bitmask (SSGI) | Chalmers thesis, Abou Dan & Jensen, 2025 [S] | partial | quoted ~2 / 6 / 26 ms (low/med/high) | L | Watch |
| 5 | Split Radiance Cascades (3D world-space sparse hashed probes) | Freeman & Sannikov, arXiv 2607.20384, Jul 2026 [S] | poor now | quoted 8.6 ms Sponza, 11.5 ms San Miguel (RTX 3080 Laptop) | L | Reject for now / Watch |
| 6 | Transform-aware SVDAG + SVDAG editing | Molenaar & Eisemann, i3D/PACMCGIT 2025; CGF 2023 [S] | poor | memory only; no quoted numbers fetched | L | Reject |
| 7 | wgpu hardware ray query (EXPERIMENTAL_RAY_QUERY) | wgpu changelog v23-v30 [S] | poor | n/a | L | Reject (revisit later) |

## 1. Occupancy bitmask volume + DDA rays for AO / shadows

- Source: Teardown frame teardown (acko.net) is already covered, so this is the generalisation. Brickmap-with-4^3-bit-mask structures benchmarked in dubiousconst282/VoxelRT (https://github.com/dubiousconst282/VoxelRT, `alt-renderers` branch): XBrickMap = 3-level grid with 4^3 occupancy bitmasks; MultiDDA = two-level grid of 8^3 bricks [S]. Rust/WGSL example of chunk DDA: roxlap-gpu `chunk_dda.wgsl` on docs.rs (seen in search results only, not read) [U].
- What: keep a GPU texture/buffer of 1 bit per voxel (32^3 chunk = 4 KiB, u64 columns like the mesher already builds). Trace short rays (AO: 4-8 cosine rays of 8-16 voxels; shadow: 1 ray to the sun) from the G-buffer position with a bit-level DDA. Works off-screen, no SSAO halo/disocclusion artefacts.
- Fit: good. The mesher already produces padded u64 columns; the same data layout is the occupancy format. Touches ChunkWindow (ring buffer maps directly to a 3D toroidal volume, no hashing) and a new compute/post pass. Does not touch the quad pipeline.
- Cost sizing (estimate, derived from heap numbers): radius-32 = 22,445 chunks x 4 KiB = ~88 MiB of occupancy if fully dense bits; Uniform chunks need no storage (stay in a 1-byte-per-chunk flag grid), so real cost is far lower. Measure it.
- Benefit: no quoted number. VoxelRT quotes one diffuse bounce at 0.7-0.9x of primary-ray throughput for brick/64-tree structures [S], suggesting short bit-DDA rays are cheap, but that is on an iGPU with triangle-voxelised scenes.
- Effort: M for occupancy upload + AO pass; shadows M more.
- Benchmark: new GPU timestamp pass "occ_ao"; compare against SSAO on same scenario; new metric "occupancy MiB" and "occupancy update ms per edited chunk". Needs an image-quality proxy (e.g. mean abs diff vs 64-ray reference) because frame ms alone does not judge quality.
- Verdict: Prototype, but only AFTER the plain SSAO baseline exists, so the measured question is "does world-space occupancy beat SSAO in ms-for-quality". Per CLAUDE.md, ship SSAO first.

## 2. AMD FidelityFX Brixelizer / Brixelizer GI

- Source: "GI with AMD FidelityFX Brixelizer", GDC 2024, https://gpuopen.com/gdc-presentations/2024/GDC2024_GI_with_AMD_FidelityFX_Brixelizer.pdf [S]. MIT-licensed library, compute only, no HW RT.
- What [S]: cascaded sparse distance fields as clip maps; each cascade 64^3 voxels; occupied voxels hold an 8^3 "brick" in one shared R8 atlas; 3-level AABB tree per cascade for traversal. GI uses screen-space probes (8x8 octahedral) backed by a world-space radiance cache (256^3 R11G11B10, 4^3 bricks) and SH2 irradiance cache. Radiance is injected only from screen-space info.
- Fit: partial. Clip-map cascades match planned LOD clipmap of ring buffers (ADR-0011); the design is triangle-in, ours is voxel-in so we could skip the SDF build and use occupancy directly. The "screen probes + world cache" hybrid is the pragmatic version of the screen-space-first rule.
- Benefit, quoted [S]: ~2 ms at 4K on RX 7900 XTX, ~2.2 ms on RTX 4080, ~1.7 ms at 1080p on RX 7600 XT (half-res default). Memory ~110 MB at 1080p, ~200 MB at 4K (0.5x scale). These are for the complete GI, on high-end desktop GPUs, not on an iGPU.
- Effort: L (reimplement, no wgpu port exists that I found).
- Benchmark: GPU timestamp for GI pass; VRAM metric needs a "lighting buffers MiB" line next to geometry MiB.
- Verdict: Watch. Use as a design reference for ADR on GI; the 100-500 MB footprint is large against our 58.6 MiB voxel heap.

## 3. Sparse 64-tree (contree)

- Source: dubiousconst282/VoxelRT README, Tree64 section (https://github.com/dubiousconst282/VoxelRT/blob/alt-renderers/README.md) [S]; Rust impl https://github.com/expenses/tree64 (MIT, v0.2.0, deps glam/bytemuck, optional hashing "caching" feature) [S]. Author of crate "based on a voxel ray tracing article by dubiousconst282" [S]; I could not locate the article itself.
- What [S]: 4^3 branching, each node has a u64 child-mask, 12 bytes per node; about half the memory of ESVO in rudimentary tests; edits roughly O(log m) by path copying; top-down DDA over bitmasks was 11-15% faster than ESVO for primary rays; group-shared-memory ancestor stack +9% vs local array; mirroring to negative ray octant +11%.
- Benchmarks, quoted [S] (integrated GPU; author estimates 5-10x on real hardware, which is an estimate by the author): Tree64 182.6 Mrays/s Sponza 2k, 208.9 NY Highway 4k, ~93-112 Mrays/s on Bistro / San Miguel / Forest Lake / Eco House. StdBVH 215.9 and XBrickMap 115.2 on NY Highway; Tree64 vs XBrickMap are close, so the tree is not a clear win over a plain 2-level brickmap for primary rays.
- Fit: partial. Palette chunks are already compact for storage; a 64-tree would be a derived GPU-only structure for tracing (world ~22k chunks to a top-level 64-tree). Only worth it if we trace primary rays or long secondary rays (far GI, distant shadows). For LOD, a tree gives free coarser levels, overlapping ADR-0011. Edits require rebuilding per-chunk trees (author rebuilds trees within a chunk fully) which fits our chunk-remesh flow.
- Effort: L.
- Benchmark: new "trace ms" pass, Mrays/s metric, tree MiB per scenario, rebuild ms per edited chunk.
- Verdict: Watch. Revisit only if candidate 1 proves insufficient (needs rays longer than ~32 voxels). Hierarchy of u64 masks is the same trick as our u64 column mesher, so team skill transfers.

## 4. Screen-space radiance cascades with visibility bitmasks

- Source: "Screen Space Global Illumination Using Radiance Cascades in 3D Video Games", Ghaith Abou Dan, Mati Jensen, Chalmers MSc thesis 2025, https://odr.chalmers.se/items/9e42007a-bb60-440f-ba28-04410822758f/full [S]. Companion: Kyriakos Gavras, "Exploration and Optimization of Radiance Cascades for Real-Time Applications", Chalmers 2025 (screen-space probes with world-space intervals; no timings in record) https://odr.chalmers.se/items/3ee9fb4e-1880-46c2-802a-a660e38dc9ee/full [S].
- What [S]: radiance cascades (Sannikov, Path of Exile 2) combined with Visibility Bitmask GI; probes in screen space; no temporal accumulation. Targeted at the game As We Descend.
- Benefit, quoted [S]: ~2 ms lowest, 6 ms medium, 26 ms high; memory 0.03 to 3.9 GiB depending on setting. Limits quoted: no off-screen occlusion, flicker on small motion, upscaling artefacts. Hardware not stated in the abstract.
- Fit: partial. Obeys the screen-space rule, needs depth+normal+albedo G-buffer, which Espresso lacks (forward vertex-pulled quads). Adds a deferred or at least depth/normal pre-pass.
- Effort: L.
- Benchmark: GPU timestamp for GI pass; VRAM line.
- Verdict: Watch. Do SSAO / vertex AO first; a GI pass is a later stage with a ~2 ms floor, which is a lot against a frame budget the engine is currently spending in sub-ms terrain passes (estimate).

## 5. Split Radiance Cascades (3D, sparse hash probes)

- Source: Rouli Freeman, Alexander Sannikov, "Split Radiance Cascades: Real-Time Global Illumination via Sparse Radiance Probes", arXiv 2607.20384, 22 Jul 2026, https://arxiv.org/pdf/2607.20384 [S].
- What [S]: extends radiance cascades to 3D world space; probes stored in per-cascade hashmaps (64-bit keys, 18 bits/axis + 10 for LOD) rebuilt every frame, only near visible surfaces; rays start at screen surfaces and "split" their result across cascades by hit distance; N=4 cascades, 32 base directions, l=4, K=4; accumulation by atomics. Limits: light leaking, probe interpolation bias, naive temporal, moving objects left as future work.
- Benefit, quoted [S]: Sponza 8.6 ms and San Miguel 11.5 ms total (trace alone 1.3 and 4.5 ms) on RTX 3080 Laptop, using OptiX ray tracing against triangle scenes. Not voxel scenes [S].
- Fit: poor now. Needs a ray tracer (HW or our own voxel DDA from candidates 1/3), hashmap rebuild per frame (contradicts our no-hashing ring design), and the cost (8-12 ms) is far outside a 'cheap' budget.
- Effort: L.
- Verdict: Watch (voxel DDA replacing OptiX would be a natural combo later, and blocky worlds have trivially cheap rays; untested by anyone I found).

## 6. Transform-aware SVDAG and editing of compressed voxel scenes

- Sources: Mathijs Molenaar, Elmar Eisemann, "Transform-Aware Sparse Voxel Directed Acyclic Graphs", Proc. ACM Comput. Graph. Interact. Tech. vol 8 no 1, 2025 (i3D best paper), DOI 10.1145/3728301, https://publications.graphics.tudelft.nl/papers/810 [S]; "Editing Compressed High-resolution Voxel Scenes with Attributes", CGF 2023, DOI 10.1111/cgf.14757, https://diglib.eg.org/handle/10.1111/cgf14757 [S]. Abstract-level only; I did not read full text, no numbers quoted here.
- What [S]: DAG deduplication extended to mirror symmetry and general transforms plus translation matching, new pointer encoding; 2023 paper allows interactive edits with compressed colour attributes.
- Fit: poor. Gains are for 100k^3-style static scans (Aokana, already covered, is the relevant adjacent). Our palette storage already deduplicates Uniform chunks; terrain with noise has little repetition. Edit cost and per-block-id attributes are exactly the hard part.
- Verdict: Reject.

## 7. wgpu hardware ray query status

- Source: wgpu CHANGELOG.md (raw, fetched via curl) https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/CHANGELOG.md [S].
- Facts [S]: initial AS + ray query landed v23.0.0 (2024-10-25); v27 merged AS feature into `Features::EXPERIMENTAL_RAY_QUERY` (AMD driver bug workaround); v28 (2025-12-17) added `wgpu_ray_query` WGSL enable extension; v29 (2026-03-18) added Metal AS implementation and TLAS binding arrays; v30.0.0 (2026-07-01) and Unreleased contain continued ray-query fixes (Metal guards, SPIR-V writer fixes, ray query in loops) and a separate `EXPERIMENTAL_RAY_TRACING_PIPELINES` feature. Feature is still named EXPERIMENTAL_ in what I read; I did not find a stabilisation entry. No DX12 ray-query entry found in my grep (not conclusive) [U].
- Fit: poor. Voxel worlds with edits would need BLAS per chunk (AABB or procedural geometry) rebuilt on every edit; HW RT also excludes non-RT GPUs and software adapters (which the bench already treats as second class). Teardown's author moved to HW RT with intersection shaders for a new prototype renderer ([S] https://blog.voxagon.se/2024/12/29/year-summary.html), but that is a path tracer with DLSS RR denoising, not an AO/shadow add-on.
- Verdict: Reject for now (violates screen-space-first rule, experimental API, hardware gate). Re-check at wgpu 31+.

## Related reference data points

- Teardown author's new prototype [S, blog above]: sparse voxel objects as 8x8x8 chunks tracked by a 3D bitmap; Vulkan path tracer with HW RT intersection shaders. Confirms 8^3 bit-chunk bitmaps as the pragmatic sparse unit.
- Gvox engine (Gabe Rundlett, Daxa): renderer fetched via DeepWiki summary only [S, https://deepwiki.com/GabeRundlett/gvox_engine/4-rendering-pipeline]: voxel ray tracing with a low-res depth pre-pass to speed up the full-res primary trace, shadow rays to the sun combined with particle shadow maps, Kajiya-based GI. No chunk structure or numbers on that page. The low-res depth pre-pass trick is only relevant if we ever ray-trace primaries.

## Searched but nothing usable

- Dubiousconst282's own 64-tree article: not found by search; only the VoxelRT README notes and the tree64 crate.
- Douglas Dwyer's engine: no results found in the searches I ran (not found, not disproven).
- HashDAG follow-ups 2024-26 and HPG/EGSR 2024-26 "voxel ray tracing" papers: search returned only the Molenaar papers; no further hits verified.
- Radiance cascades on voxel worlds specifically: no voxel-scene results found; Split RC uses triangle scenes.
- Surfel / probe GI for Minecraft-like games, and flood-fill-light-vs-GI comparisons: nothing quotable found.
- Voxel AO via ray-marched occupancy with measurements: only hobby devlogs surfaced (itch.io), none with numbers; not fetched.
- Seen but not read: Holographic Radiance Cascades (2D, arXiv 2505.02041; quoted 1.85 ms 512^2, 7.67 ms 1024^2 on RTX 3080 Laptop [S]) is 2D-only, irrelevant.
