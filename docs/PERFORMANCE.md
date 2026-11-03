# Performance log

Every measured result for Espresso, with date, machine, command and commit. How to measure is in `docs/BENCHMARKING.md`; how to label numbers is in `docs/DOC-GUIDELINES.md` §4. Other docs link here instead of copying these tables.

## Budgets (target)

Frame ≤ 4 ms CPU and ≤ 8 ms GPU in the `default` scenario on a mid-range GPU, and no frame above 16 ms while streaming at sprint speed.

## Hardware baseline: `noah-msi-rtx2060` (2026-11-03)

- **Machine:** MSI laptop, Intel Core i7-9750H (6 cores, 12 threads), 16 GiB, Windows 11 Home, on AC power, "High performance" power plan.
- **GPU:** NVIDIA GeForce RTX 2060, Vulkan, driver 616.92, render tier `GpuDrivenWithCount`, timestamps available. The Intel UHD 630 iGPU was not used.
- **Build:** commit `77dacdfae05a`, release, rustc 1.97.1, no RUSTFLAGS.
- **Command:** `scripts/bench-baseline.ps1` twice, about 4 minutes apart; default settings (5 repeats, 120 warm-up + 600 measured frames, 1920×1080, 12 threads).
- **Raw files:** `benchmarks/baselines/noah-msi-rtx2060/`. Run 2 is the reference that later changes compare against.

### Content (identical in both runs)

| Scenario | Chunks generated | Chunks meshed | Quads | Draws | Voxel heap | Geometry buffers | Mesh hash |
|---|---|---|---|---|---|---|---|
| `small` | 1,805 | 748 | 259,091 | 2,506 | 4.7 MiB | 2.0 MiB | `65d196ce12d01116` |
| `default` | 6,125 | 2,826 | 986,380 | 9,492 | 16.1 MiB | 7.7 MiB | `fab6872fe1853d8e` |
| `far` | 22,445 | 10,858 | 3,792,695 | 36,275 | 58.6 MiB | 29.7 MiB | `4229b10348a4dc42` |

### Timings (measured; median of 5 repeat medians, run 1 / run 2, repeat spread in brackets)

| Metric | `small` | `default` | `far` |
|---|---|---|---|
| World generation | 71.1 / 78.6 ms | 260.5 / 267.4 ms | 1,081.6 / 934.8 ms |
| Meshing | 27.9 / 31.0 ms | 103.2 / 109.8 ms | 447.8 / 393.0 ms |
| Upload | 3.2 / 4.7 ms (26% / 44%) | 15.2 / 13.9 ms (43% / 33%) | 59.3 / 66.4 ms (9% / 50%) |
| Frame CPU encode | 171 / 182 µs | 497 / 480 µs | 1,740 / 1,341 µs (5% / 39%) |
| Frame GPU terrain pass | 170 / 169 µs | 522 / 520 µs | 1,943 / 1,976 µs |
| Frame wall | 461 / 467 µs | 1,302 / 1,251 µs | 4,307 / 3,685 µs |
| Process resident memory | 82 / 82 MiB | 94 / 95 MiB | 132 / 131 MiB |

Frame p99 across all frames of run 2 (measured): `default` GPU terrain pass 729 µs and frame wall 3.09 ms; `far` GPU terrain pass 3.54 ms and frame wall 8.83 ms.

### Stability (`vx bench-compare`, run 1 → run 2)

- `default`: every metric within noise. This is the scenario to judge changes with on this machine.
- `small`: upload flagged SLOWER (+47%, noise 44%); everything else within noise.
- `far`: world generation (−14%, noise 12%) and frame wall (−14%, noise 13%) flagged FASTER; everything else within noise.

The `small` and `far` flags are run-to-run variation on the laptop, not code changes. Treat upload and the `far` CPU timings on this machine as noisy.

### Findings

- The GPU is lightly loaded: the terrain pass takes ~0.52 ms in `default` and ~1.95 ms in `far`.
- Frame CPU encode grows with draw count although the renderer issues one `multi_draw_indirect` call, about 40–70 ns per draw (computed). Tracked as `docs/BACKLOG.md` B-02.
- World generation costs ~2.5× meshing at every scale, so generation dominates region build time.

## Textured vs flat shading: `noah-msi-rtx2060` (2026-11-03)

- **Change:** 16 px block texture array (PR #8, ADR-0014). `--shading flat` is the previous shader, built as a separate pipeline.
- **Machine:** as the hardware baseline above (i7-9750H, RTX 2060, Vulkan, driver 616.92, AC power).
- **Build:** commit `8a8bea163fc5`, release, rustc 1.97.1, default bench settings (5 repeats, 120 warm-up + 600 measured frames, 1920×1080).
- **Command:** `vx bench --scenario <s> --shading flat|textured`, then `vx bench-compare`. `default` ran twice, the second time textured first.
- **Raw files:** `benchmarks/textures/noah-msi-rtx2060/` (PR #10).

Content is identical between flat and textured (same `mesh_hash` per scenario). Texture memory: 10,912 B (measured, `content.texture_bytes`).

### GPU terrain pass (measured; median of 5 repeat medians)

| Scenario | Order | Flat | Textured | Change | Noise band | Budget (≤ +10%) |
|---|---|---|---|---|---|---|
| `default` | flat first | 508.8 µs | 552.7 µs | +8.6% | 6.9% | met |
| `default` | textured first | 520.2 µs | 561.0 µs | +7.8% | 3.6% | met |
| `far` | flat first | 1,769.5 µs | 1,816.8 µs | +2.7% | 2.0% | met |

### Other metrics

- World generation, meshing and upload are within noise in every pair, as expected: texturing does not touch them.
- Frame CPU encode and frame wall in the first `default` pair: +28.8% and +28.0%. The reverse-order pair gave −0.3% and +3.2% (within noise). The first flat run had an unusually low encode time (341.9 µs against 438.1 µs in the second), so this is CPU-side run-to-run variation, not a texture cost.
- `far`: CPU encode +2.8% and frame wall +2.6%, just outside the 2% noise floor; a single run, so not judged (sprint rule 6).

### Findings

- Textured shading costs about 40–45 µs per frame at radius 16 and about 47 µs at radius 32 (computed from the table). The cost barely grows with 3.8× the quads, which points to a per-pixel cost at a fixed resolution rather than the per-vertex face lookup (inferred, not profiled).
- `default` uses most of the 10% budget. The levers, if a later change needs the headroom: anisotropy 4 instead of 8, and dropping the texel snap (it costs ALU, not fetches). Both are quick A/B runs with `--shading textured` on both sides.

## Occlusion scenarios and culling ladder: `noah-msi-rtx2060` (2026-11-03)

- **Build:** PR #9 at `b2ee6a175ccc` (candidate) and `229487f9d90f` (base), release, rustc 1.97.1, built in a clean worktree; every report has `git_dirty=false`.
- **Machine and settings:** as in the hardware baseline above (driver 616.92, AC power, High performance plan), default settings.
- **Raw files:** `benchmarks/baselines/noah-msi-rtx2060/*-b2ee6a175ccc.json`.

### Culling ladder (measured; mean of 24 sampled path frames at 1920×1080)

What each culling stage would leave, as quads and as a share of resident quads. Nothing is culled at render time yet (`docs/BENCHMARKING.md` § Culling ladder).

| Scenario | Resident chunks / quads | Frustum | Frustum + facing | Pixel-visible chunks | Pixel-visible quads |
|---|---|---|---|---|---|
| `default` | 2,826 / 986,380 | 352,755 (35.8%) | 242,105 (24.5%) | 435.2 | 183,421 (18.6%) |
| `far` | 10,858 / 3,792,695 | 1,274,671 (33.6%) | 866,093 (22.8%) | 1,087.4 | 484,225 (12.8%) |
| `cave` | 2,826 / 986,480 | 331,539 (33.6%) | 108,589 (11.0%) | 17.4 | 75 (0.008%) |
| `mountain` | 2,826 / 986,380 | 327,929 (33.2%) | 203,986 (20.7%) | 48.3 | 21,573 (2.2%) |
| `fly-fast` | 2,826 / 986,380 | 369,232 (37.4%) | 252,356 (25.6%) | 349.1 | 152,206 (15.4%) |

The same ladder from lavapipe on the cloud VM (measured, all six scenarios, same code, 1920×1080) agrees to within 0.2 pixel-visible chunks per frame, which confirms the counts do not depend on the GPU.

Findings:

- Frustum culling alone leaves 33–37% of quads in every scenario, and adding the face-direction test leaves 11–26% (computed from the table). B-06 would cut submitted quads by ~4× from the air (estimate).
- Occlusion beyond that is scene dependent. From the air (`default`, `fly-fast`) a perfect chunk-granular occlusion culler keeps 65–76% of the frustum+facing quads. On the ground (`mountain`) it keeps ~11%, underground (`cave`) under 0.1% (computed). B-07 is worth it for ground-level and underground views, not for flight.

### Timings of the new scenarios (measured; median of 5 repeat medians, repeat spread in brackets)

| Metric | `cave` | `mountain` | `fly-fast` |
|---|---|---|---|
| World generation | 175.1 ms | 179.9 ms | 178.8 ms |
| Meshing | 67.0 ms | 68.5 ms | 69.4 ms |
| Frame CPU encode | 315 µs (34%) | 434 µs (6%) | 433 µs (31%) |
| Frame GPU terrain pass | 464 µs (4%) | 486 µs (5%) | 497 µs (5%) |
| Frame wall | 917 µs (30%) | 1,212 µs (6%) | 1,200 µs (25%) |
| GPU terrain pass p99 (pooled) | 634 µs | 813 µs | 1,082 µs |

Every chunk is still drawn, so these GPU times show the cost of drawing occluded geometry: `cave` shows ~0.01% of resident quads on screen (computed from the ladder) and still costs ~0.46 ms of GPU time.

### PR #9 before/after (`vx bench-compare`, base `229487f` → candidate `b2ee6a1`)

| Metric | `default` run 1 | `default` run 2 | `far` |
|---|---|---|---|
| World generation | +3.1%, within noise | +0.3%, within noise | +4.1% (noise 3.6%), SLOWER |
| Meshing | within noise | within noise | within noise |
| Frame CPU encode | −2.0%, within noise | +16.1% (noise 22.8%), within noise | −0.6%, within noise |
| Frame GPU terrain pass | +0.0%, within noise | −0.7%, within noise | −0.0%, within noise |
| Frame wall | −1.7%, within noise | +20.1% (noise 28.6%), within noise | −0.2%, within noise |

The run-2 encode and wall changes come from one outlier base run (frame wall 1,020 µs against 1,221–1,242 µs in the other three `default` runs). The `far` world-generation flag is just past the noise band and not repeated in `default`. Its only plausible source was a refactor of `TerrainGenerator::surface_heights` through a new `surface_height_at`; that refactor was reverted before merge, so the merged generation loop is the measured base code plus one per-chunk `Option` check. The reverted state was not re-measured.

## Cloud VM reference (2026-11-03)

- **Machine:** Intel Xeon @ 2.30 GHz VM, 4 vCPUs, 16 GiB, Ubuntu 24.04, rustc 1.97.1, no RUSTFLAGS.
- **GPU:** llvmpipe (lavapipe, Mesa 25.2.8), a software adapter. Its GPU and frame numbers only show that the path works and say nothing about GPU performance.
- The VM is shared; repeat-to-repeat spread was 12–28% for the world phases.

### `vx bench --scenario default --frames 30 --warmup 5 --resolution 640x360` (measured, two back-to-back runs)

Content is identical in both runs (hash `fab6872fe1853d8e`): 6,125 chunks generated, 2,826 meshed, 986,380 quads, 9,492 draws, 16.1 MiB voxel heap, 7.7 MiB geometry buffers, 129 MiB RSS.

| Metric (median of 5 repeat medians) | Run 1 | Run 2 | `bench-compare` verdict |
|---|---|---|---|
| World generation, 4 threads | 162.4 ms | 171.8 ms | within noise (band 17%) |
| Meshing, 4 threads | 71.8 ms | 70.6 ms | within noise (band 28%) |
| Upload | 3.9 ms | 3.6 ms | within noise (band 241%, not trustworthy) |
| Frame CPU encode | 325 µs | 333 µs | within noise |
| Frame GPU terrain pass (software adapter, not GPU performance) | 154 ms | 155 ms | within noise |

**Superseded (2026-11-03):** the bootstrap commit quoted single cold runs (radius 16: gen 212 ms, mesh 62 ms) with no warm-up and no repeats. The runs above replace them.

### Micro-benchmarks, `cargo bench-cpu` (measured, single thread, divan medians)

| Bench | Median |
|---|---|
| Binary greedy mesh, surface chunk (640 quads) | 33–39 µs |
| Binary greedy mesh, 3D checkerboard (98,304 quads) | 0.77–1.05 ms |
| Naive culled reference mesh, surface chunk | 211–241 µs |
| Padded gather (centre + 6 neighbour slabs) | 29–35 µs |
| Terrain generate, surface chunk | 140–149 µs |
| Palette decode, surface chunk | 17–24 µs |

## Comparison with Decaf and Lemon

### Mesher on identical input (measured, cloud VM, `tools/legacy-compare`)

All three meshers produce exactly the same set of faces on every scene. Times are µs per chunk on one pinned thread, the median of 5 invocations. Method, adaptations and caveats: `docs/research/fair-mesher-comparison.md`.

| Scene | Espresso | Lemon (exact neighbours) | Decaf (as shipped) | Espresso vs Lemon | Espresso vs Decaf |
|---|---|---|---|---|---|
| Terrain surface | 38.6 | 298 | 965 | 7.7× | 25× |
| Solid with air border | 58.6 | 64.3 | 1,305 | 1.1× | 22× |
| Random 50% fill | 926 | 1,664 | 3,573 | 1.8× | 3.9× |
| 3D checkerboard | 836 | 981 | 1,569 | 1.2× | 1.9× |
| Deep with air pockets | 54.3 | 280 | 1,309 | 5.2× | 24× |

The speed-up depends on the scene, from 1.1× to 7.7× against Lemon, so no single headline number is honest. Lemon was given exact neighbour data here. In its shipped async path it also predicted borders with `boundary_planes`, which the Lemon audit measured at 1.4 ms for a surface chunk (quoted, different terrain, not re-measured). Espresso pays for `gather_padded` instead: 29–35 µs per surface chunk (measured, micro-bench above).

### Memory for voxel data

| | Decaf | Lemon | Espresso |
|---|---|---|---|
| Per air chunk | 128 KiB (computed from the `u32` layout) | 64 KiB (computed from the `u16` layout) | 0 (measured: `Uniform` has no heap) |
| `far` region, 22,445 chunks | 2.74 GiB (computed) | 1.37 GiB (computed) | 58.6 MiB (measured, voxel heap) |

### Not comparable

Decaf's 4.95 ms/chunk and Lemon's 297–350 µs/chunk generation times (quoted from the audits) used different terrain generators with very different complexity. They cannot be compared with Espresso's 140–149 µs, so no ratio is claimed. The only real-GPU frame numbers from either repo are Lemon's 500–750 FPS on an Intel iGPU at render distance 32 (quoted, commit e97f5c9). They cannot be compared with the RTX 2060 baseline: different GPU, scene and view distance.
