# Benchmarking

How to measure the engine so that two runs can be compared fairly. Read this before quoting any number in a commit, PR or doc.

## What `vx bench` measures

`vx bench` runs one fixed, deterministic scenario. It needs no window and renders offscreen at a fixed resolution.

| Phase | What is timed | Samples |
|---|---|---|
| World generation | Generate every chunk of the region on a dedicated rayon pool (`--threads`, default all logical cores) | 1 discarded warm-up + `repeats` runs |
| Meshing | Gather padded neighbours and run the binary greedy mesher for every non-air chunk | same |
| Upload | Build and upload the quad, origin and indirect buffers, and wait for the GPU | 1 discarded warm-up + `repeats` runs |
| Frame CPU encode | Camera upload, pass encoding and submit; also split into `record` (command encoder through `finish`) and `submit` (`queue.submit`) | `warmup` discarded frames + `frames` measured frames, `repeats` times |
| Frame GPU terrain pass | GPU timestamp at the start and end of the terrain pass | same (only if the adapter supports timestamp queries) |
| Frame wall | Encode + submit + wait for GPU completion | same |
| Memory | Process resident memory after region build, peak resident memory over the run (Linux `VmHWM`, Windows `PeakWorkingSetSize`), and GPU memory from wgpu's allocator report (Vulkan and D3D12 only) grouped by resource label, taken after the timed frames while every buffer and render target is alive | once per run |

The camera follows a fixed path per scenario, so every run renders exactly the same frames. The CPU waits for the GPU every frame. That serialises the work: the numbers are per-frame costs (latency), not pipelined throughput, and there is no vsync or compositor in the loop.

### Windowed mode (`--windowed`)

`vx bench --windowed` renders the same scenario path to a real swapchain in a borderless fullscreen window, with no CPU wait for the GPU, so the driver, presentation and frame pipelining are in the loop. It picks `Immediate`, then `Mailbox`, then falls back to `Fifo` with a warning (vsync-bound, pacing meaningless). The resolution is the monitor's, recorded in `scenario.width/height`; the present mode and frame latency are recorded in `scenario.frame_sync`.

| Metric | Windowed meaning |
|---|---|
| `frame_wall_us` | present-to-present interval, printed as `frame interval (present)` |
| `frame_cpu_encode_us` | camera upload through `present` returning (swapchain acquire is outside it) |
| `frame_cpu_submit_us` | `queue.submit` + `present` |
| `frame_gpu_terrain_pass_us` | n/a (reading timestamps back would reintroduce the per-frame wait) |
| `visibility` | n/a (the offscreen run reports it) |

The report also prints a pacing line: p50, p99, max, standard deviation and p99/p50 of the interval. `bench-compare` refuses to compare a windowed run with an offscreen one because `frame_sync` differs. Keep the window in focus and the machine idle: the compositor and other windows are part of what this mode measures.

### Scenarios

| Name | View radius (chunks) | Region | Camera path | Purpose |
|---|---|---|---|---|
| `small` | 8 | 19×19×5 chunks | orbit at y 150, looking down ahead | quick check, CI smoke |
| `default` | 16 | 35×35×5 chunks | same orbit | the number we track |
| `far` | 32 | 67×67×5 chunks | same orbit | scaling and memory |
| `cave` | 16 | 35×35×5 chunks | along a sealed 6×8-block tunnel carved at y 8–16 under the whole terrain, swaying ±0.35 rad | occlusion: almost nothing behind the tunnel walls is visible |
| `mountain` | 16 | 35×35×5 chunks | ground-level walk (eye 2.7 blocks above the surface) across the hills, panning ±0.6 rad | occlusion by ridges in front of valleys |
| `fly-fast` | 16 | 35×35×5 chunks | two straight passes at y 125, ~2.6 blocks per frame, with a teleport to the second pass halfway | streaming stress once streaming exists; today a static region |

`cave` uses its own terrain (the default heightmap plus the carved tunnel, `TerrainSettings::carved_air`), so its mesh hash differs from the other scenarios. The other five share the `default` heightmap. `vx --screenshot out.png --scenario cave --at 0.3` renders one frame of any scenario's path for a visual check.

Defaults: 5 repeats, 120 warm-up frames, 600 measured frames, 1920×1080. `--repeats`, `--frames`, `--warmup`, `--threads` and `--resolution WxH` override them. `--shading flat|textured` (default `flat`, the mode of every baseline) picks the terrain surface; the report records it with the texture hash. `bench-compare` refuses two runs with different shading or textures unless `--allow-shading-change` is passed, so a flat-vs-textured comparison is always deliberate. `--indirect-validation on|off` (default `off` in release, ADR-0015) switches wgpu's per-draw indirect-argument validation; it is recorded as `content.indirect_validation` and `bench-compare` warns when the two runs differ. Reports from before 2026-11-03 ran with it on, so compare new runs against them only with `--indirect-validation on`. Compare only runs that use the same values: `bench-compare` refuses to compare runs whose scenario parameters differ.

### Fairness guards built into the tool

- **Determinism check.** Each repeat hashes all generated quads. If the hash changes between repeats, the run aborts. The hash is stored as `content.mesh_hash`.
- **Same-work check.** `bench-compare` warns when the two runs rendered different content (quad count or hash differ), because their timings then cover different work.
- **Environment capture.** Each report records the commit, dirty flag, build profile, rustc version, RUSTFLAGS, OS, CPU, core counts, RAM, GPU name, backend, driver and render tier. `bench-compare` warns when machine, GPU, driver or compiler differ.
- **Build checks.** Debug builds are flagged and `bench-compare` refuses them. Runs on software adapters (lavapipe, WARP) are flagged as meaningless for GPU numbers.

### Culling ladder

After the timed frames, `vx bench` samples 24 evenly spaced frames of the measured path and records how much work each culling stage would leave (`visibility.ladder` in the JSON, printed after the timings). Nothing is culled at render time today: the ladder says what a culler could save, and it is the yardstick for backlog B-06 and B-07.

| Stage | Chunks counted | Draws and quads counted |
|---|---|---|
| `resident` | every uploaded chunk | every direction draw |
| `frustum` | chunk AABB intersects the view frustum (CPU test, reverse-Z near plane, no far plane) | all draws of those chunks |
| `frustum+facing` | chunks with at least one direction that can face the camera | only those directions (+X faces need `camera.x > chunk.min.x + 1`, and so on) |
| `pixel-visible` | chunks that own at least one pixel in a chunk-id render at the bench resolution | the facing draws of those chunks |

`pixel-visible` is the ceiling for a perfect chunk-granular occlusion culler: it renders the scene with each pixel holding its chunk slot (`ChunkIdPass`, `R32Uint`), reads it back and counts distinct slots. Its quads are whole facing draws, not individually visible quads. The ladder depends only on the scene, the path and the resolution, not on the GPU, so any machine produces the same counts (rasterisation rules may change a chunk at a pixel edge). The graph and Hi-Z rungs from backlog B-07 get their own stages when those cullers exist, together with the culling-off image-identity check.

### How `bench-compare` decides

For each metric it takes the median of each repeat, then the median of those medians (the "center"). The noise band is the larger of the two runs' relative spreads, `(max − min) / median` of the per-repeat medians, and is never below 2%. The verdict is FASTER or SLOWER only when the change exceeds the noise band, and there must be at least 3 repeats. Otherwise the verdict is "within noise". A large noise percentage means that metric is not trustworthy on that machine. Today that applies to upload, which is a few milliseconds of mostly driver allocation.

## Establishing a hardware baseline on your machine

### Prerequisites

- Rust from <https://rustup.rs>. The repo pins the toolchain in `rust-toolchain.toml`, so rustup installs the right one on the first build.
- An up-to-date GPU driver. Windows and Linux use Vulkan (DX12 also works on Windows), and macOS uses Metal.
- Git.

### Prepare the machine

1. Plug in the laptop and set the power plan to high performance. On Windows 11: Settings → System → Power & battery → Power mode → Best performance. On Linux: `powerprofilesctl set performance`.
2. Close games, browsers, video and anything else using the GPU. Pause cloud sync and antivirus scans if you can.
3. On laptops with two GPUs, make sure `vx` runs on the discrete GPU. The `--probe` output shows which adapter it picked.
4. Let the machine idle for a minute after boot or after a heavy build.

### Run

```sh
git clone https://github.com/Kinheadpump/espresso-voxel-engine
cd espresso-voxel-engine
# Linux / macOS
scripts/bench-baseline.sh noah-desktop
# Windows (PowerShell)
.\scripts\bench-baseline.ps1 -Label noah-desktop
```

The script builds the release binary, saves `--probe` output to `adapter.txt`, then runs all six scenarios (`small`, `default`, `far`, `cave`, `mountain`, `fly-fast`) with default settings. Everything goes to `benchmarks/baselines/<label>/`. On a desktop GPU each scenario should take well under a few minutes. Use a stable, descriptive label such as `noah-desktop-rtx4070`, and keep using it.

Run the script twice, a few minutes apart, and compare the two `default` files:

```sh
./target/release/vx bench-compare benchmarks/baselines/noah-desktop/default-<commit>.json <second run>.json
```

Every metric should come back "within noise". That confirms the machine is stable enough to judge changes. If something is not, close more background work and run again.

Commit the `benchmarks/baselines/<label>/` folder, or send it to Claude. Those files become the reference that later PRs are measured against, and their summary goes into `docs/PERFORMANCE.md`.

Existing baselines: `noah-msi-rtx2060` (i7-9750H, RTX 2060, Vulkan; 2026-11-03).

### Comparing a change

1. Check out the baseline commit and run `vx bench --scenario default --out base.json`.
2. Check out the change and run `vx bench --scenario default --out candidate.json`.
3. Run `vx bench-compare base.json candidate.json`.

Run both on the same machine and session, back to back, with the same settings. Never compare against a number from a different machine.

## Micro-benchmarks

`cargo bench-cpu` runs divan benches for the mesher, gather, generation and palette decode on a single thread. Use them to iterate on one function. `tools/legacy-compare` is the fair mesher comparison against Decaf and Lemon on identical input (see `docs/research/fair-mesher-comparison.md`).

## Tracy zones

`cargo run --profile profiling --features tracy -- bench` (or `-- run`) streams `profiling` zones to a Tracy viewer. The viewer must match `tracy-client` 0.18.5 / `tracy-client-sys` 0.29.0 (see the version table in the `tracy-client` README). Without the feature the `profiling` macros expand to nothing, so normal `vx bench` numbers carry no Tracy cost.

| Zone | Where | Thread |
|---|---|---|
| `build_region`, `assemble_and_mesh_region` | `region::build_region` | caller |
| `generate_chunk` | `TerrainGenerator::generate` | rayon worker |
| `gather_padded`, `mesh_chunk` | neighbour gather and `BinaryGreedyMesher::mesh` | rayon worker |
| `upload_static` | `TerrainRenderer::upload_static` | main |
| `record_terrain_draws` | `terrain::record_draws` (encode) | main |
| `queue_submit` / `queue_submit_present` | bench frame / windowed frame | main |
| `bench_frame` / `frame` + frame mark | one per frame in `vx bench` / `vx run` | main |

Never put Tracy-on numbers into a baseline or Stats section: the zones cost real time per call.

## Reporting rules

Labels (**measured**, **quoted**, **computed**, **estimate**, **target**) and how to write them: `docs/DOC-GUIDELINES.md` §4. The two rules specific to benchmarks:

- **Measured** means produced by `vx bench`, `cargo bench-cpu` or `tools/legacy-compare` in a release build, with the machine named.
- A speed-up claim needs both runs from the same machine and session, and a verdict outside the noise band.
