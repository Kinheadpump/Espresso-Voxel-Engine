use divan::black_box;
use legacy_compare::{
    adapt::{DecafInput, LemonInput, decaf_mesh, lemon_mesh},
    decaf::ChunkMeshSlices,
    scenes::{SCENE_NAMES, build},
};
use vx_mesh::{BinaryGreedyMesher, ChunkMesh};

fn main() {
    divan::main();
}

#[divan::bench(args = SCENE_NAMES)]
fn espresso_binary_greedy(bencher: divan::Bencher, scene: &str) {
    let voxels = build(scene);
    let mut mesher = BinaryGreedyMesher::default();
    let mut mesh = ChunkMesh::default();
    mesher.mesh(&voxels, &mut mesh);
    bencher.bench_local(|| mesher.mesh(black_box(&voxels), &mut mesh));
}

#[divan::bench(args = SCENE_NAMES)]
fn decaf_as_shipped_with_instant(bencher: divan::Bencher, scene: &str) {
    let input = DecafInput::from_padded(&build(scene));
    let mut out = ChunkMeshSlices::new();
    decaf_mesh::<true>(&input, &mut out);
    bencher.bench_local(|| decaf_mesh::<true>(black_box(&input), &mut out));
}

#[divan::bench(args = SCENE_NAMES)]
fn decaf_without_instant(bencher: divan::Bencher, scene: &str) {
    let input = DecafInput::from_padded(&build(scene));
    let mut out = ChunkMeshSlices::new();
    decaf_mesh::<false>(&input, &mut out);
    bencher.bench_local(|| decaf_mesh::<false>(black_box(&input), &mut out));
}

#[divan::bench(args = SCENE_NAMES)]
fn lemon_exact_neighbours(bencher: divan::Bencher, scene: &str) {
    let input = LemonInput::from_padded(&build(scene));
    bencher.bench_local(|| lemon_mesh(black_box(&input)));
}

#[divan::bench(args = SCENE_NAMES)]
fn context_convert_padded_to_decaf(bencher: divan::Bencher, scene: &str) {
    let voxels = build(scene);
    bencher.bench_local(|| DecafInput::from_padded(black_box(&voxels)));
}

#[divan::bench(args = SCENE_NAMES)]
fn context_convert_padded_to_lemon(bencher: divan::Bencher, scene: &str) {
    let voxels = build(scene);
    bencher.bench_local(|| LemonInput::from_padded(black_box(&voxels)));
}
