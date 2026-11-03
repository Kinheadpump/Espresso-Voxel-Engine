use glam::IVec3;
use vx_core::{BlockId, CHUNK_SIZE, ChunkPos, FaceDir, LocalPos, PADDED_VOLUME, padded_index};

use crate::{ChunkVoxels, ChunkWindow};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingNeighbour(pub ChunkPos);

type Padded = [BlockId; PADDED_VOLUME];

pub fn gather_padded(
    window: &ChunkWindow<ChunkVoxels>,
    center: ChunkPos,
    out: &mut Padded,
) -> Result<(), MissingNeighbour> {
    profiling::scope!("gather_padded");
    let center_voxels = window.get(center).ok_or(MissingNeighbour(center))?;
    let mut neighbours = [center_voxels; 6];
    for (neighbour, dir) in neighbours.iter_mut().zip(FaceDir::ALL) {
        let pos = center.offset(IVec3::from_array(dir.normal()));
        *neighbour = window.get(pos).ok_or(MissingNeighbour(pos))?;
    }

    decode_interior(center_voxels, out);
    for (dir, neighbour) in FaceDir::ALL.into_iter().zip(neighbours) {
        copy_border_slab(dir, neighbour, out);
    }
    Ok(())
}

fn source_row(y: usize, z: usize) -> usize {
    (y * CHUNK_SIZE + z) * CHUNK_SIZE
}

fn padded_row(out: &mut Padded, y: usize, z: usize) -> &mut [BlockId] {
    let start = padded_index(1, y, z);
    &mut out[start..start + CHUNK_SIZE]
}

fn decode_interior(voxels: &ChunkVoxels, out: &mut Padded) {
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            voxels.decode_row(source_row(y, z), padded_row(out, y + 1, z + 1));
        }
    }
}

fn border_depths(dir: FaceDir) -> (usize, usize) {
    if dir.is_positive() {
        (0, CHUNK_SIZE + 1)
    } else {
        (CHUNK_SIZE - 1, 0)
    }
}

fn copy_border_slab(dir: FaceDir, neighbour: &ChunkVoxels, out: &mut Padded) {
    let (source_depth, padded_depth) = border_depths(dir);
    match dir {
        FaceDir::PosY | FaceDir::NegY => {
            for z in 0..CHUNK_SIZE {
                neighbour.decode_row(
                    source_row(source_depth, z),
                    padded_row(out, padded_depth, z + 1),
                );
            }
        }
        FaceDir::PosZ | FaceDir::NegZ => {
            for y in 0..CHUNK_SIZE {
                neighbour.decode_row(
                    source_row(y, source_depth),
                    padded_row(out, y + 1, padded_depth),
                );
            }
        }
        FaceDir::PosX | FaceDir::NegX => copy_x_slab(neighbour, source_depth, padded_depth, out),
    }
}

fn copy_x_slab(neighbour: &ChunkVoxels, source_x: usize, padded_x: usize, out: &mut Padded) {
    if let Some(block) = neighbour.uniform_block() {
        for y in 1..=CHUNK_SIZE {
            for z in 1..=CHUNK_SIZE {
                out[padded_index(padded_x, y, z)] = block;
            }
        }
        return;
    }
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            out[padded_index(padded_x, y + 1, z + 1)] =
                neighbour.get(LocalPos::new(source_x as u8, y as u8, z as u8));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TerrainGenerator, TerrainSettings, new_dense_chunk};

    fn reference_padded(window: &ChunkWindow<ChunkVoxels>, center: ChunkPos) -> Vec<BlockId> {
        let size = CHUNK_SIZE as i32;
        let mut expected = vec![BlockId::AIR; PADDED_VOLUME];
        let origin = center.min_block();
        for y in 0..=size + 1 {
            for z in 0..=size + 1 {
                for x in 0..=size + 1 {
                    let outside = [x, y, z]
                        .iter()
                        .filter(|&&c| c == 0 || c == size + 1)
                        .count();
                    if outside > 1 {
                        continue;
                    }
                    let world = origin + IVec3::new(x - 1, y - 1, z - 1);
                    let chunk = ChunkPos::containing_block(world);
                    expected[padded_index(x as usize, y as usize, z as usize)] =
                        window.get(chunk).map_or(BlockId::AIR, |voxels| {
                            voxels.get(LocalPos::of_world_block(world))
                        });
                }
            }
        }
        expected
    }

    #[test]
    fn gather_matches_per_voxel_lookup_at_sea_level() {
        let generator = TerrainGenerator::new(TerrainSettings::default());
        let center = ChunkPos::containing_block(IVec3::new(96, 64, -160));
        let mut window = ChunkWindow::for_view_radius(2, 3);
        let mut scratch = new_dense_chunk();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let pos = center.offset(IVec3::new(dx, dy, dz));
                    window.insert(pos, generator.generate(pos, &mut scratch));
                }
            }
        }
        let mut padded = vec![BlockId::AIR; PADDED_VOLUME];
        let out: &mut Padded = padded.as_mut_slice().try_into().expect("padded volume");
        gather_padded(&window, center, out).expect("all neighbours generated");
        assert_eq!(padded, reference_padded(&window, center));
    }
}
