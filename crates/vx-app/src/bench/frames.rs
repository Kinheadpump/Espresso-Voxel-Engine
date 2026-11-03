use std::time::Instant;

use glam::Vec3;
use vx_core::CHUNK_SIZE;
use vx_render::{
    FlyCamera, GpuContext, GpuPassTimer, OffscreenTarget, TerrainRenderer, begin_terrain_pass,
};

use super::BenchOptions;

pub struct FrameMicros {
    pub cpu_encode: Vec<Vec<f64>>,
    pub gpu_pass: Option<Vec<Vec<f64>>>,
    pub wall: Vec<Vec<f64>>,
}

const ORBIT_HEIGHT: f32 = 150.0;
const LOOK_AHEAD_RADIANS: f32 = 0.6;
const LOOK_DOWN_HEIGHT: f32 = 70.0;

fn camera_on_orbit(view_radius: u32, progress: f32) -> FlyCamera {
    let orbit_radius = view_radius as f32 * CHUNK_SIZE as f32 * 0.5;
    let angle = progress * std::f32::consts::TAU;
    let point_on_orbit =
        |angle: f32| Vec3::new(angle.cos() * orbit_radius, 0.0, angle.sin() * orbit_radius);
    let position = point_on_orbit(angle) + Vec3::Y * ORBIT_HEIGHT;
    let target = point_on_orbit(angle + LOOK_AHEAD_RADIANS) + Vec3::Y * LOOK_DOWN_HEIGHT;
    FlyCamera::looking_at(position, target)
}

pub fn measure(options: &BenchOptions, gpu: &GpuContext, terrain: &TerrainRenderer) -> FrameMicros {
    let target = OffscreenTarget::new(&gpu.device, options.width, options.height);
    let color_view = target.color_view();
    let timer = GpuPassTimer::new(&gpu.device, &gpu.queue);
    let aspect = options.width as f32 / options.height as f32;
    let view_radius = options.scenario.view_radius();
    let mut samples = FrameMicros {
        cpu_encode: Vec::new(),
        gpu_pass: timer.as_ref().map(|_| Vec::new()),
        wall: Vec::new(),
    };
    for _ in 0..options.repeats {
        let mut cpu_encode = Vec::with_capacity(options.measured_frames as usize);
        let mut gpu_pass = Vec::with_capacity(options.measured_frames as usize);
        let mut wall = Vec::with_capacity(options.measured_frames as usize);
        for frame in 0..options.warmup_frames + options.measured_frames {
            let measured_index = frame.checked_sub(options.warmup_frames);
            let path_progress =
                measured_index.unwrap_or(frame) as f32 / options.measured_frames as f32;
            let started = Instant::now();
            terrain.update_camera(
                &gpu.queue,
                &camera_on_orbit(view_radius, path_progress).uniform(aspect),
            );
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
            gpu.queue.submit([encoder.finish()]);
            let encoded = started.elapsed();
            let gpu_time = if let Some(timer) = &timer {
                timer.read_after_submit(&gpu.device)
            } else {
                gpu.device.poll(wgpu::PollType::wait_indefinitely()).ok();
                None
            };
            let completed = started.elapsed();
            if measured_index.is_some() {
                cpu_encode.push(encoded.as_secs_f64() * 1e6);
                wall.push(completed.as_secs_f64() * 1e6);
                if let Some(gpu_time) = gpu_time {
                    gpu_pass.push(gpu_time.as_secs_f64() * 1e6);
                }
            }
        }
        samples.cpu_encode.push(cpu_encode);
        samples.wall.push(wall);
        if let Some(repeats) = samples.gpu_pass.as_mut() {
            if gpu_pass.is_empty() {
                samples.gpu_pass = None;
            } else {
                repeats.push(gpu_pass);
            }
        }
    }
    samples
}
