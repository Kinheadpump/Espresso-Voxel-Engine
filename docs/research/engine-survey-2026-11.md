# Survey of "best voxel engine" resources (2026-11-03)

Noah's four links, plus the primary sources they point to. Labels: **[quoted]** = stated by the source (URL given), **[measured]** = measured by us (see `docs/PERFORMANCE.md`), **[computed]** = arithmetic from other numbers, **[unverified]** = not checked this session. Proposals derived from this survey are in `docs/PROPOSALS.md`.

## Verdict on the four links

| Link | What it actually contains | Usable for Espresso |
|---|---|---|
| [Rosebud: best voxel game engines 2026](https://lab.rosebud.ai/blog/best-voxel-game-engines-2026) | Vendor list (Rosebud ranks itself first): Noa, Rosebud, Hytopia, Luanti, IOLITE, Godot + Voxel Tools, Unity, GDevelop. Almost no internals. | Only as pointers. Voxel Tools and IOLITE were followed up below. |
| [r/VoxelGameDev: best voxel game engine](https://www.reddit.com/r/VoxelGameDev/comments/1p2k5f6/best_voxel_game_engine/) | Not read: reddit.com is blocked for our fetch tool. | Nothing yet. Paste the thread if it matters. |
| [voxely.net: The Perfect Voxel Engine](https://voxely.net/blog/the-perfect-voxel-engine/) (John Lin, 2021) | Architecture essay: format-agnostic storage via allocation, attribute tagging and conversion operators. Rendering post was promised and never published. No numbers except SVOs holding "a couple billion voxels". | One narrow idea (separate attribute channels). The general framework is rejected, see below. |
| [HN 28656317](https://news.ycombinator.com/item?id=28656317) | Discussion of the voxely post. Top comment: "this post is mostly fluff". Links to 0fps, blackflux, procworld, Voxlap, Teardown. | The linked references are the valuable part. |

## Primary sources followed up

### Godot Voxel Tools (Zylann) [quoted]

Sources: [performance](https://voxel-tools.readthedocs.io/en/latest/performance/), [smooth terrain / LOD](https://voxel-tools.readthedocs.io/en/latest/smooth_terrain/), [block format v4](https://voxel-tools.readthedocs.io/en/latest/specs/block_format_v4/), [compressed container](https://voxel-tools.readthedocs.io/en/latest/specs/compressed_container/).

- Blocks default to 16³, can be 32³. LOD grids double block size per level ("the root grid can be read as an octree"). Two streaming modes: `Octree` (one viewer) and `Clipbox` (concentric boxes, multiple viewers).
- LOD seams: Transvoxel transition meshes for smooth terrain; LOD fade by hashed `discard` over `lod_fade_duration`.
- Main-thread work (mesh upload, colliders) is capped by `voxel/threads/main/time_budget_ms`, default about 8 ms. Godot 4 Vulkan mesh deallocation on the main thread causes spikes; their workaround is 32³ mesh blocks.
- Generator graphs use range analysis (interval arithmetic) per 16³ sub-block to skip blocks with no surface.
- Edits: only on full-resolution loaded blocks. A single `set_voxel` is about 12 steps (lock, lookup, remesh...), so bulk ops (`do_sphere`, `do_box`, `paste`) are the recommended path. Locking moved from per-buffer RWLocks to one `SpatialLock3D` (2023).
- Storage per channel: a compression nibble (none / uniform) and a depth nibble. Disk container: LZ4 by default, Zstandard optional.

### IOLITE [quoted, thin]

Homepage: "The real-time voxel game engine powered by path tracing." Its blog has no posts on voxel rendering, GI or data structures. Internals [unverified].

### 0fps, "An analysis of Minecraft-like engines" (2012) [quoted]

[Link](https://0fps.net/2012/01/14/an-analysis-of-minecraft-like-engines/). Thesis: "The main bottleneck in voxel engines is iteration — not random access reads and writes!" µs per read on a 256³ world:

| Structure | Random | Sequential (r=0) | Sequential (r=1) |
|---|---|---|---|
| Flat array | 0.224 | 0.178 | 0.060 |
| Virtual array (hash of chunks) | 0.278 | 0.210 | 0.107 |
| Octree | 2.05 | 0.981 | 0.933 |
| Interval tree (RLE) + hash | 0.571 | 0.003 | 0.006 |

Octrees are ~9× slower on random reads than flat arrays. RLE wins iteration because it skips runs.

### 0fps, "Ambient occlusion for Minecraft-like worlds" (2013) [quoted]

[Link](https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/). Per vertex: `side1 && side2 ? 0 : 3 - (side1 + side2 + corner)`. Merge greedy faces only when their four AO values match. Fix anisotropy by flipping the quad diagonal when `a00 + a11 > a01 + a10`.

### procworld, "Voxel occlusion" (2015) [quoted]

[Link](http://procworld.blogspot.com/2015/08/voxel-occlusion.html). Per chunk, find a few large conservative occluder quads fully inside solid voxels (scan each axis, grow maximal rectangles while depth varies less than a threshold). Rasterise them into a 64×64 software depth buffer and cull chunks whose screen rect is covered. The author calls it "probably the best optimization we have ever done". No timings given.

### blackflux, "Meshing in voxel engines, part 1" (2014) [quoted]

[Link](https://blackflux.wordpress.com/2014/02/23/meshing-in-voxel-engines-part-1/). Greedy meshing produces T-junctions. Monotone meshing that avoids them in 2D raised an example from 18 to 26 triangles and still left T-junctions against neighbouring faces in 3D, so the author abandoned it.

### HN comment threads worth keeping

- Teardown and John Lin's engine are named as the visual benchmark (both ray-traced, small worlds).
- Voxlap / Ace of Spades: software ray casting, destructible scenes.
- Battlefield-style destruction is mostly pre-baked, not voxel simulation.
- Demand for sub-block detail (Minecraft "chisels & bits" mod).

## How this compares with Espresso

| Topic | Espresso today [measured where numbered] | What the sources do |
|---|---|---|
| Storage | 32³ palette chunks with `Uniform` shortcut, 58.6 MiB for 22,445 chunks | Voxel Tools: 16³/32³ per-channel uniform flag. 0fps: flat arrays inside a chunk hash; octrees lose. We already match the best practice here. |
| Generation skip | Heightmap min/max returns `Uniform` air/stone without filling | Voxel Tools: interval range analysis per 16³ sub-block. Same idea; ours does not cover 3D noise yet. |
| Mesh input | Decode to dense (17–24 µs) + gather padded (29–35 µs), then mesh (33–39 µs) | 0fps: iteration dominates. Our data movement before meshing costs more than the mesher itself. |
| AO | None. 15 free quad bits reserved | 0fps vertex AO with equal-AO merging and quad flip. Needs edge and corner neighbours, which our 6-neighbour gather lacks. |
| Occlusion | None (Hi-Z planned, ADR-0010) | procworld: CPU occluder quads + 64×64 depth buffer, no GPU readback latency. |
| LOD | None (ADR-0011) | Voxel Tools: 2× block size per LOD level, clipbox streaming. |
| Upload | One static upload | Voxel Tools: 8 ms main-thread budget; per-mesh dealloc spikes. |
| Edits | `ChunkVoxels::set` only | Voxel Tools: bulk shape edits, one lock, remesh once. |
| Lighting | Direction shade + fog | IOLITE / Teardown: path or ray tracing on voxel data. |
