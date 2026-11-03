use vx_core::{BlockId, CHUNK_SIZE, padded_index};
use vx_mesh::PaddedVoxels;

use crate::{decaf, lemon};

const REGISTERED_SOLID_BLOCKS: u16 = 5;

fn neighbour_plane_coords(axis: usize, positive: bool, a: usize, b: usize) -> ([usize; 3], [usize; 3]) {
    let (local_depth, padded_depth) = if positive { (0, CHUNK_SIZE + 1) } else { (CHUNK_SIZE - 1, 0) };
    let (ua, ub) = ((axis + 1) % 3, (axis + 2) % 3);
    let mut local = [0; 3];
    let mut padded = [0; 3];
    local[axis] = local_depth;
    padded[axis] = padded_depth;
    local[ua] = a;
    padded[ua] = a + 1;
    local[ub] = b;
    padded[ub] = b + 1;
    (local, padded)
}

pub struct DecafInput {
    pub chunk: decaf::Chunk,
    pub neighbors: decaf::NeighborChunks,
    pub registry: decaf::ResolvedBlockRegistry,
}

impl DecafInput {
    pub fn from_padded(padded: &PaddedVoxels) -> Self {
        let mut chunk = decaf::Chunk::new();
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let block = padded[padded_index(x + 1, y + 1, z + 1)];
                    chunk.set(x, y, z, decaf::Voxel::from_block_id(decaf::BlockId(block.0)));
                }
            }
        }
        let neighbors = std::array::from_fn(|dir| {
            let (axis, positive) = (dir / 2, dir % 2 == 0);
            let mut neighbour = decaf::Chunk::new();
            for a in 0..CHUNK_SIZE {
                for b in 0..CHUNK_SIZE {
                    let (l, p) = neighbour_plane_coords(axis, positive, a, b);
                    let block = padded[padded_index(p[0], p[1], p[2])];
                    neighbour.set(l[0], l[1], l[2], decaf::Voxel::from_block_id(decaf::BlockId(block.0)));
                }
            }
            neighbour
        });
        Self {
            chunk,
            neighbors: decaf::NeighborChunks { chunks: neighbors },
            registry: decaf::ResolvedBlockRegistry::uniform_opaque(REGISTERED_SOLID_BLOCKS),
        }
    }
}

pub fn decaf_mesh<const PROFILE: bool>(
    input: &DecafInput,
    out: &mut decaf::ChunkMeshSlices,
) -> decaf::MeshingBuildProfile {
    decaf::rebuild_chunk_mesh_slices::<PROFILE>(&input.chunk, &input.neighbors, &input.registry, out)
}

pub struct LemonInput {
    pub chunk: Box<lemon::Chunk>,
    pub neighbors: [Box<lemon::Chunk>; 6],
}

impl LemonInput {
    pub fn from_padded(padded: &PaddedVoxels) -> Self {
        let mut chunk = Box::new(lemon::Chunk::empty());
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let block: BlockId = padded[padded_index(x + 1, y + 1, z + 1)];
                    chunk.set_block(x as i32, y as i32, z as i32, block.0);
                }
            }
        }
        let neighbors = std::array::from_fn(|dir| {
            let (axis, positive) = (dir / 2, dir % 2 == 1);
            let mut neighbour = Box::new(lemon::Chunk::empty());
            for a in 0..CHUNK_SIZE {
                for b in 0..CHUNK_SIZE {
                    let (l, p) = neighbour_plane_coords(axis, positive, a, b);
                    let block = padded[padded_index(p[0], p[1], p[2])];
                    neighbour.set_block(l[0] as i32, l[1] as i32, l[2] as i32, block.0);
                }
            }
            neighbour
        });
        Self { chunk, neighbors }
    }
}

pub fn lemon_mesh(input: &LemonInput) -> lemon::DirectionalMesh {
    let neighbors = std::array::from_fn(|dir| Some(&*input.neighbors[dir]));
    lemon::mesh_chunk(&input.chunk, 0, 0, 0, neighbors, |_, _, _| false)
}
