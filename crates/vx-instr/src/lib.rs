//! Deterministic inputs for the instruction-count benches.

use vx_core::{BlockId, ChunkPos, PADDED_SIZE, padded_index};
use vx_mesh::{PaddedVoxels, new_padded_voxels};
use vx_world::{
    ChunkVoxels, ChunkWindow, TerrainGenerator, TerrainSettings, gather_padded, new_dense_chunk,
};

pub const SURFACE_CHUNK: ChunkPos = ChunkPos::new(3, 2, -5);

pub fn surface_neighbourhood() -> ChunkWindow<ChunkVoxels> {
    let generator = TerrainGenerator::new(TerrainSettings::default());
    let mut window = ChunkWindow::for_view_radius(2, 2);
    let mut scratch = new_dense_chunk();
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let pos = ChunkPos::new(
                    SURFACE_CHUNK.0.x + dx,
                    SURFACE_CHUNK.0.y + dy,
                    SURFACE_CHUNK.0.z + dz,
                );
                window.insert(pos, generator.generate(pos, &mut scratch));
            }
        }
    }
    window
}

pub fn padded_surface_chunk() -> Box<PaddedVoxels> {
    let window = surface_neighbourhood();
    let mut padded = new_padded_voxels();
    gather_padded(&window, SURFACE_CHUNK, &mut new_dense_chunk(), &mut padded)
        .expect("all 26 neighbours were generated");
    padded
}

pub fn padded_checkerboard_chunk() -> Box<PaddedVoxels> {
    let mut padded = new_padded_voxels();
    for y in 0..PADDED_SIZE {
        for z in 0..PADDED_SIZE {
            for x in 0..PADDED_SIZE {
                if (x + y + z) % 2 == 0 {
                    padded[padded_index(x, y, z)] = BlockId::STONE;
                }
            }
        }
    }
    padded
}

pub fn packed_surface_chunk() -> ChunkVoxels {
    let generator = TerrainGenerator::new(TerrainSettings::default());
    let voxels = generator.generate(SURFACE_CHUNK, &mut new_dense_chunk());
    assert!(
        matches!(voxels, ChunkVoxels::Packed(_)),
        "surface chunk must exercise the palette path"
    );
    voxels
}
