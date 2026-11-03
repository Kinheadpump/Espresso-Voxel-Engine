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

## Sprint S1 start baseline: `noah-msi-rtx2060` (2026-11-03)

The reference every S1 gate compares against (`docs/SPRINTS.md` rule 1).

- **Build:** commit `7cf5ed4fd962` (main after PR #12), release, clean tree, flat shading, wgpu indirect validation off (ADR-0015).
- **Machine:** as in the hardware baseline (driver 616.92, AC power, High performance plan). Nothing else running.
- **Command:** `scripts/bench-baseline.ps1 -Label noah-msi-rtx2060/sprint-S1-start`, then a second `default` run.
- **Raw files:** `benchmarks/baselines/noah-msi-rtx2060/sprint-S1-start/`.

### Timings (measured; median of 5 repeat medians, repeat spread in brackets)

| Scenario | World generation | Meshing | Upload | Frame CPU encode (record / submit) | Frame GPU terrain pass | Frame wall |
|---|---|---|---|---|---|---|
| `small` | 52.1 ms (12%) | 18.6 ms (8%) | 2.8 ms (12%) | 65.4 µs (26.1 / 32.8) | 167.2 µs (85%) | 332.1 µs (54%) |
| `default` | 165.3 ms (4%) | 64.4 ms (8%) | 7.3 ms (13%) | 143.4 µs (61.8 / 64.8) | 509.7 µs (7%) | 943.2 µs (17%) |
| `far` | 618.9 ms (5%) | 253.8 ms (8%) | 46.1 ms (4%) | 162.3 µs (74.1 / 67.9) | 1,771.5 µs (1%) | 2,208.3 µs (3%) |
| `cave` | 166.9 ms (9%) | 65.0 ms (13%) | 7.3 ms (6%) | 145.1 µs (62.0 / 66.3) | 460.4 µs (3%) | 886.1 µs (10%) |
| `mountain` | 169.1 ms (2%) | 66.4 ms (1%) | 7.4 ms (8%) | 144.2 µs (61.6 / 65.2) | 479.2 µs (2%) | 911.8 µs (10%) |
| `fly-fast` | 171.5 ms (3%) | 67.8 ms (2%) | 7.3 ms (14%) | 144.8 µs (62.2 / 65.5) | 496.1 µs (5%) | 919.7 µs (8%) |

Content hashes match earlier runs (`default` `fab6872fe1853d8e`, `cave` `c105e9031c73c4ce`), and the culling ladders are identical to the B-01 section below.

Stability (`vx bench-compare`, `default` run 1 → run 2): every metric within noise; largest change meshing +2.7% (noise 8.3%). Frame CPU encode spread reached 40% in run 2, so CPU-encode changes below ~40% cannot be called against this pair.

Findings:

- World generation and meshing are 35–40% faster than in the first hardware baseline (`default` 165 vs 260–267 ms) with no generator or mesher change between them; the laptop's CPU state differed between sessions. Compare only within one session, as `docs/BENCHMARKING.md` requires.
- Frame CPU encode has a rare spike: pooled max 1.48–1.73 ms against p99 0.14–0.33 ms in every scenario (measured). The max sits in `submit` (1.39–1.65 ms), not in `record`. The windowed bench (roadmap 2) should show whether such spikes reach presentation.
- `small` frame numbers are too noisy on this laptop for verdicts (54–85% spread).

## ADR-0003 chunk size, 32³ vs 62³: `noah-msi-rtx2060` (2026-11-03)

- **Build:** `a502153d25b6` (both sizes from one commit; 62³ via `--features chunk62`), release, clean worktree, `git_dirty=false`.
- **Equal reach:** scenarios are defined in blocks. `default` = 512 blocks (radius 16 × 32 or 8 × 62; meshed width 1,056 vs 1,054 blocks), `far` = 1,024 blocks (radius 32 vs 17; 2,080 vs 2,170 blocks). World height is 160 blocks (5 × 32 vs 3 × 62 = 186).
- **Runs:** `default` twice per size (interleaved), `far` once, windowed `default` once at 3440×1440 `Immediate`.
- **Raw files:** `benchmarks/baselines/noah-msi-rtx2060/adr-0003-chunk-size/`.

### `default` (measured; two runs each)

| Metric | 32³ | 62³ |
|---|---|---|
| Chunks generated / meshed | 6,125 / 2,826 | 1,083 / 567 |
| Quads / draws | 986,380 / 9,492 | 969,912 / 2,859 |
| World generation | 149.6 / 157.9 ms | 197.9 / 203.6 ms |
| Generation per generated voxel (computed) | 0.75–0.79 ns | 0.77–0.79 ns |
| Meshing | 58.9 / 61.5 ms | 69.1 / 69.8 ms |
| Meshing per meshed voxel (computed) | 0.64–0.66 ns | 0.51–0.52 ns |
| Frame CPU encode | 152.2 / 153.3 µs | 145.3 / 143.8 µs |
| Frame GPU terrain pass | 504.4 / 507.9 µs | 512.2 / 513.7 µs |
| Frame wall | 945.4 / 942.8 µs | 952.2 / 944.0 µs |
| Windowed frame interval (pacing p99/p50) | 907.5 µs (1.65) | 922.2 µs (1.72) |
| Voxel heap | 16.1 MiB | 38.1 MiB |
| Peak RSS | 172–178 MiB | 170–175 MiB |
| Pixel-visible quads (perfect chunk occlusion) | 183,421 (18.6%) | 219,675 (22.6%) |

### `far` (measured; one run each)

| Metric | 32³ (radius 32) | 62³ (radius 17) |
|---|---|---|
| Quads / draws | 3,792,695 / 36,275 | 4,063,557 / 11,993 |
| World generation | 597.4 ms (0.81 ns/voxel) | 811.0 ms (0.83 ns/voxel) |
| Meshing | 245.3 ms (0.69 ns/voxel) | 301.7 ms (0.53 ns/voxel) |
| Frame GPU terrain pass | 1,768.8 µs | 1,891.7 µs |
| Frame wall | 2,209.2 µs | 2,326.5 µs |
| Voxel heap / peak RSS | 58.6 / 219 MiB | 143.2 / 316 MiB |
| Pixel-visible quads | 484,225 (12.8%) | 657,527 (16.2%) |

Instruction counts (gungraun, cloud container, one surface chunk at sea level, computed per voxel): mesher 20.1 vs 13.8 instructions per voxel, generation 73.5 vs 59.3, gather 17.4 vs 14.0, palette decode 10.3 vs 10.3.

Findings:

- **Frame time does not move.** Encode, GPU pass, wall and windowed interval at `default` are within run-to-run noise of each other. 3.3× fewer draws buy nothing since ADR-0015 removed the per-draw CPU cost (measured: encode 143–153 µs for both).
- **The mesher is ~20% cheaper per voxel at 62³** (bench and instruction counts agree), but 62³ chunks cover more volume at the same reach (+29% generated voxels at `default`, +33% at `far`, from coarser region edges and a 186-block world), so total meshing is 15–23% slower and generation 25–36% slower (measured).
- **Culling gets coarser.** A perfect chunk-granular occlusion culler keeps 20% more quads at 62³ on `default` (computed from the ladder); frustum+facing keeps 26.8% vs 24.5%.
- **Memory grows.** 62³ chunks are rarely uniform, so the voxel heap is 2.4× larger (38.1 vs 16.1 MiB at `default`, 143 vs 59 MiB at `far`).
- An edit remeshes 238,328 voxels instead of 32,768 (7.3×, computed).

Recommendation: keep 32³ (ADR-0003 Accepted with these numbers; 62³ rejected). Revisit only if GPU culling (S4) turns out bound by draw or chunk count.

## AO probe (B-09 part 1, 2026-11-03)

`cargo run --release -p vx-instr --example ao_probe -- <view_blocks>`: plain greedy vs AO-safe greedy (merge only faces whose four 0fps corner AO values match) over the scenario region, plus the meshable area under the 6- vs 26-neighbour rule for a circular load radius. Measured in the cloud container (counts are deterministic).

| Region | Plain greedy quads | AO-safe quads | Increase |
|---|---|---|---|
| 32³, 512 blocks (`default`) | 986,380 | 1,645,740 | +66.8% |
| 32³, 1,024 blocks (`far`) | 3,792,700 | 6,352,117 | +67.5% |
| 62³, 512 blocks | 969,912 | 1,633,738 | +68.4% |

The plain count matches the binary mesher on `default` exactly and on `far` to 5 quads in 3.8 M (0.0001%).

| Load radius | Meshable columns, 6-rule | 26-rule | Meshed reach lost |
|---|---|---|---|
| 16 chunks (32³) | 709 | 673 (−5.1%) | 12.4 blocks |
| 32 chunks (32³) | 3,029 | 2,957 (−2.4%) | 11.9 blocks |

Findings: AO-safe merging adds +67% quads, far above B-09's ~30% threshold (estimate). The rolling heightmap makes AO vary along almost every slope, so faces stop merging. The 26-neighbour rule costs only ~12 blocks of meshed reach, but it buys nothing without vertex AO. Recommendation: SSAO for ambient occlusion (S8), ADR-0006 stays at 6 neighbours, and B-03 keeps the 6-slab gather.

## Sprint S1 exit gate and windowed bench: `noah-msi-rtx2060` (2026-11-03)

- **Build:** `main` at `fbdc3d99a264` (after PRs #13–#15: memory reporting, Tracy zones with the feature off, windowed mode), release, clean detached worktree, `git_dirty=false` in every report.
- **Machine:** as in the S1 start baseline (driver 616.92). Windowed runs at the monitor's 3440×1440, present mode `Immediate`.
- **Commands:** `vx bench --scenario default` twice, `--scenario far` once, `--scenario default --windowed` twice; `vx bench-compare` against `sprint-S1-start`.
- **Raw files:** `benchmarks/baselines/noah-msi-rtx2060/sprint-S1-gate/` (5 JSON, 4 compare outputs, run log).

### Gate: Tracy off vs S1 start (measured; `bench-compare` verdicts)

| Metric | `default` run 1 | `default` run 2 | `far` |
|---|---|---|---|
| World generation | 165.3 → 145.9 ms (−11.7%, FASTER) | 168.6 → 149.6 ms (−11.3%, FASTER) | 618.9 → 559.3 ms (−9.6%, FASTER) |
| Meshing | 64.4 → 57.6 ms (−10.6%, FASTER) | 66.1 → 58.4 ms (−11.7%, FASTER) | 253.8 → 232.8 ms (−8.3%, within noise) |
| Upload | +4.6%, within noise | +5.1%, within noise | +2.2%, within noise |
| Frame CPU encode | 143.4 → 146.1 µs, within noise | 145.0 → 146.5 µs, within noise | 162.3 → 166.4 µs, within noise |
| Frame GPU terrain pass | 509.7 → 501.5 µs, within noise | 515.0 → 503.8 µs, within noise | 1,771.5 → 1,751.6 µs, within noise |
| Frame wall | 943.2 → 932.5 µs, within noise | 943.1 → 943.7 µs, within noise | 2,208.3 → 2,192.8 µs, within noise |

Content hashes are unchanged (`default` `fab6872fe1853d8e`, `far` `4229b10348a4dc42`).

Verdict: **gate passed for every frame metric.** World generation and meshing read ~10% FASTER, but between `7cf5ed4` and `fbdc3d9` the only change to `vx-world`, `vx-mesh` and `region.rs` is seven `profiling::scope!` lines that expand to nothing without the `tracy` feature. The two windowed runs, the same build minutes apart, also differ by −9.8% in world generation (FASTER). That is drift in the laptop's CPU state between sessions, larger than the within-session noise band. Rule kept from the S1 start findings: CPU-side world numbers are compared only within one session, back to back.

### Windowed repeatability (measured; same build, run 1 → run 2)

| Metric | Run 1 | Run 2 | Change (noise) |
|---|---|---|---|
| Frame interval p50 (center) | 869.9 µs | 885.2 µs | +1.8% (2.2%), within noise |
| Interval p99 | 1,467.7 µs | 1,509.6 µs | |
| Interval max | 61,319.5 µs | 2,565.9 µs | |
| Interval stddev | 1,115.2 µs | 174.8 µs | |
| p99/p50 | 1.69 | 1.71 | |
| Frame CPU encode (center) | 265.6 µs | 392.6 µs | +47.8% (46.8%), SLOWER |

Findings:

- **Threshold:** the windowed frame interval repeats within ~2% (measured). Windowed gates use the interval center and p99/p50 with a 2% minimum noise band (the tool's floor); windowed CPU encode and submit (47% noise) are not usable for verdicts.
- Run 1 had a single 60 ms hitch, inside `submit` + `present` (encode max 60,053 µs, submit 60,004.7 µs). One frame in 3,000, not repeated in run 2. Gates must not use the interval max or stddev until hitches are reproducible.
- The windowed interval at 3440×1440 (870 µs) is close to the offscreen frame wall at 1920×1080 (933–944 µs), because pipelining hides the CPU wait (computed). The resolutions differ, so neither number is a substitute for the other.

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

## B-02 wgpu indirect validation on vs off: `noah-msi-rtx2060` (2026-11-03)

- **Build:** commit `2ea1cabb2dac` (PR #12 before the default changed), release, one binary for every run, built in a clean worktree.
- **Machine:** as in the hardware baseline (driver 616.92, AC power, High performance plan). Nothing else running.
- **Command:** `vx bench --scenario <s> --shading flat --indirect-validation on|off`, default settings (5 repeats, 120 warm-up + 600 measured frames, 1920×1080), on and off back to back; `default` twice, plus one `--shading textured` pair.
- **Raw files:** on the MSI in `D:\RustProjects\reworked_voxel_engine\bench-b02\` (not committed).

### Timings (measured; `vx bench-compare` on → off, median of repeat medians, noise band in brackets)

| Metric | `small` (2,506 draws) | `default` run 1 (9,492) | `default` run 2 | `far` (36,275) | `default` textured |
|---|---|---|---|---|---|
| Frame CPU encode | 230.6 → 63.9 µs, FASTER (36%) | 437.2 → 148.4 µs, FASTER (15%) | 428.1 → 150.9 µs, FASTER (14%) | 1,260.9 → 165.1 µs, FASTER (14%) | 432.9 → 165.5 µs, FASTER (13%) |
| of which record | 147.1 → 25.7 µs, FASTER | 341.2 → 64.3 µs, FASTER | 335.2 → 63.2 µs, FASTER | 1,165.8 → 76.0 µs, FASTER | 338.0 → 76.3 µs, FASTER |
| of which submit | 65.6 → 32.1 µs, FASTER (31%) | 75.0 → 66.9 µs, FASTER (7%) | 71.9 → 66.7 µs, FASTER (6%) | 72.2 → 68.0 µs, within noise | 73.3 → 68.5 µs, FASTER (6%) |
| Frame GPU terrain pass | 274.1 → 166.7 µs, within noise (85%) | 511.2 → 512.3 µs, within noise | 516.6 → 523.5 µs, within noise | 1,770.2 → 1,793.3 µs, within noise | 571.4 → 584.7 µs, within noise |
| Frame wall | 802.2 → 369.2 µs, FASTER (44%) | 1,232.9 → 939.1 µs, FASTER (12%) | 1,227.8 → 949.4 µs, FASTER (11%) | 3,340.9 → 2,236.6 µs, FASTER (3%) | 1,290.8 → 1,009.3 µs, FASTER (11%) |

World generation and upload were within noise in every pair; meshing too, except `far` (+2.4%, noise 2.2%, SLOWER), which the switch cannot affect (render-only) and is treated as run-to-run drift. `small` is noisy on this laptop (31–85% bands). Content hashes are identical on and off.

Findings:

- With validation on, frame CPU record grows ~30 ns per draw (computed: (1,165.8 − 147.1) µs / (36,275 − 2,506) draws). With it off, record is 26–76 µs (measured) at every draw count.
- The GPU terrain pass does not change, so the validation dispatch costs no measurable GPU time here.
- Screenshots with validation on and off are byte-identical on the RTX 2060 (`default` 0.1, `cave` 0.4, `far` 0.3, SHA-256) and on lavapipe.
- Decision: off in release, ADR-0015. The `noah-msi-rtx2060` baseline above ran with it on; compare against it with `--indirect-validation on`.

## Sprint S2 gate: `noah-msi-rtx2060` (2026-11-03)

Base `6b20c4e` (`main`, S2 start) against candidate `62aa6a6` (B-03 padded decode, per-width decode unrolling, column-shared height noise), both clean release builds at 32³, offscreen, flat shading, Vulkan 616.92. Data and logs in `benchmarks/baselines/noah-msi-rtx2060/sprint-S2-gate/`. Content is identical: `default` hash `fab6872fe1853d8e` (6,125 generated, 2,826 meshed, 986,380 quads, 9,492 draws), `far` hash `4229b10348a4dc42` (22,445 generated, 36,275 draws).

### `vx bench-compare` base → candidate (measured; median of repeat medians, noise band in brackets)

| Metric | `default` run 1 | `default` run 2 | `far` |
|---|---|---|---|
| World generation | 152.4 → 73.5 ms, −51.8% (9.0%) FASTER | 160.3 → 70.6 ms, −56.0% (8.8%) FASTER | 609.7 → 270.2 ms, −55.7% (4.2%) FASTER |
| Meshing | 59.4 → 54.9 ms, −7.6% (8.3%) within noise | 61.1 → 52.3 ms, −14.5% (3.5%) FASTER | 246.8 → 210.0 ms, −14.9% (7.7%) FASTER |
| Upload | within noise | within noise | within noise |
| Frame CPU encode | within noise | within noise | within noise |
| Frame GPU terrain pass | 504.5 → 508.4 µs, within noise | 511.8 → 512.2 µs, within noise | 1,778.9 → 1,793.1 µs, within noise |
| Frame wall | 932.5 → 923.5 µs, within noise | 934.7 → 945.2 µs, within noise | 2,219.1 → 2,231.5 µs, within noise |

Gate reading: world generation at `default` is at most half the S2 start (−51.8% and −56.0%, both runs) and `mesh_hash` is unchanged, so that gate passes. Meshing at `far` is faster on two agreeing runs (`far` and `default` run 2; `default` run 1 is inside its noise band). Frame metrics are unaffected, as expected for CPU-side work. Meshing at `default` is 52.3–54.9 ms, under the 80 ms long-term target (measured). SIMD noise (scope 4) is not in this run.

## Sprint S3 start baseline: `noah-msi-rtx2060` (2026-11-03)

`c830ca7` (`main` after PR #17), clean release build, flat shading, offscreen 1920×1080 unless stated, Vulkan 616.92. Data in `benchmarks/baselines/noah-msi-rtx2060/sprint-S3-start/`. Every S3 gate compares against these runs (`docs/SPRINTS.md` rule 1).

| Scenario | Content (generated / meshed chunks, quads, draws, hash) | Frame wall | GPU terrain pass | World generation | Meshing |
|---|---|---|---|---|---|
| `small` | 1,805 / 748, 259,091, 2,506, `65d196ce12d01116` | 381.2 µs (spread 69%) | 173.1 µs (spread 39%) | 20.2 ms | 14.1 ms |
| `default` | 6,125 / 2,826, 986,380, 9,492, `fab6872fe1853d8e` | 935.0 µs | 507.8 µs | 66.7 ms | 50.3 ms |
| `far` | 22,445 / 10,858, 3,792,695, 36,275, `4229b10348a4dc42` | 2,186.6 µs | 1,750.3 µs | 249.3 ms | 192.7 ms |
| `cave` | 6,125 / 2,826, 986,480, 9,559, `c105e9031c73c4ce` | 879.4 µs | 457.0 µs | 68.7 ms | 50.7 ms |
| `mountain` | same content as `default` | 887.4 µs | 476.4 µs | 69.0 ms | 51.3 ms |
| `fly-fast` | same content as `default` | 709.8 µs | 491.7 µs | 75.3 ms | 55.0 ms |

All values measured (median of repeat medians). `small` frame numbers are too noisy to gate on. Windowed `default` (borderless 3440×1440, present mode Immediate): frame interval p50 896.3 µs, p99 1,486.5 µs, max 51,813.4 µs, repeat spread 3.2%, p99/p50 1.66; CPU encode p50 358.1 µs (measured).

## Camera-relative rendering (ADR-0016): `noah-msi-rtx2060` (2026-11-03)

Base `c830ca7` against candidate `d2a2f59`, alternating runs, flat shading, data in `benchmarks/baselines/noah-msi-rtx2060/camera-relative/`. Content hashes are identical (`default` `fab6872fe1853d8e`, `far` `4229b10348a4dc42`).

| Metric (measured, `vx bench-compare`) | `default` run 1 | `default` run 2 | `far` |
|---|---|---|---|
| Frame GPU terrain pass | 503.7 → 509.6 µs, +1.2% (7.6%) within noise | 509.9 → 514.0 µs, +0.8% (7.8%) within noise | 1,785.1 → 1,803.7 µs, +1.0% (2.0%) within noise |
| Frame wall | +1.3% within noise | −2.1% within noise | +0.8% within noise |
| Frame CPU encode | within noise | within noise | within noise |

Every other metric is within noise in all three compares. The extra `i32` subtract and mask per vertex has no measurable cost. Correctness (lavapipe and CI): a screenshot moved by 1,000,064 chunks (32 M blocks) is byte-identical to the one at the origin.

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
