use std::time::Instant;

use vx_render::{
    CameraUniform, GpuContext, GpuPassTimer, OffscreenTarget, TerrainRenderer, begin_terrain_pass,
};
use vx_world::TerrainGenerator;

use super::{
    BenchOptions,
    memory::{self, GpuMemoryReport},
};

pub struct FrameMicros {
    pub cpu_encode: Vec<Vec<f64>>,
    pub cpu_record: Vec<Vec<f64>>,
    pub cpu_submit: Vec<Vec<f64>>,
    pub gpu_pass: Option<Vec<Vec<f64>>>,
    pub wall: Vec<Vec<f64>>,
    pub gpu_memory: Option<GpuMemoryReport>,
}

/// Camera for every frame of one repeat: `warmup` frames then `measured` frames, both walking the
/// scenario path from its start so warm-up touches the same data as the measured frames.
pub fn path_cameras(options: &BenchOptions, terrain: &TerrainGenerator) -> Vec<CameraUniform> {
    let aspect = options.width as f32 / options.height as f32;
    let warmup = (0..options.warmup_frames).map(|frame| frame as f32);
    let measured = (0..options.measured_frames).map(|frame| frame as f32);
    warmup
        .chain(measured)
        .map(|frame| {
            let progress = frame / options.measured_frames as f32;
            options
                .scenario
                .camera_at(terrain, progress.fract())
                .uniform(aspect)
        })
        .collect()
}

pub fn measure(
    options: &BenchOptions,
    gpu: &GpuContext,
    terrain: &TerrainRenderer,
    cameras: &[CameraUniform],
) -> FrameMicros {
    let target = OffscreenTarget::new(&gpu.device, options.width, options.height);
    let color_view = target.color_view();
    let timer = GpuPassTimer::new(&gpu.device, &gpu.queue);
    let mut samples = FrameMicros {
        cpu_encode: Vec::new(),
        cpu_record: Vec::new(),
        cpu_submit: Vec::new(),
        gpu_pass: timer.as_ref().map(|_| Vec::new()),
        wall: Vec::new(),
        gpu_memory: None,
    };
    for _ in 0..options.repeats {
        let mut cpu_encode = Vec::with_capacity(options.measured_frames as usize);
        let mut cpu_record = Vec::with_capacity(options.measured_frames as usize);
        let mut cpu_submit = Vec::with_capacity(options.measured_frames as usize);
        let mut gpu_pass = Vec::with_capacity(options.measured_frames as usize);
        let mut wall = Vec::with_capacity(options.measured_frames as usize);
        for (frame, camera) in (0u32..).zip(cameras) {
            profiling::scope!("bench_frame");
            let measured_index = frame.checked_sub(options.warmup_frames);
            let started = Instant::now();
            terrain.update_camera(&gpu.queue, camera);
            let record_started = Instant::now();
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("bench_frame"),
                });
            {
                let mut pass = begin_terrain_pass(
                    &mut encoder,
                    &color_view,
                    target.depth_view(),
                    timer.as_ref().map(GpuPassTimer::pass_writes),
                );
                terrain.draw(&mut pass);
            }
            if let Some(timer) = &timer {
                timer.resolve(&mut encoder);
            }
            let command_buffer = encoder.finish();
            let submit_started = Instant::now();
            {
                profiling::scope!("queue_submit");
                gpu.queue.submit([command_buffer]);
            }
            let submitted = Instant::now();
            let encoded = submitted - started;
            let gpu_time = if let Some(timer) = &timer {
                timer.read_after_submit(&gpu.device)
            } else {
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).ok();
                None
            };
            let completed = started.elapsed();
            if measured_index.is_some() {
                cpu_encode.push(encoded.as_secs_f64() * 1e6);
                cpu_record.push((submit_started - record_started).as_secs_f64() * 1e6);
                cpu_submit.push((submitted - submit_started).as_secs_f64() * 1e6);
                wall.push(completed.as_secs_f64() * 1e6);
                if let Some(gpu_time) = gpu_time {
                    gpu_pass.push(gpu_time.as_secs_f64() * 1e6);
                }
            }
            profiling::finish_frame!();
        }
        samples.cpu_encode.push(cpu_encode);
        samples.cpu_record.push(cpu_record);
        samples.cpu_submit.push(cpu_submit);
        samples.wall.push(wall);
        if let Some(repeats) = samples.gpu_pass.as_mut() {
            if gpu_pass.is_empty() {
                samples.gpu_pass = None;
            } else {
                repeats.push(gpu_pass);
            }
        }
    }
    samples.gpu_memory = memory::gpu_memory(&gpu.device);
    samples
}
