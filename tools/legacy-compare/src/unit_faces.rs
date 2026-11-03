use vx_core::Axis;
use vx_mesh::{ChunkMesh, PackedQuad};

use crate::{decaf, lemon};

pub type UnitFaceKeys = Vec<u32>;

const AXIS_X: usize = 0;
const AXIS_Y: usize = 1;
const AXIS_Z: usize = 2;

/// (u axis, v axis) per direction in PosX, NegX, PosY, NegY, PosZ, NegZ order (Decaf order).
const DECAF_UV_AXES: [[usize; 2]; 6] = [
    [AXIS_Y, AXIS_Z],
    [AXIS_Z, AXIS_Y],
    [AXIS_Z, AXIS_X],
    [AXIS_X, AXIS_Z],
    [AXIS_X, AXIS_Y],
    [AXIS_Y, AXIS_X],
];

/// Lemon order NegX, PosX, NegY, PosY, NegZ, PosZ.
const LEMON_UV_AXES: [[usize; 2]; 6] = [
    [AXIS_Z, AXIS_Y],
    [AXIS_Y, AXIS_Z],
    [AXIS_X, AXIS_Z],
    [AXIS_Z, AXIS_X],
    [AXIS_Y, AXIS_X],
    [AXIS_X, AXIS_Y],
];

/// Lemon direction index to vx_core::FaceDir index (PosX, NegX, PosY, NegY, PosZ, NegZ).
const LEMON_TO_ENGINE_DIR: [usize; 6] = [1, 0, 3, 2, 5, 4];

fn key(position: [usize; 3], dir: usize, block: u16) -> u32 {
    position[0] as u32
        | (position[1] as u32) << 5
        | (position[2] as u32) << 10
        | (dir as u32) << 15
        | u32::from(block) << 18
}

fn expand_rect(
    keys: &mut UnitFaceKeys,
    anchor: [usize; 3],
    uv_axes: [usize; 2],
    size: [usize; 2],
    dir: usize,
    block: u16,
) {
    for dv in 0..size[1] {
        for du in 0..size[0] {
            let mut position = anchor;
            position[uv_axes[0]] += du;
            position[uv_axes[1]] += dv;
            keys.push(key(position, dir, block));
        }
    }
}

pub fn from_engine_quads(quads: &[PackedQuad]) -> UnitFaceKeys {
    let mut keys = Vec::new();
    for quad in quads {
        let dir = quad.dir();
        let uv_axes = match dir.axis() {
            Axis::X => [AXIS_Y, AXIS_Z],
            Axis::Y => [AXIS_X, AXIS_Z],
            Axis::Z => [AXIS_X, AXIS_Y],
        };
        let [x, y, z] = quad.origin();
        expand_rect(
            &mut keys,
            [x as usize, y as usize, z as usize],
            uv_axes,
            [quad.width() as usize, quad.height() as usize],
            dir.index(),
            quad.block().0,
        );
    }
    keys.sort_unstable();
    keys
}

pub fn from_engine_mesh(mesh: &ChunkMesh) -> UnitFaceKeys {
    from_engine_quads(&mesh.quads)
}

pub fn from_decaf(slices: &decaf::ChunkMeshSlices) -> (UnitFaceKeys, usize, usize) {
    let mut keys = Vec::new();
    let mut quads = 0;
    let mut transparent_quads = 0;
    for (bucket, per_dir) in slices.faces.iter().enumerate() {
        for (dir, per_depth) in per_dir.iter().enumerate() {
            for faces in per_depth {
                for face in faces {
                    let v = face.value;
                    let anchor = [(v & 31) as usize, (v >> 5 & 31) as usize, (v >> 10 & 31) as usize];
                    let block = (v >> 15 & 0x7F) as u16;
                    let size = [(v >> 22 & 31) as usize + 1, (v >> 27 & 31) as usize + 1];
                    expand_rect(&mut keys, anchor, DECAF_UV_AXES[dir], size, dir, block);
                    quads += 1;
                    transparent_quads += bucket;
                }
            }
        }
    }
    keys.sort_unstable();
    (keys, quads, transparent_quads)
}

pub fn from_lemon(mesh: &lemon::DirectionalMesh) -> (UnitFaceKeys, usize) {
    let mut keys = Vec::new();
    let mut quads = 0;
    for (lemon_dir, faces) in mesh.faces.iter().enumerate() {
        for &v in faces {
            let anchor = [(v & 31) as usize, (v >> 5 & 31) as usize, (v >> 10 & 31) as usize];
            let block = (v >> 15 & 0x7F) as u16;
            let size = [(v >> 22 & 31) as usize + 1, (v >> 27 & 31) as usize + 1];
            expand_rect(
                &mut keys,
                anchor,
                LEMON_UV_AXES[lemon_dir],
                size,
                LEMON_TO_ENGINE_DIR[lemon_dir],
                block,
            );
            quads += 1;
        }
    }
    keys.sort_unstable();
    (keys, quads)
}
