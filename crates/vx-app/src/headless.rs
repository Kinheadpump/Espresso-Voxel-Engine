use std::path::Path;

use anyhow::{Context, Result};
use glam::{IVec3, Vec3};
use vx_core::CHUNK_SIZE_I32;
use vx_render::{ChunkMeshRef, FlyCamera, GpuContext, OffscreenTarget, TerrainRenderer};
use vx_world::{TerrainGenerator, TerrainSettings};

use crate::{options::Options, region};

pub fn probe() -> Result<()> {
    let gpu = pollster::block_on(GpuContext::new(GpuContext::create_instance(), None))?;
    println!("{}", gpu.describe());
    Ok(())
}

pub fn screenshot(options: &Options, path: &Path) -> Result<()> {
    const WIDTH: u32 = 1280;
    const HEIGHT: u32 = 720;
    let gpu = pollster::block_on(GpuContext::new(
        GpuContext::create_instance_with(options.indirect_validation),
        None,
    ))?;
    let terrain_settings = options
        .screenshot_view
        .map_or_else(TerrainSettings::default, |view| {
            view.scenario.terrain_settings()
        });
    let region = region::build_region(options.view_radius, terrain_settings);
    region.log_summary();

    let mut terrain = TerrainRenderer::new(
        &gpu.device,
        &gpu.queue,
        OffscreenTarget::FORMAT,
        options.shading,
    )?;
    let shift = IVec3::new(options.world_shift_chunks, 0, options.world_shift_chunks);
    terrain.upload_static(
        &gpu.device,
        region.meshes.iter().map(|(pos, mesh)| ChunkMeshRef {
            pos: pos.offset(shift),
            mesh,
        }),
    );
    let mut camera = options.screenshot_view.map_or_else(
        || FlyCamera::looking_at(Vec3::new(-60.0, 110.0, -60.0), Vec3::new(20.0, 70.0, 20.0)),
        |view| {
            view.scenario
                .camera_at(&TerrainGenerator::new(terrain_settings), view.progress)
        },
    );
    camera.anchor += shift * CHUNK_SIZE_I32;
    terrain.update_camera(&gpu.queue, &camera.uniform(WIDTH as f32 / HEIGHT as f32));
    let target = OffscreenTarget::new(&gpu.device, WIDTH, HEIGHT);
    let pixels = target.render_rgba(&gpu.device, &gpu.queue, &terrain);

    let file = std::fs::File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    log::info!(
        "wrote {} ({} draws, {} quads)",
        path.display(),
        terrain.draw_count(),
        terrain.quad_count()
    );
    Ok(())
}
