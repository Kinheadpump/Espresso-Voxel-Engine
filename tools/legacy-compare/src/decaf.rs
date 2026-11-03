#![allow(dead_code, clippy::all, clippy::pedantic)]
use std::{cell::RefCell, sync::Arc, thread_local, time::Instant};

pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
pub const CHUNK_COLUMN_COUNT: usize = CHUNK_SIZE * CHUNK_SIZE;
pub const DEFAULT_BIOME_TINT: [u8; 3] = [255, 255, 255];
const SLICE_AREA: usize = CHUNK_SIZE * CHUNK_SIZE;

pub const FACE_TINT_MODE_NONE: u32 = 0;
pub const FACE_TINT_MODE_GRASS: u32 = 1;
pub const FACE_TINT_MODE_MULTIPLY: u32 = 2;
pub const DEFAULT_FACE_TINT: u32 = pack_face_tint(FACE_TINT_MODE_NONE, [255, 255, 255]);

#[inline]
pub const fn pack_rgb8(color: [u8; 3]) -> u32 {
    (color[0] as u32) | ((color[1] as u32) << 8) | ((color[2] as u32) << 16)
}

#[inline]
pub const fn pack_face_tint(mode: u32, color: [u8; 3]) -> u32 {
    pack_rgb8(color) | ((mode & 0xff) << 24)
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceDir {
    PosX = 0,
    NegX = 1,
    PosY = 2,
    NegY = 3,
    PosZ = 4,
    NegZ = 5,
}

impl FaceDir {
    pub const ALL: [FaceDir; 6] =
        [FaceDir::PosX, FaceDir::NegX, FaceDir::PosY, FaceDir::NegY, FaceDir::PosZ, FaceDir::NegZ];
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(pub u16);

impl BlockId {
    pub const AIR: Self = Self(0);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Voxel(pub u32);

impl Voxel {
    pub const AIR: Self = Self(BlockId::AIR.0 as u32);

    #[inline]
    pub const fn from_block_id(block_id: BlockId) -> Self {
        Self(block_id.0 as u32)
    }

    #[inline]
    pub fn block_id(self) -> BlockId {
        BlockId(self.0 as u16)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockFlags(pub u16);

impl BlockFlags {
    pub const SOLID: Self = Self(1 << 0);
    pub const OPAQUE: Self = Self(1 << 1);
    pub const TRANSPARENT: Self = Self(1 << 2);
    pub const NO_CULL: Self = Self(1 << 3);

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BiomeTint {
    #[default]
    None = 0,
    Grass = 1,
    Foliage = 2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResolvedFaceTints {
    pub pos_x: BiomeTint,
    pub neg_x: BiomeTint,
    pub pos_y: BiomeTint,
    pub neg_y: BiomeTint,
    pub pos_z: BiomeTint,
    pub neg_z: BiomeTint,
}

impl ResolvedFaceTints {
    pub fn get(&self, dir: FaceDir) -> BiomeTint {
        use FaceDir::*;
        match dir {
            PosX => self.pos_x,
            NegX => self.neg_x,
            PosY => self.pos_y,
            NegY => self.neg_y,
            PosZ => self.pos_z,
            NegZ => self.neg_z,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResolvedFaceTextures {
    pub pos_x: u16,
    pub neg_x: u16,
    pub pos_y: u16,
    pub neg_y: u16,
    pub pos_z: u16,
    pub neg_z: u16,
}

impl ResolvedFaceTextures {
    pub fn get(&self, dir: FaceDir) -> u16 {
        use FaceDir::*;
        match dir {
            PosX => self.pos_x,
            NegX => self.neg_x,
            PosY => self.pos_y,
            NegY => self.neg_y,
            PosZ => self.pos_z,
            NegZ => self.neg_z,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResolvedBlock {
    pub id: BlockId,
    pub flags: BlockFlags,
    pub textures: ResolvedFaceTextures,
    pub tints: ResolvedFaceTints,
}

impl ResolvedBlock {
    #[inline]
    pub fn is_air(&self) -> bool {
        self.id == BlockId::AIR
    }

    #[inline]
    pub fn is_opaque(&self) -> bool {
        self.flags.contains(BlockFlags::OPAQUE)
    }

    #[inline]
    pub fn is_transparent(&self) -> bool {
        self.flags.contains(BlockFlags::TRANSPARENT)
    }

    #[inline]
    pub fn is_no_cull(&self) -> bool {
        self.flags.contains(BlockFlags::NO_CULL)
    }
}

#[derive(Clone)]
pub struct ResolvedBlockRegistry {
    blocks: Vec<ResolvedBlock>,
}

impl ResolvedBlockRegistry {
    #[inline]
    pub fn air(&self) -> ResolvedBlock {
        self.blocks[BlockId::AIR.0 as usize]
    }

    pub fn get(&self, id: BlockId) -> ResolvedBlock {
        self.blocks.get(id.0 as usize).copied().unwrap_or_else(|| self.air())
    }

    pub fn get_voxel(&self, voxel: Voxel) -> ResolvedBlock {
        self.get(voxel.block_id())
    }

    /// Registry with air plus `solid_block_count` opaque solid blocks; block `n` uses texture `n`
    /// on every face and carries no biome tint.
    pub fn uniform_opaque(solid_block_count: u16) -> Self {
        let mut blocks = Vec::new();
        for id in 0..=solid_block_count {
            let flags = if id == 0 {
                BlockFlags(0)
            } else {
                BlockFlags::SOLID.union(BlockFlags::OPAQUE)
            };
            blocks.push(ResolvedBlock {
                id: BlockId(id),
                flags,
                textures: ResolvedFaceTextures {
                    pos_x: id,
                    neg_x: id,
                    pos_y: id,
                    neg_y: id,
                    pos_z: id,
                    neg_z: id,
                },
                tints: ResolvedFaceTints::default(),
            });
        }
        Self { blocks }
    }
}

#[inline]
pub fn voxel_index(x: usize, y: usize, z: usize) -> usize {
    x + z * CHUNK_SIZE + y * CHUNK_SIZE * CHUNK_SIZE
}

#[inline]
pub fn column_index(x: usize, z: usize) -> usize {
    x + z * CHUNK_SIZE
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnBiomeTints {
    pub grass: [u8; 3],
    pub foliage: [u8; 3],
}

impl Default for ColumnBiomeTints {
    fn default() -> Self {
        Self { grass: DEFAULT_BIOME_TINT, foliage: DEFAULT_BIOME_TINT }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LocalVoxelPos([u32; 3]);

impl LocalVoxelPos {
    #[inline]
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        Self([x, y, z])
    }

    #[inline]
    pub fn x(self) -> usize {
        self.0[0] as usize
    }

    #[inline]
    pub fn y(self) -> usize {
        self.0[1] as usize
    }

    #[inline]
    pub fn z(self) -> usize {
        self.0[2] as usize
    }
}

#[derive(Clone)]
pub struct Chunk {
    pub voxels: Arc<[Voxel; CHUNK_VOLUME]>,
    pub column_biome_tints: Arc<[ColumnBiomeTints; CHUNK_COLUMN_COUNT]>,
    pub generation: u32,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            voxels: Arc::new([Voxel::AIR; CHUNK_VOLUME]),
            column_biome_tints: Arc::new([ColumnBiomeTints::default(); CHUNK_COLUMN_COUNT]),
            generation: 0,
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Voxel {
        self.voxels[voxel_index(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, voxel: Voxel) {
        Arc::make_mut(&mut self.voxels)[voxel_index(x, y, z)] = voxel;
        self.generation = self.generation.wrapping_add(1);
    }

    #[inline]
    pub fn get_local(&self, local: LocalVoxelPos) -> Voxel {
        self.get(local.x(), local.y(), local.z())
    }

    #[inline]
    pub fn biome_tints(&self, x: usize, z: usize) -> ColumnBiomeTints {
        self.column_biome_tints[column_index(x, z)]
    }
}

/// Six neighbour chunks indexed by `FaceDir as usize`; only the plane adjacent to the centre
/// chunk is populated (all that the mesher reads).
pub struct NeighborChunks {
    pub chunks: [Chunk; 6],
}

impl NeighborChunks {
    #[inline]
    fn get_chunk_neighbor(&self, dir: FaceDir) -> Option<&Chunk> {
        Some(&self.chunks[dir as usize])
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PackedFace {
    pub value: u32,
    pub tint: u32,
}

impl PackedFace {
    pub fn pack(
        x: u32,
        y: u32,
        z: u32,
        texture_id: u32,
        width_minus_one: u32,
        height_minus_one: u32,
        tint: u32,
    ) -> Self {
        Self {
            value: x
                | (y << 5)
                | (z << 10)
                | (texture_id << 15)
                | (width_minus_one << 22)
                | (height_minus_one << 27),
            tint,
        }
    }
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderBucket {
    #[default]
    Opaque = 0,
    Transparent = 1,
}

impl RenderBucket {
    pub const ALL: [Self; 2] = [Self::Opaque, Self::Transparent];
}

pub struct ChunkMeshCpu {
    pub faces: [[Vec<PackedFace>; 6]; 2],
    pub source_generation: u32,
}

impl ChunkMeshCpu {
    pub fn new() -> Self {
        Self {
            faces: std::array::from_fn(|_| std::array::from_fn(|_| Vec::new())),
            source_generation: 0,
        }
    }
}

#[derive(Clone)]
pub struct ChunkMeshSlices {
    pub faces: [[[Vec<PackedFace>; CHUNK_SIZE]; 6]; 2],
    pub source_generation: u32,
}

impl ChunkMeshSlices {
    pub fn new() -> Self {
        Self {
            faces: std::array::from_fn(|_| {
                std::array::from_fn(|_| std::array::from_fn(|_| Vec::new()))
            }),
            source_generation: 0,
        }
    }

    pub fn flatten(&self) -> ChunkMeshCpu {
        let mut out = ChunkMeshCpu::new();

        for bucket in RenderBucket::ALL {
            for dir in 0..6usize {
                let dst = &mut out.faces[bucket as usize][dir];
                let total_faces: usize =
                    self.faces[bucket as usize][dir].iter().map(Vec::len).sum();
                dst.reserve(total_faces);
                for depth in 0..CHUNK_SIZE {
                    dst.extend_from_slice(&self.faces[bucket as usize][dir][depth]);
                }
            }
        }

        out.source_generation = self.source_generation;
        out
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeshingBuildProfile {
    pub faces_emitted: u32,
    pub slice_buffer_growths: u32,
    pub dirty_slice_count: u32,
    pub build_cpu_time_ns: u64,
    pub snapshot_capture_cpu_time_ns: u64,
    pub slice_construction_cpu_time_ns: u64,
    pub greedy_merge_cpu_time_ns: u64,
    pub flatten_cpu_time_ns: u64,
}

impl MeshingBuildProfile {
    pub(crate) fn recompute_total(&mut self) {
        self.build_cpu_time_ns = self
            .snapshot_capture_cpu_time_ns
            .saturating_add(self.slice_construction_cpu_time_ns)
            .saturating_add(self.greedy_merge_cpu_time_ns)
            .saturating_add(self.flatten_cpu_time_ns);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MaskCell {
    visible: bool,
    block_id: BlockId,
    texture_id: u16,
    bucket: RenderBucket,
    tint: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DirtySlice {
    dir: FaceDir,
    depth: usize,
}

struct MeshingScratch {
    mask: Box<[MaskCell; SLICE_AREA]>,
    used: Box<[bool; SLICE_AREA]>,
    neighbor_voxels: Box<[Voxel; SLICE_AREA]>,
    dirty_slices: Vec<DirtySlice>,
}

impl Default for MeshingScratch {
    fn default() -> Self {
        Self {
            mask: Box::new([MaskCell::default(); SLICE_AREA]),
            used: Box::new([false; SLICE_AREA]),
            neighbor_voxels: Box::new([Voxel::AIR; SLICE_AREA]),
            dirty_slices: Vec::with_capacity(FaceDir::ALL.len() * CHUNK_SIZE),
        }
    }
}

thread_local! {
    static MESHING_SCRATCH: RefCell<MeshingScratch> =
        RefCell::new(MeshingScratch::default());
}

fn collect_all_slices(out: &mut Vec<DirtySlice>) {
    out.clear();
    for dir in FaceDir::ALL {
        for depth in 0..CHUNK_SIZE {
            out.push(DirtySlice { dir, depth });
        }
    }
}

/// `PROFILE = true` keeps the shipped `Instant::now()` pairs; `false` compiles them out.
pub fn rebuild_chunk_mesh_slices<const PROFILE: bool>(
    chunk: &Chunk,
    neighbors: &NeighborChunks,
    resolved_blocks: &ResolvedBlockRegistry,
    out: &mut ChunkMeshSlices,
) -> MeshingBuildProfile {
    let profile = MESHING_SCRATCH.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        collect_all_slices(&mut scratch.dirty_slices);

        let mut profile = MeshingBuildProfile {
            dirty_slice_count: scratch.dirty_slices.len() as u32,
            ..Default::default()
        };

        for dirty_slice_index in 0..scratch.dirty_slices.len() {
            let dirty_slice = scratch.dirty_slices[dirty_slice_index];
            let (opaque_buckets, transparent_buckets) = out.faces.split_at_mut(1);
            let slice_faces = [
                &mut opaque_buckets[RenderBucket::Opaque as usize][dirty_slice.dir as usize]
                    [dirty_slice.depth],
                &mut transparent_buckets[0][dirty_slice.dir as usize][dirty_slice.depth],
            ];
            build_direction_slice_into::<PROFILE>(
                chunk,
                neighbors,
                resolved_blocks,
                dirty_slice,
                slice_faces,
                &mut scratch,
                &mut profile,
            );
        }

        profile
    });

    out.source_generation = chunk.generation;
    let mut profile = profile;
    profile.recompute_total();
    profile
}

fn build_direction_slice_into<const PROFILE: bool>(
    chunk: &Chunk,
    neighbors: &NeighborChunks,
    resolved_blocks: &ResolvedBlockRegistry,
    dirty_slice: DirtySlice,
    mut out: [&mut Vec<PackedFace>; 2],
    scratch: &mut MeshingScratch,
    profile: &mut MeshingBuildProfile,
) {
    let dir = dirty_slice.dir;
    let depth = dirty_slice.depth;
    let axis_map = SliceAxisMap::for_face(dir);
    let mask = &mut scratch.mask[..];
    let used = &mut scratch.used[..];
    let neighbor_voxels = &mut scratch.neighbor_voxels[..];
    let slice_construction_started_at = PROFILE.then(Instant::now);

    prefetch_neighbor_slice(chunk, neighbors, axis_map, dirty_slice, neighbor_voxels);
    used.fill(false);

    for faces in &mut out {
        faces.clear();
    }

    for v_index in 0..CHUNK_SIZE {
        for u_index in 0..CHUNK_SIZE {
            let slice_index = slice_index(u_index, v_index);
            let local = axis_map.local(depth, u_index, v_index);
            let voxel = chunk.get_local(local);
            let block_id = voxel.block_id();
            let block = resolved_blocks.get_voxel(voxel);

            if block.is_air() {
                mask[slice_index] = MaskCell::default();
                continue;
            }

            let neighbor_voxel = neighbor_voxels[slice_index];
            let neighbor_id = neighbor_voxel.block_id();
            let neighbor = resolved_blocks.get_voxel(neighbor_voxel);

            let visible = face_visible_between(block_id, block, neighbor_id, neighbor);

            mask[slice_index] = if visible {
                MaskCell {
                    visible: true,
                    block_id,
                    texture_id: block.textures.get(dir),
                    bucket: if block.is_transparent() {
                        RenderBucket::Transparent
                    } else {
                        RenderBucket::Opaque
                    },
                    tint: resolve_face_tint(chunk, block, dir, local.x(), local.z()),
                }
            } else {
                MaskCell::default()
            };
        }
    }
    if let Some(started_at) = slice_construction_started_at {
        profile.slice_construction_cpu_time_ns = profile
            .slice_construction_cpu_time_ns
            .saturating_add(duration_to_nanos(started_at.elapsed()));
    }

    let greedy_merge_started_at = PROFILE.then(Instant::now);

    for v_index in 0..CHUNK_SIZE {
        for u_index in 0..CHUNK_SIZE {
            let cell_index = slice_index(u_index, v_index);
            if used[cell_index] {
                continue;
            }

            let cell = mask[cell_index];
            if !cell.visible {
                continue;
            }

            let block_id = cell.block_id;
            let texture_id = cell.texture_id;
            let bucket = cell.bucket;

            let mut width = 1usize;
            while u_index + width < CHUNK_SIZE {
                let index = slice_index(u_index + width, v_index);
                let c = mask[index];
                if used[index]
                    || !c.visible
                    || c.block_id != block_id
                    || c.texture_id != texture_id
                    || c.bucket != bucket
                    || c.tint != cell.tint
                {
                    break;
                }
                width += 1;
            }

            let mut height = 1usize;
            'outer: while v_index + height < CHUNK_SIZE {
                for x in 0..width {
                    let index = slice_index(u_index + x, v_index + height);
                    let c = mask[index];
                    if used[index]
                        || !c.visible
                        || c.block_id != block_id
                        || c.texture_id != texture_id
                        || c.bucket != bucket
                        || c.tint != cell.tint
                    {
                        break 'outer;
                    }
                }
                height += 1;
            }

            for dy in 0..height {
                let row_start = slice_index(u_index, v_index + dy);
                for used_cell in used[row_start..row_start + width].iter_mut() {
                    *used_cell = true;
                }
            }

            let anchor = axis_map.local(depth, u_index, v_index);

            let packed = PackedFace::pack(
                anchor.x() as u32,
                anchor.y() as u32,
                anchor.z() as u32,
                texture_id as u32,
                (width as u32) - 1,
                (height as u32) - 1,
                cell.tint,
            );

            let faces = &mut out[bucket as usize];
            let previous_capacity = faces.capacity();
            faces.push(packed);
            profile.faces_emitted += 1;
            if faces.capacity() > previous_capacity {
                profile.slice_buffer_growths += 1;
            }
        }
    }
    if let Some(started_at) = greedy_merge_started_at {
        profile.greedy_merge_cpu_time_ns = profile
            .greedy_merge_cpu_time_ns
            .saturating_add(duration_to_nanos(started_at.elapsed()));
    }
}

fn prefetch_neighbor_slice(
    center_chunk: &Chunk,
    neighbors: &NeighborChunks,
    axis_map: SliceAxisMap,
    dirty_slice: DirtySlice,
    out: &mut [Voxel],
) {
    let boundary_face = is_boundary_slice(dirty_slice.dir, dirty_slice.depth);
    if boundary_face {
        let Some(neighbor_chunk) = neighbors.get_chunk_neighbor(dirty_slice.dir) else {
            out.fill(Voxel::AIR);
            return;
        };
        let neighbor_depth = boundary_neighbor_depth(dirty_slice.dir);
        fill_slice_voxels(neighbor_chunk, axis_map, neighbor_depth, out);
    } else {
        let neighbor_depth = adjacent_depth(dirty_slice.dir, dirty_slice.depth);
        fill_slice_voxels(center_chunk, axis_map, neighbor_depth, out);
    }
}

fn fill_slice_voxels(chunk: &Chunk, axis_map: SliceAxisMap, depth: usize, out: &mut [Voxel]) {
    for v_index in 0..CHUNK_SIZE {
        let row_start = v_index * CHUNK_SIZE;
        for u_index in 0..CHUNK_SIZE {
            let local = axis_map.local(depth, u_index, v_index);
            out[row_start + u_index] = chunk.get_local(local);
        }
    }
}

#[inline]
fn slice_index(u: usize, v: usize) -> usize {
    u + v * CHUNK_SIZE
}

#[inline]
fn is_boundary_slice(dir: FaceDir, depth: usize) -> bool {
    match dir {
        FaceDir::PosX | FaceDir::PosY | FaceDir::PosZ => depth + 1 == CHUNK_SIZE,
        FaceDir::NegX | FaceDir::NegY | FaceDir::NegZ => depth == 0,
    }
}

#[inline]
fn adjacent_depth(dir: FaceDir, depth: usize) -> usize {
    match dir {
        FaceDir::PosX | FaceDir::PosY | FaceDir::PosZ => depth + 1,
        FaceDir::NegX | FaceDir::NegY | FaceDir::NegZ => depth - 1,
    }
}

#[inline]
fn boundary_neighbor_depth(dir: FaceDir) -> usize {
    match dir {
        FaceDir::PosX | FaceDir::PosY | FaceDir::PosZ => 0,
        FaceDir::NegX | FaceDir::NegY | FaceDir::NegZ => CHUNK_SIZE - 1,
    }
}

#[derive(Clone, Copy)]
pub struct SliceAxisMap {
    x_from: usize,
    y_from: usize,
    z_from: usize,
}

impl SliceAxisMap {
    #[inline]
    pub fn for_face(dir: FaceDir) -> Self {
        match dir {
            FaceDir::PosX => Self { x_from: 0, y_from: 1, z_from: 2 },
            FaceDir::NegX => Self { x_from: 0, y_from: 2, z_from: 1 },
            FaceDir::PosY => Self { x_from: 2, y_from: 0, z_from: 1 },
            FaceDir::NegY => Self { x_from: 1, y_from: 0, z_from: 2 },
            FaceDir::PosZ => Self { x_from: 1, y_from: 2, z_from: 0 },
            FaceDir::NegZ => Self { x_from: 2, y_from: 1, z_from: 0 },
        }
    }

    #[inline]
    pub fn local(self, depth: usize, u: usize, v: usize) -> LocalVoxelPos {
        let components = [depth as u32, u as u32, v as u32];
        LocalVoxelPos::new(
            components[self.x_from],
            components[self.y_from],
            components[self.z_from],
        )
    }
}

fn face_visible_between(
    current_id: BlockId,
    current: ResolvedBlock,
    neighbor_id: BlockId,
    neighbor: ResolvedBlock,
) -> bool {
    if current.is_air() {
        return false;
    }

    if current.is_no_cull() {
        return true;
    }

    if neighbor.is_air() {
        return true;
    }

    if neighbor.is_opaque() && !neighbor.is_no_cull() {
        return false;
    }

    if current_id == neighbor_id && current.is_transparent() && neighbor.is_transparent() {
        return false;
    }

    true
}

fn resolve_face_tint(chunk: &Chunk, block: ResolvedBlock, dir: FaceDir, x: usize, z: usize) -> u32 {
    let tint = chunk.biome_tints(x, z);

    match block.tints.get(dir) {
        BiomeTint::None => DEFAULT_FACE_TINT,
        BiomeTint::Grass => pack_face_tint(FACE_TINT_MODE_GRASS, tint.grass),
        BiomeTint::Foliage => pack_face_tint(FACE_TINT_MODE_MULTIPLY, tint.foliage),
    }
}

#[inline]
fn duration_to_nanos(duration: std::time::Duration) -> u64 {
    duration.as_nanos().min(u128::from(u64::MAX)) as u64
}
