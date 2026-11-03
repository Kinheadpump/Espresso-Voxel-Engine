#!/usr/bin/env bash
# Usage: scripts/bench-baseline.sh <machine-label> [extra vx bench args...]
set -euo pipefail
label="${1:?usage: scripts/bench-baseline.sh <machine-label> [extra args]}"
shift
cd "$(dirname "$0")/.."
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "warning: working tree is dirty; results will be marked dirty" >&2
fi
out="benchmarks/baselines/${label}"
mkdir -p "$out"
cargo build --release -p vx-app
./target/release/vx --probe | tee "$out/adapter.txt"
for scenario in small default far cave mountain fly-fast; do
    ./target/release/vx bench --scenario "$scenario" --out "$out/${scenario}-$(git rev-parse --short=12 HEAD).json" "$@"
done
echo "results in $out"
