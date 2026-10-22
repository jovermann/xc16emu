// Copyright (C) 2026 Johannes Overmann and contributors.
// SPDX-License-Identifier: GPL-2.0-or-later
//! TED scan-line renderer. Register changes take effect between individual lines.

use crate::Machine;

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
    fn video_ram(&self, address: usize) -> u8 {
        self.ram[address & self.ram_mask as usize]
    }

    /// Read a glyph or bitmap byte from RAM or the selected ROM bank.
    fn video_bank(&self, address: usize, rom: bool) -> u8 {
        if !rom {
            return self.video_ram(address);
        }
        let bank = if address & 0xc000 == 0xc000 {
            self.high_bank * 2 + 1
        } else {
            self.low_bank * 2
        };
        self.roms[bank][address & 0x3fff]
    }

    /// Render one PAL raster line into the persistent frame buffer.
    /// CPU instructions can change FF06/FF07 during the frame, so this method
    /// deliberately reads the mode registers anew on every call.
    pub(crate) fn render_line(&mut self, raster: u16) {
        let line = raster as usize;
        if line >= HEIGHT {
            return;
        }
        let row = (line + Y_OFFSET) % HEIGHT;
        let start = row * WIDTH;
        let con1 = self.ted.registers[6];
        let con2 = self.ted.registers[7];
        let background = self.ted.registers[0x15] & 0x7f;
        let border = self.ted.registers[0x19] & 0x7f;
        self.line_modes[row] = con1;
        self.pixels[start..start + WIDTH].fill(border);
        if !(3..203).contains(&line) || con1 & 0x10 == 0 {
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
            self.video_matrix_latch = (self.ted.registers[0x14] as usize & 0xf8) << 8;
        }
        let matrix = self.video_matrix_latch;
        let char_base = (self.ted.registers[0x13] as usize & 0xfc) << 8;
        let bitmap_base = (self.ted.registers[0x12] as usize & 0x38) << 10;
        let rom_chars = self.ted.registers[0x12] & 4 != 0 && char_base >= 0x8000;
        let rom_bitmap = self.ted.registers[0x12] & 4 != 0 && bitmap_base >= 0x8000;
        let multicolor = con2 & 0x10 != 0;
        let bitmap = con1 & 0x20 != 0;
        let extended = con1 & 0x40 != 0;
        let x_scroll = (con2 & 7) as usize;
        let char_row = d / 8;

        for col in 0..40 {
            let cell = char_row * 40 + col;
            let color = self.video_ram(matrix + cell);
            let code = self.video_ram(matrix + 0x400 + cell);
            let (pattern, colors, double_pixel) = if bitmap {
                let address = bitmap_base + char_row * 320 + col * 8 + d % 8;
                let pattern = self.video_bank(address, rom_bitmap);
                let c0 = (code & 15) | (color & 0x70);
                let c1 = ((code >> 4) | (color << 4)) & 0x7f;
                (
                    pattern,
                    [background, c1, c0, self.ted.registers[0x16]],
                    multicolor,
                )
            } else {
                let glyph = if extended { code & 0x3f } else { code & 0x7f };
                let address = char_base + glyph as usize * 8 + d % 8;
                let mut pattern = self.video_bank(address, rom_chars);
                if code & 0x80 != 0 && con2 & 0x80 == 0 {
                    pattern = !pattern;
                }
                if self.ted.frames & 16 != 0
                    && self.ted.registers[0x0c] as usize * 256 + self.ted.registers[0x0d] as usize
                        == cell
                {
                    pattern = !pattern;
                }
                if extended {
                    let backgrounds = [
                        background,
                        self.ted.registers[0x16],
                        self.ted.registers[0x17],
                        self.ted.registers[0x18],
                    ];
                    (
                        pattern,
                        [backgrounds[(code >> 6) as usize], color, color, color],
                        false,
                    )
                } else {
                    (
                        pattern,
                        [background, color, self.ted.registers[0x17], color],
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
                } & 0x7f;
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
