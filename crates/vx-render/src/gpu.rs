use anyhow::{Context, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderTier {
    GpuDrivenWithCount,
    MultiDrawIndirect,
}

impl RenderTier {
    fn from_features(features: wgpu::Features) -> Self {
        if features.contains(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT) {
            Self::GpuDrivenWithCount
        } else {
            Self::MultiDrawIndirect
        }
    }
}

pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub tier: RenderTier,
    pub timestamps: bool,
}

/// wgpu's per-draw check of indirect arguments (`InstanceFlags::VALIDATION_INDIRECT_CALL`), on by
/// default even in release. Off is only sound when every indirect argument is in range, and on
/// D3D12 it also drops `first_vertex`/`first_instance` from the vertex builtins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndirectValidation {
    On,
    Off,
}

impl IndirectValidation {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "on" => Some(Self::On),
            "off" => Some(Self::Off),
            _ => None,
        }
    }
}

const REQUIRED_FEATURES: wgpu::Features = wgpu::Features::INDIRECT_FIRST_INSTANCE;
const OPTIONAL_FEATURES: wgpu::Features = wgpu::Features::MULTI_DRAW_INDIRECT_COUNT
    .union(wgpu::Features::TIMESTAMP_QUERY)
    .union(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES)
    .union(wgpu::Features::SHADER_DRAW_INDEX);

impl GpuContext {
    pub fn create_instance() -> wgpu::Instance {
        Self::create_instance_with(IndirectValidation::On)
    }

    pub fn create_instance_with(indirect_validation: IndirectValidation) -> wgpu::Instance {
        let mut flags = wgpu::InstanceFlags::from_build_config();
        flags.set(
            wgpu::InstanceFlags::VALIDATION_INDIRECT_CALL,
            indirect_validation == IndirectValidation::On,
        );
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            flags,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        })
    }

    pub async fn new(
        instance: wgpu::Instance,
        surface: Option<&wgpu::Surface<'_>>,
    ) -> Result<Self> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .context("no compatible GPU adapter")?;
        let available = adapter.features();
        anyhow::ensure!(
            available.contains(REQUIRED_FEATURES),
            "adapter {:?} lacks required features {:?}",
            adapter.get_info().name,
            REQUIRED_FEATURES - available
        );
        let enabled = REQUIRED_FEATURES | (OPTIONAL_FEATURES & available);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("vx_device"),
                required_features: enabled,
                required_limits: wgpu::Limits {
                    max_storage_buffer_binding_size: adapter
                        .limits()
                        .max_storage_buffer_binding_size,
                    max_buffer_size: adapter.limits().max_buffer_size,
                    ..wgpu::Limits::default()
                },
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .context("device request failed")?;
        let info = adapter.get_info();
        log::info!(
            "adapter: {} ({:?}, {:?})",
            info.name,
            info.backend,
            info.device_type
        );
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            tier: RenderTier::from_features(enabled),
            timestamps: enabled.contains(wgpu::Features::TIMESTAMP_QUERY),
        })
    }

    pub fn describe(&self) -> String {
        let info = self.adapter.get_info();
        format!(
            "adapter: {} | backend: {:?} | type: {:?} | driver: {} {}\ntier: {:?} | timestamps: {}\nfeatures: {:?}\nmax_storage_buffer_binding_size: {} MiB | max_buffer_size: {} MiB",
            info.name,
            info.backend,
            info.device_type,
            info.driver,
            info.driver_info,
            self.tier,
            self.timestamps,
            self.device.features(),
            self.device.limits().max_storage_buffer_binding_size >> 20,
            self.device.limits().max_buffer_size >> 20,
        )
    }
}
