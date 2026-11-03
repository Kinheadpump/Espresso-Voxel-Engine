# World generation plan

The staged plan for Espresso's second terrain generator ("terrain v2"): what it computes, in which order the stages land, which sprint carries each stage, and the benchmark gate that keeps or reverts it. Written 2026-11-03 against `main` at `6b20c4e`, with the S2 work in PR #17 assumed merged. The evidence is in `docs/research/worldgen-2026-11/`, the ranking in `docs/BACKLOG.md` (B-11, B-18 to B-25), the architecture decision in ADR-0017. Numbers follow `docs/DOC-GUIDELINES.md` §4.

## Goal

A world that looks designed at every scale (continents, mountain ranges with eroded valleys, rivers, biomes, caves, trees) while keeping the rules of ADR-0016: unbounded, deterministic from seed and integer coordinates, generated chunk by chunk without reading neighbour chunks, identical in quality a million blocks from the origin, and able to produce far LOD levels and the horizon tier directly from the same functions at coarse spacing.

## Now

- `crates/vx-world/src/terrain.rs` (terrain v1): one 2D OpenSimplex2 fBm heightmap (5 octaves, wavelength 250 blocks, amplitude 48) through fastnoise-lite with `f32` block coordinates, grass/dirt/stone layers, `Uniform` air or stone above and below the column's height range.
- World generation at `default` is 70.6–73.5 ms for 6,125 chunks on `noah-msi-rtx2060`, down from 152.4–160.3 ms in the same session, after the S2 column-shared height noise (measured, S2 gate, `docs/PERFORMANCE.md` in PR #17). That is about 0.14 ms of core time per chunk (computed: 72 ms × 12 threads / 6,125).
- No biomes, water, caves, rivers or decoration. Coordinates go into the noise as `f32`, which ADR-0016 says must change.

## Architecture (ADR-0017)

Every stage is a pure function of `(seed, integer coordinates, LOD level)`. 2D stages run once per chunk column per LOD level and are cached with it; 3D work runs only for chunks whose bounds straddle the surface or that can contain caves.

| Stage | Computes | Per | LOD rule | Research |
|---|---|---|---|---|
| W0 Lattice noise | Integer-lattice gradient/value noise, power-of-two wavelengths, own integer hash, per-octave seed mix, SoA kernel over 1,024 columns, scalar = SSE2 = AVX2 bit for bit | sample | Defines the prefix property every later stage relies on | `noise-performance-lod.md` §1, §2, §7 |
| W1 Climate and shape | Continentalness, erosion, ridges (folded into peaks and valleys), temperature, humidity on a coarse 2D lattice; cubic splines map them to surface offset, factor and jaggedness | climate cell (32–64 blocks), interpolated | Coarse by construction; identical at every LOD | `minecraft.md` §2, §6; `other-engines.md` §1 |
| W2 Height and bounds | Height = spline offset + mountain fBm (damped or swiss, with analytic gradient); per-sub-block min/max for classification | column | Drop octaves with wavelength < 2 voxels; error bound = dropped amplitude sum | `noise-performance-lod.md` §3, §4; `erosion-rivers-papers.md` §1 |
| W3 Erosion and rivers | Johansen-style erosion filter on height and gradient; rivers first as folded-ridge bands, then a jittered-grid river graph | column | Filter octaves and river levels truncate like W2 | `erosion-rivers-papers.md` §1.4, §2.4 |
| W4 Surface rules and biomes | Biome from a climate lookup table (no RTree); top/filler/stone by depth, slope and altitude; sea-level water | column | At LOD ≥ 1 only the top block matters | `minecraft.md` §4; `other-engines.md` §9 |
| W5 3D density | Overhangs (jaggedness band) and caves (cheese, spaghetti) on a 4×8×4 block lattice in absolute coordinates, trilinear, written straight into palette storage | chunk, surface band and cave band only | Off at LOD ≥ 2; columns solid up to the W2/W3 height | `minecraft.md` §2–3; B-11 |
| W6 Decoration | Trees and later structures from jittered hash cells of size ≥ the largest feature radius; each chunk stamps only its own part | chunk | Exact placement up to LOD 2, per-column hash probability beyond, none in the horizon tier | `other-engines.md` §1, §3, §9 |
| Horizon | W1–W3 sampled at 64–256 block spacing into a heightmap clipmap beyond the last voxel ring | tile | 2–4 octaves left | `noise-performance-lod.md` §6; ADR-0011 |

Data layout: SoA `f32` rows, x contiguous, 32-byte aligned; positions as integer cell plus exact `f32` fraction; a `u32` solid bitmask per column per chunk where it feeds the mesher; an LRU of `(tile, LOD)` 2D tiles. No allocation per chunk (workers reuse scratch, `AGENTS.md`).

## Staging

Terrain v2 lives beside v1 behind `--terrain v1|v2` in `vx`, `vx bench` and `--screenshot`. v1 stays the default, so every S3–S5 GPU gate keeps its v1 content hashes (sprint rule 3). v2 becomes the default at the S6 start baseline, where S6 already re-baselines for caves. Until then v2 work is CPU-only in `vx-world` and runs as a parallel track that never touches render code.

| Step | Items | Sprint | Gate (all on `noah-msi-rtx2060`, same session, `vx bench-compare`; budgets are **targets**) |
|---|---|---|---|
| 1. Lattice and tooling | B-18; `--terrain` switch; `vx worldmap` (top-down height and biome PNG of a 16 km square, for review); divan and gungraun benches per stage | S3 (implements S3's ADR-0016 noise-precision item) | v2 with v1-equivalent settings: world generation at `default` within noise of v1 or faster; identical content hash for scalar, SSE2 and AVX2 builds; far-origin test at block 2^30 against an `f64` reference |
| 2. Shape and classification | B-19 climate + splines + folded ridges; B-20 part 1 per-octave range bounds | S3–S4 track | v2 world generation at `default` ≤ 1.25× v1 (same session); uniform-chunk ratio at `default` and `far` recorded and ≥ v1's |
| 3. Surface and biomes | B-25 surface rules, biome lookup, water at sea level, new block ids with textures (ADR-0014 style) | S4 track | v2 ≤ 1.35× v1 world generation; `vx worldmap` and three fixed `--screenshot` views committed to `docs/worldgen/` for Noah's review |
| 4. LOD-direct sampling | B-20 part 2: octave truncation per LOD, tail-bound test, horizon height function | S5 (before the first LOD ring) | Unit test: \|H_L − H_0\| ≤ tail(L) over 10⁷ samples; 2D cost per far chunk at LOD ≥ 3 ≤ 60% of level 0 (target from `noise-performance-lod.md` §8) |
| 5. Erosion and rivers | B-21 erosion filter; B-22 ridge-band rivers, then the river graph if the bands look wrong | S5–S6 track | v2 ≤ 1.6× v1 world generation; river test: no uphill step along any river centreline over a 16 km square; chunk-seam test (heights equal across every chunk border) |
| 6. Caves and overhangs | B-11 (cheese and spaghetti caves, coarse lattice, interval bounds, direct palette writes), B-23 overhangs | S6 | S6's `caves` gate; v2 with caves ≤ 2.0× v1 world generation at `default` (S2-end v1 as the reference) |
| 7. Trees | B-24 hash-cell decoration with per-chunk feature lists | S6 stretch, else S7 | Zero cut-off trees across chunk borders over 10⁴ chunks; decoration ≤ 10% of v2 world generation |

Each step is a separate PR with its own `bench-compare` table in `docs/PERFORMANCE.md`. A step that misses its budget is reworked or put behind a switch that the bench defaults to off (sprint rule 5). The cumulative budgets assume S2-end v1 at 70.6–73.5 ms (measured); they are re-checked once step 1 measures what v2 with v1-equivalent settings costs.

## Visual review

Speed gates cannot judge whether terrain looks good. From step 3 on, every step that changes the look adds the same three fixed views (`--screenshot`, textured shading, seed 1337: valley, ridge, coast) and a `vx worldmap` PNG to `docs/worldgen/`, so Noah can compare steps side by side. A look change merges only after Noah has seen the images; speed-only changes follow the normal rules.

## Risks

- **Budget creep.** Minecraft-style per-voxel density costs about 14k–26k octave evaluations per chunk (estimate, `minecraft.md` §9), several times today's generator. The plan keeps 3D work to the surface and cave bands and gates every step on world generation ms.
- **Determinism.** A kernel that differs between SIMD widths changes `mesh_hash` per machine. Step 1 makes the cross-path hash test a CI test before any terrain shape changes.
- **Licence.** The reference erosion filter code is MPL-2.0 (`erosion-rivers-papers.md` §1.4). B-21 is a clean-room implementation from the published description; the ADR records it.
- **Rivers.** A local river graph may not guarantee downhill flow everywhere. Ridge-band rivers ship first; the graph is a prototype with a pass/fail test.
- **Conflict with GPU sprints.** v2 stays opt-in until S6, so S3–S5 gates compare v1 against v1.
