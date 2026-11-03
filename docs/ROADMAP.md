# Roadmap

Ordered by expected impact on the core goal (view distance and frame time per watt), measurement infrastructure first. Each item names how it will be verified. Unscheduled ideas from other engines, with their measurements, live in `docs/PROPOSALS.md`.

## Done (2026-11-03)

- [x] Workspace, toolchain pin, lints, CI.
- [x] Palette chunk storage with uniform shortcut, ring-buffer window, heightmap terrain.
- [x] Binary greedy mesher with reference-equivalence tests (~35 µs per surface chunk).
- [x] wgpu 30 vertex-pulling terrain pipeline with one `multi_draw_indirect`.
- [x] Windowed fly-cam, headless `--probe` and `--screenshot` on lavapipe.
- [x] `vx bench` (deterministic scenarios, warm-up, repeats, GPU timestamps, environment capture, determinism hash, JSON) and `vx bench-compare` (noise-aware verdicts, fairness warnings). See `docs/BENCHMARKING.md`.
- [x] Fair mesher comparison against Decaf and Lemon on identical input (`tools/legacy-compare`).
- [x] Baseline scripts for Linux/macOS and Windows.

## Next: solid foundation before features

1. **First real-GPU baseline** from Noah's machine (`scripts/bench-baseline.*`), committed to `benchmarks/baselines/`. Run twice to confirm the machine's noise band. Blocks every GPU-side optimisation.
2. **Windowed present-mode benchmark** (`vx bench --windowed`): the same orbit with a real swapchain, vsync off, recording frame pacing (p99, frame-time variance). The offscreen bench measures serialised per-frame cost and does not capture presentation or pipelining.
3. **Extend GPU timestamps** to every pass as passes are added (cull, shadow, post). Keep one JSON metric per pass.
4. **Peak memory and VRAM reporting**: peak RSS (Linux `VmHWM`, Windows `PeakWorkingSetSize`) and wgpu allocator reports, instead of current RSS only.
5. **Tracy behind a feature flag** via the `profiling` crate (zones in gen, mesh, upload, frame). Verify: `bench-compare` shows no change with the feature off.
6. **CI regression signal**: instruction-count benches (gungraun or CodSpeed) for the mesher and generator, because wall-clock on shared runners is too noisy (measured 12–28% repeat spread on a cloud VM).

   Also before culling work: **occlusion counters and scenarios** (`cave`, `mountain`, `fly-fast`; counter ladder frustum → graph → Hi-Z → quads submitted, and a culling-off image-identity check). Candidate list from the 2024–2026 research survey: `docs/research/recent-2024-2026/README.md` (R-01).

## Then: streaming core

7. **Chunk streaming** around the camera using `ChunkWindow`: dedicated rayon pool, priority heap, bounded in-flight jobs, generation-counter cancellation (ADR-0005), and meshing only when 6 neighbours exist (ADR-0006). Verify: no frame over budget while flying at sprint speed; chunks/s throughput.
8. **GPU buffer suballocation** with `offset-allocator` for quads (one big buffer, O(1) alloc/free) and staged uploads with a byte budget per frame. Verify: upload ms per frame, fragmentation after long flights.
9. **GPU culling** (ADR-0010): frustum + per-direction back-face rejection in compute, `multi_draw_indirect_count` where available. Verify: GPU ms and draws issued at radius 32/48.

## Then: scale

10. **Chunk-size experiment** 32³ vs 62³ (ADR-0003) and **4-byte vs 8-byte quads** (ADR-0004), using the bench mode.
11. **Two-phase Hi-Z occlusion** if overdraw shows. Verify: GPU ms in caves and mountains.
12. **Far-field LOD** prototypes (ADR-0011).
13. **SIMD terrain noise** (ADR-0009) if generation dominates the streaming profile.

## Then: looks (each with a cost budget)

14. Editing with remesh of affected neighbours; persistence (region files, palette-compressed).
15. Per-vertex AO in the free quad bits (only merge equal-AO faces; measure the quad-count increase) or SSAO.
16. Flood-fill sky and block light.
17. Texture array, transparent pass (water/glass), cascaded shadows with per-cascade GPU culling and cached far cascades.
18. Camera-relative rendering for large coordinates (world origin rebasing).

## Known issues

- The mesher is 1.1–1.2× faster than Lemon's on dense chunks (solid, checkerboard) but up to 7.7× on terrain. The dense path is dominated by building the column masks and emitting quads; it is worth profiling once streaming exists.
- Quad counts differ slightly from Lemon and Decaf because of merge scan order: 5 more on random fill (42,169 vs 42,164) and 5 fewer on terrain (640 vs 645). This is negligible.

- Greedy meshing can create T-junctions; a faint sub-pixel seam was visible in one headless frame. Check on real GPUs before deciding whether it needs fixing.
- Region build in the app is a fixed square; no streaming yet.
