# Performance log

Method and rules: `docs/BENCHMARKING.md`. Every number is labelled:

- **measured**: produced by `vx bench`, `cargo bench-cpu` or `tools/legacy-compare` in a release build, with the machine named.
- **quoted**: copied from another document and not re-run here.
- **computed**: derived arithmetically from other numbers.

Budgets to hold as features land (targets, not measurements): frame ≤ 4 ms CPU and ≤ 8 ms GPU at the `default` scenario on a mid-range GPU, and no frame above 16 ms while streaming at sprint speed.

## Hardware baselines

None yet. The first real-GPU baseline will come from Noah's machine via `scripts/bench-baseline.*` (see `docs/BENCHMARKING.md`) and go into `benchmarks/baselines/<label>/`.

## Cloud VM reference (2026-11-03)

Machine: Intel Xeon @ 2.30 GHz VM, 4 vCPUs, 16 GiB, Ubuntu 24.04, rustc 1.97.1, no RUSTFLAGS. GPU: llvmpipe (lavapipe, Mesa 25.2.8), a software rasteriser. The GPU and frame numbers below only show that the path works; they say nothing about real hardware. The VM is shared, and repeat-to-repeat spread was 12–28% for the world phases.

### `vx bench --scenario default --frames 30 --warmup 5 --resolution 640x360` (measured, two back-to-back runs)

Content is identical in both runs (hash `fab6872fe1853d8e`): 6,125 chunks generated, 2,826 meshed, 986,380 quads, 9,492 draws, 16.1 MiB voxel heap, 7.7 MiB geometry buffers, 129 MiB RSS.

| Metric (median of 5 repeat medians) | Run 1 | Run 2 | `bench-compare` verdict |
|---|---|---|---|
| World generation, 4 threads | 162.4 ms | 171.8 ms | within noise (band 17%) |
| Meshing, 4 threads | 71.8 ms | 70.6 ms | within noise (band 28%) |
| Upload | 3.9 ms | 3.6 ms | within noise (band 241%, not trustworthy) |
| Frame CPU encode | 325 µs | 333 µs | within noise |
| Frame GPU terrain pass (software) | 154 ms | 155 ms | within noise |

Superseded: the bootstrap commit quoted single cold runs (radius 16: gen 212 ms, mesh 62 ms). Those runs had no warm-up and no repeats, and the results above replace them.

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

### Mesher, identical input (measured, `tools/legacy-compare`)

All three meshers produce exactly the same set of faces on every scene. Times are µs per chunk on one pinned thread, the median of 5 invocations. Full method, adaptations and caveats are in `docs/research/fair-mesher-comparison.md`.

| Scene | Espresso | Lemon (exact neighbours) | Decaf (as shipped) | Espresso vs Lemon | Espresso vs Decaf |
|---|---|---|---|---|---|
| Terrain surface | 38.6 | 298 | 965 | 7.7× | 25× |
| Solid with air border | 58.6 | 64.3 | 1,305 | 1.1× | 22× |
| Random 50% fill | 926 | 1,664 | 3,573 | 1.8× | 3.9× |
| 3D checkerboard | 836 | 981 | 1,569 | 1.2× | 1.9× |
| Deep with air pockets | 54.3 | 280 | 1,309 | 5.2× | 24× |

How to read this: the speed-up depends on the scene, from 1.1× to 7.7× against Lemon, so no single headline number is honest. Lemon was given exact neighbour data here. In its shipped async path it also predicted borders with `boundary_planes`, which the Lemon audit measured at 1.4 ms for a surface chunk (quoted, different terrain, not re-measured). Espresso pays for `gather_padded` instead: 29–35 µs per surface chunk (measured, micro-bench above).

### Memory for voxel data

| | Decaf | Lemon | Espresso |
|---|---|---|---|
| Per air chunk | 128 KiB (computed from the `u32` layout) | 64 KiB (computed from the `u16` layout) | 0 (measured: `Uniform` has no heap) |
| `far` region, 22,445 chunks | 2.74 GiB (computed) | 1.37 GiB (computed) | 58.6 MiB (measured, voxel heap) |

### Not comparable

Decaf's 4.95 ms/chunk and Lemon's 297–350 µs/chunk generation times (quoted from the audits) used different terrain generators with very different complexity. They cannot be compared with Espresso's 140–149 µs, so no ratio is claimed. The only real-GPU frame numbers from either repo are Lemon's 500–750 FPS on an Intel iGPU at render distance 32 (quoted, commit e97f5c9), and those cannot be compared with anything measured here.
