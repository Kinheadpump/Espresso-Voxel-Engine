use std::hint::black_box;

use gungraun::{library_benchmark, library_benchmark_group, main};
use vx_mesh::{BinaryGreedyMesher, ChunkMesh, PaddedVoxels, new_padded_voxels};
use vx_world::{
    ChunkVoxels, ChunkWindow, DenseChunk, TerrainGenerator, TerrainSettings, gather_padded,
    new_dense_chunk,
};

#[library_benchmark]
#[bench::surface(vx_instr::padded_surface_chunk())]
#[bench::checkerboard(vx_instr::padded_checkerboard_chunk())]
fn mesh_chunk(voxels: Box<PaddedVoxels>) -> usize {
    let mut mesh = ChunkMesh::default();
    BinaryGreedyMesher::default().mesh(black_box(&voxels), &mut mesh);
    mesh.quads.len()
}

#[library_benchmark]
#[bench::surface((TerrainGenerator::new(TerrainSettings::default()), new_dense_chunk()))]
fn generate_chunk((generator, mut scratch): (TerrainGenerator, Box<DenseChunk>)) -> ChunkVoxels {
    generator.generate(black_box(vx_instr::SURFACE_CHUNK), &mut scratch)
}

#[library_benchmark]
#[bench::surface((vx_instr::packed_surface_chunk(), new_dense_chunk()))]
fn palette_decode((voxels, mut dense): (ChunkVoxels, Box<DenseChunk>)) -> Box<DenseChunk> {
    black_box(&voxels).decode_into(&mut dense);
    dense
}

#[library_benchmark]
#[bench::surface((vx_instr::surface_neighbourhood(), new_dense_chunk(), new_padded_voxels()))]
fn gather_padded_chunk(
    (window, mut scratch, mut padded): (
        ChunkWindow<ChunkVoxels>,
        Box<DenseChunk>,
        Box<PaddedVoxels>,
    ),
) -> Box<PaddedVoxels> {
    gather_padded(
        &window,
        black_box(vx_instr::SURFACE_CHUNK),
        &mut scratch,
        &mut padded,
    )
    .expect("all 26 neighbours were generated");
    padded
}

library_benchmark_group!(
    name = world_and_mesh;
    benchmarks = mesh_chunk, generate_chunk, palette_decode, gather_padded_chunk
);

main!(library_benchmark_groups = world_and_mesh);
