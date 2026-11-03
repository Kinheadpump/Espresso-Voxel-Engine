# Benchmarking

How to measure the engine so that two runs can be compared fairly. Read this before quoting any number in a commit, PR or doc.

## What `vx bench` measures

`vx bench` runs one fixed, deterministic scenario. It needs no window and renders offscreen at a fixed resolution.

| Phase | What is timed | Samples |
|---|---|---|
| World generation | Generate every chunk of the region on a dedicated rayon pool (`--threads`, default all logical cores) | 1 discarded warm-up + `repeats` runs |
| Meshing | Gather padded neighbours and run the binary greedy mesher for every non-air chunk | same |
| Upload | Build and upload the quad, origin and indirect buffers, and wait for the GPU | 1 discarded warm-up + `repeats` runs |
| Frame CPU encode | Camera upload, pass encoding and submit | `warmup` discarded frames + `frames` measured frames, `repeats` times |
| Frame GPU terrain pass | GPU timestamp at the start and end of the terrain pass | same (only if the adapter supports timestamp queries) |
| Frame wall | Encode + submit + wait for GPU completion | same |

The camera flies a fixed orbit, so every run renders exactly the same frames. The CPU waits for the GPU every frame. That serialises the work: the numbers are per-frame costs (latency), not pipelined throughput, and there is no vsync or compositor in the loop.

### Scenarios

| Name | View radius (chunks) | Region | Purpose |
|---|---|---|---|
| `small` | 8 | 19×19×5 chunks | quick check, CI smoke |
| `default` | 16 | 35×35×5 chunks | the number we track |
| `far` | 32 | 67×67×5 chunks | scaling and memory |

Defaults: 5 repeats, 120 warm-up frames, 600 measured frames, 1920×1080. `--repeats`, `--frames`, `--warmup`, `--threads` and `--resolution WxH` override them. Compare only runs that use the same values: `bench-compare` refuses to compare runs whose scenario parameters differ.

### Fairness guards built into the tool

- **Determinism check.** Each repeat hashes all generated quads. If the hash changes between repeats, the run aborts. The hash is stored as `content.mesh_hash`.
- **Same-work check.** `bench-compare` warns when the two runs rendered different content (quad count or hash differ), because their timings then cover different work.
- **Environment capture.** Each report records the commit, dirty flag, build profile, rustc version, RUSTFLAGS, OS, CPU, core counts, RAM, GPU name, backend, driver and render tier. `bench-compare` warns when machine, GPU, driver or compiler differ.
- **Build checks.** Debug builds are flagged and `bench-compare` refuses them. Runs on software adapters (lavapipe, WARP) are flagged as meaningless for GPU numbers.

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

The script builds the release binary, saves `--probe` output to `adapter.txt`, then runs `small`, `default` and `far` with default settings. Everything goes to `benchmarks/baselines/<label>/`. On a desktop GPU each scenario should take well under a few minutes. Use a stable, descriptive label such as `noah-desktop-rtx4070`, and keep using it.

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

## Reporting rules

Labels (**measured**, **quoted**, **computed**, **estimate**, **target**) and how to write them: `docs/DOC-GUIDELINES.md` §4. The two rules specific to benchmarks:

- **Measured** means produced by `vx bench`, `cargo bench-cpu` or `tools/legacy-compare` in a release build, with the machine named.
- A speed-up claim needs both runs from the same machine and session, and a verdict outside the noise band.
