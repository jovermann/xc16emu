// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! Display palettes. These change RGB output only, never TED color numbers.

/// The available mappings from TED color numbers to host RGB.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Palette {
    /// Samples from the centers of the 16x8 swatches in keep/C16colors.png.
    #[default]
    C16Colors,
    /// The sampled palette with HSV saturation multiplied by 1.25.
    C16Colors125,
    /// The sampled palette with HSV saturation multiplied by 1.5.
    C16Colors150,
    /// The emulator's original synthetic hue and brightness calculation.
    Legacy,
}

impl Palette {
    /// Parse a CLI or settings value.
    /// Arguments: name is a palette identifier. Returns: its palette, if known.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "c16colors" => Some(Self::C16Colors),
            "c16colors-125" => Some(Self::C16Colors125),
            "c16colors-150" => Some(Self::C16Colors150),
            "legacy" => Some(Self::Legacy),
            _ => None,
        }
    }

    /// Return the stable CLI and settings identifier.
    /// Arguments: self is the palette. Returns: its lowercase name.
    pub fn name(self) -> &'static str {
        match self {
            Self::C16Colors => "c16colors",
            Self::C16Colors125 => "c16colors-125",
            Self::C16Colors150 => "c16colors-150",
            Self::Legacy => "legacy",
        }
    }

    /// Convert a TED color number to packed 24-bit RGB.
    /// Arguments: self selects the palette; color holds hue in bits 0-3 and
    /// luminance in bits 4-6. Returns: 0xRRGGBB.
    pub fn rgb(self, color: u8) -> u32 {
        match self {
            Self::C16Colors => sampled_rgb(color),
            Self::C16Colors125 => boost_saturation(sampled_rgb(color), 5, 4),
            Self::C16Colors150 => boost_saturation(sampled_rgb(color), 3, 2),
            Self::Legacy => legacy_rgb(color),
        }
    }
}

// The image runs blue → cyan → green → yellow → red → purple → gray → black.
// TED's low nibble has a different hue order, hence this column mapping.
const C16_HUE_COLUMNS: [usize; 16] = [15, 14, 10, 2, 12, 4, 0, 7, 8, 9, 5, 11, 3, 1, 13, 6];

// Exact center-pixel RGB samples from keep/C16colors.png, left to right, top to bottom.
const C16_COLORS: [[u32; 16]; 8] = [
    [
        0x27188e, 0x0e2784, 0x00424c, 0x004e0c, 0x004e00, 0x184800, 0x244400, 0x303e00, 0x463000,
        0x582100, 0x621307, 0x630448, 0x510378, 0x33118e, 0x2c2c2c, 0x000000,
    ],
    [
        0x36289b, 0x1f3691, 0x00505a, 0x005c1d, 0x125d00, 0x285700, 0x345200, 0x3f4c00, 0x553f00,
        0x663100, 0x702419, 0x711656, 0x601685, 0x42229b, 0x3b3b3b, 0x000000,
    ],
    [
        0x3e30a2, 0x273e98, 0x055861, 0x046325, 0x1b6400, 0x305e00, 0x3b5900, 0x475400, 0x5c4700,
        0x6d3900, 0x772c21, 0x771f5d, 0x661e8c, 0x492aa1, 0x424242, 0x000000,
    ],
    [
        0x4c3faf, 0x364ca5, 0x17656f, 0x177135, 0x2b7100, 0x3f6b00, 0x4a6700, 0x556200, 0x6a5500,
        0x7a4709, 0x843b31, 0x852f6b, 0x742e99, 0x5739ae, 0x515151, 0x000000,
    ],
    [
        0x766ad5, 0x6276cb, 0x468e97, 0x459960, 0x57992e, 0x6a9419, 0x748f14, 0x7e8a13, 0x927e20,
        0xa2713a, 0xac665c, 0xac5a93, 0x9c5ac0, 0x8064d4, 0x7a7a7a, 0x000000,
    ],
    [
        0x9185ed, 0x7d91e4, 0x62a8b1, 0x62b37b, 0x73b34c, 0x85ae38, 0x8faa34, 0x99a433, 0xac993e,
        0xbb8c57, 0xc58178, 0xc676ad, 0xb675d9, 0x9b80ed, 0x959595, 0x000000,
    ],
    [
        0xab9fff, 0x97abfd, 0x7dc2ca, 0x7dcc96, 0x8dcd68, 0x9fc755, 0xa9c351, 0xb3be51, 0xc6b35b,
        0xd5a673, 0xde9b93, 0xdf91c7, 0xcf90f2, 0xb59aff, 0xafafaf, 0x000000,
    ],
    [
        0xddd2ff, 0xcbddff, 0xb2f4fc, 0xb1fec9, 0xc1fe9d, 0xd2f98c, 0xdbf588, 0xe5f088, 0xf7e591,
        0xffd9a8, 0xffcfc6, 0xffc4f9, 0xffc4ff, 0xe7cdff, 0xe1e1e1, 0x000000,
    ],
];

/// Look up an unchanged RGB sample from the source image.
/// Arguments: color is a TED color number. Returns: packed 24-bit RGB.
fn sampled_rgb(color: u8) -> u32 {
    let row = ((color >> 4) & 7) as usize;
    let column = C16_HUE_COLUMNS[(color & 15) as usize];
    C16_COLORS[row][column]
}

/// Increase HSV saturation while preserving the highest RGB channel (value).
/// Arguments: rgb is 0xRRGGBB; numerator/denominator is the saturation scale.
/// Returns: packed RGB, with channels clipped at zero and grays unchanged.
fn boost_saturation(rgb: u32, numerator: u32, denominator: u32) -> u32 {
    let red = (rgb >> 16) & 255;
    let green = (rgb >> 8) & 255;
    let blue = rgb & 255;
    let peak = red.max(green).max(blue);
    let stretch = |channel: u32| {
        peak.saturating_sub(((peak - channel) * numerator + denominator / 2) / denominator)
    };
    (stretch(red) << 16) | (stretch(green) << 8) | stretch(blue)
}

/// Reproduce the original computed RGB mapping for compatibility.
/// Arguments: color is a TED color number. Returns: packed 24-bit RGB.
fn legacy_rgb(color: u8) -> u32 {
    const HUES: [[u8; 3]; 16] = [
        [0, 0, 0],
        [255, 255, 255],
        [255, 0, 0],
        [0, 255, 255],
        [127, 0, 255],
        [0, 255, 0],
        [0, 0, 255],
        [255, 255, 0],
        [255, 128, 0],
        [128, 128, 0],
        [128, 255, 0],
        [200, 0, 200],
        [0, 255, 128],
        [0, 128, 255],
        [0, 32, 255],
        [128, 255, 128],
    ];
    const LEVELS: [u32; 8] = [30, 44, 58, 72, 86, 100, 114, 128];
    let h = HUES[(color & 15) as usize];
    let level = LEVELS[((color >> 4) & 7) as usize];
    ((h[0] as u32 * level / 128) << 16)
        | ((h[1] as u32 * level / 128) << 8)
        | (h[2] as u32 * level / 128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Check source swatches, TED remapping, and the retained legacy colors.
    /// Arguments: none. Returns: nothing; assertions signal a mapping error.
    fn palette_samples_and_selection() {
        assert_eq!(Palette::default().rgb(0x00), 0x000000);
        assert_eq!(Palette::default().rgb(0x71), 0xe1e1e1);
        assert_eq!(Palette::default().rgb(0x16), 0x36289b);
        assert_eq!(Palette::Legacy.rgb(0x71), 0xffffff);
        assert_eq!(Palette::parse("c16colors"), Some(Palette::C16Colors));
        assert_eq!(Palette::parse("c16colors-125"), Some(Palette::C16Colors125));
        assert_eq!(Palette::parse("c16colors-150"), Some(Palette::C16Colors150));
        assert_eq!(Palette::parse("legacy"), Some(Palette::Legacy));
        assert_eq!(Palette::C16Colors125.rgb(0x71), 0xe1e1e1);
        assert_eq!(Palette::C16Colors150.rgb(0x00), 0x000000);
        assert_eq!(Palette::C16Colors125.rgb(0x16), 0x1d0b9b);
        assert_eq!(Palette::C16Colors150.rgb(0x16), 0x03009b);
        for color in 0..128 {
            let channels = |palette: Palette| {
                let rgb = palette.rgb(color);
                [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255]
            };
            let original = channels(Palette::C16Colors);
            let vivid = channels(Palette::C16Colors125);
            let bold = channels(Palette::C16Colors150);
            assert_eq!(original.iter().max(), vivid.iter().max());
            assert_eq!(original.iter().max(), bold.iter().max());
            assert!(original.iter().min() >= vivid.iter().min());
            assert!(vivid.iter().min() >= bold.iter().min());
        }
    }
}
