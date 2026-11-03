use crate::{
    TerrainRenderer,
    offscreen::create_depth_view,
    readback::TextureReadback,
    terrain::{TerrainShaderEntry, create_terrain_pipeline},
};

/// Renders the terrain with each pixel holding `chunk_slot + 1` (0 = sky), to count chunks that
/// contribute at least one pixel: the ceiling a perfect occlusion culler could reach.
pub struct ChunkIdPass {
    pipeline: wgpu::RenderPipeline,
    ids: wgpu::Texture,
    depth_view: wgpu::TextureView,
    readback: TextureReadback,
}

impl ChunkIdPass {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;

    pub fn new(device: &wgpu::Device, terrain: &TerrainRenderer, width: u32, height: u32) -> Self {
        let ids = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("chunk_ids"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Self {
            pipeline: create_terrain_pipeline(
                device,
                terrain.bind_group_layout(),
                Self::FORMAT,
                TerrainShaderEntry::ChunkId,
            ),
            ids,
            depth_view: create_depth_view(device, width, height),
            readback: TextureReadback::new(device, width, height, 4),
        }
    }

    pub fn render_slot_ids(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        terrain: &TerrainRenderer,
    ) -> Vec<u32> {
        let ids_view = self
            .ids
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("chunk_id_frame"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("chunk_id_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &ids_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            terrain.record_draws(&mut pass);
        }
        self.readback.copy_from(&mut encoder, &self.ids);
        queue.submit([encoder.finish()]);
        bytemuck::cast_slice(&self.readback.read_after_submit(device)).to_vec()
    }
}
