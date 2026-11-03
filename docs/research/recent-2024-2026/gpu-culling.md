# GPU-driven rendering and culling for voxel terrain

> Snapshot, 2026-11-03. Tags [S] and [U] and the number labels as in `docs/DOC-GUIDELINES.md` §4. Ranked candidates from this report are in `docs/BACKLOG.md`.

Tags: [S] = read on a fetched page (URL given), [U] = background knowledge or not verified. "Quoted" = number from the source. "Estimate" = mine.
Caveat: the DeepWiki pages for Voxy/Nvidium are auto-generated summaries of source code, not author writing. Treat as [S-secondary]. Meshelium TECHNICAL.md / PERFORMANCE.md and zeux.io failed to fetch, so Meshelium internals and Zeux's two-phase write-up are NOT verified here.

## Ranked summary

| # | Candidate | Source / year | Fit | Benefit | Effort | Verdict |
|---|-----------|---------------|-----|---------|--------|---------|
| 1 | Per-chunk face-direction mask + frustum culling in compute, atomic-compacted draw list (Voxy `cmdgen` pattern) | Voxy (MCRcortex), DeepWiki 2025-26 | Good: matches ADR-0010 and per-direction quad grouping | Estimate: cuts quads submitted ~up to 3x for back-facing dirs; no quoted number | M | Adopt-candidate |
| 2 | Two-phase Hi-Z occlusion (previous-frame visible set -> HZB -> test rest -> late draw) | Bevy 0.16 (2025), Voxy HiZ, vkguide | Good for dense terrain/caves, partial for open fields | Quoted: none for Bevy; Voxy none. Estimate only | M-L | Prototype (after #1, measure first) |
| 3 | Voxel visibility graph: advanced cave culling + angle clamping, async CPU BFS | Hitchman 2025-11; Sodium PR #2811; Sodium 0.9 async graph culling | Good: ring-buffer ChunkWindow is a natural BFS grid; 15 free quad bits irrelevant, needs per-chunk 15-bit connectivity | Quoted: +5-25% sections culled, ~0.5 ms / 8k sections on CPU | M | Prototype (complement or alternative to #2) |
| 4 | Mesh-shader terrain (Nvidium/Meshelium) | Meshelium 2026, Nvidium | Partial: wgpu mesh shaders experimental, Naga Vulkan-only | Quoted: 1.5x at RD12 to 5.3x at RD64 (607 vs 114 FPS) vs vanilla MC | L | Watch |
| 5 | GPU hierarchical LOD node traversal (BFS ping-pong, screen-space size test, Hi-Z) | Voxy | Partial: only when ADR-0011 clipmap lands | none quoted | L | Watch |
| 6 | wgpu 30 plumbing: MDIC + INDIRECT_FIRST_INSTANCE + SPD-style HZB | wgpu docs, Bevy source | Good: required substrate for #1/#2 | n/a | S-M | Adopt-candidate (as groundwork) |
| 7 | Hierarchical Raster Occlusion Culling | Lee et al., EG/CGF 2021 | Poor | Quoted: 2-3x vs ROC on some scenes | L | Reject |
| 8 | Occlusion-effectiveness measurement plan | Bevy docs, Hitchman, own design | Good | n/a | S | Adopt-candidate |

Recommended order: 6 -> 1 -> 8 (instrument) -> 3 or 2 (whichever 8 says wins on the `far` scenario) -> 4/5 later.

---

## 1. Per-chunk face-direction + frustum culling in compute (Voxy `cmdgen` pattern)

- Source: Voxy by MCRcortex, DeepWiki "Compute Shader Summary" https://deepwiki.com/MCRcortex/voxy/5.2-compute-shaders (2025-26, secondary, [S]).
- What: `prep.comp` (1 thread) resets counters and sets the dispatch size; `cmdgen.comp` (workgroup 128, one thread per section) checks per-section visibility against `frameId`, builds a face bitmask "from section face occupancy and camera-relative position", writes draw commands via `atomicAdd` on a draw counter. A subgroup prefix-sum shader exists for compaction. [S]
- Fit: good. Espresso already groups quads per face direction, so one indirect command per (chunk, dir) or one command with only visible direction ranges maps directly. Touches: draw submission, offset-allocator metadata (needs per-chunk, per-dir offset/count), the `multi_draw_indirect` call (switch to `_count` when feature present).
- Direction test [U, standard]: for a chunk AABB, +X faces are visible only if camera.x > chunk.min.x (and -X if camera.x < chunk.max.x). Six compares per chunk, no matrix math. Skip dirs with zero quads.
- Benefit: no quoted numbers. Estimate: on a typical view ~half the direction ranges of every chunk are back-facing, so vertex-pulling invocations drop up to ~2x for far chunks; real saving depends on whether vertex stage is the bottleneck (needs measuring).
- Effort: M.
- Measure: existing `frame GPU ms` terrain pass p50/p99 (needs a real GPU, not software adapter), plus new counters: `chunks_in`, `draws_out`, `quads_submitted` (sum of counts of emitted commands), compared to `quads_total`. Content hash must stay identical (pixel-identical screenshot hash at fixed orbit frames as a correctness check).
- Verdict: Adopt-candidate. Cheapest first step of ADR-0010, correctness is trivially checkable (output image identical).

## 2. Two-phase Hi-Z occlusion culling

- Sources:
  - Bevy 0.16 release notes https://bevy.org/news/bevy-0-16/ (2025, [S]) and `OcclusionCulling` docs https://doc.qu1x.dev/bevy_trackball/bevy/render/occlusion_culling/struct.OcclusionCulling.html [S].
  - vkguide GPU compute culling https://vkguide.dev/docs/gpudriven/compute_culling/ [S] (single-phase, previous-frame depth).
  - Voxy HiZ https://deepwiki.com/MCRcortex/voxy/4.6-occlusion-culling [S, secondary].
  - Zeux/niagara write-up: could not fetch; description below of the generic algorithm is [U].
- What (Bevy, [S]): early depth prepass renders objects visible last frame; build HZB from that depth; test all bounding boxes against it; late prepass renders the rest; main passes follow. Only opaque/alpha-tested objects occlude. Bevy flags it experimental "because of precision issues that can wrongly mark meshes as occluded" and says it "can be slower on small scenes". No benchmark numbers given.
- wgpu specifics (Bevy source, [S] https://doc.qu1x.dev/bevy_trackball/src/bevy_core_pipeline/mip_generation/experimental/depth.rs.html): HZB uses single-pass-downsampler logic with a min filter but runs as TWO compute passes because WGSL/wgpu has no `coherent` buffers; R32Float texture with STORAGE_BINDING|TEXTURE_BINDING, non-filtering sampler; max 12 mips; base is previous power of two of the view; second pass only when mip count >= 7. So there is no sampler min-reduction path in wgpu; a manual compute downsample is the known approach. (I did not find a wgpu min/max sampler feature in the search; [U] that none exists.)
- Voxy (secondary [S]): HiZ chain built by fullscreen quad per mip (OpenGL), compute traversal tests 8 projected AABB corners against 6 frustum planes then Hi-Z.
- Reverse-Z note [U]: with reverse-Z the farthest depth per 2x2 is the MIN, and the chunk's nearest depth is its max; vkguide leaves the convention ambiguous, Bevy page does not say.
- Fit: good for caves/mountains/dense overhead, partial on open flat terrain where little is occluded. Granularity: per 32^3 chunk (22k chunks at radius 32 is trivial for one compute dispatch). Needs depth of a terrain prepass or last frame's depth; Espresso has no depth prepass, so phase 1 = draw last-frame visible set, build HZB, phase 2 = test rest + draw newly visible. Touches: render graph (adds 2 draw passes + HZB build + 2 cull dispatches), visible-set buffer persisted across frames.
- Benefit: Bevy/Voxy give none. Estimate: unknown; depends on scene. Caution: the extra pass splitting costs fixed GPU time; Bevy itself warns of overhead on simple scenes.
- Effort: M-L.
- Measure: new GPU timestamps `hzb_build`, `cull_early`, `cull_late`; counters `chunks_frustum_pass`, `chunks_hiz_pass`, `quads_drawn`; scenario with underground camera and mountain-occluded view (current `far` orbit is likely above ground and shows little gain). Accept only if net GPU frame ms p50 improves on a real GPU in at least one scenario and does not regress in the others.
- Verdict: Prototype after #1, only once #8 shows how many chunks are occluded in the scenarios.

## 3. Voxel visibility graph (cave culling, angle-clamped BFS)

- Sources: Ryan Hitchman, "Voxel Grid Visibility", 2025-11-03, https://cod.ifies.com/voxel-visibility [S]. Sodium PR #2811 (angle-based path occlusion) https://github.com/CaffeineMC/sodium/pull/2811 [S]. Sodium mc26.1.2-0.9.0-beta.1 release notes (PR #2887 asynchronous graph culling) https://github.com/CaffeineMC/sodium/releases/tag/mc26.1.2-0.9.0-beta.1 [S].
- What: classic Tommo (2014) cave culling: per 16^3 section store which pairs of its 6 faces are connected through air; BFS from camera section, only moving away, only through allowed face pairs. Sodium #2811 forbids paths whose incoming angle is too shallow: quoted 4,875 -> 4,370 sections (~11%) and BFS time -13% in the author's test scene. Hitchman's frustum-clamping variant tracks an angle interval per plane, merges with min/max (conservative, never wrongly hides), uses a ~4 KiB lookup table for x,y in [0,31]. Quoted: ~60 ns per BFS step, ~0.5 ms for 8,000 sections, culling an extra 5-25%. Results hold for any viewer position inside a section so they can be cached per section. Sodium 0.9 moves graph culling off the main thread (no numbers given).
- Also [S]: Hitchman reports vanilla Minecraft 1.21.10 has an occlusion bug causing visible holes; suggests a torture test with random solid faces.
- Fit: good. ChunkWindow ring buffer gives O(1) neighbour lookup (no hashing). Needs per-chunk 15-bit connectivity (6 faces -> 15 pairs), computed by a flood fill over the 32^3 voxels at mesh time (one flood fill of air per chunk; cost to be measured against the ~35 us mesh budget). Runs on a rayon task, result consumed by cull compute as a visible bitset. Works regardless of GPU (also on the software adapter, so it is measurable in CI).
- Benefit: quoted above. At radius 32 with 32^3 chunks the BFS visits <=22k chunks; scale estimate from Hitchman: ~22k * 60 ns ~ 1.3 ms CPU on a worker thread (estimate, extrapolated linearly, not measured).
- Limits: only helps terrain with enclosed air (caves, overhangs). Surface views get little. Does not handle edits instantly: chunk edits invalidate connectivity bits (recompute on remesh).
- Effort: M.
- Measure: new `vis_graph_ms` (worker), `chunks_visible_after_graph` vs after frustum; flood-fill cost in the mesher divan bench; correctness via a torture-test world (random solid faces) comparing rendered image against no-graph render.
- Verdict: Prototype. Cheap, hardware-independent, and complementary to Hi-Z. Run #8 first to decide whether cave scenarios matter for the game.

## 4. Mesh-shader terrain (Nvidium, Meshelium)

- Sources: Meshelium README https://github.com/ded811/Meshelium (Minecraft 26.2, VK_EXT_mesh_shader, [S]); Nvidium architecture via DeepWiki https://deepwiki.com/drouarb/nvidium/3-core-architecture (a fork, secondary [S]); wgpu Features docs https://docs.rs/wgpu/latest/wgpu/struct.Features.html [S].
- What: terrain drawn by mesh shaders; Nvidium culls regions then sections on GPU with NVIDIA's representative-fragment-test extension to write visibility without shading, 8x4x8 sections per region, 48 B per section, 16 B per region (quoted, DeepWiki). Meshelium: occlusion culling option (auto-on beyond a distance), optional greedy meshing, small-detail culling. Quoted benchmark (1080p, RX 9070 XT, one world, authors' own): ~1.5x faster at render distance 12, ~5.3x at 64 (607 vs 114 FPS) vs vanilla. Baseline is vanilla Minecraft, not an optimized MDI renderer, so the gain over Espresso's current MDI path will be smaller (estimate).
- wgpu 30 status [S]: `EXPERIMENTAL_MESH_SHADER` on Vulkan, DX12, Metal; Naga supports it only on Vulkan, elsewhere passthrough shaders; native only; some Mesa drivers (llvmpipe, not RADV) fail to run it. Recommends `create_shader_module_trusted` with `ShaderRuntimeChecks::unchecked()` to avoid workgroup-memory zeroing cost.
- Fit: partial. Espresso's u64 quads + vertex pulling already avoids vertex fetch; a mesh shader mostly adds per-meshlet (e.g., 64 quads) culling and removes the index/vertex pipeline stage. Benefit for a MDI renderer is unproven. Experimental feature, no software-adapter testing.
- Effort: L. Measure: terrain pass GPU ms with same content hash; needs real GPU.
- Verdict: Watch. Revisit when Naga supports DX12/Metal paths and #1/#2 are done.

## 5. GPU hierarchical LOD traversal (Voxy)

- Source: https://deepwiki.com/MCRcortex/voxy/4.6-occlusion-culling [S, secondary].
- What: LOD nodes (16 B each) traversed breadth-first on GPU by `traversal_dev.comp` with ping-pong queues, MAX_ITERATIONS = 6 levels, per node: frustum (8 projected corners vs 6 planes), Hi-Z test, and a screen-space-size test (`shouldDecend`) deciding subdivide vs draw. A request queue collects nodes needing meshes; a per-node last-rendered-frame buffer drives eviction. Says "thousands of nodes" per frame (quoted, vague).
- Fit: partial now, good later: Espresso's planned clipmap of ring buffers (ADR-0011) is a fixed-level structure, so a flat per-level dispatch is simpler than a tree traversal. Take only the screen-space-size test and the request queue idea.
- Effort: L. Verdict: Watch until ADR-0011 is implemented.

## 6. wgpu 30 plumbing (groundwork)

All [S] from https://docs.rs/wgpu/latest/wgpu/struct.Features.html (wgpu 30.0.1) unless noted:
- `MULTI_DRAW_INDIRECT_COUNT`: `multi_draw_indirect_count` / `multi_draw_indexed_indirect_count`; DX12 and Vulkan 1.2+ (or VK_KHR_draw_indirect_count); native only. Without it, plain `multi_draw_indirect` is emulated by looping single indirect draws on backends lacking native multi-draw; with it enabled they are not emulated. Keep the existing optional-feature path; fallback = fixed-count MDI over the compacted list with zeroed commands (zero-count draws cost GPU front-end time; vkguide notes this tradeoff [S]).
- `INDIRECT_FIRST_INSTANCE`: needed if indirect commands carry non-zero `first_instance` (the usual trick to pass a chunk/draw ID to vertex pulling). Without it, a non-zero value "may be ignored, run as zero, or run correctly". Vulkan (mostly), DX12, WebGPU, Metal Apple3+/Mac1+. Request it explicitly in Espresso's device setup; if the vertex shader gets the chunk index from `first_instance` (or `instance_index`), this is mandatory. I did not find a wgpu-specific DX12 caveat in the sources fetched [U: check wgpu-hal source/issues before relying on instance_index vs a separate draw-id buffer].
- HZB build: no sampler min/max reduction; follow Bevy's R32Float compute downsample (2 passes, no coherent buffers) [S].
- Verdict: Adopt-candidate groundwork. Effort S-M.

## 7. Hierarchical Raster Occlusion Culling (HROC)

- Source: Lee, Jeong, Seok, Lee, Eurographics/CGF 40(2) 2021, https://diglib7.eg.org/bitstream/handle/10.1111/cgf142649/v40i2pp489-495.pdf [S].
- What: renders temporally coherent occluders, rasterizes BVH-node bounds to find visible groups, per-pixel rays to test objects. Quoted (2080 Ti, 1080p): Satellites 807M tris/120K objects 9.2 ms vs 14.6 ms ROC vs 88.1 ms CHC++; Factory 1.6 ms vs 3.9 ms ROC.
- Fit: poor. Scenes are large irregular objects; Espresso has a regular grid where compute Hi-Z over 22k chunks is already trivial. Uses fragment-shader tricks and counter readback.
- Verdict: Reject. Older than 2024 and no advantage over #2 for a regular grid.

## 8. How to measure occlusion effectiveness (new benchmark metrics)

Own design built from the above; the practices are from Bevy docs ("enable only after measuring"), vkguide caveats, Hitchman's torture-test idea.
1. Counter ladder per frame (GPU atomics into a small buffer, read back through a staging buffer 2-3 frames late so the main thread never stalls): `chunks_resident` -> `chunks_frustum` -> `chunks_graph` (if #3) -> `chunks_hiz` -> `draws_emitted`, plus `quads_submitted` = sum of emitted counts. Report ratios to `quads_total`.
2. Ground truth for false positives (holes): in debug/bench-verify mode, render the same frame with culling off and compare image hash/diff. Any difference with culling on = bug. Run on orbit frames plus an underground scenario and a random-solid-faces torture world.
3. Net benefit, not gross: always report `cull passes GPU ms` (HZB + early + late) next to `terrain pass GPU ms` saved. Cull only counts as a win if the sum of GPU frame ms p50 and p99 drops under `bench-compare`.
4. Scenarios: `far` (high above terrain, little occlusion), a new `cave` (camera inside a hill/cave), a new `mountain` (ridge in front of a wide valley), and a fast-turn scenario for temporal artefacts (one-frame-late Hi-Z pop-in, vkguide mentions the latency caveat [S]).
5. Report on the real GPU only; per CLAUDE.md software-adapter frame numbers stay out of Stats. Graph culling (#3) and counter ladder up to `chunks_graph` are CPU-side and CAN be validated on the software adapter.

---

## Searched but nothing usable

- Two-phase Hi-Z write-ups 2024-2026: Zeux's niagara blog (zeux.io) did not fetch; `old.fbpe.org` mirror is a content-free index page. Only Bevy docs, vkguide, and Voxy summaries were verifiable. No new voxel-specific two-phase paper found.
- Nvidium/Meshelium internals: Meshelium TECHNICAL.md and PERFORMANCE.md failed to fetch (client error), so its occlusion algorithm and its on/off numbers are unknown. Nvidium DeepWiki says occlusion uses `GL_REPRESENTATIVE_FRAGMENT_TEST_NV` but does not describe the logic.
- Voxy: DeepWiki only; no author write-up found. Frame ordering and Hi-Z depth source not stated.
- Visibility-buffer rendering applied to voxels: no 2024-2026 source found (searches returned hobby engines only). Aokana is already covered elsewhere.
- Sodium PR #2887 (async graph culling) body not read; release notes give no numbers.
- Bevy meshlet pages found (`cull_bvh.wgsl`, `cull_clusters.wgsl` on docs.rs for 0.19) but not fetched; Bevy GPU-driven number quoted (10.16 ms vs 33.55 ms, Caldera, 0.16 vs 0.15) is for mesh-instance CPU overhead and is irrelevant to a single-MDI voxel renderer.
- wgpu DX12-specific limitations of indirect first-instance / multi-draw-count: not found in fetched sources (Dawn doc only describes API level).
- Hierarchical-Z for reverse-Z conventions: no authoritative source fetched; stated as [U].
