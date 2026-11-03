use crate::BlockId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum TextureLayer {
    Missing = 0,
    Stone = 1,
    Dirt = 2,
    GrassTop = 3,
    GrassSide = 4,
    Sand = 5,
    Snow = 6,
    SnowSide = 7,
}

impl TextureLayer {
    pub const ALL: [Self; 8] = [
        Self::Missing,
        Self::Stone,
        Self::Dirt,
        Self::GrassTop,
        Self::GrassSide,
        Self::Sand,
        Self::Snow,
        Self::SnowSide,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Stone => "stone",
            Self::Dirt => "dirt",
            Self::GrassTop => "grass_top",
            Self::GrassSide => "grass_side",
            Self::Sand => "sand",
            Self::Snow => "snow",
            Self::SnowSide => "snow_side",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockFaces {
    pub top: TextureLayer,
    pub side: TextureLayer,
    pub bottom: TextureLayer,
    pub rotate_top_bottom: bool,
}

impl BlockFaces {
    const LAYER_BITS: u32 = 10;
    const ROTATE_BIT: u32 = 30;

    const fn uniform(layer: TextureLayer) -> Self {
        Self {
            top: layer,
            side: layer,
            bottom: layer,
            rotate_top_bottom: true,
        }
    }

    const fn capped(top: TextureLayer, side: TextureLayer, bottom: TextureLayer) -> Self {
        Self {
            top,
            side,
            bottom,
            rotate_top_bottom: true,
        }
    }

    /// GPU word mirrored in `terrain.wgsl`: top | side << 10 | bottom << 20 | rotate << 30.
    pub const fn packed(self) -> u32 {
        self.top as u32
            | (self.side as u32) << Self::LAYER_BITS
            | (self.bottom as u32) << (2 * Self::LAYER_BITS)
            | (self.rotate_top_bottom as u32) << Self::ROTATE_BIT
    }
}

impl BlockId {
    pub const COUNT: usize = 6;

    pub const fn faces(self) -> BlockFaces {
        match self {
            Self::STONE => BlockFaces::uniform(TextureLayer::Stone),
            Self::DIRT => BlockFaces::uniform(TextureLayer::Dirt),
            Self::GRASS => BlockFaces::capped(
                TextureLayer::GrassTop,
                TextureLayer::GrassSide,
                TextureLayer::Dirt,
            ),
            Self::SAND => BlockFaces::uniform(TextureLayer::Sand),
            Self::SNOW => BlockFaces::capped(
                TextureLayer::Snow,
                TextureLayer::SnowSide,
                TextureLayer::Dirt,
            ),
            _ => BlockFaces::uniform(TextureLayer::Missing),
        }
    }
}

pub fn block_face_table() -> [u32; BlockId::COUNT] {
    std::array::from_fn(|id| BlockId(id as u16).faces().packed())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_order_matches_discriminants() {
        for (index, layer) in TextureLayer::ALL.iter().enumerate() {
            assert_eq!(*layer as usize, index);
        }
    }

    #[test]
    fn packed_faces_round_trip() {
        let word = BlockId::GRASS.faces().packed();
        assert_eq!(word & 0x3FF, TextureLayer::GrassTop as u32);
        assert_eq!((word >> 10) & 0x3FF, TextureLayer::GrassSide as u32);
        assert_eq!((word >> 20) & 0x3FF, TextureLayer::Dirt as u32);
        assert_eq!(word >> 30, 1);
    }

    #[test]
    fn every_solid_block_has_real_textures() {
        for id in 1..BlockId::COUNT as u16 {
            let faces = BlockId(id).faces();
            assert_ne!(faces.top, TextureLayer::Missing, "block {id}");
            assert_ne!(faces.side, TextureLayer::Missing, "block {id}");
        }
    }
}
