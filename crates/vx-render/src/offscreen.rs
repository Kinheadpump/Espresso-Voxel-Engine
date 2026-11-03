use crate::{DEPTH_FORMAT, TerrainRenderer, readback::TextureReadback};

pub struct OffscreenTarget {
    pub width: u32,
    pub height: u32,
    color: wgpu::Texture,
    depth_view: wgpu::TextureView,
    readback: TextureReadback,
}

impl OffscreenTarget {
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_color"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let depth_view = create_depth_view(device, width, height);
        Self {
            width,
            height,
            color,
            depth_view,
            readback: TextureReadback::new(device, width, height, 4),
        }
    }

    pub fn color_view(&self) -> wgpu::TextureView {
        self.color
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    pub fn render_rgba(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        terrain: &TerrainRenderer,
    ) -> Vec<u8> {
        let color_view = self
            .color
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("offscreen"),
        });
        {
            let mut pass = begin_terrain_pass(&mut encoder, &color_view, &self.depth_view, None);
            terrain.draw(&mut pass);
        }
        self.readback.copy_from(&mut encoder, &self.color);
        queue.submit([encoder.finish()]);
        self.readback.read_after_submit(device)
    }
}

pub fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

pub fn begin_terrain_pass<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    color_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> wgpu::RenderPass<'e> {
    const SKY: wgpu::Color = wgpu::Color {
        r: 0.34,
        g: 0.54,
        b: 0.83,
        a: 1.0,
    };
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("terrain_pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(SKY),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}
