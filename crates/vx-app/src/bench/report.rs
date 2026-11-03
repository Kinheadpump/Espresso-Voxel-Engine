use serde::{Deserialize, Serialize};
use sysinfo::{ProcessesToUpdate, System};
use vx_render::GpuContext;

use super::{memory::GpuMemoryReport, stats::Metric, visibility::VisibilityCounters};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchReport {
    pub schema: u32,
    pub unix_time: u64,
    pub build: BuildInfo,
    pub system: SystemInfo,
    pub gpu: GpuInfo,
    pub scenario: ScenarioInfo,
    pub content: ContentInfo,
    pub results: Results,
    #[serde(default)]
    pub visibility: Option<VisibilityCounters>,
    #[serde(default)]
    pub gpu_memory: Option<GpuMemoryReport>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildInfo {
    pub git_commit: String,
    pub git_dirty: String,
    pub profile: String,
    pub debug_assertions: bool,
    pub rustc: String,
    pub rustflags: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
    pub total_memory_mib: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GpuInfo {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
    pub driver_info: String,
    pub tier: String,
    pub timestamps: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioInfo {
    pub name: String,
    pub seed: i32,
    pub view_radius: u32,
    pub vertical_chunks: u32,
    pub threads: u32,
    pub width: u32,
    pub height: u32,
    pub warmup_frames: u32,
    pub measured_frames: u32,
    pub repeats: u32,
    pub frame_sync: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentInfo {
    pub chunks_generated: usize,
    pub chunks_meshed: usize,
    pub quads: usize,
    pub mesh_hash: String,
    pub voxel_heap_bytes: usize,
    pub geometry_buffer_bytes: u64,
    pub draws: u32,
    #[serde(default = "flat_shading")]
    pub shading: String,
    #[serde(default)]
    pub texture_bytes: u64,
    #[serde(default)]
    pub texture_hash: String,
    #[serde(default = "indirect_validation_on")]
    pub indirect_validation: bool,
}

fn indirect_validation_on() -> bool {
    true
}

fn flat_shading() -> String {
    "flat".into()
}

impl ContentInfo {
    pub fn same_geometry(&self, other: &Self) -> bool {
        self.chunks_generated == other.chunks_generated
            && self.chunks_meshed == other.chunks_meshed
            && self.quads == other.quads
            && self.mesh_hash == other.mesh_hash
            && self.draws == other.draws
    }

    pub fn same_surface(&self, other: &Self) -> bool {
        self.shading == other.shading && self.texture_hash == other.texture_hash
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Results {
    pub world_generation_ms: Metric,
    pub meshing_ms: Metric,
    pub upload_ms: Metric,
    pub frame_cpu_encode_us: Metric,
    #[serde(default)]
    pub frame_cpu_record_us: Option<Metric>,
    #[serde(default)]
    pub frame_cpu_submit_us: Option<Metric>,
    pub frame_gpu_terrain_pass_us: Option<Metric>,
    pub frame_wall_us: Metric,
    pub process_resident_mib: Option<f64>,
    #[serde(default)]
    pub process_peak_resident_mib: Option<f64>,
}

impl BuildInfo {
    pub fn current() -> Self {
        Self {
            git_commit: env!("VX_GIT_COMMIT").to_owned(),
            git_dirty: env!("VX_GIT_DIRTY").to_owned(),
            profile: env!("VX_PROFILE").to_owned(),
            debug_assertions: cfg!(debug_assertions),
            rustc: env!("VX_RUSTC_VERSION").to_owned(),
            rustflags: env!("VX_RUSTFLAGS").to_owned(),
        }
    }
}

impl SystemInfo {
    pub fn current() -> Self {
        let mut system = System::new().ok();
        if let Some(system) = system.as_mut() {
            system.refresh_cpu_all();
            system.refresh_memory();
        }
        let cpu = system
            .as_ref()
            .and_then(|system| {
                system
                    .cpus()
                    .first()
                    .map(|cpu| cpu.brand().trim().to_owned())
            })
            .unwrap_or_else(|| "unknown".into());
        Self {
            os: System::long_os_version().unwrap_or_else(|_| std::env::consts::OS.into()),
            arch: std::env::consts::ARCH.into(),
            cpu,
            physical_cores: System::physical_core_count().ok(),
            logical_cores: std::thread::available_parallelism().map_or(1, usize::from),
            total_memory_mib: system
                .as_ref()
                .map_or(0, |system| system.total_memory() >> 20),
        }
    }
}

impl GpuInfo {
    pub fn of(gpu: &GpuContext) -> Self {
        let info = gpu.adapter.get_info();
        Self {
            name: info.name,
            backend: format!("{:?}", info.backend),
            device_type: format!("{:?}", info.device_type),
            driver: info.driver,
            driver_info: info.driver_info,
            tier: format!("{:?}", gpu.tier),
            timestamps: gpu.timestamps,
        }
    }

    pub fn is_software(&self) -> bool {
        self.device_type == "Cpu"
    }
}

pub fn process_resident_mib() -> Option<f64> {
    let pid = sysinfo::get_current_pid().ok()?;
    let mut system = System::new().ok()?;
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
    system
        .process(pid)
        .map(|process| process.memory() as f64 / f64::from(1u32 << 20))
}
