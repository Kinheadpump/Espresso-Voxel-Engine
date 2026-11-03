# AGENTS.md

Guide for any coding agent (and human) working in this repo. Read this, then `docs/ARCHITECTURE.md`. Claude sessions also read `CLAUDE.md`. Anyone writing docs follows `docs/DOC-GUIDELINES.md`.

## What this is

A performance-first blocky voxel engine in Rust on wgpu. Top-level goal: an infinite world with nearly infinite render distance (ADR-0016); no system may assume a bounded map or a world origin near the camera. It is the third attempt; the first two (Decaf, Lemon) are preserved in their own repos and summarised in `docs/HISTORY.md`. Their full audits live in `docs/research/`.

## Repo map

| Path | Purpose | Depends on |
|---|---|---|
| `crates/vx-core` | Chunk constants, `BlockId`, `ChunkPos`/`LocalPos`, `Axis`/`FaceDir`, padded indexing | glam, bytemuck |
| `crates/vx-world` | Palette chunk storage, ring-buffer `ChunkWindow`, terrain generator, padded-neighbour gather | vx-core, fastnoise-lite |
| `crates/vx-mesh` | Binary greedy mesher, 8-byte `PackedQuad`, culled reference mesher for tests | vx-core |
| `crates/vx-render` | wgpu device/tier negotiation, terrain pipeline (vertex pulling + multi-draw indirect), camera, offscreen target | vx-core, vx-mesh, wgpu |
| `crates/vx-instr` | Instruction-count benches (gungraun on Valgrind) and their fixed inputs; CI fails on a >1% `Ir` regression vs the base branch | vx-core, vx-world, vx-mesh |
| `crates/vx-app` | `vx` binary: winit window, region build on rayon, headless `--probe` / `--screenshot` | all |
| `tools/legacy-compare` | Standalone crate: Decaf and Lemon meshers vs ours on identical input (not a workspace member) | vx-core, vx-world, vx-mesh |
| `scripts/` | `bench-baseline.sh` / `.ps1` hardware baseline runs | |
| `benchmarks/baselines/` | Committed reference runs per machine (`results/` is git-ignored scratch) | |
| `docs/` | Architecture, decisions (ADRs), roadmap, sprint plan, ranked backlog, benchmarking guide, performance log, history, research. Where each kind of fact lives: `docs/DOC-GUIDELINES.md` §1 | |

Rule: `vx-core`, `vx-world` and `vx-mesh` never depend on wgpu or winit, so CPU benches compile fast and run anywhere.

## Commands

```sh
cargo build --release                 # builds the vx binary (default member)
cargo run --release -- --radius 16    # windowed fly-cam (click to grab mouse, WASD/Space/Ctrl, Shift = fast)
cargo probe                           # print adapter, render tier, features, limits
cargo run --release -- --screenshot out.png --radius 16   # headless render, works on lavapipe
cargo test --workspace
cargo lint                            # clippy pedantic, -D warnings
cargo fmt-check
cargo bench-cpu                       # divan benches for mesh + world
cargo bench-instr                     # gungraun instruction counts (needs valgrind + gungraun-runner 0.20.0)
cargo run --release -- bench --scenario default --out base.json   # end-to-end benchmark (docs/BENCHMARKING.md)
cargo run --release -- bench-compare base.json candidate.json     # noise-aware comparison
```

Headless GPU: install `mesa-vulkan-drivers` (lavapipe). It exposes every feature we use, so it validates the full path; its timings are meaningless for GPU performance.

## Invariants

- `CHUNK_SIZE` is 32 and `PADDED_SIZE = CHUNK_SIZE + 2` must stay ≤ 64 (a padded column is one `u64`). Changing chunk size is an experiment with an ADR, not a drive-by.
- Voxel index order is `x + S*(z + S*y)` for both `LocalPos::index` and `padded_index`. Rows along x are contiguous.
- `PackedQuad` bit layout (`vx-mesh/src/quad.rs`) is mirrored in `vx-render/src/shaders/terrain.wgsl`. Change both together; the mesher-vs-reference test and the WGSL validation test must stay green.
- Meshing only reads a fully gathered padded chunk. A chunk is meshed only when its six face neighbours exist. Never predict neighbour data from the generator (that was Lemon's biggest cost and bug source).
- Reverse-Z: depth clears to 0, compare `Greater`, infinite far plane.
- No allocation in per-frame code paths; workers reuse scratch buffers (`map_init`, scratch structs).
- Large arrays go on the heap (`new_dense_chunk`, `new_padded_voxels`); clippy's `large_stack_arrays` lint enforces this.

## Workflow for a change

1. Find the roadmap item in `docs/ROADMAP.md`, or promote one from `docs/BACKLOG.md`, or add one.
2. If it is a design choice with alternatives, write an ADR in `docs/DECISIONS.md` first (status Proposed), with the measurement that will decide it.
3. Implement with tests. Hot-path changes need a divan bench or an end-to-end number.
4. Measure in release before and after with `vx bench` + `vx bench-compare` on the same machine (`docs/BENCHMARKING.md`). Label every number as in `docs/DOC-GUIDELINES.md` §4. Record results in `docs/PERFORMANCE.md`.
5. `cargo fmt --all && cargo lint && cargo test --workspace`.
6. Conventional commit with the Stats section (see `CLAUDE.md`).

## Parallel agents

When splitting work across agents: give each a disjoint crate or doc, a separate `CARGO_TARGET_DIR` if they build concurrently, and require a written report in `docs/research/` that follows `docs/DOC-GUIDELINES.md` §7 (sources linked, [S]/[U] tags). The orchestrator reviews it, ranks its candidates in `docs/BACKLOG.md` and records decisions in `docs/DECISIONS.md`.
