use vx_core::{BlockId, ChunkPos, PADDED_SIZE, padded_index};
use vx_mesh::{PaddedVoxels, new_padded_voxels};
use vx_world::{
    ChunkVoxels, ChunkWindow, TerrainGenerator, TerrainSettings, gather_padded, new_dense_chunk,
};

pub const SCENE_NAMES: [&str; 5] = ["surface", "solid", "random50", "checker", "deep"];

pub fn build(name: &str) -> Box<PaddedVoxels> {
    match name {
        "surface" => surface(),
        "solid" => solid_with_air_border(),
        "random50" => random_half_fill(),
        "checker" => checkerboard(),
        "deep" => deep_with_pockets(),
        _ => panic!("unknown scene {name}"),
    }
}

fn xorshift(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn surface() -> Box<PaddedVoxels> {
    let generator = TerrainGenerator::new(TerrainSettings::default());
    let center = ChunkPos::new(3, 2, -5);
    let mut window: ChunkWindow<ChunkVoxels> = ChunkWindow::for_view_radius(2, 2);
    let mut scratch = new_dense_chunk();
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let pos = ChunkPos::new(center.0.x + dx, center.0.y + dy, center.0.z + dz);
                window.insert(pos, generator.generate(pos, &mut scratch));
            }
        }
    }
    let mut padded = new_padded_voxels();
    gather_padded(&window, center, &mut padded).expect("neighbours present");
    padded
}

fn solid_with_air_border() -> Box<PaddedVoxels> {
    let mut padded = new_padded_voxels();
    for y in 1..=vx_core::CHUNK_SIZE {
        for z in 1..=vx_core::CHUNK_SIZE {
            for x in 1..=vx_core::CHUNK_SIZE {
                padded[padded_index(x, y, z)] = BlockId::STONE;
            }
        }
    }
    padded
}

fn random_half_fill() -> Box<PaddedVoxels> {
    let mut rng = 0x9E37_79B9_7F4A_7C15_u64;
    let mut padded = new_padded_voxels();
    for block in padded.iter_mut() {
        let roll = xorshift(&mut rng);
        if roll & 1 == 0 {
            *block = BlockId(1 + ((roll >> 32) % 3) as u16);
        }
    }
    padded
}

fn checkerboard() -> Box<PaddedVoxels> {
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

fn deep_with_pockets() -> Box<PaddedVoxels> {
    let mut rng = 0x0123_4567_89AB_CDEF_u64;
    let mut padded = new_padded_voxels();
    padded.fill(BlockId::STONE);
    for _ in 0..24 {
        let roll = xorshift(&mut rng);
        let size = 2 + (roll & 1) as usize;
        let origin = |shift: u32| 2 + ((roll >> shift) % 28) as usize;
        let (ox, oy, oz) = (origin(8), origin(24), origin(40));
        for y in oy..(oy + size).min(PADDED_SIZE - 1) {
            for z in oz..(oz + size).min(PADDED_SIZE - 1) {
                for x in ox..(ox + size).min(PADDED_SIZE - 1) {
                    padded[padded_index(x, y, z)] = BlockId::AIR;
                }
            }
        }
    }
    padded
}
