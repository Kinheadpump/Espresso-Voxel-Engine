use std::{sync::Arc, time::Instant};

use anyhow::{Context, Result, anyhow};
use vx_render::{
    CameraUniform, GpuContext, TerrainRenderer, begin_terrain_pass, create_depth_view,
};
use vx_world::TerrainGenerator;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window, WindowId},
};

use super::{
    BenchOptions, MeasuredWorld, RenderedRun, content_info, frames, log_frame_plan, measure_upload,
    measure_world, memory, report::GpuInfo, warn_if_software,
};

const PRESENT_MODE_PREFERENCE: [wgpu::PresentMode; 2] =
    [wgpu::PresentMode::Immediate, wgpu::PresentMode::Mailbox];

pub fn run(options: &BenchOptions) -> Result<(MeasuredWorld, RenderedRun)> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut bench = WindowedBench {
        options,
        session: None,
        outcome: None,
    };
    event_loop.run_app(&mut bench)?;
    bench
        .outcome
        .context("window closed before the windowed bench finished")?
}

struct WindowedBench<'o> {
    options: &'o BenchOptions,
    session: Option<Session>,
    outcome: Option<Result<(MeasuredWorld, RenderedRun)>>,
}

struct Session {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    depth_view: wgpu::TextureView,
    gpu: GpuContext,
    gpu_info: GpuInfo,
    terrain: TerrainRenderer,
    world: MeasuredWorld,
    upload_ms: Vec<f64>,
    cameras: Vec<CameraUniform>,
    samples: RepeatSamples,
    completed: frames::FrameMicros,
    frame: usize,
    previous_frame_started: Option<Instant>,
    resized_while_measuring: bool,
}

#[derive(Default)]
struct RepeatSamples {
    cpu_encode: Vec<f64>,
    cpu_record: Vec<f64>,
    cpu_submit: Vec<f64>,
    interval: Vec<f64>,
}

fn micros(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1e6
}

fn choose_present_mode(capabilities: &wgpu::SurfaceCapabilities) -> wgpu::PresentMode {
    PRESENT_MODE_PREFERENCE
        .into_iter()
        .find(|mode| capabilities.present_modes.contains(mode))
        .unwrap_or_else(|| {
            log::warn!("no tearing or mailbox present mode: frame pacing is vsync-bound");
            wgpu::PresentMode::Fifo
        })
}

impl Session {
    fn new(event_loop: &ActiveEventLoop, options: &BenchOptions) -> Result<Self> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("vx bench")
                    .with_fullscreen(Some(Fullscreen::Borderless(None))),
            )?,
        );
        let instance = GpuContext::create_instance_with(options.indirect_validation);
        let surface = instance.create_surface(window.clone())?;
        let gpu = pollster::block_on(GpuContext::new(instance, Some(&surface)))?;
        let gpu_info = GpuInfo::of(&gpu);
        warn_if_software(&gpu_info);
        let size = window.inner_size();
        let mut surface_config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| anyhow!("surface unsupported by adapter"))?;
        surface_config.present_mode = choose_present_mode(&surface.get_capabilities(&gpu.adapter));
        surface.configure(&gpu.device, &surface_config);
        let depth_view =
            create_depth_view(&gpu.device, surface_config.width, surface_config.height);

        let world = measure_world(options)?;
        let mut terrain = TerrainRenderer::new(
            &gpu.device,
            &gpu.queue,
            surface_config.format,
            options.shading,
        )?;
        let upload_ms = measure_upload(options, &gpu, &mut terrain, &world.region);
        let aspect = surface_config.width as f32 / surface_config.height as f32;
        let generator = TerrainGenerator::new(options.scenario.terrain_settings());
        let cameras = frames::path_cameras(options, &generator, aspect);
        log_frame_plan(options, surface_config.width, surface_config.height);
        log::info!("present mode {:?}", surface_config.present_mode);
        Ok(Self {
            window,
            surface,
            surface_config,
            depth_view,
            gpu,
            gpu_info,
            terrain,
            world,
            upload_ms,
            cameras,
            samples: RepeatSamples::default(),
            completed: frames::FrameMicros {
                cpu_encode: Vec::new(),
                cpu_record: Vec::new(),
                cpu_submit: Vec::new(),
                gpu_pass: None,
                wall: Vec::new(),
                gpu_memory: None,
            },
            frame: 0,
            previous_frame_started: None,
            resized_while_measuring: false,
        })
    }

    fn frames_per_repeat(&self) -> usize {
        self.cameras.len()
    }

    fn total_frames(&self, options: &BenchOptions) -> usize {
        self.frames_per_repeat() * options.repeats as usize
    }

    fn is_measuring(&self, options: &BenchOptions) -> bool {
        self.frame % self.frames_per_repeat() >= options.warmup_frames as usize
    }

    fn resize(&mut self, width: u32, height: u32, options: &BenchOptions) {
        if (width, height) == (self.surface_config.width, self.surface_config.height) {
            return;
        }
        self.resized_while_measuring |= self.is_measuring(options);
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.surface
            .configure(&self.gpu.device, &self.surface_config);
        self.depth_view = create_depth_view(
            &self.gpu.device,
            self.surface_config.width,
            self.surface_config.height,
        );
    }

    fn acquire(&mut self) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface
                    .configure(&self.gpu.device, &self.surface_config);
                None
            }
            _ => None,
        }
    }

    fn render_frame(&mut self, options: &BenchOptions) {
        profiling::scope!("bench_windowed_frame");
        let frame_started = Instant::now();
        let Some(surface_texture) = self.acquire() else {
            return;
        };
        let encode_started = Instant::now();
        let camera = &self.cameras[self.frame % self.frames_per_repeat()];
        self.terrain.update_camera(&self.gpu.queue, camera);
        let record_started = Instant::now();
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bench_windowed_frame"),
            });
        {
            let mut pass = begin_terrain_pass(&mut encoder, &view, &self.depth_view, None);
            self.terrain.draw(&mut pass);
        }
        let command_buffer = encoder.finish();
        let submit_started = Instant::now();
        {
            profiling::scope!("queue_submit_present");
            self.gpu.queue.submit([command_buffer]);
            self.gpu.queue.present(surface_texture);
        }
        let presented = Instant::now();
        if self.is_measuring(options)
            && let Some(previous) = self.previous_frame_started
        {
            self.samples
                .cpu_encode
                .push(micros(presented - encode_started));
            self.samples
                .cpu_record
                .push(micros(submit_started - record_started));
            self.samples
                .cpu_submit
                .push(micros(presented - submit_started));
            self.samples.interval.push(micros(frame_started - previous));
        }
        self.previous_frame_started = Some(frame_started);
        self.frame += 1;
        if self.frame.is_multiple_of(self.frames_per_repeat()) {
            self.close_repeat();
        }
        profiling::finish_frame!();
    }

    fn close_repeat(&mut self) {
        let samples = std::mem::take(&mut self.samples);
        self.completed.cpu_encode.push(samples.cpu_encode);
        self.completed.cpu_record.push(samples.cpu_record);
        self.completed.cpu_submit.push(samples.cpu_submit);
        self.completed.wall.push(samples.interval);
    }

    fn finish(mut self, options: &BenchOptions) -> (MeasuredWorld, RenderedRun) {
        if self.resized_while_measuring {
            log::warn!("surface resized during measured frames: pacing includes reconfiguration");
        }
        self.completed.gpu_memory = memory::gpu_memory(&self.gpu.device);
        let frame_sync = format!(
            "windowed borderless swapchain, present mode {:?}, max frame latency {}, no CPU wait; frame_wall_us is the present-to-present interval",
            self.surface_config.present_mode, self.surface_config.desired_maximum_frame_latency
        );
        let rendered = RenderedRun {
            gpu: self.gpu_info,
            frame_sync,
            width: self.surface_config.width,
            height: self.surface_config.height,
            content: content_info(options, &self.world.region, &self.terrain),
            upload_ms: self.upload_ms,
            frames: self.completed,
            visibility: None,
        };
        drop(self.window);
        (self.world, rendered)
    }
}

impl ApplicationHandler for WindowedBench<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.session.is_some() || self.outcome.is_some() {
            return;
        }
        match Session::new(event_loop, self.options) {
            Ok(session) => self.session = Some(session),
            Err(error) => {
                self.outcome = Some(Err(error));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(session) = self.session.as_mut() {
                    session.resize(size.width, size.height, self.options);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.render_frame(self.options);
        if session.frame >= session.total_frames(self.options) {
            let session = self.session.take().expect("session checked above");
            self.outcome = Some(Ok(session.finish(self.options)));
            event_loop.exit();
        }
    }
}
