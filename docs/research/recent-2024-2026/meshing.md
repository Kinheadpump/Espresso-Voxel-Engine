# Meshing and geometry formats: research for Espresso

Date: 2026-11-03. Tags: [S] = read on a page I fetched (URL given); [U] = background knowledge / my own reasoning, not verified. "Quoted" = number from the source. "Estimate" = mine.

Honest note: the 2024-2026 literature on blocky-voxel meshing is thin. No SIGGRAPH/HPG/arXiv paper turned up that beats the binary-greedy approach Espresso already has. Most new material is crates, mods and blogs. Espresso's mesher (~35 us/surface chunk, u64 quads) is already in the same range as the best published crate numbers, so the wins are in AO, LOD and the draw path, not in raw meshing speed.

## Ranked summary

| # | Candidate | Source / year | Fit | Benefit | Effort | Verdict |
|---|-----------|---------------|-----|---------|--------|---------|
| 1 | AO-safe merge restriction + AO bits in the 15 free quad bits | block-mesh-bgm 0.2.4, 2026; 0fps AO (covered) | good: mesher + quad format + fragment shader | AO at zero extra bytes; mesher cost within ~0-15% (quoted microbenches) | M | Adopt-candidate |
| 2 | 4-byte quad (32^3 chunk, material via per-chunk palette index) | derived; refs: Sodium 20 B/vertex, Exile 8 B/vertex, remorses 4 B/vertex | good: quad buffer, vertex pulling, offset allocator | geometry MiB halves vs u64 (estimate) | M | Prototype (after AO decision) |
| 3 | Texture array with 12-bit layer id, UV = quad extent, repeat sampler | Exile 2018 | good: fragment shader | no atlas bleed/mip trouble; no extra fetch (estimate) | S | Adopt-candidate |
| 4 | wgpu mesh shaders (EXPERIMENTAL_MESH_SHADER) for per-chunk/meshlet culling and quad expansion | wgpu 30 docs, issue #7197; Meshelium 2026 | partial: replaces MDI draw path; competes with ADR-0010 compute culling | quoted 3.1x-5.3x FPS at RD 32-64 in a Minecraft mod (not comparable) | L | Watch |
| 5 | Geomorphed POP-buffer LOD for blocky meshes | 0fps 2018 (LOD post) | partial: alternative to ADR-0011 skirts | no skirts, no per-LOD storage; no numbers in source | L | Prototype only if skirts show artefacts |
| 6 | Opacity-mask fast path in binary mesher | binary-greedy-meshing crate 0.5.2, 2026-01 | partial: Espresso already ~35 us | quoted 4x between its two modes (300 -> 65 us) | S | Watch |
| 7 | GPU compute meshing | SpawnDev.VoxelEngine claims | poor: CPU mesh is 35 us, rayon already hides it | claims 50-200 us/chunk (unverified) | L | Reject |
| 8 | Geometry-shader point-per-quad expansion | emamaker voxel2 blog | poor: wgpu has no geometry shaders; vertex pulling is the same idea | quoted 82 -> 20 MB VRAM | n/a | Reject |

## 1. AO-safe binary greedy merging

- Source: block-mesh-bgm 0.2.4 (docs.rs page dated 2026-10-20) https://docs.rs/block-mesh-bgm and its README https://docs.rs/crate/block-mesh-bgm/latest/source/README.md [S]. Background: 0fps AO is covered already.
- What it does [S]: `binary_greedy_quads_ao_safe` runs the same u64-column visibility pipeline, then looks at opaque occupancy in the plane just outside each visible face (shifts and masks, whole rows at once) to classify cells as unit-only, one-direction-only or free-merge. This keeps merge boundaries where per-vertex AO would change across a quad. AO values themselves are computed afterwards over the final quads, not inside the mesher. Padded 64^3 limit (62 interior) [S].
- Quoted microbenchmarks (author's machine, Criterion medians) [S]: dense-sphere-ao 30.8 us plain vs 35.6 us AO-safe; ao-boundary-stress 16.1 vs 16.5 us; layered-caves-2x2x2 534 us vs 413 us (AO-safe faster there). So AO-safe costs roughly 0-15% on small cases.
- Fit: good. Espresso quads have 15 free bits; 4 corners x 2 bit AO = 8 bits fit with no size change [computed, ADR-0004 layout]. Touches the mesher (merge predicate on the 34^3 padded input) and a post-pass over quads. The Espresso padding is 1 voxel per side, which is enough for the plane-outside test; diagonal corner voxels are needed for corner AO, they are not: today's gather fills only the 6 face slabs, so this depends on the 26-neighbour gather in PROPOSALS P-03.
- Alternative to compare against: SSAO (no mesher change, costs a depth/normal pass, noisy on thin voxel edges) [U]. CLAUDE.md prefers screen-space; vertex AO is cheaper on GPU (zero fetches) but costs mesher time and quad count. Measure both.
- Also needed: the AO anisotropy diagonal flip must be done in the vertex-pulling shader (pick triangle split from 4 AO values, ALU only) [U].
- Expected benefit: visual only. Cost: mesh ms up some %, quad count up (estimate: 10-30% more quads on rough terrain, unmeasured), geometry MiB up with it.
- Effort: M.
- Measure: existing `mesh ms`, `quads`, `geometry MiB` from `vx bench` + divan mesher microbench; content hash will change (expected). New: quads-per-chunk histogram, frame GPU ms terrain pass.
- Verdict: Adopt-candidate. Fits the roadmap item "vertex AO or SSAO"; run it as an A/B against SSAO.

## 2. 4-byte quads

- Sources for reference formats: Sodium CompactChunkVertex (20 bytes per vertex, 4 vertices per quad = 80 B/quad; RG32 position split hi/lo 10 bits, RGBA8 colour with AO multiplied in, 2x16 UV, light/material/section) https://github.com/CaffeineMC/sodium/blob/dev/common/src/main/java/net/caffeinemc/mods/sodium/client/render/chunk/vertex/format/impl/CompactChunkVertex.java [S]. Exile: 8 B/vertex, 4x2-bit AO per vertex, 12-bit texture id, 31x31 chunks because 8-bit coords cannot hold 32 https://thenumb.at/Voxel-Meshing-in-Exile/ [S] (2018). remorses/cgerikj mesher: 4 B per vertex (6b xyz, type, light, normal, AO) https://github.com/remorses/binary-greedy-meshing [S]. Voxel Project note: 4 B packed vertex, 6b/axis + 3b normal + 11b texture https://publish.obsidian.md/ve/Projects/Voxel+Project [S].
- Takeaway: Espresso's 8 B per quad is already 10x smaller than Sodium's 80 B/quad and at the level of the best Rust crate (u64 per quad, binary-greedy-meshing 0.5.2 [S]). No published engine is at 4 B per quad; this would be new work.
- Layout proposal (my arithmetic [U]): chunk-local x,y,z 5b each (0..31) = 15; w-1, h-1 5b each = 10; dir 3 = 28 bits; 4 bits left. Block id does not fit. Options: (a) 4 bits AO-less and material fetched from a per-chunk-per-direction side buffer indexed by quad order (extra fetch, which CLAUDE.md wants to avoid); (b) keep 8 B quads and spend savings elsewhere; (c) split material into a second u32 stream only when the chunk has more than N materials. With AO (8 bits) 4 B is impossible; 6 B/8 B stays. So #1 and #2 conflict: pick one.
- Fit: good for geometry MiB, but geometry is not the memory bottleneck: the default scenario measured 7.7 MiB of geometry buffers (docs/PERFORMANCE.md), so halving saves ~4 MiB (computed). Benefit looks small unless quad bandwidth shows up in GPU terrain pass time. Estimate: negligible at default scenario.
- Effort: M. Measure: geometry MiB, GPU terrain-pass ms in `far` scenario (only meaningful on a hardware GPU; software adapter numbers are excluded by the repo rules).
- Verdict: Prototype only if the far scenario shows the terrain pass is memory-bound. Otherwise skip; AO in the free bits (#1) is a better use.

## 3. Texture array vs atlas for greedy quads

- Source: Exile post (2018) [S]: texture ids index a texture array (up to 4096 layers, high bits select banks because hardware layer limits are lower), sampled at (u/8, v/8, layer). Reasoning below is mine [U].
- Why: greedy quads tile the texture w x h times. With an atlas, tiling needs fract() and manual gradient handling to avoid mip bleed and seams, which costs ALU and breaks anisotropic filtering at tile edges. With an array, a repeat sampler and UV = quad-local (0..w, 0..h) just works, mips are per-layer, no padding in textures.
- Limits [U]: wgpu default max_texture_array_layers is 256; request higher limit (adapter often 2048+). Espresso block ids are 16b, but texture layer needs a block-to-layer lookup (small uniform/storage table, one fetch; or put layer in the quad directly if the mesher knows it). Per-face textures (grass top/side) need the lookup by (block id, dir).
- Fit: good (fragment shader + material table). Effort: S. Measure: GPU terrain pass ms, image diff at distance for mip artefacts. No quad/mesh metric changes.
- Verdict: Adopt-candidate when textures arrive.

## 4. wgpu mesh shaders

- Sources: wgpu 30 docs https://wgpu.rs/doc/wgpu/documentation/extensions/mesh_shading/index.html [S]; tracking issue https://github.com/gfx-rs/wgpu/issues/7197 [S]; Meshelium mod https://modrinth.com/mod/meshelium [S].
- Status [S]: still `EXPERIMENTAL_MESH_SHADER` in wgpu 30 with warnings about major bugs and breaking changes. API: `create_mesh_pipeline`, `draw_mesh_tasks`, indirect variants (each with its own feature). Task stage optional. Backends: Vulkan done, Metal and DX12 implemented. Naga: WGSL input supported, SPIR-V/HLSL/MSL writers supported; maintainer calls it "feature complete" in the issue. Limits: no queries (so no pipeline-statistics, and per issue timestamp-in-pass use unclear, check), primitive index builtin not generally supported, a redesign of output API (#9170) is planned and could break shaders. Maintainer perf tips: set `zero_initialize_workgroup_memory: false`; keep task payload small (NVIDIA cliffs at 108 and 236 bytes); NVIDIA likes 32-thread groups, AMD 64.
- Evidence of benefit [S]: Meshelium (mod, 2026) draws Minecraft terrain via VK_EXT_mesh_shader with GPU visibility checks and optional occlusion culling. Quoted on one RX 9070 XT, 1080p, no Sodium: RD12 1621 -> 2437 FPS (1.5x), RD32 393 -> 1206 (3.1x), RD64 114 -> 607 (5.3x). Quoted: ~2x over Sodium alone at RD 64/96. Single machine, author's numbers, baseline is the vanilla renderer, so it says little about an already-MDI + vertex-pulling engine.
- Fit: partial. A task shader per chunk could do frustum + per-direction culling and a mesh shader could expand quads to 4 vertices with the position decode in registers (no vertex-pull index math). That overlaps ADR-0010 (compute culling + MDI), which works on every backend. Mesh shaders would add a hardware gate (RDNA2+/Turing+/Arc) and a fallback path, i.e. two renderers.
- Expected benefit: estimate, large only at huge draw counts / overdraw-heavy views; at 22k chunks, one MDI call with GPU culling is likely already fine. Unmeasured.
- Effort: L. Measure: terrain-pass GPU ms (timestamps) in `far`; new scenario at radius 64+.
- Verdict: Watch. Revisit after ADR-0010 ships and only if the terrain pass is vertex/primitive-bound. Re-check when #9170 lands.

## 5. Geomorphed POP-buffer LOD for blocky meshes

- Source: 0fps, "A level of detail method for blocky voxels", 2018 https://0fps.net/2018/03/03/a-level-of-detail-method-for-blocky-voxels/ [S]. Not in the covered list (only 0fps meshing/AO/Minecraft are).
- What it does [S]: quads are stored once, sorted coarse-to-fine (O(n) counting sort). Per-quad LOD = coarsest level where the quad is non-degenerate (trailing-zero count of corner coords for unit squares; leading-zero count of lo XOR hi for greedy quads). At draw time the vertex shader rounds coordinates to the LOD grid; quads whose corners collapse are skipped (draw a prefix of the sorted buffer). Cracks are removed by geomorphing with a continuous per-vertex LOD, with a max-over-neighbours fixed-point iteration (2-3 iterations, unrollable in the vertex shader). Author says skirts/seams are not needed. No benchmarks in the post.
- Fit: partial. Fits quad-per-chunk storage and vertex pulling nicely (one buffer, draw count = prefix), and removes per-LOD meshes. But it conflicts with ADR-0011 (separate clipmap ring buffers with skirts, coarser voxel data per ring) because rounding fine geometry gives wrong results for materials/thin features and the voxel data for far rings would still be needed. Also changes the per-direction quad grouping (sorting by LOD inside each group).
- Expected benefit: removes skirts and extra LOD meshing work; quad-count reduction at distance is unquantified (no numbers in source).
- Effort: L. Measure: quads drawn per frame, geometry MiB, frame GPU ms, plus a crack/popping visual test; new metric "quads submitted per LOD".
- Verdict: Prototype only if skirts show artefacts or LOD meshing time dominates. Otherwise keep ADR-0011.

## 6. Opacity-mask fast path

- Source: binary-greedy-meshing 0.5.2 (published 2026-01-07), https://docs.rs/crate/binary-greedy-meshing/latest [S]. Rust port of BGM v2 with transparency; quads `[Vec<u64>; 6]` with layout `(type<<32)|(h<<24)|(w<<18)|(z<<12)|(y<<6)|x` [S] (almost identical to Espresso's).
- Quoted (Ryzen 5 5500) [S]: `fast_mesh` opaque 65 us vs `mesh` 300 us; with transparency 90 vs 340 us; claims ~30x faster than block-mesh-rs (3 ms). `fast_mesh` needs precomputed opacity/transparency masks.
- Fit: partial. Espresso is 35 us/surface chunk on its own hardware, so it is already at or below these (different CPUs, different chunk size; not comparable strictly). The takeaway is only that keeping per-chunk opacity bitmasks (built during generation or edit) lets the mesher skip the per-voxel type compare when building columns; Espresso's palette `Uniform`/`Packed` storage could produce masks cheaply per palette entry (palette of N entries -> N-bit lookup) [U].
- Effort: S. Measure: divan mesher bench, `mesh ms`. Check first where the 35 us goes (column build vs merge); if column build is under ~30%, skip.
- Verdict: Watch; profile first.

## 7. GPU compute meshing

- Source: SpawnDev.VoxelEngine (C#/WebGPU), https://github.com/LostBeard/SpawnDev.VoxelEngine [S, claims only]. Claims: binary greedy meshing in compute, 50-200 us per chunk, 80-90% vertex reduction vs naive, 6 orientation groups with back-facing groups skipped (claimed 36% reduction), one small counter readback per column. No hardware or scene stated.
- Fit: poor. Espresso CPU mesh is 35 us per chunk on worker threads; GPU meshing needs voxel upload, output size readback (or indirect count) and buffer allocation from GPU-written sizes, which fights the offset allocator. Worth it only if edit latency or worker CPU load becomes the problem.
- Verdict: Reject for now. Per-direction grouping is already in Espresso.

## 8. Geometry-shader point-per-quad

- Source: https://emamaker.codeberg.page/projects/voxel2/better-meshing.html [S]. One point per quad (pos, extents, type, backface) expanded in a geometry shader; VRAM 82 -> 20 MB (-75%), 8 MB with ints, on old GPUs (2008 laptop).
- Fit: poor. wgpu has no geometry stage; vertex pulling from a storage buffer already gives the same compression without the geometry shader cost. Useful only as confirmation that per-quad storage is the right unit.
- Verdict: Reject.

## Searched but nothing usable

- Vercidium: only a DeepWiki summary of the patreon repo turned up, no formats, bit packing, SIMD or numbers (https://deepwiki.com/vercidium-patreon/meshing/4-meshing-system [S]); the video titles ("500 million voxels/sec") are unsupported by text I could fetch. No usable source.
- Sodium: page confirmed 20 B/vertex compact format; I did not find a write-up with measured gains from it.
- Nvidium (mesh-shader Minecraft mod): searches returned only mod listing pages; Meshelium page is the only mesh-shader voxel source with numbers.
- Meshlet/cluster approaches for voxels: no paper or engine write-up found; vkguide ascendant page (https://vkguide.dev/docs/ascendant/ascendant_geometry/) says it does not use mesh shaders or meshlets [S] but is useful as a reference for GPU-culled indirect draws: compute culling emits indirect commands plus count, one command per chunk with an offset into a ~400 MB gigabuffer, 8^3 chunks, 12 B vertices, no Hi-Z yet, up to ~400k chunks [S].
- SIMD meshing (AVX2/AVX-512/NEON) beyond u64 bit tricks: nothing found; BGM is already scalar-u64 bit-parallel.
- T-junction-free / watertight greedy meshes: only the 0fps LOD post above (geomorph) and the already-covered blackflux; a 2026 transvoxel blog exists (app.cinevva.com) but is smooth-terrain, not blocky, not fetched.
- 2024-2026 papers on blocky voxel meshing (SIGGRAPH/HPG/I3D/arXiv): none found by search; search engine is weak on arXiv.
- Per-vertex AO + lighting-friendly merges beyond block-mesh-bgm: nothing newer than 0fps (covered).
- Godot forum GPU compute mesh example: page content not retrievable (metadata only; author states the code is LLM-generated and unvalidated).
