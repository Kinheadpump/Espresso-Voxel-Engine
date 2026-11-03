use std::time::Duration;

pub struct GpuPassTimer {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    tick_period_ns: f64,
}

const TIMESTAMP_BYTES: u64 = 2 * size_of::<u64>() as u64;

impl GpuPassTimer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("pass_timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let buffer = |label, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: TIMESTAMP_BYTES,
                usage,
                mapped_at_creation: false,
            })
        };
        Some(Self {
            query_set,
            resolve: buffer(
                "timestamp_resolve",
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            ),
            readback: buffer(
                "timestamp_readback",
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            ),
            tick_period_ns: f64::from(queue.get_timestamp_period()),
        })
    }

    pub fn pass_writes(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }
    }

    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.query_set, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, TIMESTAMP_BYTES);
    }

    pub fn read_after_submit(&self, device: &wgpu::Device) -> Option<Duration> {
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
        let ticks: [u64; 2] = {
            let mapped = self.readback.slice(..).get_mapped_range().ok()?;
            bytemuck::pod_read_unaligned(&mapped[..TIMESTAMP_BYTES as usize])
        };
        self.readback.unmap();
        let elapsed_ticks = ticks[1].checked_sub(ticks[0])?;
        Some(Duration::from_nanos(
            (elapsed_ticks as f64 * self.tick_period_ns) as u64,
        ))
    }
}
