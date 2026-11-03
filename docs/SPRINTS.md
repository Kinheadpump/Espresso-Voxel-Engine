# Sprint plan

How Espresso gets from today's measured foundation to the engine we want: ten sprints, each with a goal, scope, exit gates read from `vx bench`, dependencies and risks, plus the long-term targets they add up to. Written 2026-11-03 against `main` at `0212861`. `docs/ROADMAP.md` stays the numbered work list; this doc groups those items and the `docs/BACKLOG.md` items into sprints and says when each one is done. Numbers follow `docs/DOC-GUIDELINES.md` §4.

## Plan at a glance

| Sprint | Goal | Main items | Gate scenario(s) | Size (estimate) |
|---|---|---|---|---|
| S0 | Close the work in flight | B-01 (scenarios, CPU counters), B-02 (find), textures phase 2 (ADR-0014) | `default`, `far`, `cave`, `mountain` | in progress |
| S1 | See every cost | Roadmap 2–6: windowed bench, per-pass timestamps, peak memory/VRAM, Tracy, CI instruction counts; B-02 fix | all | 1 week |
| S2 | Fix the chunk shape, double throughput | ADR-0003 chunk size, B-03 with 26-neighbour gather, SIMD noise (ADR-0009), AO probe (B-09 part 1) | `default`, `far`, divan | 1–2 weeks |
| S3 | Stream without hitches | Roadmap 7–8, B-04, B-05, ADR-0005, `fly-fast` finalised | `fly-fast` | 1–2 weeks |
| S4 | GPU decides what to draw | Roadmap 9–10, B-06, ADR-0010 phase 1, ADR-0004 quad size | `default`, `far` | 1–2 weeks |
| S5 | See to the horizon | B-08 LOD rings (ADR-0011), roadmap 18 camera-relative rendering | new `horizon` | 2 weeks |
| S6 | A world you can change | B-13 edits, B-12 persistence, B-11 caves, B-14 channels | new `edit`, `save-load`, `caves` | 2 weeks |
| S7 | Cull what is hidden | B-07 (graph, occluder quads or two-phase Hi-Z) on real caves | `cave`, `mountain`, `caves` | 1 week |
| S8 | Light and depth | B-09 vertex AO or SSAO, B-15 light propagation, B-10 seam re-check | `default`, `far`, `caves` | 1–2 weeks |
| S9 | Finish the frame | Transparent pass, cascaded shadows, godrays, B-16 spike | `default`, `far`, windowed | 2 weeks |

Sizes are estimates for planning order only; a sprint ends when its exit gates pass, not on a date. B-17 (hierarchical GPU LOD) is not scheduled: it becomes a sprint only if S5 shows that distance rings miss the reach target.

Why this order: measurement first (S0–S1); then the decisions that every later system builds on, chunk size and the neighbour rule (S2), before streaming (S3), culling (S4) and LOD (S5) are written against them. Occlusion culling (S7) waits for caves (S6), because heightmap terrain has almost no enclosed air for it to remove. Looks come last, each with its own GPU budget.

## Rules for every sprint

These extend the measure-first rule in `CLAUDE.md` and `docs/BENCHMARKING.md` to sprint level.

1. **Start baseline.** The sprint begins with `scripts/bench-baseline.ps1` on `noah-msi-rtx2060` at the sprint's start commit, stored as `benchmarks/baselines/noah-msi-rtx2060/sprint-Sn-start/`. Every gate in the sprint compares against that run, never against a number from an earlier sprint.
2. **No feature without a gate.** Each item lands with the `vx bench` metric that judges it and the threshold that keeps or reverts it, written in its backlog entry before code is written. An item whose gate needs a metric the bench does not report yet adds that metric first, in the same sprint.
3. **Content hash discipline.** A change that should not alter geometry keeps `content.mesh_hash` identical. A change that alters geometry (chunk size, AO splits, caves) says so in its PR and gets a new scenario or a new hash recorded in `docs/PERFORMANCE.md`, so old baselines stay comparable.
4. **One shading mode per comparison.** Until S8, GPU gates and targets use `--shading flat`, the mode of every existing baseline. Textured runs are recorded beside them as a second column, never compared against flat ones.
5. **Revert what does not help.** An optimisation whose `bench-compare` verdict is "within noise" on its gate metric is reverted, even if it looks cleaner. A feature that exceeds its cost budget is reworked or put behind a switch that the bench defaults to off.
6. **Judge on the real GPU.** Lavapipe proves correctness only. GPU and frame numbers come from the RTX 2060 (or a later labelled machine). On the laptop, `default` is the judging scenario; `far` and upload are noisy there (measured, `docs/PERFORMANCE.md`), so a `far` verdict needs a repeat run that agrees.
7. **Sprint review.** The sprint closes with an end baseline, a `bench-compare` start → end per scenario, results in `docs/PERFORMANCE.md`, ADR status updates, ticked roadmap items and a re-ranked backlog. Anything not done moves to the next sprint explicitly.
8. **One owner per doc section.** Parallel agents append to `docs/PERFORMANCE.md` and `docs/BACKLOG.md` in separate sections and rebase before merging; nobody rewrites another item's section.

## S0: Close the work in flight

**Goal:** finish the three streams already running, so S1 starts with occlusion scenarios, an explained B-02 and a decided ADR-0014.

**Scope:**
- B-01 (thread "Work through the top backlog items"): `cave` and `mountain` scenarios, plus the `fly-fast` camera path (flight and teleport). The renderer has no culling yet (only back-face `cull_mode`), so the counters are CPU recounts per frame: chunks resident, chunks in the frustum, chunks with faces toward the camera, `quads_total`. The GPU counter ladder and the culling-off vs culling-on image test arrive with the cull pass in S4.
- B-02 (same thread): split frame CPU encode into encode and `queue.submit`, sweep draw count at a fixed scene, read wgpu 30's `multi_draw_indirect` path. The fix may slip to S1.
- Textures phase 2 (thread "Design the voxel texture art concept"): 16 px `Rgba8UnormSrgb` array, `block_faces` table, texel-snap filter, rotation hash, `vx bench --shading flat|textured` with the compare guard (`docs/TEXTURES.md`). The bench default stays `flat` (rule 4).

**Exit gates:**
- `cave` and `mountain` are deterministic: identical `mesh_hash` across repeats and across two runs.
- CPU counters match an independent recount on the same frame (unit test).
- B-02 has a recorded cause in its backlog entry: encode vs submit share and the per-draw slope (ns per draw) from the sweep, measured on the RTX 2060.
- `--shading textured` keeps `mesh_hash` unchanged and adds at most 10% (target, ADR-0014) to opaque-pass GPU ms over `flat` in `default` and `far`. ADR-0014 becomes Accepted or is amended.

**Dependencies:** Noah's machine for every GPU gate (Claude may run it remotely).

**Risks:**
- The texture pass and B-01 both touch `vx-render` and the bench report schema. Whichever merges second rebases and re-runs the determinism check.
- `bench-compare` must refuse mixed `--shading` runs, or every later sprint compares textured against flat by accident.
- B-02 may sit inside wgpu-core and not be fixable by us. Then the remedy is fewer draws (S4), and the backlog entry records the per-draw cost as a known constant.

## S1: See every cost

**Goal:** the bench reports every cost a later sprint will be judged on, so no sprint builds its own instrument mid-way.

**Scope:**
- Roadmap 2: `vx bench --windowed` with a real swapchain, vsync off, frame-pacing p50/p99/max and frame-time variance.
- Roadmap 3: one GPU timestamp metric per pass, with a naming convention (`gpu_<pass>_ms`). Every pass added in S4–S9 adds its metric in the same PR.
- Roadmap 4: peak RSS (`VmHWM` / `PeakWorkingSetSize`) and wgpu allocator reports (VRAM by buffer class).
- Roadmap 5: Tracy behind a feature flag via `profiling`, zones in gen, mesh, upload, encode, submit, frame.
- Roadmap 6: instruction-count benches in CI (gungraun) for the mesher, generator and decode.
- B-02 fix, if S0 found one we control.

**Exit gates:**
- With the Tracy feature off, `bench-compare` against the S1 start baseline is within noise on every metric.
- Windowed bench: two runs recorded on the RTX 2060, and the repeatability threshold for later gates set from their spread. No threshold is assumed before those runs exist.
- CI instruction counts repeat within 1% on an unchanged commit (target).
- If B-02 was fixed: the per-draw slope from S0's sweep drops, and frame CPU encode at `default` is faster outside noise. `far` encode is not the gate: its repeat spread reached 39% (measured).

**Dependencies:** S0's report schema.

**Risks:**
- Windowed timing on a Windows laptop includes compositor and power-state effects. Record present mode and power plan in the environment block; compare only same-mode runs.
- gungraun needs Valgrind, so instruction counts run on Linux CI only. That is enough: they guard CPU code, not the GPU.

## S2: Fix the chunk shape, double throughput

**Goal:** settle the chunk size and the neighbour rule that streaming, culling and LOD will all be built on, and make the CPU produce chunks fast enough that S3 is limited by budget choices, not by generation.

**Now (measured, RTX 2060 machine, `default`, 12 threads):** world generation 260.5–267.4 ms for 6,125 chunks, meshing 103.2–109.8 ms for 2,826 meshed chunks, about 23k generated and 26–27k meshed chunks/s (computed).

**Scope, in this order:**
1. ADR-0003: prototype 62³ data padded to 64³ against 32³ behind `CHUNK_SIZE`, judged on mesh µs per voxel, draws and frame CPU encode at the same view distance. The rest of S2 builds on the winner.
2. AO probe (B-09 part 1): run the AO-safe merge offline on `default` and `far` and count quads; also compute how much later chunks become meshable at the streaming edge when 26 neighbours are required instead of 6. This sets ADR-0006's neighbour rule (6 or 26) for S3. A quad increase under ~30% (estimate threshold, B-09) keeps vertex AO as the plan.
3. B-03: decode straight into the padded input. If the probe chose vertex AO, the gather also fills the 12 edge rows and 8 corner voxels, so it is not rewritten later.
4. SIMD terrain noise (roadmap 13, ADR-0009): hand-written `wide::f32x8` fBm against fastnoise-lite. fastnoise2 (backlog Watch) is the comparison point only if the hand-written version falls short.

**Exit gates:**
- ADR-0003 Accepted or Rejected with its numbers.
- World generation at `default` at most half the S2 start baseline (target), `mesh_hash` unchanged within the chosen chunk size.
- Divan `gather_and_mesh_surface_chunk` faster outside noise; meshing ms at `far` faster on two agreeing runs.
- B-09 records the quad increase and the extra streaming-edge delay; ADR-0006 is amended if the rule becomes 26 neighbours.

**Dependencies:** S1 instruction-count benches (they catch regressions the laptop noise hides).

**Risks:**
- A chunk-size switch touches storage, mesher and every bench hash. Prototype behind the constant and the existing tests; merge only the winning side, and re-baseline once in its own PR.
- SIMD noise must produce identical heights, or `mesh_hash` changes. If exact equality is impossible (FMA, octave order), declare a new terrain version and re-baseline once.
- AVX2 is not on every CPU. `wide` falls back to scalar lanes; bench both paths.

## S3: Stream without hitches

**Goal:** fly at sprint speed through an endless world with no frame over budget.

**Scope:**
- Roadmap 7: streaming around the camera on `ChunkWindow`, dedicated pool, priority heap, bounded in-flight jobs, generation-counter cancellation. ADR-0005 goes from Proposed to Accepted or Rejected. The neighbour rule comes from S2.
- Roadmap 8 and B-05: `offset-allocator` suballocation, staged uploads with a byte and time budget, frees only through the allocator.
- B-04: forward-frustum priority, teleport boost, slot-epoch staleness, retire-frame frees.
- `fly-fast` finalised in two modes. **Frame-stepped** (a fixed number of job completions per frame, deterministic, compare-grade) is what `bench-compare` judges. **Wall-clock** (real worker timing) is recorded for p99 and max but only informs.
- New bench metrics: time-to-fill-view, wasted-job share, upload ms per frame, largest free block after the flight.
- Profile the dense-chunk mesher path with Tracy now that streaming feeds it (known issue in `docs/ROADMAP.md`).

**Exit gates (`fly-fast`, radius 16, RTX 2060):**
- Wall-clock mode: no frame above 16 ms, p99 at or below 8.3 ms (target, 120 Hz) over 60 s at sprint speed. For reference, the offscreen `default` frame wall p99 is 3.09 ms (measured, static camera).
- Frame-stepped mode: identical content hash across repeats after the flight.
- Wasted-job share under 10% (target); time-to-fill-view after a teleport recorded.
- Upload stays inside its per-frame budget on every frame; largest free block at least 50% of free bytes after the flight (target).
- `default` static frame within noise of the S3 start baseline.

**Dependencies:** S1 windowed bench and VRAM report, S2 chunk shape and throughput.

**Risks:**
- Highest-risk sprint: Decaf streamed on the main thread and Lemon on predicted borders (`docs/HISTORY.md`). The main thread only pops results, uploads and draws; Tracy zones prove it.
- Rayon has no priorities or cancellation (ADR-0005). If the heap + epoch scheme shows overhead in Tracy, a custom pool needs its own ADR.

## S4: GPU decides what to draw

**Goal:** the CPU stops paying per chunk per frame; the GPU culls and compacts draws.

**Scope:**
- Roadmap 9, B-06, ADR-0010 phase 1: compute frustum + per-direction face cull, `atomicAdd` compaction, `multi_draw_indirect_count` with a zero-instance fallback, `gpu_cull_ms`.
- The GPU counter ladder from B-01 (`chunks_frustum`, `draws_emitted`, `quads_submitted`, read back 2–3 frames late) and the culling-off vs culling-on image identity test.
- Roadmap 10, ADR-0004: 8-byte vs 4-byte quads, measured on terrain-pass GPU ms at `far`, but only if S2 chose SSAO. If S2 chose vertex AO, the 4-byte format cannot hold the AO bits; ADR-0004 then records the 4-byte variant as rejected by that dependency, not by a measurement.

**Exit gates:**
- `quads_submitted` drops in `default` and `far`; images identical with culling on and off.
- Cull-pass ms plus terrain-pass ms is lower than the S4 start terrain-pass ms at `far` on two agreeing runs (net win, B-01 rule).
- Frame CPU encode no longer grows with draw count (per-draw slope from S0's sweep near zero).
- Both render tiers benched: with `MULTI_DRAW_INDIRECT_COUNT` (RTX 2060) and the fallback (forced off), so a tier we do not ship on still gets a number.

**Dependencies:** S3 (draw list owned by the streaming allocator), S2 (AO decision for ADR-0004).

**Risks:** if the terrain pass is not vertex-bound, face culling saves little GPU time. Then the gate is the CPU and draw-count win, and mesh shaders (backlog Watch) stay parked.

## S5: See to the horizon

**Goal:** the largest view distance at the frame budget, without seams or pop.

**Scope:** B-08 (Cubyz-style rings, real-child downsample, border faces instead of skirts, hand-off when all 8 children are meshed), the ADR-0011 prototype comparison, roadmap 18 camera-relative rendering. A new ADR amends ADR-0006 for LOD borders: the finer chunk meshes its border against the coarser neighbour's real voxels, which the current "real same-level neighbours only" rule does not cover. New `horizon` scenario: fixed camera, LOD levels 0–5, written before the first LOD code.

**Exit gates:**
- Reach at least 8× level 0's radius at frame GPU ≤ 4 ms and VRAM ≤ 1 GiB on the RTX 2060 (targets).
- Seam screenshot test: zero sky pixels inside terrain silhouettes at LOD borders.
- `fly-fast` still meets S3's gates with LOD on.

**Dependencies:** S3 streaming, S4 culling, S2 chunk size.

**Risks:** Lemon tried LOD in debug builds and deleted it two days later (`docs/HISTORY.md`): release builds only, `horizon` scenario first. Hierarchical traversal (B-17) is the fallback if rings miss the target.

## S6: A world you can change

**Goal:** editing, saving and caves, each with a cost that the bench sees.

**Scope:** B-13 batched edits with neighbour remesh (6 or 26 neighbours, per S2's rule), B-12 persistence (edited chunks only, lz4 vs zstd-3), B-11 3D noise for caves (coarse grid, interval bounds, direct palette writes) as a new terrain version, B-14 attribute channels (prepares light). New scenarios: `edit` (scripted edit sequence), `save-load`, `caves` (cave terrain version, frame-stepped streaming).

**Exit gates:**
- Edit-to-visible within 2 frames (target); one remesh per touched chunk per frame; edited LOD levels refresh.
- Content hash identical after a save-load round trip; load faster than regenerate for edited chunks.
- `caves` generation per chunk at most 1.5× S2's heightmap generation (target); uniform-chunk ratio recorded.
- `default` and `far` (heightmap terrain) within noise of the S6 start baseline.

**Dependencies:** S3 (remesh runs as streaming jobs), S5 (edits invalidate LOD levels).

**Risks:** caves add 3D noise cost to the path S2 just optimised. The interval bounds (B-11 step 2) are part of the scope, not a later optimisation.

## S7: Cull what is hidden

**Goal:** remove chunks that are inside the frustum but hidden behind terrain, where real caves and ridges exist.

**Scope:** B-07, candidates in this order: (a) visibility graph (CPU, testable headless, helps enclosed air), (b) occluder quads with a small CPU depth buffer, (c) two-phase Hi-Z (ADR-0010 phase 2). The first candidate that wins is kept; the others are prototyped only if it fails. Counters `chunks_graph` or `chunks_hiz` are added with their candidate.

**Entry condition:** S4's counters show at least 20% of frustum-passing chunks without visible pixels in `cave`, `mountain` or `caves` (target threshold, set from B-07's quoted 5–25% extra culled). Below that, the sprint is skipped and B-07 stays in the backlog with the numbers.

**Exit gates:** net frame ms falls in at least one of `cave`, `mountain`, `caves` and regresses in none (B-07 rule); images identical with the pass on and off.

**Dependencies:** S4 counters, S6 caves and edits (edits invalidate graph connectivity bits).

**Risks:** Hi-Z readback latency is what made Decaf disable it (`docs/HISTORY.md`). Candidate (c) stays GPU-only, with no CPU readback in the frame.

## S8: Light and depth

**Goal:** the biggest visual step per millisecond: ambient occlusion and propagated light. From here, gates use `--shading textured` as the shipped look, with flat kept as the reference column.

**Scope:** B-09 vertex AO (26-neighbour gather, AO-safe merge, 2-bit corners) or SSAO, per S2's probe; B-15 light propagation into B-14's light channel (sky and block light, per-vertex sampling); B-10 T-junction seam check on the RTX 2060 now that AO splits and textures are in, fixed by dilation only if visible.

**Exit gates:**
- AO: opaque-pass GPU ms increase at `far` at most 0.3 ms (target), mesh µs per chunk at most +15% (quoted range for block-mesh-bgm, B-09), screenshot pair recorded.
- Light: propagation µs per chunk recorded against Starlight's 174 µs torch baseline (quoted, B-15); light heap MiB at `far` recorded; edit-to-visible still within 2 frames.
- Seams: sky-pixel count inside terrain silhouettes is zero on the fixed test view.

**Dependencies:** S2 probe, S6 channels and edits.

**Risks:** vertex AO splits greedy quads; if the real increase exceeds the probe, SSAO is the fallback and the measured numbers go into B-09.

## S9: Finish the frame

**Goal:** the remaining frame features, each inside its own GPU budget.

**Scope:** transparent pass (water, glass, alpha-tested leaves), cascaded shadows with per-cascade GPU culling and cached far cascades, screen-space godrays (radial blur), and a B-16 spike (ray-marched AO or contact shadows through occupancy) judged against SSAO + CSM.

**Exit gates (targets, RTX 2060, 1080p, `default`, textured):** shadows ≤ 1.5 ms GPU, transparent pass ≤ 0.5 ms, godrays ≤ 0.3 ms, whole frame wall ≤ 4 ms. B-16 is adopted only if it beats SSAO + CSM on GPU ms at comparable quality.

**Dependencies:** S4 culling (per-cascade culling reuses it), S8 lighting.

**Risks:** shadows redraw geometry per cascade, and Lemon drew the union of chunks into every cascade (`docs/HISTORY.md`). Per-cascade culling is part of the scope, not an optimisation for later.

## Long-term targets

All values in the Target column are **targets** on `noah-msi-rtx2060` at 1920×1080, flat shading unless stated. The Now column is measured (runs 1 and 2, `docs/PERFORMANCE.md`) or computed where said. The budget in `docs/PERFORMANCE.md` (frame ≤ 4 ms CPU, ≤ 8 ms GPU, no frame above 16 ms while streaming) stays the hard ceiling.

| Metric | Now | Target | Sprint |
|---|---|---|---|
| Frame CPU encode, `far` | 1.34–1.74 ms (36,275 draws) | ≤ 0.2 ms, no growth with draw count | S1, S4 |
| GPU terrain pass, `far` | 1.94–1.98 ms | ≤ 1.0 ms including the cull pass | S4 |
| GPU terrain pass at radius 48 | not measured | ≤ 2.5 ms including the cull pass | S4–S5 |
| Frame wall, `default` | 1.25–1.30 ms | ≤ 0.8 ms flat; ≤ 4 ms textured with every S8–S9 feature | S4, S9 |
| World generation, `default` | 260.5–267.4 ms | ≤ 50% of the S2 start baseline | S2 |
| Meshing, `default` | 103.2–109.8 ms | ≤ 80 ms | S2 |
| Frame p99 while flying (`fly-fast`, radius 16) | not measured | ≤ 8.3 ms, max ≤ 16 ms | S3 |
| Time to fill the view after a teleport, radius 16 | not measured | ≤ 0.5 s | S3 |
| Reach with LOD at frame GPU ≤ 4 ms | no LOD | ≥ 8 km (≥ 256 chunk radius equivalent at 32³) | S5 |
| VRAM at maximum reach | not measured | ≤ 1 GiB | S5 |
| Voxel heap per generated chunk, `far` | 2.7 KiB (computed: 58.6 MiB / 22,445) | ≤ 3 KiB, light channels included | S6, S8 |
| Edit-to-visible latency | no editing | ≤ 2 frames | S6 |
| Process resident memory, `far` | 131–132 MiB | ≤ 256 MiB at radius 32 with LOD and light | S8 |

A target that turns out wrong is changed in this table with the date and the measurement that showed it, never silently.

## In-flight work at writing time (2026-11-03)

- Thread "Work through the top backlog items": B-01 scenarios and counters, then B-02. Mapped to S0.
- Thread "Design the voxel texture art concept": textures phase 2 and the `--shading` switch. Mapped to S0; the GPU decision for ADR-0014 needs Noah's machine.
- S1 can start once S0's report schema is merged; it does not need ADR-0014 decided.
