use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::IVec3;
use vx_core::{BlockId, CHUNK_AREA, CHUNK_SIZE, CHUNK_SIZE_I32, ChunkPos};

use crate::{ChunkVoxels, DenseChunk};

#[derive(Clone, Copy, Debug)]
pub struct TerrainSettings {
    pub seed: i32,
    pub sea_level: i32,
    pub amplitude: f32,
    pub frequency: f32,
    pub octaves: i32,
    pub carved_air: Option<BlockBox>,
}

/// Axis-aligned block range, `min` inclusive and `max` exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockBox {
    pub min: IVec3,
    pub max: IVec3,
}

impl BlockBox {
    fn clipped_to_chunk(self, pos: ChunkPos) -> Option<(IVec3, IVec3)> {
        let chunk_min = pos.min_block();
        let local_min = (self.min - chunk_min).max(IVec3::ZERO);
        let local_max = (self.max - chunk_min).min(IVec3::splat(CHUNK_SIZE_I32));
        local_min
            .cmplt(local_max)
            .all()
            .then_some((local_min, local_max))
    }
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            seed: 1337,
            sea_level: 64,
            amplitude: 48.0,
            frequency: 0.004,
            octaves: 5,
            carved_air: None,
        }
    }
}

pub struct TerrainGenerator {
    settings: TerrainSettings,
    height_noise: FastNoiseLite,
}

impl TerrainGenerator {
    pub fn new(settings: TerrainSettings) -> Self {
        let mut height_noise = FastNoiseLite::with_seed(settings.seed);
        height_noise.set_noise_type(Some(NoiseType::OpenSimplex2));
        height_noise.set_fractal_type(Some(FractalType::FBm));
        height_noise.set_fractal_octaves(Some(settings.octaves));
        height_noise.set_frequency(Some(settings.frequency));
        Self {
            settings,
            height_noise,
        }
    }

    pub fn surface_heights(&self, pos: ChunkPos, heights: &mut [i32; CHUNK_AREA]) {
        let origin = pos.min_block();
        for (column, height) in heights.iter_mut().enumerate() {
            let x = (origin.x + (column % CHUNK_SIZE) as i32) as f32;
            let z = (origin.z + (column / CHUNK_SIZE) as i32) as f32;
            let noise = self.height_noise.get_noise_2d(x, z);
            *height = self.settings.sea_level + (noise * self.settings.amplitude) as i32;
        }
    }

    pub fn surface_height_at(&self, x: i32, z: i32) -> i32 {
        let noise = self.height_noise.get_noise_2d(x as f32, z as f32);
        self.settings.sea_level + (noise * self.settings.amplitude) as i32
    }

    pub fn generate(&self, pos: ChunkPos, dense_scratch: &mut DenseChunk) -> ChunkVoxels {
        profiling::scope!("generate_chunk");
        let mut heights = [0i32; CHUNK_AREA];
        self.surface_heights(pos, &mut heights);
        let chunk_bottom = pos.min_block().y;
        let chunk_top = chunk_bottom + CHUNK_SIZE_I32;
        let (lowest, highest) = heights
            .iter()
            .fold((i32::MAX, i32::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
        if chunk_bottom > highest {
            return ChunkVoxels::Uniform(BlockId::AIR);
        }
        let carved = self
            .settings
            .carved_air
            .and_then(|carved| carved.clipped_to_chunk(pos));
        if chunk_top + 4 <= lowest && carved.is_none() {
            return ChunkVoxels::Uniform(BlockId::STONE);
        }
        for y in 0..CHUNK_SIZE {
            let world_y = chunk_bottom + y as i32;
            for (column, &surface) in heights.iter().enumerate() {
                dense_scratch[y * CHUNK_AREA + column] = Self::block_at_depth(surface - world_y);
            }
        }
        if let Some((local_min, local_max)) = carved {
            carve_air(dense_scratch, local_min, local_max);
        }
        ChunkVoxels::from_dense(dense_scratch)
    }

    #[inline(always)]
    fn block_at_depth(depth_below_surface: i32) -> BlockId {
        match depth_below_surface {
            i32::MIN..0 => BlockId::AIR,
            0 => BlockId::GRASS,
            1..4 => BlockId::DIRT,
            _ => BlockId::STONE,
        }
    }
}

fn carve_air(dense: &mut DenseChunk, local_min: IVec3, local_max: IVec3) {
    let x_range = local_min.x as usize..local_max.x as usize;
    for y in local_min.y as usize..local_max.y as usize {
        for z in local_min.z as usize..local_max.z as usize {
            let row_start = y * CHUNK_AREA + z * CHUNK_SIZE;
            dense[row_start + x_range.start..row_start + x_range.end].fill(BlockId::AIR);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_dense_chunk;

    #[test]
    fn carved_box_is_air_and_rest_matches_uncarved() {
        let carved_air = BlockBox {
            min: IVec3::new(4, 8, -3),
            max: IVec3::new(20, 16, 3),
        };
        let plain = TerrainGenerator::new(TerrainSettings::default());
        let carved = TerrainGenerator::new(TerrainSettings {
            carved_air: Some(carved_air),
            ..TerrainSettings::default()
        });
        let mut scratch = new_dense_chunk();
        for pos in [ChunkPos::new(0, 0, 0), ChunkPos::new(0, 0, -1)] {
            let plain_voxels = plain.generate(pos, &mut scratch);
            let carved_voxels = carved.generate(pos, &mut scratch);
            for index in 0..vx_core::CHUNK_VOLUME {
                let local = vx_core::LocalPos::from_index(index);
                let world =
                    pos.min_block() + IVec3::new(local.x.into(), local.y.into(), local.z.into());
                let inside = world.cmpge(carved_air.min).all() && world.cmplt(carved_air.max).all();
                let expected = if inside {
                    BlockId::AIR
                } else {
                    plain_voxels.get(local)
                };
                assert_eq!(carved_voxels.get(local), expected, "at {world}");
            }
        }
    }
}
