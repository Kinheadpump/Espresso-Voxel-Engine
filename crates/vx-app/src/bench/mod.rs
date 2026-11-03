mod compare;
mod frames;
mod report;
mod scenario;
mod stats;
mod visibility;

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result};
use vx_render::{CameraUniform, ChunkMeshRef, GpuContext, Shading, TerrainRenderer};
use vx_world::{TerrainGenerator, TerrainSettings};

pub use compare::compare;
use report::{BenchReport, BuildInfo, ContentInfo, GpuInfo, Results, ScenarioInfo, SystemInfo};
pub use scenario::Scenario;
use stats::Metric;

use crate::region::{self, MeshedRegion, VERTICAL_CHUNKS};

pub struct BenchOptions {
    pub scenario: Scenario,
    pub repeats: u32,
    pub warmup_frames: u32,
    pub measured_frames: u32,
    pub threads: u32,
    pub width: u32,
    pub height: u32,
    pub shading: Shading,
    pub out: Option<PathBuf>,
}

impl BenchOptions {
    pub fn for_scenario(scenario: Scenario) -> Self {
        Self {
            scenario,
            repeats: 5,
            warmup_frames: 120,
            measured_frames: 600,
            threads: std::thread::available_parallelism().map_or(1, |n| n.get() as u32),
            width: 1920,
            height: 1080,
            shading: Shading::Textured,
            out: None,
        }
    }
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

pub fn run(options: &BenchOptions) -> Result<()> {
    let build = BuildInfo::current();
    if build.debug_assertions {
        log::warn!("DEBUG BUILD: results are not comparable; use `cargo run --release -- bench`");
    }
    let terrain_settings = options.scenario.terrain_settings();
    let gpu = pollster::block_on(GpuContext::new(GpuContext::create_instance(), None))?;
    let gpu_info = GpuInfo::of(&gpu);
    if gpu_info.is_software() {
        log::warn!(
            "software adapter ({}): GPU timings are not representative of real hardware",
            gpu_info.name
        );
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads as usize)
        .build()?;

    log::info!(
        "world: 1 warmup + {} repeats of generate + mesh at radius {}",
        options.repeats,
        options.scenario.view_radius()
    );
    let (region, generation_ms, meshing_ms) = measure_region(options, terrain_settings, &pool)?;
    let process_resident_mib = report::process_resident_mib();

    let mut terrain = TerrainRenderer::new(
        &gpu.device,
        &gpu.queue,
        vx_render::OffscreenTarget::FORMAT,
        options.shading,
    )?;
    let upload_ms = measure_upload(options, &gpu, &mut terrain, &region);

    log::info!(
        "frames: {} repeats of {} warmup + {} measured at {}x{}",
        options.repeats,
        options.warmup_frames,
        options.measured_frames,
        options.width,
        options.height
    );
    let (frames, visibility) = measure_path(options, &gpu, &terrain, &region);

    let report = BenchReport {
        schema: report::SCHEMA_VERSION,
        unix_time: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        build,
        system: SystemInfo::current(),
        gpu: gpu_info,
        scenario: ScenarioInfo {
            name: options.scenario.name().into(),
            seed: terrain_settings.seed,
            view_radius: options.scenario.view_radius(),
            vertical_chunks: VERTICAL_CHUNKS.len() as u32,
            threads: options.threads,
            width: options.width,
            height: options.height,
            warmup_frames: options.warmup_frames,
            measured_frames: options.measured_frames,
            repeats: options.repeats,
            frame_sync: "offscreen, CPU waits for GPU completion every frame".into(),
        },
        content: ContentInfo {
            chunks_generated: region.generated_chunks,
            chunks_meshed: region.meshes.len(),
            quads: region.quad_count(),
            mesh_hash: format!("{:016x}", region.content_hash()),
            voxel_heap_bytes: region.voxel_heap_bytes,
            geometry_buffer_bytes: terrain.geometry_buffer_bytes(),
            draws: terrain.draw_count(),
            shading: terrain.shading().name().into(),
            texture_bytes: match terrain.shading() {
                Shading::Flat => 0,
                Shading::Textured => terrain.texture_bytes(),
            },
            texture_hash: match terrain.shading() {
                Shading::Flat => String::new(),
                Shading::Textured => format!("{:016x}", terrain.texture_hash()),
            },
        },
        results: Results {
            world_generation_ms: Metric::from_single_samples("ms", &generation_ms),
            meshing_ms: Metric::from_single_samples("ms", &meshing_ms),
            upload_ms: Metric::from_single_samples("ms", &upload_ms),
            frame_cpu_encode_us: Metric::from_repeats("us", &frames.cpu_encode),
            frame_gpu_terrain_pass_us: frames
                .gpu_pass
                .map(|repeats| Metric::from_repeats("us", &repeats)),
            frame_wall_us: Metric::from_repeats("us", &frames.wall),
            process_resident_mib,
        },
        visibility: Some(visibility),
    };
    let path = options
        .out
        .clone()
        .unwrap_or_else(|| default_output_path(&report));
    write_report(&path, &report)?;
    compare::print_report(&report);
    println!("\nwrote {}", path.display());
    Ok(())
}

const VISIBILITY_SAMPLES: usize = 24;

fn measure_path(
    options: &BenchOptions,
    gpu: &GpuContext,
    terrain: &TerrainRenderer,
    region: &MeshedRegion,
) -> (frames::FrameMicros, visibility::VisibilityCounters) {
    let generator = TerrainGenerator::new(options.scenario.terrain_settings());
    let cameras = frames::path_cameras(options, &generator);
    let frames = frames::measure(options, gpu, terrain, &cameras);
    log::info!("visibility: culling ladder over {VISIBILITY_SAMPLES} path frames");
    let visibility = visibility::measure(
        gpu,
        terrain,
        &visibility::ChunkDrawTable::new(region.meshes.iter().map(|(pos, mesh)| (*pos, mesh))),
        &sampled_path_cameras(&cameras[options.warmup_frames as usize..]),
        (options.width, options.height),
    );
    (frames, visibility)
}

fn sampled_path_cameras(measured: &[CameraUniform]) -> Vec<CameraUniform> {
    let stride = measured.len().div_ceil(VISIBILITY_SAMPLES).max(1);
    measured.iter().step_by(stride).copied().collect()
}

fn measure_region(
    options: &BenchOptions,
    terrain_settings: TerrainSettings,
    pool: &rayon::ThreadPool,
) -> Result<(MeshedRegion, Vec<f64>, Vec<f64>)> {
    let mut generation_ms = Vec::new();
    let mut meshing_ms = Vec::new();
    let mut last: Option<MeshedRegion> = None;
    pool.install(|| region::build_region(options.scenario.view_radius(), terrain_settings));
    for _ in 0..options.repeats {
        let region =
            pool.install(|| region::build_region(options.scenario.view_radius(), terrain_settings));
        generation_ms.push(millis(region.generate_time));
        meshing_ms.push(millis(region.mesh_time));
        if let Some(previous) = &last {
            anyhow::ensure!(
                previous.content_hash() == region.content_hash(),
                "world output differs between repeats: generation or meshing is not deterministic"
            );
        }
        last = Some(region);
    }
    Ok((
        last.context("at least one repeat")?,
        generation_ms,
        meshing_ms,
    ))
}

fn measure_upload(
    options: &BenchOptions,
    gpu: &GpuContext,
    terrain: &mut TerrainRenderer,
    region: &MeshedRegion,
) -> Vec<f64> {
    (0..=options.repeats)
        .map(|_| {
            let started = Instant::now();
            terrain.upload_static(
                &gpu.device,
                region
                    .meshes
                    .iter()
                    .map(|(pos, mesh)| ChunkMeshRef { pos: *pos, mesh }),
            );
            gpu.queue.submit([]);
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).ok();
            millis(started.elapsed())
        })
        .skip(1)
        .collect()
}

fn default_output_path(report: &BenchReport) -> PathBuf {
    let dirty = if report.build.git_dirty == "false" {
        ""
    } else {
        "-dirty"
    };
    Path::new("benchmarks/results").join(format!(
        "{}-{}{}-{}.json",
        report.scenario.name, report.build.git_commit, dirty, report.unix_time
    ))
}

fn write_report(path: &Path, report: &BenchReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json).with_context(|| format!("write {}", path.display()))
}
