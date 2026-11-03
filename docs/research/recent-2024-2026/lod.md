# LOD and far terrain for blocky voxel worlds (research for Espresso)

> Snapshot, 2026-11-03. Tags [S] and [U] and the number labels as in `docs/DOC-GUIDELINES.md` §4. Ranked candidates from this report are in `docs/BACKLOG.md`.

Date of research: 2026-11-03. Tags: [S] = verified by reading a fetched page or cloned source this session (URL / repo given); [U] = background knowledge, not verified. "quoted" = number is in the source; "estimate" = mine.

Method note: WebSearch returned mostly junk for Voxy / DH / Nvidium (no blog posts with internals exist that I could find). The best primary sources are the source repos themselves, so I cloned and read them: Voxy (GitHub MCRcortex/voxy, HEAD dated 2026-10-19), Distant Horizons core (GitLab jeseibel/distant-horizons-core, HEAD 2026-11-02), Cubyz (GitHub PixelGuys/Cubyz, HEAD 2026-11-02).

## Ranked summary

| # | Candidate | Source / year | Fit | Benefit | Effort | Verdict |
|---|-----------|---------------|-----|---------|--------|---------|
| 1 | Cubyz-style LOD: same 32^3 chunk, voxel size doubles per level, up to 6 levels, "pick a real child block" downsample, separate `neighborLod` border face sets | Cubyz repo, 2026 [S] | Good (ring buffer per LOD, same chunk size, same quad pipeline) | Every LOD chunk costs the same as a near chunk: 32^3 per level => radius R at level L covers 2^L times the distance. Number: none quoted; estimate: 5 extra levels at radius 16 reach ~2^5*16*32 = 16k blocks with ~6x chunk count of one level | M-L | Adopt-candidate (this is basically ADR-0011) |
| 2 | Voxy GPU hierarchical LOD traversal (screen-space-size subdivision, Hi-Z on nodes, request queue back to CPU) | Voxy repo, MCRcortex, 2026 [S] | Good for planned GPU culling (ADR-0010); partial for CPU-side streaming | Default split threshold 64x64 px area, auto-tuned 28..256 by FPS (quoted from code). No frame-time numbers quoted | L | Prototype (after plain clipmap LOD works) |
| 3 | Voxy/DH mip rule: pick one representative block, not average; DH keeps vertical column slices | Voxy `Mipper`, DH `RenderDataPointUtil` 2026 [S] | Good (data stays palette/BlockId) | Quality only | S | Adopt-candidate (rule choice, not a subsystem) |
| 4 | 0fps POP-buffer + geomorphing for blocky LOD (crack-free without skirts) | 0fps 2018-03 [S] | Partial: needs per-vertex shader math; our quads are vertex-pulled, so doable, but LOD sizes in our scheme are power-of-two chunks | No numbers quoted | M-L | Watch (try skirts / border-face sets first) |
| 5 | DH column-heightmap far field (64x64 columns, N vertical slices, 128-bit-ish render point) | DH core 2026 [S] | Partial: great for >2 km, but separate data model | Memory per column tiny; no numbers quoted | L | Watch / possible far-far tier |
| 6 | Transvoxel-style transition cells | Cinevva blog 2026-02 [S] | Poor (marching cubes tables, not blocky) | n/a | L | Reject for blocky |
| 7 | Anisotropic voxel LOD (SGGX-H) for fine geometry | arXiv 2604.13191, 2026 [S] | Poor (microgeometry, CUDA, offline) | quoted voxelization 3.45 s vs 81 s raster on specific fabric scene; irrelevant to terrain | L | Reject |

## 1. Cubyz LOD (Zig, Vulkan, open source)

- Source: Cubyz (PixelGuys), https://github.com/PixelGuys/Cubyz, files `src/chunk.zig`, `src/renderer/mesh_storage.zig`, `src/renderer/chunk_meshing.zig`, `src/settings.zig` (HEAD 2026-11-02). [S]
- What it does:
  - `highestSupportedLod = 5` => levels 0..5, voxel size 1,2,4,...,32; chunk is always 32^3 voxels (`comptime assert chunkSize == 32`), so a level-L chunk spans 32*2^L blocks. [S]
  - Storage per LOD: `storageLists[lod]` indexed by `(pos >> (lod + chunkShift)) & storageMask`, i.e. a per-LOD ring buffer without hashing. Same idea as our ChunkWindow, one ring per level. [S]
  - Downsample (`ServerChunk.updateFromLowerResolution`, chunk.zig ~588): each 2x2x2 group of finer voxels is replaced by ONE of its eight blocks. The pick: score every voxel by a neighbour count (air = 0, a transparent neighbour adds 5, chunk border adds 1, base 128), take the max score in the octant, then among the voxels within 1 of the max pick one using a position-parity-dependent permutation `(x&1)*4+(y&1)*2+(z&1)` XOR i, "that keeps high resolution patterns in lower resolution"; "-1 to avoid pattern breaks at chunk borders". Block types go through `lodReplacement()` (e.g. leaves/plants to a simple block). Result is exposed-surface-biased (prefers voxels with many transparent neighbours, i.e. surface voxels) and never invents a blended block. Air is avoided unless all eight are air. Sky chunks (z>9000) skip LOD above voxel size 4. [S]
  - Seams: no skirts, no stitching. Instead each chunk mesh keeps face lists per direction in two variants: `neighborN` (neighbour is same LOD) and `neighborLodN` (neighbour is the coarser LOD, `isNeighborLod[6]`). `uploadData` picks one set per side at upload time, and faces are sorted by normal direction for GPU backface culling (`byNormalCount[14]`). The finer chunk generates its boundary faces by testing its own blocks against the 2x coarser neighbour's blocks (`getNeighbor(pos, 2*voxelSize, neighbor)`). The coarse side emits nothing extra. So the boundary is closed by the finer side's faces only. [S] I did not verify what the screen looks like (hairline gaps from the 1-unit vs 2-unit face mismatch are possible; unverified).
  - Rendering: indirect draws per LOD (`drawChunksIndirect`, `chunkIds[highestSupportedLod+1]`, one `faceBuffers[lod]` suballocator per LOD). [S]
  - Hand-off between LODs: `finishedMeshingHigherResolution` bitmask per parent node (8 children bits) so a coarse node is dropped only when all 8 finer children are meshed. This avoids holes while streaming. [S]
- Fit: good. Touches ChunkWindow (one ring per level), mesher (needs a per-border-side "neighbour is coarser" variant), quad buffer (needs per-level instance scale; our quad has 15 free bits), ADR-0011. 32^3 is already our chunk size, which is the same choice Cubyz made. Per-direction quad grouping is already in place (Cubyz does the same for backface culling).
- Expected benefit: no numbers in source. Estimate: with R_lod=16 chunks per level and 5 levels, ring-buffer voxel heap grows about 5x the near ring at same radius (palette storage mostly Uniform at coarse levels is cheaper); coverage becomes ~16k blocks. Measure, do not trust.
- Effort: M-L. Pieces: downsample kernel (S), per-level rings (M), border-face variant (M), instance scale in vertex pulling (S), hand-off mask (S).
- Bench: new `far` scenario already exists; add metrics `lod_levels`, `quads_per_level`, `voxel_heap_MiB_per_level`, `lod_build_ms` (downsample), `mesh_ms_per_level`; content hash per level; frame GPU terrain pass p50/p99.
- Verdict: Adopt-candidate. Closest proven design to our architecture.

## 2. Voxy hierarchical GPU LOD traversal

- Source: Voxy (Cortex), https://github.com/MCRcortex/voxy, files `shaders/lod/hierarchical/{traversal_dev.comp,node.glsl,screenspace.glsl,queue.glsl}`, `HierarchicalOcclusionTraverser.java`, `WorldEngine.java`, `WorldUpdater.java`. Docs pages on deepwiki/grokipedia were too shallow to use. [S]
- What it does:
  - Data: section = 32^3 of u64 voxel (block id 27 bits at shift 27? from `Mapper.withBlockBiome`: block <<27, biome <<47, light in the top 8 bits), 5 mip levels above 0 (`MAX_LOD_LAYER = 4`; `WorldUpdater` propagates every inserted section up the mips immediately on the inserting thread). Storage is a pluggable section DB (RocksDB, LMDB, Redis, in memory; LUT-compressed sections in `SaveLoadSystem3`). [S]
  - Mesher output is a 64-bit quad: pos 3x5 bits, size 2x4 bits (+1), face 3 bits, block state 16 bits, biome 9, light 8 (`quad_format.glsl`). Rendered by vertex pulling with `lodScale = 1<<lodLevel`, MDI with count (`glMultiDrawElementsIndirectCountARB`). [S]
  - Traversal: nodes are 16 bytes in a GPU buffer (24-bit mesh ptr, 24-bit child ptr, flags, packed pos+level). One compute dispatch per LOD level (`MAX_ITERATIONS = MAX_LOD_LAYER+1`), a queue of nodes; each node does frustum test, Hi-Z test against a depth pyramid (8 projected corners, mip chosen by bbox size), then `shouldDecend() = screenArea > minSSS`. If it should split and has children: enqueue children, else render self; if a node has no mesh yet it adds a request to a GPU->CPU request queue and falls back to its children. Soft and hard request queue limits via atomics. [S]
  - Default split threshold `subDivisionSize = 64` (area threshold 64^2 px), with a runtime auto-tuner that moves it between 28 and 256 depending on FPS (VoxyRenderSystem.java:433-437). [S] (quoted from code)
  - Mip rule (`Mipper.mip`): picks one of the eight child voxels, the one with max (opacity<<4 | corner index) so more opaque wins, ties go to the top corner (111); if all air, averages light. Source comments admit TODO: want per-level behaviour (details at low levels, "more air" at high levels). [S]
  - Seams: grep for "skirt" finds nothing; no stitching code seen. Section borders are meshed "hacky side faces without checking the neighbour section" (RenderDataFactory comment) [S]. How visible the resulting cracks are: not verified.
- Fit: the traversal maps directly onto ADR-0010 (GPU culling) plus ADR-0011 (LOD), and it is vertex-pulling + MDI like ours. wgpu limitation: no `multi_draw_indirect_count` guarantee (we have it optional), and compute-per-level dispatch is fine. The CPU request-queue read-back is the awkward part in wgpu (async map, one-frame latency). Our static LOD ring (Cubyz style) does not need it.
- Expected benefit: no frame numbers quoted. Estimate: node-based culling avoids submitting hidden far sections; value depends on terrain occlusion.
- Effort: L.
- Bench: new scenario `far` with camera in a valley vs on a ridge (occlusion differs); metrics: nodes traversed per level, nodes rendered per level (Voxy has `HAS_STATISTICS` counters for exactly this), GPU cull pass ms, request queue length, frame GPU ms.
- Verdict: Prototype, but only after #1 works with a simple distance-based CPU selection. Take the node layout and screen-space-size rule, skip the DB.

## 3. Downsample rules that keep silhouettes (comparison)

- Cubyz: surface-biased pick with parity permutation (see #1). Voxy: opacity-max pick, top-corner tie-break. Both pick an existing block and never average. DH: keeps per-column vertical slices with height and min-Y (12 bits each) plus colour, so overhangs survive in coarse columns; render point is 64 bits colour/material/H/D/light. [S] (`FullDataPointUtil`: id 32b, height 12b, minY 12b, sky 4b, block 4b; `RenderDataPointUtil` bit layout in file header; `ColumnRenderSource.WIDTH = 64`; `maxVerticalSliceCount` configurable.)
- Recommended rule for Espresso (my synthesis, not verified): priority = non-air over air, opaque over transparent, then highest exposed-face count (Cubyz) with a deterministic parity tie-break to preserve checker-like patterns, and a block-class remap table (leaves, tall grass, flowers => solid proxy) as in Cubyz `lodReplacement()`. Run it as a 1-pass u64-column bit trick on the occupancy masks first (bit-or of 2x2x2 occupancy) and only look up block ids for set bits; our mesher already works on u64 columns, so the occupancy half is nearly free (estimate).
- Effort: S for the rule, M if bit-parallel.
- Bench: `lod_build_ms` per chunk (divan microbench next to mesher), plus a silhouette error metric: render LOD level L and level 0 from the bench orbit and compare coverage mask (new tool, count of pixels that differ). Not available today.
- Verdict: Adopt-candidate.

## 4. 0fps geomorphed, crack-free blocky LOD

- Source: Mikola Lysenko, "A level of detail method for blocky voxels", 2018-03-03, https://0fps.net/2018/03/03/a-level-of-detail-method-for-blocky-voxels/ . The 0fps meshing, AO and engine-analysis posts are covered in `docs/research/engine-survey-2026-11.md` (PR #2); this LOD post is a different article. [S]
- What it does: snaps quad vertices to a multiple of 2^i; a quad whose vertices collapse to the same point is dropped at that level; quads are counting-sorted coarse-to-fine into one buffer so each LOD is a prefix (draw a shorter range); a quad's own LOD = trailing zeros of coordinates (min over axes via leading zeros of lo^hi for big quads). Cracks are removed by geomorphing between levels with fractional LOD t, using a fixed point over chunk + neighbour max LOD (2-3 iterations). Author concludes no skirts needed. No benchmark numbers quoted; reference code is gone. [S]
- Fit: partial. Our chunk stores quads per face direction; the sort-by-LOD prefix trick could live inside each direction group. But it is vertex-shader work per quad corner, and it keeps full 32^3 detail data in memory (the voxel heap does not shrink with distance), which fights the 58.6 MiB/22k chunk budget. Good as a cheap "near-LOD" mechanism (levels 0-2) only.
- Effort: M-L. Bench: vertex stage GPU time (timestamps per pass), quads drawn per level, visual crack count (screenshot diff).
- Verdict: Watch.

## 5. DH column far field

- Source: Distant Horizons core, https://gitlab.com/jeseibel/distant-horizons-core (via the main repo's `coreSubProjects` submodule), `DhSectionPos.java`, `ColumnRenderSource.java`, `ColumnRenderBufferBuilder.java`, `FullDataPointUtil.java`. [S]
- What it does: LODs are 2D sections (detail levels, 1 block at 0, 16 at 4, 512 at 9; minimum section detail 6 so a section is 64x64 columns at WIDTH = 64), each column holds up to N vertical slices (min Y, height, block id, light). Buffers built per section with adjacent columns checked, including a coarser neighbour (`adjDetailLevel >= thisDetailLevel` asserted; the comment says other cases never fired), so faces between a fine and a coarse column are produced by the finer side. Quad builder merges columns into boxes. [S]
- Fit: partial. Cheaper and far smaller than 3D LOD for very distant terrain (2D columns, no caves) but breaks "everything is the same palette chunk" and needs its own mesher. Reasonable as a tier beyond Cubyz-style LOD 5 if view range needs to exceed ~10 km. Not a priority at current radius 32.
- Effort: L. Bench: columns/MiB, mesh ms per section, quads.
- Verdict: Watch.

## 6. Transvoxel stitching (rejected)

- Source: Cinevva "Open world browser part 09: transvoxel first cut", 2026-02-25, https://app.cinevva.com/blog/2026-02-25-open-world-browser-part-09-transvoxel-first-cut . Quoted: 512 raw transition cases, 73 equivalence classes, 3x3 vs 2x2 sample faces. Marching-cubes only. [S]
- Verdict: Reject for blocky terrain; the blocky equivalent is Cubyz's border-face variant (#1).

## 7. SGGX-H anisotropic LOD (rejected)

- Source: arXiv 2604.13191 "Fast Voxelization and Level of Detail for Microgeometry Rendering" (2026). Quoted: GTX 1080 Ti, fabric flat 3.450 s vs 81.117 s raster; 6 bytes per SGGX distribution; at most 3 distributions per block. For volumetric ray marching of fibre/cloth; not for terrain. [S]
- Verdict: Reject.

## Practical design notes for ADR-0011 (my synthesis from the above, [U] where marked)

1. Level-of-detail selection: start with distance rings (Cubyz) and a 2:1 level ratio between neighbouring levels; Voxy's screen-space-area rule is a drop-in replacement later (node area vs 64^2 px) and also gives a free auto-quality knob.
2. Seam policy: border faces on the finer chunk, tested against the coarser neighbour's voxels (Cubyz). Keep two border face sets per side only for the 6 sides; memory cost is the border-face quad count, small compared with the chunk body (estimate). Skirts are not used by either Voxy or Cubyz (grep verified) [S]; skirts remain the fallback if hairline gaps show in testing.
3. LOD hand-off: parent is hidden only when all 8 children are meshed (Cubyz bitmask); children hidden when parent exists and camera moved out. Avoids holes, allows hysteresis. Dithered cross-fade or geomorph transitions: not found in either implementation, [U] only.
4. Quad format: add `lod` (3 bits) into our spare 15 bits, or per-instance scale in the draw's base-instance buffer like Voxy's `positionBuffer[gl_BaseInstance]` (pos + level packed). Second option costs nothing per quad. [S]
5. Edits: Voxy inserts a changed section and mips it up through all levels synchronously on the caller thread (WorldUpdater). We should do the same but on the rayon pool with dirty bits per parent.

## Searched but nothing usable

- Voxy: no external write-up with internals or numbers found (deepwiki child pages for storage/traversal/culling were not reachable; grokipedia page has only version history and requirements: OpenGL 4.6, 0.2.13-alpha 2026-03-26). Everything above is from reading the repo. Performance figures: none found.
- Distant Horizons: no published perf numbers; the mod's own docs site not fetched. Source only.
- Bobby: client-side chunk cache for Minecraft (keeps old chunks), no LOD technique worth taking; not fetched [U].
- Nvidium: mesh-shader/NV-only occlusion culling for Sodium; search returned only marketing pages and an NVIDIA mesh-shader intro. wgpu 30 has no mesh shader support I can confirm [U]; skipped.
- Papers 2024-2026 on voxel LOD for terrain: the arXiv search returned Aokana (excluded), line-set ray tracing, microgeometry voxelization (candidate 7), and robotics mapping. Nothing on blocky terrain LOD in SIGGRAPH/HPG/I3D that I could locate.
- Dithered / geomorph LOD transitions for blocky voxels in 2024-2026: nothing found beyond 0fps 2018.
- Clipmap streaming for voxels (as a published technique): only Wikipedia's generic Clipmap page appeared; no voxel-specific write-up. Cubyz's per-LOD ring storage is the closest implementation.
- Reddit r/VoxelGameDev threads appeared in search results ("LOD chunk merging system") but reddit is blocked for fetching.
- itch.io devlogs (VoxelMancy LODs) failed to fetch; BAA repo page had only qualitative claims.
