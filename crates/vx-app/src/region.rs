use std::time::{Duration, Instant};

use rayon::prelude::*;
use vx_core::{BlockId, CHUNK_SIZE, ChunkPos};
use vx_mesh::{BinaryGreedyMesher, ChunkMesh, new_padded_voxels};
use vx_world::{
    ChunkVoxels, ChunkWindow, TerrainGenerator, TerrainSettings, gather_padded, new_dense_chunk,
};

pub const WORLD_HEIGHT_BLOCKS: usize = 160;
pub const VERTICAL_CHUNKS: std::ops::Range<i32> =
    0..WORLD_HEIGHT_BLOCKS.div_ceil(CHUNK_SIZE) as i32;

pub struct MeshedRegion {
    pub meshes: Vec<(ChunkPos, ChunkMesh)>,
    pub generated_chunks: usize,
    pub voxel_heap_bytes: usize,
    pub generate_time: Duration,
    pub mesh_time: Duration,
}

fn columns_within(radius: i32) -> impl Iterator<Item = ChunkPos> {
    (-radius..=radius).flat_map(move |x| {
        (-radius..=radius).flat_map(move |z| VERTICAL_CHUNKS.map(move |y| ChunkPos::new(x, y, z)))
    })
}

pub fn build_region(view_radius: u32, terrain: TerrainSettings) -> MeshedRegion {
    profiling::scope!("build_region");
    let radius = view_radius as i32;
    let generator = TerrainGenerator::new(terrain);
    let generate_started = Instant::now();
    let generated: Vec<(ChunkPos, ChunkVoxels)> = columns_within(radius + 1)
        .collect::<Vec<_>>()
        .into_par_iter()
        .map_init(new_dense_chunk, |scratch, pos| {
            (pos, generator.generate(pos, scratch))
        })
        .collect();
    let generate_time = generate_started.elapsed();
    profiling::scope!("assemble_and_mesh_region");

    let mut window = ChunkWindow::for_view_radius(view_radius + 1, VERTICAL_CHUNKS.len() as u32);
    let generated_chunks = generated.len();
    let voxel_heap_bytes = generated
        .iter()
        .map(|(_, voxels)| voxels.heap_bytes())
        .sum();
    for (pos, voxels) in generated {
        window.insert(pos, voxels);
    }
    let border = ChunkVoxels::Uniform(BlockId::AIR);
    for x in -radius - 1..=radius + 1 {
        for z in -radius - 1..=radius + 1 {
            window.insert(
                ChunkPos::new(x, VERTICAL_CHUNKS.start - 1, z),
                border.clone(),
            );
            window.insert(ChunkPos::new(x, VERTICAL_CHUNKS.end, z), border.clone());
        }
    }

    let mesh_started = Instant::now();
    let meshes = columns_within(radius)
        .filter(|pos| window.get(*pos).and_then(ChunkVoxels::uniform_block) != Some(BlockId::AIR))
        .collect::<Vec<_>>()
        .into_par_iter()
        .map_init(
            || {
                (
                    BinaryGreedyMesher::default(),
                    new_dense_chunk(),
                    new_padded_voxels(),
                )
            },
            |(mesher, dense, padded), pos| {
                let mut mesh = ChunkMesh::default();
                if gather_padded(&window, pos, dense, padded).is_ok() {
                    mesher.mesh(padded, &mut mesh);
                }
                (pos, mesh)
            },
        )
        .filter(|(_, mesh)| !mesh.quads.is_empty())
        .collect();
    MeshedRegion {
        meshes,
        generated_chunks,
        voxel_heap_bytes,
        generate_time,
        mesh_time: mesh_started.elapsed(),
    }
}

impl MeshedRegion {
    pub fn quad_count(&self) -> usize {
        self.meshes.iter().map(|(_, mesh)| mesh.quads.len()).sum()
    }

    pub fn content_hash(&self) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0100_0000_01b3;
        let mix = |hash: u64, value: u64| (hash ^ value).wrapping_mul(FNV_PRIME);
        self.meshes.iter().fold(FNV_OFFSET, |hash, (pos, mesh)| {
            mesh.quads
                .iter()
                .fold(mix(hash, pos.packed_key()), |hash, quad| mix(hash, quad.0))
        })
    }

    pub fn log_summary(&self) {
        let quads = self.quad_count();
        log::info!(
            "region: {} chunks generated in {:.1} ms ({:.1} MiB voxel heap), {} non-empty meshes / {} quads ({:.1} MiB) in {:.1} ms on {} threads",
            self.generated_chunks,
            self.generate_time.as_secs_f64() * 1e3,
            self.voxel_heap_bytes as f64 / f64::from(1u32 << 20),
            self.meshes.len(),
            quads,
            (quads * 8) as f64 / f64::from(1u32 << 20),
            self.mesh_time.as_secs_f64() * 1e3,
            rayon::current_num_threads(),
        );
    }
}
