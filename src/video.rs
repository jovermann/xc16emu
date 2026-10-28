// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! TED scan-line renderer. Register changes take effect between individual lines.

use crate::{memory_map::*, Machine};

/// Width of the PAL frame including the emulator's border.
pub const WIDTH: usize = 432;
/// Height of the PAL frame including the emulator's border.
pub const HEIGHT: usize = 312;
/// Width of the cropped display with an 8-pixel border on each side.
pub const COMPACT_WIDTH: usize = 336;
/// Height of the cropped display with an 8-pixel border on each side.
pub const COMPACT_HEIGHT: usize = 216;
/// Pixel width of the compact display's visible border: one character cell.
pub const COMPACT_BORDER: usize = 8;
const X_OFFSET: usize = 56;
const Y_OFFSET: usize = 53;
/// First visible horizontal pixel.
pub(crate) const VISIBLE_X: usize = X_OFFSET;
/// First displayed pixel line: raster line 3 is the start of the picture.
pub(crate) const VISIBLE_Y: usize = Y_OFFSET + 3;

impl Machine {
    /// Read one byte from physical RAM, with the configured RAM mirror.
    /// Arguments: address is the video fetch address. Returns: the RAM byte.
    fn video_ram(&self, address: usize) -> u8 {
        self.ram[address & self.ram_mask as usize]
    }

    /// Read a glyph or bitmap byte from RAM or the selected ROM bank.
    /// Arguments: address is the fetch address; rom chooses ROM when true. Returns: the fetched byte.
    fn video_bank(&self, address: usize, rom: bool) -> u8 {
        if !rom {
            return self.video_ram(address);
        }
        let bank = if address & UPPER_ROM_START as usize == UPPER_ROM_START as usize {
            self.high_bank * ROM_IMAGES_PER_BANK + UPPER_ROM_IMAGE_OFFSET
        } else {
            self.low_bank * ROM_IMAGES_PER_BANK
        };
        self.roms[bank][address & ROM_BANK_MASK]
    }

    /// Render one PAL raster line into the persistent frame buffer.
    /// CPU instructions can change FF06/FF07 during the frame, so this method
    /// deliberately reads the mode registers anew on every call.
    /// Arguments: raster is the emulated raster counter; display_line is the physical output row. Returns: nothing; the persistent pixel buffer changes.
    pub(crate) fn render_line(&mut self, raster: u16, display_line: u16) {
        let line = raster as usize;
        if line >= HEIGHT || display_line as usize >= HEIGHT {
            return;
        }
        // The raster counter is writable. Programs such as TOM jump it
        // backward or forward for IRQ effects, but the electron beam still
        // advances to the next physical row of the output frame.
        let row = (display_line as usize + Y_OFFSET) % HEIGHT;
        let start = row * WIDTH;
        let con1 = self.ted.registers[TED_CONTROL_1 as usize];
        let con2 = self.ted.registers[TED_CONTROL_2 as usize];
        let background = self.ted.registers[TED_BACKGROUND_COLOR as usize] & TED_COLOR_VALUE_MASK;
        let border = self.ted.registers[TED_BORDER_COLOR as usize] & TED_COLOR_VALUE_MASK;
        self.line_modes[row] = con1;
        self.pixels[start..start + WIDTH].fill(border);
        if !(3..203).contains(&line) || con1 & TED_SCREEN_ENABLE_BIT == 0 {
            return;
        }
        if con1 & 8 == 0 && !(7..199).contains(&line) {
            return;
        }
        self.pixels[start + X_OFFSET..start + X_OFFSET + 320].fill(background);
        let d = line as isize - (con1 & 7) as isize;
        if !(0..200).contains(&d) {
            return;
        }
        let d = d as usize;
        // TED fetches a screen matrix address once per character row.
        if d & 7 == 0 {
            self.video_matrix_latch =
                (self.ted.registers[TED_MATRIX_BASE as usize] as usize & 0xf8) << 8;
        }
        let matrix = self.video_matrix_latch;
        let char_base = (self.ted.registers[TED_CHARACTER_BASE as usize] as usize & 0xfc) << 8;
        let bitmap_base =
            (self.ted.registers[TED_BITMAP_AND_TONE_1_HIGH as usize] as usize & 0x38) << 10;
        let rom_chars =
            self.ted.registers[TED_BITMAP_AND_TONE_1_HIGH as usize] & TED_ROM_PATTERN_BIT != 0
                && char_base >= LOWER_ROM_START as usize;
        let rom_bitmap =
            self.ted.registers[TED_BITMAP_AND_TONE_1_HIGH as usize] & TED_ROM_PATTERN_BIT != 0
                && bitmap_base >= LOWER_ROM_START as usize;
        let multicolor = con2 & 0x10 != 0;
        let bitmap = con1 & TED_BITMAP_MODE_BIT != 0;
        let extended = con1 & TED_EXTENDED_MODE_BIT != 0;
        let x_scroll = (con2 & 7) as usize;
        let char_row = d / 8;

        for col in 0..40 {
            let cell = char_row * 40 + col;
            let color = self.video_ram(matrix + cell);
            let code = self.video_ram(matrix + SCREEN_CODE_OFFSET + cell);
            let (pattern, colors, double_pixel) = if bitmap {
                let address = bitmap_base + char_row * 320 + col * 8 + d % 8;
                let pattern = self.video_bank(address, rom_bitmap);
                let c0 = (code & 15) | (color & 0x70);
                let c1 = ((code >> 4) | (color << 4)) & TED_COLOR_VALUE_MASK;
                (
                    pattern,
                    [
                        background,
                        c1,
                        c0,
                        self.ted.registers[TED_EXTRA_COLOR_1 as usize],
                    ],
                    multicolor,
                )
            } else {
                // With FF07 bit 7 set, TED uses all 256 glyph indices.
                // Otherwise bit 7 requests a reversed glyph from the first
                // 128. TOM switches this bit at the raster split.
                let glyph = if extended {
                    code & 0x3f
                } else if con2 & TED_FULL_GLYPH_SET_BIT != 0 {
                    code
                } else {
                    code & TED_COLOR_VALUE_MASK
                };
                let address = char_base + glyph as usize * 8 + d % 8;
                let mut pattern = self.video_bank(address, rom_chars);
                if !multicolor
                    && !extended
                    && code & 0x80 != 0
                    && con2 & TED_FULL_GLYPH_SET_BIT == 0
                {
                    pattern = !pattern;
                }
                if self.ted.frames & 16 != 0
                    && self.ted.registers[TED_CURSOR_HIGH as usize] as usize * 256
                        + self.ted.registers[TED_CURSOR_LOW as usize] as usize
                        == cell
                {
                    pattern = !pattern;
                }
                if extended {
                    let backgrounds = [
                        background,
                        self.ted.registers[TED_EXTRA_COLOR_1 as usize],
                        self.ted.registers[TED_EXTRA_COLOR_2 as usize],
                        self.ted.registers[TED_EXTRA_COLOR_3 as usize],
                    ];
                    (
                        pattern,
                        [backgrounds[(code >> 6) as usize], color, color, color],
                        false,
                    )
                } else {
                    (
                        pattern,
                        [
                            background,
                            color,
                            self.ted.registers[TED_EXTRA_COLOR_2 as usize],
                            color,
                        ],
                        multicolor && color & 8 != 0,
                    )
                }
            };

            for pixel in 0..8 {
                let color_index = if double_pixel {
                    colors[((pattern >> (6 - (pixel / 2) * 2)) & 3) as usize]
                } else if pattern & (0x80 >> pixel) != 0 {
                    colors[1]
                } else {
                    colors[0]
                } & TED_COLOR_VALUE_MASK;
                let x = X_OFFSET + x_scroll + col * 8 + pixel;
                if x < X_OFFSET + 320 {
                    self.pixels[start + x] = color_index;
                }
            }
        }
        if con2 & 8 == 0 {
            self.pixels[start + X_OFFSET..start + X_OFFSET + 8].fill(border);
            self.pixels[start + X_OFFSET + 312..start + X_OFFSET + 320].fill(border);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Check full glyph indices and reverse-mode interpretation.
    /// Arguments: none. Returns: nothing; assertions fail on regression.
    fn reverse_mode_selects_full_character_index_when_disabled() {
        let mut machine =
            Machine::from_rom_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms")).unwrap();
        machine.ted.registers[6] = 0x1b; // picture on, raster 3 is glyph row 0
        machine.ted.registers[0x13] = 0x20; // character RAM at $2000
        machine.ted.registers[0x14] = 0x08; // matrix at $0800
        machine.ram[0x0800] = 1; // foreground color
        machine.ram[0x0c00] = 0x80; // glyph 128, or reversed glyph 0
        machine.ram[0x2000] = 0x80;
        machine.ram[0x2400] = 0x80;
        let pixel = (3 + Y_OFFSET) * WIDTH + X_OFFSET;

        machine.ted.registers[7] = 0x88; // use all 256 character indices
        machine.render_line(3, 3);
        assert_eq!(machine.pixels[pixel], 1);

        machine.ted.registers[7] = 0x08; // high bit reverses glyph 0
        machine.render_line(3, 3);
        assert_eq!(machine.pixels[pixel], 0);

        machine.ted.registers[7] = 0x18; // multicolor mode masks bit 7, without reversing
        machine.render_line(3, 3);
        assert_eq!(machine.pixels[pixel], 1);
    }
}
