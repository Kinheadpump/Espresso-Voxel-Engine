pub struct TextureReadback {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    row_bytes: u32,
    padded_bytes_per_row: u32,
}

impl TextureReadback {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, bytes_per_pixel: u32) -> Self {
        let row_bytes = width * bytes_per_pixel;
        let padded_bytes_per_row = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("texture_readback"),
            size: u64::from(padded_bytes_per_row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            width,
            height,
            row_bytes,
            padded_bytes_per_row,
        }
    }

    pub fn copy_from(&self, encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture) {
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn read_after_submit(&self, device: &wgpu::Device) -> Vec<u8> {
        self.buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).ok();
        let mapped = self
            .buffer
            .slice(..)
            .get_mapped_range()
            .expect("readback buffer is mapped after a blocking poll");
        let row_bytes = self.row_bytes as usize;
        let mut bytes = Vec::with_capacity(row_bytes * self.height as usize);
        for row in mapped.chunks_exact(self.padded_bytes_per_row as usize) {
            bytes.extend_from_slice(&row[..row_bytes]);
        }
        drop(mapped);
        self.buffer.unmap();
        bytes
    }
}
