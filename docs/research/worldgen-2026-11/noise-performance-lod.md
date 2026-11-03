# Fast, deterministic and LOD-correct noise evaluation

> Snapshot, 2026-11-03.

Question: how do we make the Espresso terrain generator fast (SIMD, column batches, bounds), bit-reproducible for an infinite world (ADR-0016), and LOD-correct, so that a far LOD level generated directly from noise at 2^L block spacing has the same silhouette as level 0 up to a known error? Read by whoever designs the S2/S3 generator.

Tags: **[S]** seen this session (page fetched or source read, URL given). **[U]** unverified: background knowledge or own reasoning. Numbers are labelled **quoted** (copied from a source, not re-run), **computed** (arithmetic from stated inputs), **estimate** (a measurement should replace it) or **target**. No number here was **measured** in this repo.

Related docs, not repeated here: `docs/research/rust-tooling.md` (local micro-bench of noise crates, SIMD crate list, std::simd status), `docs/research/recent-2024-2026/lod.md` (Cubyz/Voxy/DH LOD structure, mip rule, seams), `docs/research/recent-2024-2026/storage-gen-streaming.md`, `docs/BACKLOG.md` B-11.

## Summary

| # | Candidate | Source / year | Fit for Espresso | Cost per chunk / complexity | LOD-friendliness | Verdict |
|---|---|---|---|---|---|---|
| 1 | Exact integer-lattice noise: power-of-two wavelengths, integer cell + exact f32 fraction, own integer hash, per-octave seed mix | FastNoiseLite source [S]; Eiserloh GDC 2017 [S, README only] | Required by ADR-0016 | Same ALU as FNL; one shift and mask instead of float floor | Neutral, but makes coarse sample points exact | **Adopt-candidate** |
| 2 | Own SoA column-batch kernel (1,024 columns, lane-independent loops), no FMA, no table gathers | simdnoise, FastNoise2 designs [S] | Fits worker pool and ChunkWindow | 2D 5 octaves about 15–25 ns per column (target) | Neutral | **Adopt-candidate** (FastNoise2 FFI: **Watch** as speed ceiling only) |
| 3 | Prefix-causal octave truncation per LOD (drop octaves with wavelength below 2 voxels) with tail bound | Nyquist; IQ fBm [S]; Lagae survey [S] | Direct far-LOD generation | Fewer octaves at far LOD: 2D cost drops 20–60% (computed) | Error at most about 0.35 voxel (computed) | **Adopt-candidate** |
| 4 | Per-octave range bounds: `min(2a_k, a_k·Lip·f_k·r)` summed, for chunk classification and early-out | Hart sphere tracing, Galin course notes [S]; Voxy/Voxel Tools range analysis (see B-11) | Extends the uniform-chunk shortcut | 25 column samples + O(K) arithmetic per chunk column (estimate) | Works at every LOD | **Adopt-candidate** |
| 5 | Absolute-lattice coarse grids as the definition of the field (3D density at 4×8×4 blocks, climate 2D at 8 blocks) | Minecraft NoiseChunk (via B-11), Hytale notes | Cuts 3D evaluations about 81× (computed) | Interpolation error rule in section 5 | Consistent if the same lattice is used at every LOD | **Adopt-candidate** (3D only for wavelength ≥ 64 blocks) |
| 6 | Analytic derivatives for IQ-style erosion damping, 2D heights only | IQ "morenoise" [S] | Gives mountain character without simulation | Roughly +50% ALU per octave (estimate) | Safe if prefix-causal (section 3) | **Prototype** |
| 7 | GPU compute for the horizon heightmap clipmap only | GPU Gems 2 ch. 2 [S] | Data stays on GPU, no readback | Not measured | Natural (per-level tiles) | **Watch** (prototype after CPU tier exists) |
| 8 | Cellular/Worley noise | FN README table [S] | Biome cells at low resolution only | 3–10× simplex cost (quoted ratios) | Good at coarse spacing | **Watch** |
| 9 | Surface nets / Transvoxel for far LOD | `lod.md` | Engine is blocky | n/a | Not needed | **Reject** |
| 10 | Dispatch: `wide` / `multiversion` / `pulp` / `std::simd` | READMEs [S] | See section 7 | n/a | n/a | `std::simd` **Reject** (nightly), rest see section 7 |

## 1. Noise primitives and their cost

### 1.1 Published numbers

Source for the whole table: FastNoise2 README, benchmark in million points per second, Intel i7-7820X at 4.9 GHz, Windows 10, clang-cl 10, single call `-m64 /O2` [S] https://github.com/Auburn/FastNoise2 and https://github.com/Auburn/FastNoiseLite. The source gives Mpts/s only. ns per point is **computed** as 1000 / Mpts/s. One octave, no fractal loop.

3D:

| Library | Value | Perlin | (Open)Simplex | Cellular |
|---|---|---|---|---|
| FastNoise Lite (scalar) | 15.6 ns | 20.9 ns | 27.2 ns | 80.1 ns |
| FastNoise2 AVX2 | 2.0 ns | 3.8 ns | 3.7 ns | 19.1 ns |
| libnoise | n/a | 36.6 ns | n/a | 1,538 ns |

2D:

| Library | Value | Perlin | Simplex | Cellular |
|---|---|---|---|---|
| FastNoise Lite (scalar) | 8.8 ns | 10.8 ns | 14.0 ns | 25.5 ns |
| FastNoise2 AVX2 | 1.3 ns | 1.6 ns | 2.1 ns | 5.1 ns |

Derived ratios (**computed**): FNL 3D/2D is 1.8× (value), 1.9× (Perlin), 1.9× (simplex). FN2 3D/2D is 1.6×, 2.4×, 1.7×. FN2 AVX2 over FNL scalar is 4.7–8.5× in 2D and 4.1–8.2× in 3D for value/Perlin/simplex, 4.2–5.0× for cellular. The README marks the FNL 3D simplex value with an asterisk without explanation [S]. The README says nothing on OpenSimplex2S speed [S]; no source I fetched has numbers for it.

simdnoise (Rust, i7-6700 at 3.4 GHz, Criterion, single thread) [S] https://github.com/verpeteren/rust-simd-noise:

| Workload | scalar | SSE2 | SSE4.1 | AVX2 |
|---|---|---|---|---|
| 2D 3840×2160 fBm, 3 octaves, ns per sample (computed from block times 888/225/186/108 ms) | 107 | 27 | 22 | 13 |
| 3D 128³ noise, ns per sample (computed from 1,400/128/94/47 ms) | 668 | 61 | 45 | 22 |

AVX2 over scalar is 8.2× in 2D and 30× in 3D (computed); the 3D scalar number looks like a poorly optimised scalar path rather than a property of the algorithm, so use the SSE2 or AVX2 column as the SIMD reference. The 2D AVX2 figure is about 4.3 ns per octave (computed).

Local numbers (quoted from `docs/research/rust-tooling.md`, divan, 4 cores, baseline x86-64 target, noisy VM, 4-octave fBm): fastnoise-lite Perlin 2D 44 ns/sample, simdnoise 2D 14 ns/sample; 3D 32³: simdnoise 22 ns, fastnoise-lite Perlin 77 ns, OpenSimplex2 124 ns, noise-rs Perlin f64 121 ns. Direction agrees with the README table: in 3D, OpenSimplex2 in FNL is slower than Perlin.

### 1.2 What it means for Espresso

- 2D is about half the cost of 3D for every type (computed ratios above). Terrain shape belongs in 2D (one sample per column, cached per column stack), 3D only where overhangs or caves need it.
- Value noise is cheapest but has visible axis-aligned lattice structure and a weak band limit [U]. Perlin and OpenSimplex2 cost 1.2–1.9× value in 2D (computed). Keep OpenSimplex2 for the height field (already in use) unless a measurement shows Perlin's lower cost wins; the choice is a visual one, not a cost one, at the SIMD level (FN2: 1.6 vs 2.1 ns).
- Cellular is 3–5× simplex in FN2, 2–3× in FNL, and about 40× in libnoise 3D (computed). Use it only at coarse resolution (biome cells, 2D, wavelength ≥ 256 blocks).
- Cost per sample is dominated by the octave count K. At the current default K=5 the 2D height costs 5 evaluations.

### 1.3 Analytic derivatives

- Value noise with a smooth fade has closed-form derivatives from the same lattice coefficients; one `noised()` call returns value and gradient [S] https://iquilezles.org/articles/morenoise/. The author states central differences are about 5× slower than the analytic form [S, author claim].
- Erosion-like terrain: accumulate the derivative `d` over octaves and damp each octave: `a += b·n.x / (1 + dot(d,d))`, `b` halves per octave [S, same page]. Steep areas get less detail, flat areas stay flat. Cubic fade shows artefacts (discontinuous second derivative); use quintic fade [S].
- Extra cost: the derivative needs `u'(x)` (one multiply-add chain), two more lattice combinations and a divide per octave. Estimate: +40–60% ALU per octave, +1 division (a reciprocal-multiply is not allowed if it must be bit-exact; see section 2.3). Measurement that replaces it: divan `octave_with_gradient` vs `octave` on 1,024 columns.
- Because octave k only depends on octaves 0..k−1 through `d`, the formula is prefix-causal. Dropping high octaves at far LOD leaves the kept octaves exactly equal to level 0 (section 3). Domain warps must also be written prefix-causal.
- Simplex/OpenSimplex2 analytic gradients exist but I did not fetch a source [U]. Start with value or Perlin-with-quintic-fade derivative for the erosion term only if the visual benefit is confirmed.
- Chunk-local note: the derivative term needs no neighbour data (it is the analytic gradient at the same point). Finite differences would need ±1 padded samples and would break column-locality.

## 2. Determinism for an infinite world

### 2.1 Integer hashing

- FastNoiseLite hash: `(seed ^ x_primed ^ y_primed) * 0x27d4eb2d`, where each lattice coordinate was multiplied by a prime (`PRIME_X = 501125321`, `PRIME_Y = 1136930381`, `PRIME_Z = 1720413743`) using `wrapping_mul` [S] https://raw.githubusercontent.com/Auburn/FastNoiseLite/master/Rust/src/lib.rs. Three integer ops plus one multiply per corner. Cheap, and the low bits of one multiply are weak [U]; the gradient index must come from the high bits.
- Eiserloh, "Noise-based RNG" (GDC 2017): a stateless hash from an integer position and a seed to 32 bits; SquirrelNoise5 fixes a repetition at very high positions in v3; the author reports the worst input-to-output bit dependence at 49.99% (ideal 50%) [S, only the crate README: https://docs.rs/crate/squirrel_noise5/latest/source/README.md; I did not see the talk or the constants]. N-D positions are folded into one index first. No speed numbers given. Roughly three xor-shift-multiply rounds [U].
- Alternatives I know but did not fetch: PCG-family hashes (Jarzynski and Olano, JCGT 2020), xxHash32 avalanche [U]. All are 32-bit integer ALU, exact on every CPU and on the GPU.
- Recommendation: a fixed 32-bit mix with two multiplies (xorshift-multiply-xorshift-multiply-xorshift, "lowbias32"-class) applied once per lattice corner, then gradient direction from bits of the hash with arithmetic (no table gather). The cost over FNL's single multiply is about 4 extra integer ops per corner (estimate). Gate: visual autocorrelation image test, plus the speed bench in section 8.

### 2.2 Precision far from the origin

- FNL Rust: input type `Float` is `f32` or `f64` with the `f64` feature; frequency scaling happens in `Float`; the fraction is `(x - i as Float) as f32`, subtracting the integer cell before narrowing; everything after runs in f32; cell index is `i32` via `fast_floor` [S, same file]. So with `f32` input, `x * frequency` is rounded at about 2^-24 relative before the floor: at x = 2^20 blocks (about 1,048 km) the position grid is 0.125 blocks (computed: ulp of 2^20 in f32 is 2^-3). At 64 km = 2^16 the ulp is 2^-7 = 0.0078 blocks (computed). The current `TerrainGenerator` passes `x as f32`, so the height there is quantised in input; it is still deterministic but drifts in character as distance grows.
- Better: make the lattice math exact in integers. Choose the base wavelength as a power of two in blocks (for example 2^8 = 256) and lacunarity 2. Then for integer block coordinate `x` and octave k with cell size 2^(8−k): `cell = x >> (8−k)`, `frac = (x & mask) as f32 * 2^-(8−k)`. The fraction is exact in f32 as long as the cell size is at most 2^24 blocks, and the cell is an `i64` (or `i32` wrapped for hashing). Sample positions at LOD L that are voxel centres use integer coordinates in half-blocks (`2x+1`), keeping everything integer. No rotation matrices between octaves (they would reintroduce irrational floats); use an integer lattice offset per octave, derived from the octave seed, to decorrelate the lattices.
- FastNoise2 and fastnoise-lite do not do this for you [S]; FN2 README says nothing on large coordinates [S].

### 2.3 Bit-reproducibility across CPUs and SIMD paths

- Rust floating point follows IEEE 754-2008; results that are not NaN are required to match; the compiler may not fuse `a*b + c` into an FMA because rounding would change [S] https://hackmd.io/@rust-lang-team/HJNHZOSOC. Rust has no `-ffast-math`, so LLVM does not reassociate float adds, and a loop over independent lanes vectorised to SSE2, AVX2 or scalar gives the same bits for `+ − × ÷ sqrt` [U, reasoning from the rule above; verify with the test below].
- Things that break bit-exactness: explicit `mul_add` (fused only where hardware FMA exists, or via a software fallback; `f32::mul_add` was wrong on `*-pc-windows-gnu` due to a MinGW bug, rust-lang/rust#140515, still open [S] https://github.com/rust-lang/rust/issues/140515); approximate reciprocal and rsqrt instructions (`rcpps` differs between Intel and AMD [U]); libm calls such as `powf`, `exp`, `sin` (platform libm differs [U]); 32-bit x86 without SSE2 (x87 extended precision, called out in the same Rust doc [S]); 32-bit ARM NEON flushes denormals to zero [S].
- simdnoise states that its scalar engine differs from its SIMD engines "within floating-point tolerance" [S]. That is acceptable for textures and unacceptable for block heights: `floor(h * amplitude)` flips a block wherever the error crosses an integer, which shows up as seams between chunks generated on different machines or with different code paths. Same issue for FastNoise2 AVX2 (FMA-based) vs SSE2 [U]; no source states it is bit-exact across levels [S: FN2 README silent].
- Rules for our kernel: no `mul_add`, no division by approximation, no libm in the loop, no table gather; hash and lattice in `u32`/`i32`; use `f32` operations in one fixed order. Convert to integer height with `floor` implemented as `(h + BIG) as i32 - BIG_INT` or `as i32` plus sign fix, identical in all paths.
- Gate (section 8): a content-hash test of 1,000 random chunk columns generated by the scalar, SSE2 and AVX2 builds (forced dispatch) at x/z up to ±2^24 blocks; the hashes must be identical.

### 2.4 Seeding per octave

FNL increments the seed by 1 per octave (`seed += 1`) [S]. With `seed ^ coordinates` hashing, seeds differing only in the low bit give related lattices [U]. Use `seed_k = mix32(seed ^ (k * 0x9E3779B9))` and the per-octave integer offset from 2.2. Each noise layer (height, density, climate, cave) gets its own `layer_id` folded the same way, so adding a layer never changes an existing one.

## 3. LOD-correct sampling

Setting: `H(p) = Σ_{k<K} a_k · n_k(p)`, `a_k = A·G^k`, wavelength `λ_k = λ_0 / 2^k`, `|n_k| ≤ 1`. LOD level L has voxel size `s = 2^L` blocks.

### 3.1 Band-limiting rule

- Nyquist: a voxel grid of spacing s represents wavelengths of at least 2s. Octaves with `λ_k < 2s` alias when point-sampled. IQ says incremental octave construction "allows for an easy way to band limit our shapes for the purposes of LOD and filtering/antialiasing" [S] https://iquilezles.org/articles/fbm/.
- Lagae et al.: frequency clamping (cancelling bands whose frequency is too high) is the standard filter for sums of band-pass noise, works best when the noise is narrowly band-pass, and Perlin noise is "only weakly band-pass, making frequency clamping difficult to tune"; clamping is isotropic [S] https://scribblethink.org/Work/LLCDDELPZ10SPNF.pdf (Sec. 5.2.1). The survey gives no numeric cutoff [S]. So a hard cut with a bound on the removed part is the safest rule for a blocky grid, and a fade of one octave is a visual option, not a correctness one.
- Rule: at LOD L evaluate octaves `k ≤ k_max(L)` where `λ_k ≥ 2s`. Keep one extra octave (`λ_k ≥ s`) as the safe setting if a measurement shows shimmer.

### 3.2 Error bound of truncation (computed example)

Tail of dropped octaves: `|H_L − H_0| ≤ Σ_{k>k_max} a_k` (with `|n| ≤ 1`). Example from the current defaults (amplitude 48, frequency 0.004, 5 octaves, gain 0.5; the 48 blocks normalised by the octave sum 1.9375 as FNL does [U on exact FNL normalisation]): octave amplitudes 24.8, 12.4, 6.2, 3.1, 1.55 blocks; wavelengths 250, 125, 62.5, 31, 15.6 blocks.

| LOD | voxel s | drop wavelengths below | octaves kept | dropped tail bound | in voxels |
|---|---|---|---|---|---|
| 3 | 8 | 16 | 4 | 1.55 | 0.19 |
| 4 | 16 | 32 | 3 | 4.65 | 0.29 |
| 5 | 32 | 64 | 2 | 10.85 | 0.34 |

(computed from the amplitudes above.) The bound is a worst case; with random signs the typical error is a fraction of it. With gain 0.5 the dropped amplitude is a constant fraction of the voxel size at every level, so the silhouette error stays below about a third of a voxel regardless of distance. That is under the quantisation step of the heightmap, so the L0 and LOD silhouettes can differ by at most one voxel step in some columns, and not more.

Two ways to reach "LOD height equals level-0 height":

| Rule | Height at a LOD sample | Cost at far LOD | Property |
|---|---|---|---|
| A. exact point sample | full K octaves at the sample point, equals the level-0 field there exactly | full K octaves per column | Aliasing shimmer from sub-Nyquist octaves, bounded by the tail above |
| B. truncated (Nyquist) | `K_L` octaves at the sample point | 20–60% fewer octave evaluations (computed from the table: 4/5, 3/5, 2/5 kept) | Differs from level 0 by at most the tail; less shimmer |

Recommendation: B, with the tail bound written into a unit test (assert `|H_L − H_0| ≤ tail(L) + 1e-3` over random positions and seeds). It is cheaper and the tail is below one quantisation step. If a measurement on screen shows height pops at LOD transitions, switch the height channel only to A; the cost of A is the same 2D column work as level 0, which is small.

### 3.3 Mean preservation and nonlinear terms

- Octaves of ordinary gradient noise have zero mean, so dropping them keeps the mean height [U]. For ridged or billow octaves (`1 − |n|`, `|n|`) the mean is not zero: replace a dropped octave by `a_k · E[n_k]` (a constant known per noise type, to be calibrated offline by sampling; [U]).
- Terrain shaping (spline on height, `pow`) applied after the sum: truncation error is multiplied by the shaping function's Lipschitz constant. State it in the bound test.
- Thresholded 3D density (caves): dropping octaves changes the variance and hence the fraction of space below the threshold; the mean trick does not fix that [U]. Policy: caves and thin 3D features are not generated at LOD ≥ 2 (features narrower than 2 voxels vanish anyway). The surface height channel is unaffected.
- Domain warp: write it so octave k's warp comes only from octaves below k (prefix-causal). A warp that depends on high octaves breaks equality at the kept octaves.
- IQ-style erosion damping is prefix-causal by construction (section 1.3).

### 3.4 Far-silhouette consistency in the horizon tier

For a heightmap clipmap with spacing `2^L` per level and a footprint of N×N samples per level, each level evaluates only octaves with `λ_k ≥ 2·2^L`. Sample count example (computed): 10 levels × 255² samples = 650,250 samples; at 3 octaves average and an estimated 3 ns per octave-sample that is about 6 ms for a full rebuild (estimate), and an incremental toroidal update touches only the L-shaped strips [S, GPU Gems 2 describes the toroidal update]. This tier needs only the 2D height channel. Cubyz and Distant Horizons: the sources read in `docs/research/recent-2024-2026/lod.md` show Cubyz generates every LOD chunk as a 32³ chunk of doubled voxel size and uses the "pick a real child block" mip rule for edits [S, per that doc]; nothing I fetched this session states whether Cubyz or DH sample the noise directly at coarse spacing [S: Cubyz repo and devlog pages only say "LOD enables far view distances", effective about 5,000 blocks]. The DH GitLab item I fetched is only a user feature request about GPU generation, with no developer answer [S] https://gitlab.com/distant-horizons-team/distant-horizons/-/work_items/937.

### 3.5 Blocky LOD versus surface nets or Transvoxel

Surface nets and Transvoxel need a signed density at each corner and interpolate it; they hide LOD cracks by moving vertices. The engine is blocky with the binary greedy mesher; far LOD chunks are the same 32³ type with doubled voxels (`lod.md`, Cubyz style). For blocky LOD the silhouette consistency only needs the height equality of 3.2. Verdict Reject for smooth meshing; the seam policy stays the border-face variant in `lod.md`.

## 4. Conservative bounds for culling empty and solid sub-volumes

### 4.1 Lipschitz bound of fBm

- Hart: if `f` is λ-Lipschitz then `f/λ` is a distance bound; for a height field `z − h(x,y)` the bound constant is `√(1+λ²)` (slides extracted as "1 + λ²", the square root is probably lost in extraction) [S] https://perso.liris.cnrs.fr/eric.galin/M2/Landscapes/2-generation.pdf. The slides give fBm as `Σ a_k n(T_k(p)/w_k)` with `a_k = a_0·2^-k`, `w_k = w_0·2^-k` and no explicit Lipschitz constant [S].
- Own derivation (computed, [U] as a source claim): value noise with cubic fade, lattice values in [−1, 1]: along one axis the slope is at most `max|Δ| · max u'` = `2 × 1.5 = 3` per lattice cell; quintic fade: `2 × 1.875 = 3.75` (max of `30x²(x−1)²` is 1.875 at x = 0.5). Gradient noise has a similar order [U]. Let `Lip` be that per-cell constant (per axis). For octave k, cell size is `λ_k` so its Lipschitz constant per block is `a_k · Lip / λ_k = a_k · Lip · f_k`. With `G = 0.5` and lacunarity 2 every octave contributes equally: `Λ = K · a_0 · f_0 · Lip` (computed). Example: `a_0 = 24.8`, `f_0 = 0.004`, `Lip = 3.75`, K = 5 gives Λ ≈ 1.86 blocks per block (computed). That is loose against typical slopes (about 0.3–0.5 [U]), so use the per-octave form below, not Λ alone.

### 4.2 Region bound for chunks and LOD tiles

For a square footprint with centre `c` and half-diagonal `r` (blocks):

`|H(p) − H(c)| ≤ Σ_k min(2·a_k, a_k·Lip·f_k·r)`

High octaves (wavelength below about `r`) saturate at `2·a_k`; low octaves use the slope term. Evaluate `H(c)` exactly (one 2D sample), then add the bound for `[Hmin, Hmax]`. The sum has K terms of pure arithmetic. Use of the bound:

- Chunk is air if `chunk_min_y > Hmax` and no 3D feature can reach it; solid (uniform) if `chunk_max_y < Hmin − cave_depth_bound`. This extends the existing uniform-chunk shortcut in `terrain.rs`, which today needs the exact heights.
- For far LOD, `r` is large (`32·2^L·0.7` blocks) and the bound approaches `±Σ a_k` (global amplitude), so far chunks classify on amplitude plus the exact centre value, and only chunks that straddle the surface get filled.
- 3D density: bound `|density| ≤ Σ a3_k` over the 3D octaves gives interval classification per 16³ or 8³ sub-block, the Voxel Tools method referenced in B-11 and `engine-survey-2026-11.md`.
- Tighter alternative: evaluate H exactly at the 25 stride-8 points (5×5) of the footprint and use `r = 5.7` blocks (half-diagonal of an 8×8 cell, computed) inside each cell; cost 25 samples per chunk column (a 2.4% of the 1,024-column fill, computed).

Test: random `p, q`, assert `|H(p) − H(q)| ≤ Λ·|p − q|` and `H(p) ∈ [Hmin, Hmax]` over millions of samples, per seed.

## 5. Data-oriented evaluation pipeline

### 5.1 Batching

- FastNoise2 evaluates a node graph per batch (whole grid at once) rather than per point; the README documents "GenUniformGrid3D"-style calls and says nothing else on internals [S]. The fetched wiki pages did not cover batch details or seeds per node [S].
- Design for Espresso (own): per chunk column (32×32×N chunks stacked in Y), one SoA buffer set: `x_cell: [i32; 1024]`, `x_frac: [f32; 1024]` per axis, `height: [f32; 1024]`. The octave loop is outer, the 1,024-lane loop is inner and contains no branches and no gathers. Per octave per lane: two shifts and masks, two hashes per axis corner (4 corners in value noise, 3 in simplex), the polynomial, one accumulate. The inner loop is what LLVM vectorises, or what `wide::f32x8` implements explicitly (section 7).
- Cache: the 2D fields for a column stack are computed once and reused by all vertical chunks of the stack (Decaf recomputed them 9 times per column stack, `decaf-audit.md` line 37 [S]). LOD tiles cache the same 2D fields per level. Cache key `(layer_id, lod, column_x, column_z)`, stored in a flat per-worker scratch buffer that is reused (no allocation per chunk).

### 5.2 Coarse 3D grid with trilinear interpolation

- Grid points per 32³ chunk with cells of 4×8×4 blocks: `(32/4+1)·(32/8+1)·(32/4+1) = 9·5·9 = 405` (computed). With 8×8×8: `5³ = 125` (computed). Against 32,768 voxels that is 81× and 262× fewer evaluations (computed). Cell corners are absolute-coordinate multiples of the cell size, so two neighbouring chunks compute the identical value at the shared face (same integer inputs, same float ops, section 2.3).
- Interpolation error of one octave of wavelength λ, amplitude a, with cell size h along an axis: `≈ a · (2π h/λ)² / 8` (second-order linear interpolation of a sinusoid, computed [U as a general formula]). Values:

| h (blocks) | λ = 32 | λ = 64 | λ = 128 |
|---|---|---|---|
| 4 | 7.7% of a | 1.9% of a | 0.5% of a |
| 8 | 31% of a | 7.7% of a | 1.9% of a |

(computed). Rule: octaves with `λ < 8h` stay out of the interpolated grid. Fine surface octaves are evaluated exactly in 2D per column; the 3D density grid carries only octaves with `λ ≥ 64` blocks at 4×8×4 (vertical h = 8 is the weak axis: 7.7% at λ = 64, so use λ ≥ 128 for vertical or h_y = 4).
- Absolute-lattice definition: declare the field to be "bilinear/trilinear interpolation of exact lattice values on the absolute grid of cell size h". Level 0 then computes the same lattice and the same interpolation formula. LOD levels with voxel ≥ h sample the lattice points directly; finer levels interpolate. Every level agrees at lattice points exactly and differs only by the interpolation formula elsewhere, which is the same formula at every level. This avoids the error that appears if each LOD builds its own stride grid with a different h.
- Early-out with bounds (section 4) skips the grid fill for chunks classified uniform. The Minecraft and Hytale cell sizes are in B-11 and the Hytale notes; I could not re-fetch the Minecraft wiki page this session [S: fetch failed].

### 5.3 Per-chunk evaluation budget (computed)

| Stage | Count per 32³ surface chunk (K = 5 2D, 3 3D) | Naive |
|---|---|---|
| 2D height octaves | 1,024 columns × 5 = 5,120, once per column stack (shared by the vertical chunks) | same |
| 3D density grid (4×8×4) | 405 points × 3 = 1,215 | 32,768 × 3 = 98,304 |
| Classification samples | 25 | n/a |

## 6. GPU generation for the horizon tier only

- GPU Gems 2 ch. 2 (geometry clipmaps): nested n×n grids per level, each level a single-channel float texture, toroidal wrap so only an L-shaped strip is rewritten per move, coarse-to-fine update where each level upsamples the coarser one and adds residual detail from CPU decompression or GPU-generated noise; reported about 1.0 ms upsampling and 0.6 ms normals for a 255×255 level, near 0 ms synthesis, on a GeForce 6800 GT (2005); the CPU decompression at about 8 ms was the bottleneck; typical n = 255, L = 11 levels [S] https://developer.nvidia.com/gpugems/gpugems2/part-i-geometric-complexity/chapter-2-terrain-rendering-using-gpu-based-geometry.
- Viability for Espresso: the heightmap tier is consumed by a vertex shader, so compute-shader noise into a `R32Float` texture array (one slice per level) needs no readback, which removes the reason GPU worldgen was rejected for chunks (readback spikes, see `docs/research/recent-2024-2026/storage-gen-streaming.md`). The integer hash and the integer lattice math of section 2 run exactly in WGSL `u32`/`i32`.
- Risks: the float polynomial stage is not guaranteed bit-identical to the CPU (WGSL/driver FMA contraction and precision rules differ by vendor [U]), so the horizon height can differ from the CPU chunk height by about 1e-5 blocks (estimate). That is far below a block, but the horizon tier must overlap the chunk tier with a depth bias or an inset, not depend on exact equality. Never use GPU heights to decide block contents.
- Verdict: **Watch**. Build the CPU clipmap tile generator first (it is the reference and the test oracle), then port its octave kernel to a compute pass and compare per-level cost with divan numbers. No GPU number was measured here.

## 7. Rust SIMD options on stable 1.97

| Option | Status | Dispatch | Notes for determinism |
|---|---|---|---|
| `std::simd` | Not stabilised: all three tracking steps unchecked, "mostly blocked" on rust-lang/portable-simd#364, needs an RFC [S] https://github.com/rust-lang/rust/issues/86656 (consistent with `rust-tooling.md`, fetched 2026-11-03) | n/a | Reject (nightly) |
| `wide` | f32x8, u32x8 etc.; backend chosen at compile time only ("only supports detecting the available SIMD extensions at build time"); needs `-C target-cpu` or target features; MSRV 1.89 [S] https://github.com/Lokathor/wide | Compile-time | Its `mul_add` behaviour (fused vs not by cfg) was not verified [U]: do not call it |
| `multiversion` | `#[multiversion(targets("x86_64+avx", "aarch64+neon"))]` clones a function per target and picks at runtime; README gives no cost or inlining details [S] https://github.com/calebzulawski/multiversion | Runtime | Works with auto-vectorised plain loops; wide's compile-time cfg is not changed inside a clone, so `wide` types inside a clone stay at the global width [U, from wide README] |
| `pulp` | `Arch::new()` detects features, `WithSimd` trait generic over `S: Simd`; README examples show f64 only [S] https://github.com/sarah-quinones/pulp | Runtime | f32 and FMA behaviour not documented in README [S]; read source before use |
| `simdnoise` | runtime AVX-512 / AVX2 / SSE4.1 / SSE2 / scalar, last release 2020 per `rust-tooling.md`; scalar differs from SIMD within tolerance [S] | Runtime | Not bit-exact across paths: Reject as authority |
| FastNoise2 (FFI) | fastest published numbers (section 1.1); C++ toolchain; no determinism statement [S] | Runtime | Watch: use as speed ceiling in the benchmark, not in the generator |

Recommendation (own, [U] until measured): write the column kernel as plain `[f32; 1024]` loops with no gathers; compile it twice via `multiversion` (baseline x86-64 and `x86-64-v3` with AVX2) and test that both give identical bits; add an explicit `wide::f32x8` variant only if the auto-vectorised loop is measurably slower in the divan bench. Build `.cargo/config.toml` flags stay as in `rust-tooling.md` section 3. Never enable `+fma` in the generator crate: it lets LLVM keep separate multiply and add (Rust does not contract them [S]), but explicit `mul_add` would be fused only on FMA machines.

## 8. Targets and benchmark plan

### 8.1 Targets (**target**, to be replaced by measurement)

Reference: current world generation about 260 ms for 6,125 chunks at `default` on the i7-9750H with 12 threads, 42 µs of wall time per chunk (computed; given in the task context, measured upstream).

| Quantity | Target |
|---|---|
| 2D height, 5 octaves, per column (SIMD, i7-9750H) | ≤ 20 ns (about 4 ns per octave; refs: simdnoise AVX2 4.3 ns/octave on i7-6700 [computed from quoted], FN2 simplex 2.1 ns on a 4.9 GHz i7-7820X [computed from quoted]) |
| Height fill for a 32×32 column stack | ≤ 25 µs, once per stack, shared by all vertical chunks |
| 3D noise evaluations per 32³ chunk | ≤ 1,215 (4×8×4 grid, 3 octaves); zero for chunks classified uniform |
| Classification per chunk | ≤ 25 2D samples + O(K) arithmetic, under 2 µs |
| Uniform (air/solid) chunk generation | ≤ 1 µs after classification |
| Surface chunk, 2D heights from cache + palette write | ≤ 10 µs of CPU per chunk |
| Far LOD (L ≥ 3) 2D cost per chunk | ≤ 60% of the level-0 column fill (computed from octaves kept 3/5) |
| Heightmap clipmap, full rebuild of 10 levels × 255² | ≤ 6 ms; incremental update well under 1 ms per frame |
| Determinism | scalar, SSE2, AVX2 content hashes identical for 1,000 columns at |x|,|z| up to 2^24 |
| LOD bound | `|H_L − H_0| ≤ tail(L)` over 10^7 random samples |

### 8.2 Benches

- `divan` micro benches, one per stage, each 1,024 columns or one chunk, release build, pinned core as in `fair-mesher-comparison.md`: `hash32`, `octave_2d`, `octave_2d_gradient`, `height_columns_k5`, `classify_chunk`, `density_grid_405`, `trilinear_expand`, `palette_write`. Noise inputs span near (±100) and far (±2^24) blocks, to catch precision-dependent slowdowns (denormals do not occur; branches on cell sign might).
- `gungraun` (successor of iai-callgrind, v0.20.0 released 2026-09-26, Valgrind and Linux perf, x86_64 Linux documented, instruction counts that stay stable in noisy VMs) [S] https://docs.rs/crate/gungraun/: instruction count per stage, gate at +2% instructions versus the previous commit (computed rule, a regression gate, not a speed claim). Instruction counts do not capture AVX2 throughput; wall time stays in divan.
- `vx bench` world generation ms at the `default` and `far` scenarios (`docs/BENCHMARKING.md` protocol, `vx bench-compare`): gate = outside the noise band and not slower, plus the content hash unchanged except for deliberate field changes.
- Correctness gates as unit tests: determinism hash (2.3), LOD tail bound (3.2), range bound (4.2), far-origin test comparing the integer-lattice result at `x = 2^30` against an f64 reference, translation invariance (shifting `x` by one base period 2^8 blocks must permute hash inputs only).

## 9. Gaps

- Could not fetch: Minecraft wiki noise router page (client error), Lagae survey on HAL (used the scribblethink.org copy), Cubyz generation source and any DH/Cubyz statement on sampling noise at coarse spacing, the FastNoise2 benchmark site (auburn.github.io/fastnoise2benchmarking), Eiserloh's talk slides and SquirrelNoise5 constants, `wide` source for `mul_add`.
- No benchmark numbers found for OpenSimplex2S, bracket-noise, noise-rs 2D/3D beyond the local table in `rust-tooling.md`, or libnoise 2D.
- All ns/octave figures for the i7-9750H are extrapolations from other CPUs; replace with the divan numbers.
- Whether FastNoise2 AVX2 and SSE2 outputs are bit-identical: not stated anywhere I fetched.
- The Lipschitz constants (3, 3.75) are derived for value noise; gradient and OpenSimplex2 constants need a sampled maximum-gradient measurement.
- The claim that Rust autovectorised loops give identical bits to scalar is reasoning from IEEE rules and the absence of fast-math; the cross-path hash test is the check.
- WGSL float contraction rules were not checked.
