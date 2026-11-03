use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use vx_core::ChunkPos;
use vx_mesh::{ChunkMesh, PackedQuad};
use wgpu::util::DeviceExt;

use crate::{CameraUniform, TERRAIN_SHADER, block_textures::BlockTextures};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const VERTICES_PER_QUAD: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DrawIndirectArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shading {
    Flat,
    Textured,
}

impl Shading {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "flat" => Some(Self::Flat),
            "textured" => Some(Self::Textured),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Textured => "textured",
        }
    }
}

pub struct ChunkMeshRef<'a> {
    pub pos: ChunkPos,
    pub mesh: &'a ChunkMesh,
}

struct UploadedGeometry {
    bind_group: wgpu::BindGroup,
    indirect: wgpu::Buffer,
    draw_count: u32,
    quad_count: u64,
    buffer_bytes: u64,
}

pub struct TerrainRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
    block_faces: wgpu::Buffer,
    textures: BlockTextures,
    shading: Shading,
    geometry: Option<UploadedGeometry>,
}

impl TerrainRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        shading: Shading,
    ) -> anyhow::Result<Self> {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain_bind_group_layout"),
            entries: &[
                layout_entry(0, wgpu::BufferBindingType::Uniform),
                layout_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
                layout_entry(2, wgpu::BufferBindingType::Storage { read_only: true }),
                layout_entry(3, wgpu::BufferBindingType::Storage { read_only: true }),
                texture_array_layout_entry(4),
                sampler_layout_entry(5),
            ],
        });
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera_uniform"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let block_faces = storage_buffer(
            device,
            "block_faces",
            bytemuck::cast_slice(&vx_core::block_face_table()),
        );
        Ok(Self {
            pipeline: create_terrain_pipeline(
                device,
                &bind_group_layout,
                color_format,
                TerrainShaderEntry::Shaded(shading),
            ),
            bind_group_layout,
            camera_buffer,
            block_faces,
            textures: BlockTextures::new(device, queue)?,
            shading,
            geometry: None,
        })
    }

    pub fn shading(&self) -> Shading {
        self.shading
    }

    pub fn texture_bytes(&self) -> u64 {
        self.textures.bytes
    }

    pub fn texture_hash(&self) -> u64 {
        self.textures.content_hash
    }

    pub fn upload_static<'a>(
        &mut self,
        device: &wgpu::Device,
        chunks: impl IntoIterator<Item = ChunkMeshRef<'a>>,
    ) {
        let mut quads: Vec<PackedQuad> = Vec::new();
        let mut origins: Vec<[i32; 4]> = Vec::new();
        let mut draws: Vec<DrawIndirectArgs> = Vec::new();
        for chunk in chunks {
            if chunk.mesh.quads.is_empty() {
                continue;
            }
            let slot = origins.len() as u32;
            let base = quads.len() as u32;
            origins.push(chunk.pos.min_block().extend(0).to_array());
            quads.extend_from_slice(&chunk.mesh.quads);
            for range in &chunk.mesh.direction_ranges {
                push_direction_draw(&mut draws, base, range, slot);
            }
        }
        if draws.is_empty() {
            self.geometry = None;
            return;
        }
        debug_assert!(
            draws_within(&draws, quads.len(), origins.len()),
            "indirect draw arguments out of range: unsound with wgpu indirect validation off"
        );
        let quad_buffer = storage_buffer(device, "terrain_quads", bytemuck::cast_slice(&quads));
        let origin_buffer = storage_buffer(device, "chunk_origins", bytemuck::cast_slice(&origins));
        let indirect = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain_indirect"),
            contents: bytemuck::cast_slice(&draws),
            usage: wgpu::BufferUsages::INDIRECT,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: quad_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: origin_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.block_faces.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.textures.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.textures.sampler),
                },
            ],
        });
        let buffer_bytes = quad_buffer.size() + origin_buffer.size() + indirect.size();
        self.geometry = Some(UploadedGeometry {
            bind_group,
            buffer_bytes,
            indirect,
            draw_count: draws.len() as u32,
            quad_count: quads.len() as u64,
        });
    }

    pub fn geometry_buffer_bytes(&self) -> u64 {
        self.geometry
            .as_ref()
            .map_or(0, |geometry| geometry.buffer_bytes)
    }

    pub fn quad_count(&self) -> u64 {
        self.geometry
            .as_ref()
            .map_or(0, |geometry| geometry.quad_count)
    }

    pub fn draw_count(&self) -> u32 {
        self.geometry
            .as_ref()
            .map_or(0, |geometry| geometry.draw_count)
    }

    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &CameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(camera));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.geometry.is_some() {
            pass.set_pipeline(&self.pipeline);
            self.record_draws(pass);
        }
    }

    pub(crate) fn record_draws(&self, pass: &mut wgpu::RenderPass<'_>) {
        let Some(geometry) = &self.geometry else {
            return;
        };
        pass.set_bind_group(0, &geometry.bind_group, &[]);
        pass.multi_draw_indirect(&geometry.indirect, 0, geometry.draw_count);
    }

    pub(crate) fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

fn draws_within(draws: &[DrawIndirectArgs], quad_count: usize, slot_count: usize) -> bool {
    let vertex_limit = quad_count as u64 * u64::from(VERTICES_PER_QUAD);
    draws.iter().all(|draw| {
        u64::from(draw.first_vertex) + u64::from(draw.vertex_count) <= vertex_limit
            && (draw.first_instance as usize) < slot_count
            && draw.instance_count == 1
    })
}

fn push_direction_draw(
    draws: &mut Vec<DrawIndirectArgs>,
    base: u32,
    range: &Range<u32>,
    slot: u32,
) {
    if range.is_empty() {
        return;
    }
    draws.push(DrawIndirectArgs {
        vertex_count: range.len() as u32 * VERTICES_PER_QUAD,
        instance_count: 1,
        first_vertex: (base + range.start) * VERTICES_PER_QUAD,
        first_instance: slot,
    });
}

fn storage_buffer(device: &wgpu::Device, label: &str, contents: &[u8]) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

fn layout_entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_array_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2Array,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_layout_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TerrainShaderEntry {
    Shaded(Shading),
    ChunkId,
}

impl TerrainShaderEntry {
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Shaded(_) => ("vs_main", "fs_main"),
            Self::ChunkId => ("vs_chunk_id", "fs_chunk_id"),
        }
    }
}

pub(crate) fn create_terrain_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
    entry: TerrainShaderEntry,
) -> wgpu::RenderPipeline {
    let (vertex_entry, fragment_entry) = entry.names();
    let textured = f64::from(u8::from(matches!(
        entry,
        TerrainShaderEntry::Shaded(Shading::Textured)
    )));
    let constants = [("TEXTURED", textured)];
    let compilation_options = wgpu::PipelineCompilationOptions {
        constants: &constants,
        ..wgpu::PipelineCompilationOptions::default()
    };
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("terrain_shader"),
        source: wgpu::ShaderSource::Wgsl(TERRAIN_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terrain_pipeline_layout"),
        bind_group_layouts: &[Some(bind_group_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("terrain_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(vertex_entry),
            compilation_options: compilation_options.clone(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(fragment_entry),
            compilation_options,
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{DrawIndirectArgs, draws_within};

    #[test]
    fn draw_range_check_rejects_out_of_range_arguments() {
        let draw = |first_vertex, vertex_count, first_instance| DrawIndirectArgs {
            vertex_count,
            instance_count: 1,
            first_vertex,
            first_instance,
        };
        assert!(draws_within(&[draw(0, 12, 0), draw(12, 6, 1)], 3, 2));
        assert!(!draws_within(&[draw(12, 12, 0)], 3, 2));
        assert!(!draws_within(&[draw(0, 6, 2)], 3, 2));
    }

    #[test]
    fn terrain_shader_parses_and_validates() {
        let module = naga::front::wgsl::parse_str(crate::TERRAIN_SHADER).expect("WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("WGSL validates");
    }
}
