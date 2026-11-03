# World generation research (2026-11)

> Snapshot, 2026-11-03. Candidates from these reports are ranked in `docs/BACKLOG.md` (B-18 to B-25, plus additions to B-11, Watch and Not adopted). The staged plan built on them is `docs/WORLDGEN.md`; the architecture decision is ADR-0018 in `docs/DECISIONS.md`.

Question: how do Minecraft 1.18+, research papers and other engines generate convincing terrain, and which of those techniques fit Espresso's unbounded world (ADR-0016): deterministic from seed and integer coordinates, chunk-local or column-local, precise far from the origin, and able to generate far LOD levels directly at 2^L block spacing?

Tags and number labels follow `docs/DOC-GUIDELINES.md` §4: **[S]** seen this session (URL or repo given), **[U]** unverified. No number in these reports was measured on Espresso.

| Report | Topic |
|---|---|
| [minecraft.md](minecraft.md) | Minecraft 1.18+ noise router, spline shaping, multi-noise biomes, noise caves, aquifers; Tectonic, Distant Horizons, C2ME, Noisium (vanilla JSON and mod source read) |
| [erosion-rivers-papers.md](erosion-rivers-papers.md) | Erosion approximations (IQ damped fBm, swiss turbulence, Johansen erosion filter), river networks (Dendry, Génevaux), stream-power simulation, diffusion models |
| [other-engines.md](other-engines.md) | Cubyz (closest match, source read), Luanti v7, Veloren, Hytale, Distant Horizons, FastNoise2, Vintage Story, No Man's Sky |
| [noise-performance-lod.md](noise-performance-lod.md) | Noise cost tables, integer-lattice determinism, octave truncation per LOD with an error bound, range bounds, coarse 3D lattice, SIMD options on stable Rust |

## Headline findings

1. **Good terrain is mostly 2D work.** Minecraft's 1.18 look comes from three low-frequency climate noises (continentalness, erosion, ridges) shaped by cubic splines into a surface offset, a factor and a jaggedness term [S, vanilla JSON]. Cubyz, Luanti v7 and Hytale also compute height per column and use 3D noise only near the surface or for caves [S]. Espresso can get most of the visual gain while staying column-first.
2. **The best erosion look that is still a pure function is a filter, not a simulation.** Derivative-damped fBm (IQ) and Johansen's erosion filter with Phacelle noise (2025–2026) are per-point, deterministic and octave-truncatable [S]. Every stream-power or particle simulation reviewed is global over a finite grid (Cordonnier 2016, Tzathas 2024, Mei 2007, Veloren) [S]; at best they could bake an offline macro atlas.
3. **Rivers are the hardest local feature.** Cheap: Tectonic-style rivers as a band of the folded ridge noise [S]. Better: a jittered-grid river graph with bounded neighbourhoods per level (own design built on Dendry's idea, [U]). Dendry itself is local but quoted at 12–46 s per 512² samples (quoted), too slow per column.
4. **LOD-direct generation is solved by construction if the noise is built for it.** With power-of-two wavelengths on an integer lattice, level L drops the octaves whose wavelength is under two voxels; the coarse octaves are unchanged, so far terrain stays within the dropped amplitude sum of level 0: about 0.19, 0.29 and 0.34 voxel at LOD 3, 4 and 5 for today's settings (computed, `noise-performance-lod.md` §3). Cubyz does exactly this with per-LOD map fragments up to 32 blocks per voxel [S].
5. **Chunk-local decoration needs hash-cell placement, not Minecraft's 3x3 neighbourhood.** Veloren's jittered structure grid and Cubyz's per-chunk structure lists place cross-chunk features from a hash of the cell, with padding equal to the largest feature radius [S].
6. **Determinism across SIMD widths must be enforced by test.** simdnoise documents that its scalar and SIMD outputs differ "within tolerance" [S], which flips block heights. Espresso's kernel avoids FMA and table gathers and gates on identical content hashes across scalar, SSE2 and AVX2.

## Gaps

- minecraft.wiki refused automated fetches, so the density-function type semantics, aquifer algorithm and biome RTree are [U]; Kniberg's 2022 talk was not fetched.
- Shadertoy pages could not be fetched; the erosion filter was read in a WGSL/Rust port instead.
- Terra, Voxy, Vintage Story source and No Man's Sky primary talks were not read.
- No source gives per-chunk generation timings comparable to ours, except Hytale's quoted Y-sampling stage times without a machine.
