//! B-09 part 1: how many more quads an AO-safe greedy merge produces, and what the 26-neighbour
//! meshing rule (ADR-0006) costs at a circular streaming edge.
//!
//! `cargo run --release -p vx-instr --example ao_probe -- <view_radius_blocks>`

use std::collections::HashMap;

use glam::IVec3;
use rayon::prelude::*;
use vx_core::{Axis, BlockId, CHUNK_SIZE, CHUNK_SIZE_I32, ChunkPos, FaceDir, LocalPos};
use vx_world::{ChunkVoxels, TerrainGenerator, TerrainSettings, new_dense_chunk};

const WORLD_HEIGHT_BLOCKS: usize = 160;
const VERTICAL_CHUNKS: i32 = WORLD_HEIGHT_BLOCKS.div_ceil(CHUNK_SIZE) as i32;

struct World {
    chunks: HashMap<ChunkPos, ChunkVoxels>,
}

impl World {
    fn generate(radius: i32) -> Self {
        let generator = TerrainGenerator::new(TerrainSettings::default());
        let positions: Vec<ChunkPos> = (-radius - 1..=radius + 1)
            .flat_map(|x| {
                (-radius - 1..=radius + 1)
                    .flat_map(move |z| (0..VERTICAL_CHUNKS).map(move |y| ChunkPos::new(x, y, z)))
            })
            .collect();
        let chunks = positions
            .into_par_iter()
            .map_init(new_dense_chunk, |scratch, pos| {
                (pos, generator.generate(pos, scratch))
            })
            .collect();
        Self { chunks }
    }

    fn block(&self, world: IVec3) -> BlockId {
        let chunk = ChunkPos::containing_block(world);
        self.chunks.get(&chunk).map_or(BlockId::AIR, |voxels| {
            voxels.get(LocalPos::of_world_block(world))
        })
    }

    fn opaque(&self, world: IVec3) -> bool {
        self.block(world).is_opaque()
    }
}

fn unit(axis: Axis) -> IVec3 {
    match axis {
        Axis::X => IVec3::X,
        Axis::Y => IVec3::Y,
        Axis::Z => IVec3::Z,
    }
}

fn tangent_axes(axis: Axis) -> (IVec3, IVec3) {
    match axis {
        Axis::X => (IVec3::Z, IVec3::Y),
        Axis::Y => (IVec3::X, IVec3::Z),
        Axis::Z => (IVec3::X, IVec3::Y),
    }
}

/// 0fps vertex AO: 3 = open, 0 = fully occluded; two solid sides occlude fully regardless of the corner.
fn corner_ao(side_a: bool, side_b: bool, corner: bool) -> u8 {
    if side_a && side_b {
        0
    } else {
        3 - u8::from(side_a) - u8::from(side_b) - u8::from(corner)
    }
}

fn face_ao(world: &World, air_cell: IVec3, u: IVec3, v: IVec3) -> u8 {
    let mut packed = 0;
    for (index, (du, dv)) in [(-1, -1), (1, -1), (1, 1), (-1, 1)].into_iter().enumerate() {
        let side_a = world.opaque(air_cell + u * du);
        let side_b = world.opaque(air_cell + v * dv);
        let corner = world.opaque(air_cell + u * du + v * dv);
        packed |= corner_ao(side_a, side_b, corner) << (2 * index);
    }
    packed
}

fn greedy_quad_count(keys: &mut [u32], size: usize) -> usize {
    let mut quads = 0;
    for v in 0..size {
        let mut u = 0;
        while u < size {
            let key = keys[v * size + u];
            if key == 0 {
                u += 1;
                continue;
            }
            let mut width = 1;
            while u + width < size && keys[v * size + u + width] == key {
                width += 1;
            }
            let mut height = 1;
            while v + height < size
                && (0..width).all(|du| keys[(v + height) * size + u + du] == key)
            {
                height += 1;
            }
            for dv in 0..height {
                keys[(v + dv) * size + u..(v + dv) * size + u + width].fill(0);
            }
            quads += 1;
            u += width;
        }
    }
    quads
}

#[derive(Default, Clone, Copy)]
struct Counts {
    plain: usize,
    ao_safe: usize,
}

fn count_chunk(world: &World, pos: ChunkPos) -> Counts {
    let origin = pos.min_block();
    let mut counts = Counts::default();
    let mut plain = vec![0u32; CHUNK_SIZE * CHUNK_SIZE];
    let mut ao_safe = vec![0u32; CHUNK_SIZE * CHUNK_SIZE];
    for dir in FaceDir::ALL {
        let axis = dir.axis();
        let normal = unit(axis) * if dir.is_positive() { 1 } else { -1 };
        let (u_axis, v_axis) = tangent_axes(axis);
        for depth in 0..CHUNK_SIZE_I32 {
            for v in 0..CHUNK_SIZE_I32 {
                for u in 0..CHUNK_SIZE_I32 {
                    let block_pos = origin + unit(axis) * depth + u_axis * u + v_axis * v;
                    let block = world.block(block_pos);
                    let cell = (v * CHUNK_SIZE_I32 + u) as usize;
                    let exposed = block.is_opaque() && !world.opaque(block_pos + normal);
                    plain[cell] = if exposed { u32::from(block.0) + 1 } else { 0 };
                    ao_safe[cell] = if exposed {
                        let ao = face_ao(world, block_pos + normal, u_axis, v_axis);
                        ((u32::from(block.0) + 1) << 8) | u32::from(ao)
                    } else {
                        0
                    };
                }
            }
            counts.plain += greedy_quad_count(&mut plain, CHUNK_SIZE);
            counts.ao_safe += greedy_quad_count(&mut ao_safe, CHUNK_SIZE);
        }
    }
    counts
}

fn meshable_chunks(load_radius: i32, neighbours: &[IVec3]) -> usize {
    let loaded = |x: i32, z: i32| x * x + z * z <= load_radius * load_radius;
    (-load_radius..=load_radius)
        .flat_map(|x| (-load_radius..=load_radius).map(move |z| (x, z)))
        .filter(|&(x, z)| loaded(x, z) && neighbours.iter().all(|n| loaded(x + n.x, z + n.z)))
        .count()
}

fn streaming_edge_report(load_radius: i32) {
    let face: Vec<IVec3> = vec![IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];
    let all: Vec<IVec3> = (-1..=1)
        .flat_map(|x| (-1..=1).map(move |z| IVec3::new(x, 0, z)))
        .filter(|n| *n != IVec3::ZERO)
        .collect();
    let six = meshable_chunks(load_radius, &face);
    let twenty_six = meshable_chunks(load_radius, &all);
    let radius_of = |columns: usize| (columns as f64 / std::f64::consts::PI).sqrt();
    println!(
        "load radius {load_radius} chunks: meshable columns 6-rule {six}, 26-rule {twenty_six} ({:+.1}%), equivalent meshed radius {:.2} -> {:.2} chunks ({:.1} blocks)",
        100.0 * (twenty_six as f64 / six as f64 - 1.0),
        radius_of(six),
        radius_of(twenty_six),
        (radius_of(six) - radius_of(twenty_six)) * CHUNK_SIZE as f64,
    );
}

fn main() {
    let view_blocks: i32 = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(512);
    let radius = (view_blocks + CHUNK_SIZE_I32 / 2) / CHUNK_SIZE_I32;
    println!("CHUNK_SIZE {CHUNK_SIZE}, view {view_blocks} blocks = radius {radius} chunks");
    let world = World::generate(radius);
    let columns: Vec<ChunkPos> = (-radius..=radius)
        .flat_map(|x| {
            (-radius..=radius)
                .flat_map(move |z| (0..VERTICAL_CHUNKS).map(move |y| ChunkPos::new(x, y, z)))
        })
        .collect();
    let total = columns
        .par_iter()
        .map(|&pos| count_chunk(&world, pos))
        .reduce(Counts::default, |a, b| Counts {
            plain: a.plain + b.plain,
            ao_safe: a.ao_safe + b.ao_safe,
        });
    println!(
        "quads: plain greedy {}, AO-safe greedy {} ({:+.1}%)",
        total.plain,
        total.ao_safe,
        100.0 * (total.ao_safe as f64 / total.plain as f64 - 1.0)
    );
    streaming_edge_report(radius);
}
