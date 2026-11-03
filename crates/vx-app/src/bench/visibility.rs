use glam::{Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use vx_core::{CHUNK_SIZE, ChunkPos, FaceDir};
use vx_mesh::ChunkMesh;
use vx_render::{CameraUniform, ChunkIdPass, GpuContext, TerrainRenderer};

/// One rung of the culling ladder: what would be submitted if culling stopped at this stage.
/// Values are means over the sampled frames.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CullingRung {
    pub stage: String,
    pub chunks: f64,
    pub draws: f64,
    pub quads: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VisibilityCounters {
    pub sampled_frames: u32,
    pub ladder: Vec<CullingRung>,
}

pub struct ChunkDrawTable {
    chunk_min: Vec<Vec3>,
    quads_per_direction: Vec<[u32; 6]>,
}

#[derive(Clone, Copy, Default)]
struct RungTally {
    chunks: u64,
    draws: u64,
    quads: u64,
}

impl RungTally {
    fn add_chunk(&mut self, draws: u32, quads: u32) {
        if draws > 0 {
            self.chunks += 1;
            self.draws += u64::from(draws);
            self.quads += u64::from(quads);
        }
    }
}

const STAGES: [&str; 4] = ["resident", "frustum", "frustum+facing", "pixel-visible"];
const CHUNK_EXTENT: f32 = CHUNK_SIZE as f32;

impl ChunkDrawTable {
    /// Slot order matches `TerrainRenderer::upload_static`: non-empty meshes in iteration order.
    pub fn new<'a>(meshes: impl IntoIterator<Item = (ChunkPos, &'a ChunkMesh)>) -> Self {
        let mut table = Self {
            chunk_min: Vec::new(),
            quads_per_direction: Vec::new(),
        };
        for (pos, mesh) in meshes {
            if mesh.quads.is_empty() {
                continue;
            }
            table.chunk_min.push(pos.min_block().as_vec3());
            table.quads_per_direction.push(
                mesh.direction_ranges
                    .clone()
                    .map(|range| range.len() as u32),
            );
        }
        table
    }

    fn tally_frame(
        &self,
        camera: &CameraUniform,
        visible_slots: Option<&[bool]>,
    ) -> [RungTally; 4] {
        let planes = frustum_planes(camera.view_proj);
        let eye = Vec4::from_array(camera.position).truncate();
        let mut tallies = [RungTally::default(); 4];
        for (slot, (&min, quads)) in self
            .chunk_min
            .iter()
            .zip(&self.quads_per_direction)
            .enumerate()
        {
            let (all_draws, all_quads) = draws_and_quads(quads, |_| true);
            tallies[0].add_chunk(all_draws, all_quads);
            if !aabb_in_frustum(&planes, min, min + Vec3::splat(CHUNK_EXTENT)) {
                continue;
            }
            tallies[1].add_chunk(all_draws, all_quads);
            let (facing_draws, facing_quads) =
                draws_and_quads(quads, |dir| direction_may_face(dir, min, eye));
            tallies[2].add_chunk(facing_draws, facing_quads);
            if visible_slots.is_some_and(|visible| visible[slot]) {
                tallies[3].add_chunk(facing_draws, facing_quads);
            }
        }
        tallies
    }
}

fn draws_and_quads(quads: &[u32; 6], include: impl Fn(FaceDir) -> bool) -> (u32, u32) {
    FaceDir::ALL
        .into_iter()
        .filter(|&dir| quads[dir.index()] > 0 && include(dir))
        .fold((0, 0), |(draws, total), dir| {
            (draws + 1, total + quads[dir.index()])
        })
}

/// Left, right, bottom, top and near planes of a reverse-Z infinite projection (Gribb-Hartmann).
/// The far plane is at infinity and never rejects.
fn frustum_planes(view_proj: Mat4) -> [Vec4; 5] {
    let [r0, r1, r2, r3] = [0, 1, 2, 3].map(|row| view_proj.row(row));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 - r2]
}

fn aabb_in_frustum(planes: &[Vec4; 5], min: Vec3, max: Vec3) -> bool {
    planes.iter().all(|plane| {
        let normal = plane.truncate();
        let farthest_corner = Vec3::select(normal.cmpge(Vec3::ZERO), max, min);
        normal.dot(farthest_corner) + plane.w >= 0.0
    })
}

/// True if at least one face of this direction inside the chunk can face the eye.
/// Positive faces of a chunk lie on planes `min+1 ..= min+CHUNK_SIZE`, negative ones on `min ..= min+CHUNK_SIZE-1`.
fn direction_may_face(dir: FaceDir, chunk_min: Vec3, eye: Vec3) -> bool {
    let axis = dir.axis().index();
    let eye_on_axis = eye[axis];
    let min_on_axis = chunk_min[axis];
    if dir.is_positive() {
        eye_on_axis > min_on_axis + 1.0
    } else {
        eye_on_axis < min_on_axis + CHUNK_EXTENT - 1.0
    }
}

pub fn measure(
    gpu: &GpuContext,
    terrain: &TerrainRenderer,
    table: &ChunkDrawTable,
    cameras: &[CameraUniform],
    resolution: (u32, u32),
) -> VisibilityCounters {
    let id_pass = ChunkIdPass::new(&gpu.device, terrain, resolution.0, resolution.1);
    let mut visible = vec![false; table.chunk_min.len()];
    let mut sums = [[0u64; 3]; 4];
    for camera in cameras {
        terrain.update_camera(&gpu.queue, camera);
        visible.fill(false);
        for slot_plus_one in id_pass.render_slot_ids(&gpu.device, &gpu.queue, terrain) {
            if let Some(slot) = slot_plus_one.checked_sub(1) {
                visible[slot as usize] = true;
            }
        }
        for (sum, tally) in sums
            .iter_mut()
            .zip(table.tally_frame(camera, Some(&visible)))
        {
            sum[0] += tally.chunks;
            sum[1] += tally.draws;
            sum[2] += tally.quads;
        }
    }
    let frames = cameras.len().max(1) as f64;
    VisibilityCounters {
        sampled_frames: cameras.len() as u32,
        ladder: STAGES
            .iter()
            .zip(sums)
            .map(|(stage, [chunks, draws, quads])| CullingRung {
                stage: (*stage).into(),
                chunks: chunks as f64 / frames,
                draws: draws as f64 / frames,
                quads: quads as f64 / frames,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vx_render::FlyCamera;

    fn camera_looking_down_x() -> CameraUniform {
        FlyCamera::looking_at(Vec3::new(0.0, 16.0, 16.0), Vec3::new(100.0, 16.0, 16.0))
            .uniform(16.0 / 9.0)
    }

    #[test]
    fn frustum_keeps_chunks_ahead_and_rejects_chunks_behind() {
        let planes = frustum_planes(camera_looking_down_x().view_proj);
        let chunk = |x: f32| (Vec3::new(x, 0.0, 0.0), Vec3::new(x + 32.0, 32.0, 32.0));
        let (ahead_min, ahead_max) = chunk(64.0);
        let (behind_min, behind_max) = chunk(-96.0);
        let (around_min, around_max) = chunk(-16.0);
        assert!(aabb_in_frustum(&planes, ahead_min, ahead_max));
        assert!(!aabb_in_frustum(&planes, behind_min, behind_max));
        assert!(aabb_in_frustum(&planes, around_min, around_max));
    }

    #[test]
    fn facing_test_matches_face_planes() {
        let chunk_min = Vec3::ZERO;
        assert!(direction_may_face(
            FaceDir::PosX,
            chunk_min,
            Vec3::new(1.5, 0.0, 0.0)
        ));
        assert!(!direction_may_face(
            FaceDir::PosX,
            chunk_min,
            Vec3::new(1.0, 0.0, 0.0)
        ));
        assert!(direction_may_face(
            FaceDir::NegY,
            chunk_min,
            Vec3::new(0.0, CHUNK_EXTENT - 1.5, 0.0)
        ));
        assert!(!direction_may_face(
            FaceDir::NegY,
            chunk_min,
            Vec3::new(0.0, CHUNK_EXTENT - 1.0, 0.0)
        ));
    }

    #[test]
    fn ladder_rungs_never_grow() {
        let mesh = ChunkMesh {
            quads: vec![vx_mesh::PackedQuad(0); 6],
            direction_ranges: [0..1, 1..2, 2..3, 3..4, 4..5, 5..6],
        };
        let positions = [ChunkPos::new(2, 0, 0), ChunkPos::new(-3, 0, 0)];
        let table = ChunkDrawTable::new(positions.iter().map(|&pos| (pos, &mesh)));
        let tallies = table.tally_frame(&camera_looking_down_x(), Some(&[true, false]));
        assert_eq!(tallies[0].chunks, 2);
        assert_eq!(tallies[1].chunks, 1);
        assert_eq!((tallies[2].chunks, tallies[2].draws), (1, 5));
        assert_eq!(tallies[3].chunks, 1);
        for pair in tallies.windows(2) {
            assert!(pair[1].quads <= pair[0].quads);
        }
    }
}
