# Roadmap

What we build next, in order of expected impact on the core goal (view distance and frame time per watt), measurement infrastructure first. Each item names how it will be verified. Unscheduled ideas, ranked, live in `docs/BACKLOG.md` (`B-nn` ids); an item moves here when it gets a slot. Which sprint each item belongs to, and the benchmark gates that close it, are in `docs/SPRINTS.md`.

## Done (2026-11-03)

- [x] Workspace, toolchain pin, lints, CI.
- [x] Palette chunk storage with uniform shortcut, ring-buffer window, heightmap terrain.
- [x] Binary greedy mesher with reference-equivalence tests (33–39 µs per surface chunk, measured on the cloud VM, `docs/PERFORMANCE.md`).
- [x] wgpu 30 vertex-pulling terrain pipeline with one `multi_draw_indirect`.
- [x] Windowed fly-cam, headless `--probe` and `--screenshot` on lavapipe.
- [x] `vx bench` (deterministic scenarios, warm-up, repeats, GPU timestamps, environment capture, determinism hash, JSON) and `vx bench-compare` (noise-aware verdicts, fairness warnings). See `docs/BENCHMARKING.md`.
- [x] Fair mesher comparison against Decaf and Lemon on identical input (`tools/legacy-compare`).
- [x] Baseline scripts for Linux/macOS and Windows.
- [x] First real-GPU baseline, `noah-msi-rtx2060` (i7-9750H, RTX 2060, Vulkan), two runs, `default` within noise on every metric. Results in `docs/PERFORMANCE.md`.

## Next: solid foundation before features

1. **Occlusion counters and scenarios** (`cave`, `mountain`, `fly-fast`; counter ladder frustum → graph → Hi-Z → quads submitted; culling-off image-identity check). Backlog B-01. Verify: scenarios deterministic (stable mesh hash), counters match a CPU recount.
2. **Windowed present-mode benchmark** (`vx bench --windowed`): the same orbit with a real swapchain, vsync off, recording frame pacing (p99, frame-time variance). The offscreen bench measures serialised per-frame cost and does not capture presentation or pipelining.
3. **Extend GPU timestamps** to every pass as passes are added (cull, shadow, post). Keep one JSON metric per pass.
4. **Peak memory and VRAM reporting**: peak RSS (Linux `VmHWM`, Windows `PeakWorkingSetSize`) and wgpu allocator reports, instead of current RSS only.
5. **Tracy behind a feature flag** via the `profiling` crate (zones in gen, mesh, upload, encode, submit, frame). Use it first on backlog B-02 (frame CPU encode grows with draw count). Verify: `bench-compare` shows no change with the feature off.
6. **CI regression signal**: instruction-count benches (gungraun or CodSpeed) for the mesher and generator, because wall-clock on shared runners is too noisy (measured 12–28% repeat spread on a cloud VM).

## Then: streaming core

7. **Chunk streaming** around the camera using `ChunkWindow`: dedicated rayon pool, priority heap, bounded in-flight jobs, generation-counter cancellation (ADR-0005), and meshing only when 6 neighbours exist (ADR-0006). Design input: backlog B-04. Verify: no frame over budget while flying at sprint speed; chunks/s throughput.
8. **GPU buffer suballocation** with `offset-allocator` for quads (one big buffer, O(1) alloc/free) and staged uploads with a byte and time budget per frame (backlog B-05). Verify: upload ms per frame, fragmentation after long flights.
9. **GPU culling** (ADR-0010, backlog B-06): frustum + per-direction back-face rejection in compute, `multi_draw_indirect_count` where available. Verify: GPU ms and draws issued at radius 32/48.

## Then: scale

10. **Chunk-size experiment** 32³ vs 62³ (ADR-0003) and **4-byte vs 8-byte quads** (ADR-0004), using the bench mode.
11. **Occlusion culling** if B-01's counters show occlusion: two-phase Hi-Z, visibility graph or occluder quads (backlog B-07). Verify: GPU ms in caves and mountains.
12. **Far-field LOD** prototypes (ADR-0011, backlog B-08).
13. **SIMD terrain noise** (ADR-0009) if generation dominates the streaming profile.

## Then: looks (each with a cost budget)

14. Editing with remesh of affected neighbours; persistence (region files, palette-compressed).
15. Per-vertex AO in the free quad bits (only merge equal-AO faces; measure the quad-count increase) or SSAO.
16. Flood-fill sky and block light.
17. ~~Block textures~~ done (16 px texture array, `--shading flat|textured`; ADR-0014 accepted at +8.6% GPU terrain pass in `default`, measured; `docs/TEXTURES.md`). Transparent pass (water/glass), cascaded shadows with per-cascade GPU culling and cached far cascades.
18. Camera-relative rendering for large coordinates (world origin rebasing).

## Known issues

- The mesher is 1.1–1.2× faster than Lemon's on dense chunks (solid, checkerboard) but up to 7.7× on terrain. The dense path is dominated by building the column masks and emitting quads; it is worth profiling once streaming exists.
- Quad counts differ slightly from Lemon and Decaf because of merge scan order: 5 more on random fill (42,169 vs 42,164) and 5 fewer on terrain (640 vs 645). This is negligible.
- Greedy meshing can create T-junctions; a faint sub-pixel seam was visible in one headless frame. Check on the RTX 2060 before deciding whether it needs fixing (backlog B-10).
- Frame CPU encode grows with draw count (~40–70 ns per draw, computed from the RTX 2060 baseline) although only one `multi_draw_indirect` is recorded. Backlog B-02.
- Region build in the app is a fixed square; no streaming yet.
