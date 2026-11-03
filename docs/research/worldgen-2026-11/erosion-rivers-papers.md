# Erosion, rivers and mountains for an infinite chunk-local generator

> Snapshot, 2026-11-03.

Question: which published terrain techniques (erosion approximations, river networks, simulation, learned models) can run as deterministic, column-local, octave-truncatable 2D work for Espresso's infinite world (ADR-0016), and which need baking or must be rejected?

Tags: **[S]** seen this session (URL given), **[U]** unverified background knowledge or own reasoning. Numbers are labelled quoted (from the source), computed (arithmetic from stated inputs) or estimate (to be replaced by a measurement in `vx bench`). Nothing here is measured in this repo.

Questions asked of every candidate: (1) chunk-local and deterministic for an infinite world? (2) evaluable at coarse spacing for far LOD without changing the silhouette? (3) rough cost per column.

## Summary

| Candidate | Source, year | Local / infinite | Coarse LOD | Cost per column | Verdict |
|---|---|---|---|---|---|
| Derivative-damped fBm | IQ, 2008 | yes, pure function | octave truncation | 1 octave = 1 `noised` | **Adopt-candidate** (base height) |
| Swiss / Jordan turbulence | de Carpentier, 2010 | yes | octave truncation | ~1 `noised` per octave | **Adopt-candidate** (mountain base) |
| Ridged multifractal | Musgrave, 1994 [U] | yes | octave truncation | 1 noise per octave | Adopt-candidate (cheap fallback) |
| Domain warping | IQ [U] | yes | warp octaves truncate | +1–2 noise per warp | Prototype (cost vs. benefit) |
| Erosion filter + Phacelle | Johansen, 2025–2026 (builds on Clay John 2020, Fewes 2023) | yes, per point | octave truncation | 16 taps × octaves | **Adopt-candidate** (top pick) |
| Dendry | Gaillard et al., I3D 2019 | yes, bounded neighbourhood | levels `n` truncate | 12–46 s per 512² (quoted) | **Prototype at coarse tier only**, else Reject |
| Genevaux hydrology | Génevaux et al., TOG 2013 | graph is global per region | native multiresolution | seconds per several hundred km² (quoted) | Watch (baked region tiles) |
| Jittered-grid river graph (own design) | based on above [U] | yes, bounded neighbours | levels truncate | estimate below | **Prototype** |
| Stream power simulation | Cordonnier et al., 2016 | global | baked tiles only | 6–252 s per 10k–160k nodes (quoted) | Watch / offline atlas |
| Interactive uplift sim | Schott et al., TOG 2023 | global | baked tiles only | interactive (quoted, no numbers) | Watch (offline tool) |
| Analytical stream power | Tzathas et al., EG 2024 | mostly global | baked tiles only | 1.8–33.7 s multigrid, 512²–2048² (quoted) | Watch |
| Orometry synthesis | Argudo et al., TOG 2019 | needs coarse global input | partial | not found | Watch |
| GPU hydraulic erosion | Mei et al., 2007 | global grid | baked only | 91 ms per step at 4096² (quoted) | Reject for runtime |
| Particle erosion | Beyer 2015, Lague 2019 [U] | not local (path dependent) | baked only | n/a | Reject for runtime |
| Terrain Diffusion | arXiv 2512.08309, 2025 (authors not read) | yes by design (windowed) | yes, coarse model tier | GPU only, not given | **Watch** |
| Lochner diffusion authoring | EG CGF 2023 | per tile, not seamless | cascade | 0.125 s for 5 steps on RTX 3090 (quoted) | Reject |

Top recommendation: base height from derivative-damped or swiss fBm, then Johansen's erosion filter, then a jittered-grid river carve. All three are per-column 2D functions of `(seed, i64 cell, f32 fraction)`. Section "Recommended pipeline" gives the order and the numbers to measure.

## 1. Noise-based erosion approximations

### 1.1 Derivative-damped fBm (Inigo Quilez)

Source: https://iquilezles.org/articles/morenoise/ [S].

Algorithm [S]:

```
for octave: n = noised(p)          // value, d/dx, d/dy
            d += n.yz
            a += b * n.x / (1 + dot(d, d))
            b *= 0.5 ; p = m * p * 2
```

Each octave is damped by the accumulated slope of coarser octaves, so steep areas get less fine detail and flat areas stay rough or become plateaus. IQ states that analytic derivatives are almost free in `noised` and that central differences cost about 5× more (quoted, same page). He also notes the quintic fade can show artifacts because its second derivative is discontinuous and prefers the cubic one [S].

- Local / infinite: yes. Pure function of position. Hash must be integer-based (the Shadertoy `hash2` uses `fract(x*y*(x+y))` in f32, which degrades far from the origin [U, own reasoning]; use `i64` cell coordinates and an integer hash).
- Coarse LOD: truncate octaves whose wavelength is below the voxel spacing. Damping uses only coarser octaves, so coarse octaves are unchanged by truncation. Silhouette is preserved by construction.
- Cost: one `noised` (4 gradient hashes, ~25 flops, estimate) per octave. Same order as the current fBm plus a division. Output includes the analytic gradient the erosion filter needs (section 1.4), so it costs nothing extra.

### 1.2 Swiss and Jordan turbulence (Giliam de Carpentier)

Source: https://www.decarpentier.nl/scape-procedural-extensions [S].

Swiss [S]:

```
sum += amp * (1 - abs(n.x));  dsum += amp * n.yz * -n.x;
next octave samples (p + warp*dsum) * freq
amp *= gain * saturate(sum)
```

Jordan [S]: uses `n2 = n * n.x`, accumulates two sums `dsum_warp` and `dsum_damp`, damps amplitude by `1 - damp_scale/(1 + |dsum_damp|²)`. The author says the damp term loosely mimics thermal erosion and the warp term fluvial erosion [S]. He notes the exact gradient of `1 - abs(n)` has a sign discontinuity and uses `-n.x` instead [S], and suggests 4-sample central differences for the final surface's derivative [S].

- Local / infinite: yes (pure function).
- Coarse LOD: octave truncation. Swiss damping uses `sum` of coarser octaves (height-based), so truncation keeps the silhouette. Octave warp `p + warp*dsum` only depends on coarser octaves.
- Cost: one `noised` per octave plus a few flops. Caveat: swiss/jordan give the height but not an exact surface gradient; the erosion filter wants a gradient. Option: use `1.0 / (1 + dot(d,d))` damping (1.1) and keep the exact derivative sum in the loop, or take central differences once per column at 3 extra evaluations (estimate: 4× cost of the base).

### 1.3 Ridged multifractal and domain warping

Ridged multifractal (Musgrave, 1994) and IQ's domain warping `f(p + fbm(p + fbm(p)))` are textbook techniques [U]. I did not fetch primary sources for them this session. Both are pure functions and truncate per octave. Warping costs one extra fBm per warp level, and the warp destroys the analytic gradient (chain rule needs the warp Jacobian), which conflicts with the erosion filter's need for slope. Verdict: use warping only on the low-frequency continental layer, where cost per column is amortised over large cells.

### 1.4 Erosion filter and Phacelle noise (Rune Skovbo Johansen)

Sources:
- Blog (Mar 2026): https://blog.runevision.com/2026/03/ [S]
- Phacelle post: https://blog.runevision.com/2026/01/phacelle-cheap-directional-noise.html [S]
- WGSL and Rust port with full algorithm, MPL-2.0: https://github.com/korbindeman/bevy_erosion_filter, files read in a shallow clone: `src/erosion.wesl` and `src/cpu.rs` [S]
- Shadertoys (not fetchable from here, `shadertoy.com` returned client error [S]): `wXcfWn` (advanced filter), `33cXW8`, `MtGcWh` (Clay John), `7ljcRW` (Fewes).
- Ports named on the blog: Unity Burst (lpmitchell/AdvancedTerrainErosion), Hytale worldgen (ItsVerday/Hytale_Worldgen_Additions), Minecraft mod `catto` (oovaa8/catto) [S, names only; I did not read their sources]. The `catto` README gives no chunk integration details or benchmarks [S].
- License: MPL-2.0, file-level copyleft [S]. A Rust re-implementation of the algorithm (not the code) should be cleared in an ADR.

What it does [S]: a filter applied on top of any height function that also provides its gradient. Per octave it adds stripes aligned with the downhill direction (a cosine wave for height, sine for slope), so gullies and ridges branch because later octaves follow the slope already modified by earlier ones. A per-point mask fades gullies where the terrain is flat, so peaks and valleys keep their shape.

Algorithm as implemented in `erosion.wesl` [S] (parameters in brackets, defaults from the file):

```
input: p, base (h, dh/dx, dh/dy), fade_target in [-1,1]   // fade_target = clamp(h / mean_amp)
strength = strength(0.22) * scale(0.15); freq = 1 / (scale * cell_scale(0.7))
gully_slope = mix(grad, normalize(grad) * assumed_slope(0.7), blend(1.0))
combi_mask = ease_out(smooth_start(|grad| * onset.x, rounding_input * onset.x))
for i in 0..octaves(5):
    ph = phacelle(p * freq, normalize(gully_slope), cell_scale, offset=0.25, normalization=0.5)
    //   ph.x = cos wave, ph.y = sin wave, ph.zw = side dir * freq * TAU
    dir_deriv = ph.zw * -freq
    gully_slope += sign(ph.y) * dir_deriv * strength * gully_weight(0.5)   // branching
    octave = (ph.x, ph.y * dir_deriv)                                      // height + slope
    faded = mix((fade_target, 0, 0), octave * gully_weight, combi_mask)
    h_and_s += faded * strength ; fade_target = faded.x
    new_mask = ease_out(smooth_start(|ph.y| * onset.y, rounding * onset.y))
    combi_mask = pow_inv(combi_mask, detail(1.5)) * new_mask
    strength *= gain(0.5) ; freq *= lacunarity(2.0)
output: delta (h, dx, dy), ridge_map (+1 ridge, -1 crease)
```

Phacelle noise [S, both post and code]: space is split into unit cells, each with one jittered kernel. At a point, loop over a 4×4 cell window (16 iterations). For each cell, weight `w = max(0, exp(-2 d²) - 0.01111)`, and accumulate `w * (cos θ, sin θ)` with `θ = dot(v, side_dir) + offset`. Divide by the weight sum, renormalise toward unit length (`normalization` parameter). The stripe direction `side_dir` is the same for all 16 kernels (the "Simple Phacelle" variant), so there is no intermediate buffer and octaves chain linearly. The blog's own count: 16 loop iterations and 1 input sample per pixel for Simple Phacelle [S].

- Local / infinite: yes. Each point is evaluated independently from `(seed, cell, gradient)`; the blog calls it "simple to generate in chunks" [S]. The needed input is the base height and gradient at the same point, which is also a pure function. Per-column cost does not need neighbours, so no padding is needed. Cross-chunk seams can only come from the base gradient, so use analytic gradients (1.1), not finite differences across chunk borders.
- Coarse LOD: octave truncation is natural (each octave doubles frequency, `lacunarity` 2.0 [S]). Gully amplitude halves per octave (`gain` 0.5 [S]), and the filter's `magnitude` output reports the summed strength [S]. At spacing `2^L` blocks, stop when the octave wavelength is under ~2 voxels. The mask for octave `k` depends only on octaves below `k`, so the first `m` octaves are bit-identical at any LOD, and the silhouette difference is bounded by the dropped strength sum: `strength * (gain^m) / (1 - gain)` (computed from the geometric series, ignoring the mask which is ≤ 1). The result is a conservative min/max for the uniform-chunk shortcut. [U, own derivation; verify by test.]
- Cost: per octave 16 × (1 integer hash, 1 `exp`, 1 `sin_cos`, ~10 flops). Estimate: 16 × ~40 flops ≈ 650 flops per octave, ~3 300 flops for 5 octaves, per column. Estimate for a 32×32 chunk column: 1 024 × 3 300 ≈ 3.4 Mflop, ≈ 1 ms scalar on one core (assuming ~3.5 Gflop/s effective, not measured), maybe 0.2–0.3 ms with `wide::f32x8` and a polynomial `exp`/`sin_cos`. For the 0.8 ns/voxel budget one chunk is 26 µs (computed: 0.8 ns × 32 768 voxels). If a column stack is 8 chunks tall that is 8 × 26 = 210 µs per column stack, so the SIMD estimate is right at budget. Only a measurement decides. Reductions to test: (a) 3 octaves instead of 5 at LOD 0 with the finest octaves skipped when below 1 voxel, (b) evaluating the filter on a 2× coarser grid and interpolating the delta for octave 1–2 only, (c) 3×3 window if `cell_scale` is reduced (breaks equivalence with the reference, needs visual check).
- Fits Espresso: output is a 2D height delta plus a ridge map usable for tree placement and river hints [S]. It does not create overhangs. Gully depth is limited by `strength * scale` (0.033 of the mountain scale at defaults [S, computed: 0.22 × 0.15]).
- What to measure: ns per column at 3, 4, 5 octaves in scalar and `f32x8`; visual seams across chunk borders (should be none); LOD silhouette error in blocks at LOD 1–6; ridge map usefulness.

### 1.5 Clay John / Fewes "fake erosion"

Clay John's `MtGcWh` and Fewes' `7ljcRW` are the originals [S, names from the blog; shadertoy pages not fetchable]. Johansen describes their approach as scaling stripe frequency with slope, which preserves peaks but leaves bulges in valleys [S]. His own variant keeps constant stripe width and fades gullies instead [S]. Verdict: superseded by 1.4 for our purpose, same locality properties.

## 2. Infinite river networks

### 2.1 Dendry (Gaillard, Guérin, Galin et al., I3D 2019)

Sources: paper https://www.cs.purdue.edu/cgvlab/www/resources/papers/Gaillard-2019-Dendry_A_Procedural_Model_for_Dendritic_Patterns.pdf [S]; code https://github.com/mgaillard/Noise (GPL-3.0, C++, Qt/OpenCV) [S].

Algorithm [S]: the value at `p` is the distance to an implicit tree `T`, rebuilt locally per query. Level `k` has a regular grid with cell size halved per level. Each cell holds one jittered key-point (jitter limited by ε). Level 0 connects key-points in a 7×7 neighbourhood guided by a control function `c` (noise, sketch or elevation). Each level above connects its key-points to the nearest segment of the coarser levels using a 5×5 neighbourhood. Segments become cubic curves with a displacement Δ. River slope follows a simplified Flint law with Shreve order `S = ρ(2μ−1)^β`, β = −0.6, and junction angles follow Howard's relation. Terrain height is a weighted mean of constant-height primitives around the rivers.

- Local / infinite: yes in structure. The authors state each evaluation depends on a fixed neighbourhood [S]; they do not claim an infinite domain explicitly, but a regular grid with a deterministic sampler implies it [S, paper-derived, my inference]. A caveat: a control function that spans a finite sketch is not infinite, but noise as `c` is.
- Coarse LOD: `n` levels can be truncated, finest levels dropped. Level 0 fixes the coarse layout, so the silhouette is stable across LOD [S].
- Cost: about 49 cells at level 0 plus 25 per higher level per evaluation [S]. Timings quoted: 12–46 s for 512×512 at n = 1–4, 269 s for 1024×1024 at n = 3 on an i5-6300U laptop [S, quoted]. That is ≈ 180 µs per sample for the 47 s / 512² figure (computed: 47 s / 262 144). The paper also states ~10 s for 512² on a quad-core CPU in its abstract, which conflicts with its tables [S]. Either way it is ≥ 10× too slow for per-column work at 1 m spacing (target budget ≈ 0.8 ns/voxel, 1 024 columns per chunk column). It is plausible only at the coarse tier (e.g. 64–256 m spacing), baked once per tile.
- Hydrology quality (quoted): depression surface 0.8 % for Dendry versus 35.9 % multifractal and 24.5 % ridged noise [S]. This is the reason to use it for the coarse river skeleton.
- Verdict: Prototype for a coarse river field baked per 64 km tile region or per macro tile; do not evaluate per voxel.

### 2.2 Génevaux et al. 2013 (hydrology-based procedural terrain)

Sources: https://www.cs.purdue.edu/homes/bbenes/papers/Genevaux13ToG.pdf [S]; HAL page https://hal.archives-ouvertes.fr/hal-01339224 returned an Anubis access-denied page [S, not readable].

Algorithm [S]: grow a river graph from mouths on the domain boundary by a rewriting process. Node selection mixes elevation and Horton-Strahler priority (parameter ζ); branching uses continuation, symmetric and asymmetric junction probabilities; validity checks include a Lipschitz slope limit. Then a Voronoi partition of river nodes gives watersheds and crest heights. Terrain is an analytic CSG-like tree of primitives (Perlin-perturbed terrain primitives, river profile primitives) combined with blend and replace operators.

- Local / infinite: no as published. The graph grows from a finite domain boundary and the tree is built per domain [S]. To make it local, run it per fixed region (e.g. 16 km tiles), pin the river mouth/edge crossing points on shared region borders by a deterministic hash, and blend. Edge crossings must agree between neighbouring regions, which the paper does not solve [U].
- Coarse LOD: yes, native multiresolution is claimed [S]. The representation averages 1.5 kB per km² [S, quoted], so a region graph is cheap to cache.
- Cost (quoted): graph 0.1–5.3 s, tree 1.8–4.9 s per scene, mesh 0.7–1.2 s at 1024² [S].
- Limits [S]: rivers subdivide only upstream (no deltas); can cut flat through high mountains.
- Verdict: Watch. The algorithmic idea feeds 2.4.

### 2.3 Other references

- Kelley et al. 1988 (stream networks from a Poisson-like map), "Procedural river networks" tiling variants, Cubyz and Veloren rivers: not fetched, [U]. Veloren's own devblog states that it computes drainage with a single-receiver network and implicit stream power erosion on a finite world map, and that noise alone "falls short" for rivers [S: https://veloren.net/devblog-43]. Veloren is therefore a simulation-on-finite-map design, not an infinite one. I did not find a source for Cubyz's river method.
- A GameDev.net article titled "plate-local river generation, deterministic, terrain-aware, downhill flowing" matches our use case. The fetch failed (client error), so unread [S].

### 2.4 Own design: jittered-grid river graph (Prototype, [U])

Goal: rivers that are deterministic, local with bounded neighbours, and truncatable by level. Combines Dendry's neighbourhood idea with a hydrology-friendly rule.

Sketch:
1. Level `k` grid of cell size `S_k = S_0 / 2^k`. One jittered node per cell, with an integer hash from `(seed, level, cell)`.
2. Give each node a "downstream" neighbour by a rule that depends only on the base height field `H_coarse` at node positions: the lowest of the 8 neighbours in the same level, or (for ties) the one with the smaller hash. Single receiver per node gives a forest.
3. Drainage area is not computable locally in general, so approximate it by level: a node of level `k` has weight `4^(L - k)` (Strahler-like) and only coarse levels carry wide rivers. This replaces the stream-power area `A` with a deterministic order, as Dendry does with Shreve order [S].
4. Carve: for a column, find the nearest edge among the 3×3 (level 0) and 3×3 (higher levels) neighbour nodes, then carve `h -= width(level) * profile(dist)`.
5. Guarantee downhill flow: set node height to `min(H_coarse, height of receiver - ε)`, which needs the receiver chain. Bound the chain length by a depth `D`; beyond `D` accept breaches. This is the main correctness risk.

Local: yes, bounded 3×3 per level. LOD: drop fine levels. Cost estimate: 9 nodes × (hash + distance to segment) × levels 3 ≈ 27 segment tests, ≈ 1 000 flops per column, comparable to the erosion filter. Cheaper variant: compute river distance on a 4×4-block grid per chunk and bilinearly interpolate, since river banks are smooth. Measure: ns per column, fraction of rivers with an uphill step, and visual quality against Dendry on the same seed.

## 3. Simulation-based methods (global, need baking)

None of these are chunk-local; each needs a finite grid, a global flow-routing step, and iterates to a steady state. To use any of them in an infinite world: bake macro tiles (for example 50 × 50 km at 50–100 m spacing, as Cordonnier uses, see below) offline or at first visit, store the result as a coarse atlas, and treat it as the large-scale input `c(x, y)` to the local pipeline. Tile borders need a fixed boundary condition (sea level, as in Cordonnier) or overlap-and-blend; the blend region is never physically consistent [U].

### 3.1 Cordonnier et al. 2016 (stream power law, uplift)

Source: https://www.cs.purdue.edu/homes/bbenes/papers/Cordonier16CGF.pdf [S]; HAL page blocked (Anubis) [S].

Algorithm [S]: `∂h/∂t = u − k A^m s^n`, with n = 1, m = 0.5. Poisson-sampled points with a constrained Delaunay triangulation, border nodes fixed at sea level. Each node drains to its lowest neighbour (receiver), lakes are resolved with a lake graph, drainage area is accumulated in one reverse pass, and the implicit update `h_i = (h_i + δt(u_i + k A^m h_j/‖Δp‖)) / (1 + δt k A^m/‖Δp‖)` is solved in O(N) by traversing trees root to leaf. A thermal correction limits slope to 30°.

- Local: no. Drainage area depends on every upstream node.
- Numbers (quoted, 50 × 50 km, i7 3 GHz): 10 000 samples: 0.031 s per step, 6.4 s to converge; 160 000 samples: 1.273 s per step, 252 s to converge; mountains form in about 50 iterations [S].
- Detail stage [S]: the stream graph is turned into a tree of procedural primitives for ridges, valleys and riverbeds (after Génevaux and Guérin et al. 2015) and carved at any zoom. That is the relevant trick for us: coarse graph baked, fine detail procedural.
- Verdict: Watch. A 160 000-node tile costs ≈ 4 min on a 2016 desktop (quoted) and covers 2 500 km² (computed: 50 × 50 km), so a baked continent atlas of e.g. 40 such tiles is feasible offline. At 50 km / sqrt(160 000) ≈ 125 m node spacing (computed), the data supplies mountain layout and river skeleton, not blocks.

### 3.2 Schott et al. 2023

Sources: https://h-schott.github.io/p/uplift [S]; code https://github.com/H-Schott/StreamPowerErosion (MIT, CMake, C++ [inferred from build files]) [S].

Method [S]: stream power simulation in the uplift domain (the user paints uplift), with a "fast yet accurate approximation of drainage area and flow routing"; interactive. No numbers on the fetched pages. I did not read the paper PDF. Useful as a tool for authoring a baked atlas; same locality verdict as 3.1.

### 3.3 Tzathas et al. 2024

Source: https://diglib7.eg.org/bitstream/handle/10.1111/cgf15033/v43i2_31_15033.pdf [S].

Method [S]: analytical solution of the stream power law where time is a parameter; the elevation at `x` is the initial terrain at an advected point plus an integral of uplift over erosion coefficient, computed recursively in linear time along each river path; multigrid-accelerated fixed-point iteration for the 2D case; landslide and hillslope approximations added.

- Local: "mostly global" [S]: drainage area depends on all upstream cells and the river network is solved together with elevations.
- Numbers (quoted, CPU, Python/NumPy/numba, Xeon E5-2650 v4): multigrid 1.79 s (512²), 8.18 s (1024²), 33.65 s (2048²); full simulation 55.5 s, 556 s, 4 799 s for the same sizes [S]. Speed-up of multigrid over simulation: 31× at 512², 68× at 1024², 143× at 2048² (computed).
- Verdict: Watch; the best simulation for a baked tile. Time as a slider is attractive for biome-dependent erosion maturity.

### 3.4 Mei et al. 2007 and particle erosion

Mei: https://core.ac.uk/download/52631892.pdf [S]. Virtual pipe shallow-water model, sediment capacity and semi-Lagrangian advection, seven fragment passes. Quoted: 1024²: 6.65 ms simulation per step; 4096²: 91.2 ms per step on a GeForce 8800 GTX [S]. Needs thousands of steps for a settled terrain [U]. Not local; GPU readback for chunk data was already rejected in the existing docs. Reject for runtime.

Particle erosion (Hans Beyer 2015 thesis, Sebastian Lague 2019 video): droplet inertia, capacity, deposit and erode with a brush [U, not fetched this session]. Particle paths cross chunk borders and the result depends on droplet order, so it is neither chunk-local nor order-independent. Reject for runtime, usable only inside a baked tile.

### 3.5 Terrain amplification and orometry

Argudo et al. 2019 (https://perso.liris.cnrs.fr/aparis/public_html/projects/argudo2019_Orometry.html [S]): takes a coarse elevation map, builds a peak-and-saddle graph following an orometric distribution, expands it into a continuous elevation function with a derived river network and valley slopes. The page does not say whether it is local [S]. Guérin et al. 2017 terrain amplification (exemplar-based) was not fetched [U]. Verdict: Watch; the useful idea is generating peaks with a target prominence/isolation statistic at a coarse tier, which a jittered-grid peak sampler can mimic locally [U].

## 4. Learned generators (2023–2026)

### 4.1 Terrain Diffusion / InfiniteDiffusion

Source: https://arxiv.org/abs/2512.08309 and https://arxiv.org/html/2512.08309v3 (v4 PDF linked) [S]. Code for the lazy tensor framework: github.com/xandergos/infinite-tensor [S, name only; license not stated on the pages I fetched]. Paper licensed CC BY 4.0 [S].

What it is [S]: a training-free algorithm that reformulates diffusion sampling to be lazy and unbounded, aiming for seed-consistency, order-invariance and constant-time random access. A hierarchy of models couples a coarse planetary model with higher-resolution models for mountains, valleys and local relief. Laplacian encoding stabilises Earth-scale dynamic range. Trained on 90 m MERIT DEM, ETOPO1 bathymetry and WorldClim, with 2048² training tiles; overlapping sliding windows are fused with MultiDiffusion-style weighted averaging. The abstract says it "outpaces orbital velocity by 9× on a consumer GPU" (quoted) [S]; the pages I fetched give no GPU model, per-tile latency or model sizes.

- Local / infinite: yes by design (windowed, order-invariant) [S].
- Coarse LOD: the model hierarchy gives native multi-scale output [S], but the finest model's resolution is fixed by training (90 m source data [S]), so 1 m detail needs a procedural layer on top.
- Cost: neural network inference per tile, GPU-only in practice. Non-deterministic across hardware floating-point is a known issue for GPU inference [U]; a bit-exact seed contract would need integer or fixed kernels, or a precomputed atlas. Quoted speed is "9× faster than orbital velocity", which at roughly 7.8 km/s orbital speed is about 70 km/s of terrain track (computed, order of magnitude, assumes the paper's claim refers to ground track at the model's resolution; this is not verified).
- Verdict: Watch. Nothing here fits 0.8 ns/voxel at runtime. What can be precomputed: a continent-scale elevation and climate atlas at 90–500 m spacing generated offline from a seed, shipped as a tile pyramid, then refined procedurally. That breaks "deterministic from seed" unless the atlas is regenerated from the seed on the user's machine, which needs the GPU model. License of weights not checked.

### 4.2 Lochner et al. 2023

Source: https://pubs.cs.uct.ac.za/id/eprint/1583/1/Diffusion%20Terrain.pdf [S]. Cascaded DDPM U-Net, sketch-conditioned: 256² tiles at 19.1 m per pixel (≈ 4.9 km square), upscaling 153 m → 19.1 m → 2.39 m. RTX 3090: 5 timesteps in 0.125 s, 250 timesteps in 6.25 s [S, quoted]. Trained on ~1.6 M USGS terrains [S]. Limits: tile seams not addressed, flicker on edits [S]. Verdict: Reject for generation. Not infinite by itself.

Other 2024–2026 arXiv infinite generators: my search returned only the Terrain Diffusion paper; I did not find or read others.

## 5. Recommended pipeline for Espresso

All layers are per-column 2D functions of `(seed, i64 block x, z)` computed once per chunk column and cached next to the chunk window; 3D work only for overhangs and caves. Each layer has an octave or level count that is a function of `L` (LOD level).

| Stage | Function | Inputs | LOD rule | Cost class |
|---|---|---|---|---|
| 1 Continent and climate | very low-frequency fBm (scale 4–50 km), warp OK here | seed, x, z | evaluated per 64–512 block cell, bilinear | amortised, negligible |
| 2 Mountain mask and uplift | swiss or derivative-damped fBm, ridged base, amplitude by stage 1 | stage 1 | octaves with wavelength ≥ 2·2^L blocks | 1 `noised` per octave |
| 3 Erosion filter | Johansen filter on stage 2 height and analytic gradient | stage 2 `(h, ∇h)` | octaves ≥ 2·2^L blocks; drop 1 octave per LOD step | 16 taps per octave (section 1.4) |
| 4 River carve | jittered-grid graph (2.4), width by level | stage 3 height, coarse height | drop fine levels with LOD | ~1 000 flops estimate |
| 5 Surface blocks | strata, soil depth, biomes from climate and ridge map | stage 3, 4 | LOD ≥ 1: surface block only | cheap |
| 6 3D features | caves, overhangs as density only near cliffs | column slope and stage 3 steepness | skip when slope below threshold; skip LOD ≥ 2 | existing density path |

Order matters: the erosion filter needs the unmodified height and gradient (stage 2), and rivers carve after erosion so river beds are not roughened. Rivers cut by Stage 4 change the surface but the erosion filter's `fade_target` should not see them [U].

Far tier (heightmap or impostor horizon, ≥ 64 km): stages 1–3 with the octave rule above, and stage 4 only at its coarse levels. Because octave truncation keeps the coarse octaves identical, horizon silhouettes match level-0 terrain within the dropped strength sum (1.4).

Optional baked layer (Watch items 2.1, 3.1, 3.3): a macro tile atlas of drainage and uplift at 100–250 m spacing, produced by Dendry or stream power per 50 km tile with sea-level borders, used as stage 2's mask and stage 4's coarse skeleton. Add it only if the pure-noise pipeline fails an A/B visual test on river continuity, because it adds a baking step and a tile-border problem.

Staged plan with measurements:
1. Replace fBm by derivative-damped fBm with analytic gradient (1.1). Measure ns per column and `vx bench-compare` world generation.
2. Add the erosion filter with 3 octaves, then 5 (1.4). Measure ns per column scalar and `f32x8`; check chunk seams and LOD silhouette error.
3. Prototype the jittered-grid river (2.4). Measure ns per column and uphill-step rate.
4. Decide on a baked atlas only after step 3 is judged visually.

## Gaps

- Shadertoy pages (`wXcfWn`, `33cXW8`, `MtGcWh`, `7ljcRW`) could not be fetched; algorithm details come from the blog and the `bevy_erosion_filter` source, which credits `wXcfWn` as its reference.
- HAL pages for Génevaux 2013 and Cordonnier 2016 returned an Anubis access-denied page; I used the Purdue PDF copies instead.
- Not fetched: Musgrave ridged multifractal original, IQ domain warping article, Kelley 1988, Beyer thesis, Lague video, Guérin 2017 amplification, Schott 2023 PDF (only the project page), the Dendry-for-Rust or any Rust port of Dendry, Cubyz river code, the Hytale and `catto` source trees, the GameDev.net plate-local river article (fetch failed).
- Terrain Diffusion pages give no latency, GPU model, model size or weight license; the "9× orbital velocity" figure is the abstract's claim only. I found no other 2024–2026 infinite-terrain generator.
- All cost-per-column figures for Espresso are estimates from flop counts; nothing was run. The `exp` and `sin_cos` throughput in SIMD, the LOD silhouette bound and the river downhill-guarantee all need measurement.
- MPL-2.0 applies to the reference erosion code; a clean-room Rust implementation vs. a port was not decided.
