use anyhow::{Context, Result, ensure};
use vx_core::TextureLayer;
use wgpu::util::DeviceExt;

pub const TEXTURE_SIZE: u32 = 16;
const MIP_LEVELS: u32 = TEXTURE_SIZE.ilog2() + 1;
const ANISOTROPY: u16 = 8;
const TEXEL_BYTES: usize = 4;

pub struct BlockTextures {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bytes: u64,
    pub content_hash: u64,
}

fn encoded_png(layer: TextureLayer) -> Option<&'static [u8]> {
    macro_rules! asset {
        ($name:literal) => {
            include_bytes!(concat!("../../../assets/blocks/", $name, ".png")).as_slice()
        };
    }
    Some(match layer {
        TextureLayer::Missing => return None,
        TextureLayer::Stone => asset!("stone"),
        TextureLayer::Dirt => asset!("dirt"),
        TextureLayer::GrassTop => asset!("grass_top"),
        TextureLayer::GrassSide => asset!("grass_side"),
        TextureLayer::Sand => asset!("sand"),
        TextureLayer::Snow => asset!("snow"),
        TextureLayer::SnowSide => asset!("snow_side"),
    })
}

fn missing_texture() -> Vec<u8> {
    let size = TEXTURE_SIZE as usize;
    (0..size * size)
        .flat_map(|index| {
            let checker = ((index % size) / 4 + (index / size) / 4).is_multiple_of(2);
            if checker {
                [255, 0, 255, 255]
            } else {
                [0, 0, 0, 255]
            }
        })
        .collect()
}

fn decode_rgba(layer: TextureLayer, encoded: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(encoded));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0; reader.output_buffer_size().context("png too large")?];
    let info = reader.next_frame(&mut buffer)?;
    ensure!(
        info.width == TEXTURE_SIZE && info.height == TEXTURE_SIZE,
        "{} is {}x{}, expected {TEXTURE_SIZE}x{TEXTURE_SIZE}",
        layer.name(),
        info.width,
        info.height
    );
    let pixels = &buffer[..info.buffer_size()];
    Ok(match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .flat_map(|ga| [ga[0], ga[0], ga[0], ga[1]])
            .collect(),
        png::ColorType::Indexed => anyhow::bail!("{}: palette not expanded", layer.name()),
    })
}

fn srgb_to_linear(value: u8) -> f32 {
    let c = f32::from(value) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f32) -> u8 {
    let c = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round().clamp(0.0, 255.0) as u8
}

fn downsample_srgb(level: &[u8], size: usize) -> Vec<u8> {
    let half = size / 2;
    let mut out = Vec::with_capacity(half * half * TEXEL_BYTES);
    for y in 0..half {
        for x in 0..half {
            let texel = |dx: usize, dy: usize, channel: usize| {
                level[((2 * y + dy) * size + 2 * x + dx) * TEXEL_BYTES + channel]
            };
            for channel in 0..TEXEL_BYTES {
                let samples =
                    [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| texel(dx, dy, channel));
                if channel == 3 {
                    let sum: u32 = samples.iter().map(|&a| u32::from(a)).sum();
                    out.push(((sum + 2) / 4) as u8);
                } else {
                    let mean = samples.iter().map(|&s| srgb_to_linear(s)).sum::<f32>() / 4.0;
                    out.push(linear_to_srgb(mean));
                }
            }
        }
    }
    out
}

fn append_mip_chain(base: Vec<u8>, data: &mut Vec<u8>) {
    let mut size = TEXTURE_SIZE as usize;
    let mut level = base;
    loop {
        data.extend_from_slice(&level);
        if size == 1 {
            break;
        }
        level = downsample_srgb(&level, size);
        size /= 2;
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Layer-major RGBA8 sRGB data with every mip level, as `create_texture_with_data` expects.
pub fn build_layer_data() -> Result<Vec<u8>> {
    let mut data = Vec::new();
    for layer in TextureLayer::ALL {
        let base = match encoded_png(layer) {
            Some(encoded) => decode_rgba(layer, encoded)?,
            None => missing_texture(),
        };
        append_mip_chain(base, &mut data);
    }
    Ok(data)
}

impl BlockTextures {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self> {
        let data = build_layer_data()?;
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("block_textures"),
                size: wgpu::Extent3d {
                    width: TEXTURE_SIZE,
                    height: TEXTURE_SIZE,
                    depth_or_array_layers: TextureLayer::ALL.len() as u32,
                },
                mip_level_count: MIP_LEVELS,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &data,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("block_textures_view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..wgpu::TextureViewDescriptor::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("block_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: ANISOTROPY,
            ..wgpu::SamplerDescriptor::default()
        });
        Ok(Self {
            view,
            sampler,
            bytes: data.len() as u64,
            content_hash: fnv1a(&data),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layer_decodes_with_full_mip_chain() {
        let data = build_layer_data().expect("embedded block textures decode");
        let per_layer: usize = (0..MIP_LEVELS)
            .map(|level| ((TEXTURE_SIZE >> level).pow(2)) as usize * TEXEL_BYTES)
            .sum();
        assert_eq!(data.len(), per_layer * TextureLayer::ALL.len());
    }

    #[test]
    fn downsample_preserves_flat_colour() {
        let flat: Vec<u8> = [120, 80, 40, 255].repeat(4);
        assert_eq!(downsample_srgb(&flat, 2), vec![120, 80, 40, 255]);
    }
}
