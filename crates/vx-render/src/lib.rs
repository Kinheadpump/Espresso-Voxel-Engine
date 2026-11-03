mod block_textures;
mod camera;
mod chunk_id;
mod gpu;
mod gpu_timer;
mod offscreen;
mod readback;
mod terrain;

pub use camera::{CameraUniform, FlyCamera};
pub use chunk_id::ChunkIdPass;
pub use gpu::{GpuContext, IndirectValidation, RenderTier};
pub use gpu_timer::GpuPassTimer;
pub use offscreen::{OffscreenTarget, begin_terrain_pass, create_depth_view};
pub use terrain::{ChunkMeshRef, DEPTH_FORMAT, Shading, TerrainRenderer};

pub const TERRAIN_SHADER: &str = include_str!("shaders/terrain.wgsl");
