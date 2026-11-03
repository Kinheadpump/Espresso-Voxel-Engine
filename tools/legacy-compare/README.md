# legacy-compare

Standalone (own `[workspace]`) benchmark comparing the Decaf, Lemon and Espresso voxel meshers on identical padded 34^3 input. Full method, results and caveats: `docs/research/fair-mesher-comparison.md`.

The legacy mesher code in `src/decaf.rs` and `src/lemon.rs` is copied (with minimal adaptation, listed in the report) from:

- Decaf-Voxel-Engine, commit `b436080eb26b6dfb5157695b49379379c33dec11` (`src/engine/world/mesher.rs`)
- Lemon-Voxel-Engine, commit `8bc10039d253f9e5bbe0d453a26e852b43bc446f` (`src/engine/core/mesher.rs`, `src/game/world/chunk.rs`)

Reproduce:

```
cd tools/legacy-compare
export CARGO_TARGET_DIR=target   # optional: any directory outside the main workspace target
cargo run --release --offline     # correctness check (unit-face equivalence, quad counts)
cargo bench --offline             # divan benches; repeat >= 3 times, pin with taskset -c N
```
