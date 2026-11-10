// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! Extract TOM's two-level compressed labyrinth from a 64 KiB frozen image.
//!
//! The map contains 40 by 25 large blocks. A large block selects 4 by 4
//! small tiles, and each small tile contains 3 by 3 TED characters plus one
//! color attribute. Both dictionaries store rows before columns, matching
//! the game's character-expansion routine at $110C-$11CE.

use std::{env, fs, path::Path};
use xc16emu::Palette;

const IMAGE_BYTES: usize = 0x10000;
const MAP_BASE: usize = 0x2a1d;
const MAP_WIDTH: usize = 40;
const MAP_HEIGHT: usize = 25;
const LARGE_TILE_BASE: usize = 0x2e06;
const LARGE_TILE_BYTES: usize = 16;
const SMALL_TILE_BASE: usize = 0x2800;
const SMALL_TILE_BYTES: usize = 10;
const CHARACTER_BASE: usize = 0x2000;
const FIRE_GLYPH_CODE: u8 = 0x7e;
const FIRE_FRAME_BASE: usize = 0x3267;
const CHARACTERS_PER_LARGE_TILE: usize = 12;
const PIXELS_PER_CHARACTER: usize = 8;
const BORDER_BLOCKS: usize = 1;
const ROCK_BLOCK: u8 = 27;
const OUTPUT_CHAR_WIDTH: usize = (MAP_WIDTH + 2 * BORDER_BLOCKS) * CHARACTERS_PER_LARGE_TILE;
const OUTPUT_CHAR_HEIGHT: usize = (MAP_HEIGHT + 2 * BORDER_BLOCKS) * CHARACTERS_PER_LARGE_TILE;
const IMAGE_WIDTH: usize = OUTPUT_CHAR_WIDTH * PIXELS_PER_CHARACTER;
const IMAGE_HEIGHT: usize = OUTPUT_CHAR_HEIGHT * PIXELS_PER_CHARACTER;
const BACKGROUND_REGISTER: usize = 0xff15;
const EXTRA_COLOR_2_REGISTER: usize = 0xff17;

/// One screen-matrix code and its matching TED color attribute.
#[derive(Clone, Copy, Default)]
struct Cell {
    code: u8,
    color: u8,
}

/// Static shape selected from TOM's object-drawing routine at $1341-$15D6.
#[derive(Clone, Copy)]
enum ObjectShape {
    Grid { first: u8, width: u8, height: u8 },
    Arch,
    Fire,
    Single(u8),
    TwoHigh(u8, u8),
}

/// Local position, TED attribute, and sprite shape of one map marker.
struct ObjectSpec {
    x: i16,
    y: i16,
    color: u8,
    shape: ObjectShape,
}

/// Parse the two paths and report errors with a nonzero process exit status.
/// Arguments: none; paths come from the process command line. Returns: nothing.
fn main() {
    if let Err(error) = run(env::args().collect()) {
        eprintln!("tom-map: {error}");
        std::process::exit(1);
    }
}

/// Read a frozen image, decode its map, and write an indexed-color PNG.
/// Arguments: args contains the program name, input path, and output path.
/// Returns: success or a human-readable argument, I/O, or data error.
fn run(args: Vec<String>) -> Result<(), String> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!(
            "tom-map {}  Copyright (C) 2026 Johannes Overmann.\n\
             Extract TOM's full labyrinth from a 64 KiB frozen image.\n\n\
             Usage: tom-map [--palette NAME] IMAGE OUTPUT.png\n\n\
             --palette   c16colors (default), c16colors-125, c16colors-150, or legacy.\n\
             IMAGE       Frozen 64 KiB TOM image.\n\
             OUTPUT.png  Full 4032x2592 labyrinth with rock border and objects.",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    let mut palette = Palette::default();
    let mut paths = Vec::new();
    let mut options = args.into_iter().skip(1);
    while let Some(arg) = options.next() {
        if arg == "--palette" {
            let name = options
                .next()
                .ok_or("--palette needs a palette name (see --help)")?;
            palette = Palette::parse(&name).ok_or_else(|| {
                format!("unknown palette {name:?}; see --help for available palettes")
            })?;
        } else if arg.starts_with('-') {
            return Err(format!("unknown option {arg}"));
        } else {
            paths.push(arg);
        }
    }
    if paths.len() != 2 {
        return Err("usage: tom-map [--palette NAME] IMAGE OUTPUT.png (try --help)".to_string());
    }
    let memory = fs::read(&paths[0]).map_err(|e| format!("{}: {e}", paths[0]))?;
    if memory.len() != IMAGE_BYTES {
        return Err(format!("{}: expected 65536 bytes", paths[0]));
    }
    let pixels = decode_labyrinth(&memory)?;
    write_png(Path::new(&paths[1]), &pixels, palette).map_err(|e| format!("{}: {e}", paths[1]))?;
    println!("{}: {}x{} pixels", paths[1], IMAGE_WIDTH, IMAGE_HEIGHT);
    Ok(())
}

/// Convert one map byte into its large-tile dictionary index.
/// Arguments: value is a byte from the 40x25 map. Returns: its 0..65 index
/// or 99, which the game treats as a solid tile of small-tile number 52.
fn large_tile_index(value: u8) -> u8 {
    // The game replaces three transient object markers before drawing the
    // static maze; see its normalization routine at $15D9-$15EB.
    let value = match value {
        0xc8 => 0x96,
        0xc9 => 0xa0,
        0xca => 0xc7,
        _ => value,
    };
    if value >= 100 {
        value - 100
    } else {
        value
    }
}

/// Look up one of the 4x4 small tiles within a large map block.
/// Arguments: memory is the frozen image; large is the normalized map index;
/// x and y are local small-tile coordinates. Returns: a 0..53 small-tile ID.
fn small_tile_index(memory: &[u8], large: u8, x: usize, y: usize) -> Result<u8, String> {
    if large == 99 {
        return Ok(52); // the game's special solid/empty block at $10FB
    }
    if large > 65 {
        return Err(format!("unknown large-tile index {large}"));
    }
    let address = LARGE_TILE_BASE + large as usize * LARGE_TILE_BYTES + y * 4 + x;
    let small = memory[address];
    if small > 53 {
        return Err(format!(
            "invalid small-tile index {small} at ${address:04x}"
        ));
    }
    Ok(small)
}

/// Select a fixed animation frame for a visible map object.
/// Arguments: marker is the unmodified map byte. Returns: its local sprite
/// placement and original character/color values, or None for ordinary rock.
fn object_spec(marker: u8) -> Option<ObjectSpec> {
    use ObjectShape::{Arch, Fire, Grid, Single, TwoHigh};
    let triple = |first| Grid {
        first,
        width: 3,
        height: 2,
    };
    // These offsets and glyphs reproduce the game's object pass at
    // $1341-$15D6. A run of adjacent marker bytes often identifies the same
    // object from three neighboring blocks; its shifted copies coincide.
    let (x, y, color, shape) = match marker {
        0x64..=0x66 => (1 - 12 * (marker - 0x64) as i16, 7, 0x4a, triple(0xa1)),
        0x67..=0x69 => (16 - 12 * (marker - 0x67) as i16, 7, 0x4f, triple(0xd1)),
        0x6a..=0x6b => (19 - 12 * (marker - 0x6a) as i16, 7, 0x3f, triple(0xb9)),
        0x6c => (1, 7, 0x4a, triple(0x83)),
        0x6d..=0x6e => (15 - 12 * (marker - 0x6d) as i16, 7, 0x5f, triple(0xcb)),
        0x6f..=0x71 => (6 - 12 * (marker - 0x6f) as i16, 7, 0x4a, triple(0xa1)),
        0x72..=0x74 => (23 - 12 * (marker - 0x72) as i16, 7, 0x3f, triple(0xd1)),
        0x75..=0x76 => (18 - 12 * (marker - 0x75) as i16, 7, 0x4f, triple(0xcb)),
        0x77..=0x78 => (21 - 12 * (marker - 0x77) as i16, 7, 0x3f, triple(0xb9)),
        0x79..=0x7a => (
            10 - 12 * (marker - 0x79) as i16,
            8,
            0x4a,
            Grid {
                first: 0xaa,
                width: 3,
                height: 1,
            },
        ),
        0x7b => (6, 5, 0x5f, Arch),
        0x7d => (6, 5, 0x4f, Arch),
        0x7e => (7, 5, 0x4a, triple(0xb3)),
        0x8c => (4, 10, 0x6f, Single(0xdd)),
        0x92 => (8, 6, 0x4f, Single(0x81)),
        0x96 => (8, 0, 0x3a, Fire),
        0x99 => (-8, 10, 0x6f, Single(0xdd)),
        0xa0 => (8, 9, 0x25, TwoHigh(0xde, 0xdf)),
        0xc7 => (7, 10, 0x1f, triple(0xe2)),
        _ => return None,
    };
    Some(ObjectSpec { x, y, color, shape })
}

/// Expand one large block into the output character and color matrices.
/// Arguments: memory supplies both dictionaries; large is the normalized
/// block ID; block_x and block_y include the one-block border. Returns:
/// success or an error for an invalid small-tile ID.
fn fill_large_block(
    memory: &[u8],
    cells: &mut [Cell],
    large: u8,
    block_x: usize,
    block_y: usize,
) -> Result<(), String> {
    for small_y in 0..4 {
        for small_x in 0..4 {
            let small = small_tile_index(memory, large, small_x, small_y)?;
            let tile = SMALL_TILE_BASE + small as usize * SMALL_TILE_BYTES;
            let color = memory[tile + 9];
            for char_y in 0..3 {
                for char_x in 0..3 {
                    let code = memory[tile + char_y * 3 + char_x];
                    let x = block_x * 12 + small_x * 3 + char_x;
                    let y = block_y * 12 + small_y * 3 + char_y;
                    cells[y * OUTPUT_CHAR_WIDTH + x] = Cell { code, color };
                }
            }
        }
    }
    Ok(())
}

/// Place one object character, respecting the game's transparent cells.
/// Arguments: cells is the complete matrix; map_x/y locate the source block;
/// dx/dy are local character offsets, and code/color are the object's data.
/// Returns: nothing; a cell is replaced only if TOM would draw over it.
fn put_object_cell(
    cells: &mut [Cell],
    map_x: usize,
    map_y: usize,
    dx: i16,
    dy: i16,
    code: u8,
    color: u8,
) {
    let x = (map_x + BORDER_BLOCKS) as i16 * 12 + dx;
    let y = (map_y + BORDER_BLOCKS) as i16 * 12 + dy;
    if x < 0 || y < 0 || x as usize >= OUTPUT_CHAR_WIDTH || y as usize >= OUTPUT_CHAR_HEIGHT {
        return;
    }
    let cell = &mut cells[y as usize * OUTPUT_CHAR_WIDTH + x as usize];
    if cell.code == 0 || cell.code >= 0x7f {
        *cell = Cell { code, color };
    }
}

/// Draw every character of one static object frame over its map block.
/// Arguments: cells is the output matrix; marker selects TOM's sprite;
/// map_x/y locate its compressed block. Returns: nothing.
fn draw_object(cells: &mut [Cell], marker: u8, map_x: usize, map_y: usize) {
    let Some(spec) = object_spec(marker) else {
        return;
    };
    let mut put = |dx: i16, dy: i16, code: u8| {
        put_object_cell(
            cells,
            map_x,
            map_y,
            spec.x + dx,
            spec.y + dy,
            code,
            spec.color,
        )
    };
    match spec.shape {
        ObjectShape::Grid {
            first,
            width,
            height,
        } => {
            for y in 0..height {
                for x in 0..width {
                    put(x as i16, y as i16, first + y * width + x);
                }
            }
        }
        ObjectShape::Arch => {
            put(1, 0, 0x7f);
            for y in 0..2 {
                for x in 0..3 {
                    put(x, y + 1, 0x8f + y as u8 * 3 + x as u8);
                }
            }
        }
        ObjectShape::Fire => {
            for y in 0..6 {
                put(0, y, if y == 3 { 0xe1 } else { 0xe0 });
            }
        }
        ObjectShape::Single(code) => put(0, 0, code),
        ObjectShape::TwoHigh(top, bottom) => {
            put(0, 0, top);
            put(0, 1, bottom);
        }
    }
}

/// Render a single 8x8 TED character into a palette-indexed pixel buffer.
/// Arguments: memory supplies its glyph; code and color are the tile entry;
/// x and y locate its top-left pixel; background and extra2 are TED colors.
/// Returns: nothing; 64 pixels in pixels are replaced.
fn draw_character(
    memory: &[u8],
    pixels: &mut [u8],
    code: u8,
    color: u8,
    x: usize,
    y: usize,
    background: u8,
    extra2: u8,
) {
    let multicolor = color & 8 != 0;
    for glyph_y in 0..PIXELS_PER_CHARACTER {
        // TOM flashes the fire by replacing character $7E in RAM. A frozen
        // image may contain its blank phase. The game's first visible frame
        // is stored at $3267, followed immediately by its second frame.
        let pattern = if code == FIRE_GLYPH_CODE {
            memory[FIRE_FRAME_BASE + glyph_y]
        } else {
            memory[CHARACTER_BASE + code as usize * 8 + glyph_y]
        };
        let output = (y + glyph_y) * IMAGE_WIDTH + x;
        for glyph_x in 0..PIXELS_PER_CHARACTER {
            pixels[output + glyph_x] = if multicolor {
                // TED doubles each two-bit pattern horizontally. The game
                // uses FF15, the character attribute, and FF17 for colors.
                match (pattern >> (6 - (glyph_x / 2) * 2)) & 3 {
                    0 => background,
                    2 => extra2,
                    _ => color & 0x7f,
                }
            } else if pattern & (0x80 >> glyph_x) != 0 {
                color & 0x7f
            } else {
                background
            };
        }
    }
}

/// Expand all 40x25 blocks, their objects, and a one-block rock border.
/// Arguments: memory is exactly one 64 KiB frozen image. Returns: 4032x2592
/// TED palette indices, or an error if a dictionary contains an unknown ID.
fn decode_labyrinth(memory: &[u8]) -> Result<Vec<u8>, String> {
    let background = memory[BACKGROUND_REGISTER] & 0x7f;
    let extra2 = memory[EXTRA_COLOR_2_REGISTER] & 0x7f;
    let mut cells = vec![Cell::default(); OUTPUT_CHAR_WIDTH * OUTPUT_CHAR_HEIGHT];
    for block_y in 0..MAP_HEIGHT + 2 * BORDER_BLOCKS {
        for block_x in 0..MAP_WIDTH + 2 * BORDER_BLOCKS {
            let border = block_x == 0
                || block_y == 0
                || block_x == MAP_WIDTH + BORDER_BLOCKS
                || block_y == MAP_HEIGHT + BORDER_BLOCKS;
            let large = if border {
                ROCK_BLOCK
            } else {
                let map_x = block_x - BORDER_BLOCKS;
                let map_y = block_y - BORDER_BLOCKS;
                large_tile_index(memory[MAP_BASE + map_y * MAP_WIDTH + map_x])
            };
            fill_large_block(memory, &mut cells, large, block_x, block_y)?;
        }
    }
    for map_y in 0..MAP_HEIGHT {
        for map_x in 0..MAP_WIDTH {
            draw_object(
                &mut cells,
                memory[MAP_BASE + map_y * MAP_WIDTH + map_x],
                map_x,
                map_y,
            );
        }
    }
    let mut pixels = vec![background; IMAGE_WIDTH * IMAGE_HEIGHT];
    for y in 0..OUTPUT_CHAR_HEIGHT {
        for x in 0..OUTPUT_CHAR_WIDTH {
            let cell = cells[y * OUTPUT_CHAR_WIDTH + x];
            draw_character(
                memory,
                &mut pixels,
                cell.code,
                cell.color,
                x * 8,
                y * 8,
                background,
                extra2,
            );
        }
    }
    Ok(pixels)
}

/// Compute the PNG CRC-32 for a chunk type followed by its data.
/// Arguments: type_code is the four-byte chunk tag and data is its payload.
/// Returns: the IEEE CRC-32 used by PNG.
fn chunk_crc(type_code: &[u8; 4], data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in type_code.iter().chain(data) {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

/// Append one PNG chunk to the file buffer.
/// Arguments: png receives the chunk; type_code names it; data is its bytes.
/// Returns: nothing; length and CRC are included automatically.
fn append_chunk(png: &mut Vec<u8>, type_code: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(type_code);
    png.extend_from_slice(data);
    png.extend_from_slice(&chunk_crc(type_code, data).to_be_bytes());
}

/// Wrap scanlines in a zlib stream using uncompressed DEFLATE blocks.
/// Arguments: bytes are PNG-filtered scanlines. Returns: a valid zlib stream.
fn zlib_uncompressed(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len() + bytes.len() / 65_535 * 5 + 16);
    output.extend_from_slice(&[0x78, 0x01]);
    for (index, block) in bytes.chunks(65_535).enumerate() {
        output.push(u8::from((index + 1) * 65_535 >= bytes.len()));
        let length = block.len() as u16;
        output.extend_from_slice(&length.to_le_bytes());
        output.extend_from_slice(&(!length).to_le_bytes());
        output.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in bytes {
        a = (a + byte as u32) % 65_521;
        b = (b + a) % 65_521;
    }
    output.extend_from_slice(&((b << 16) | a).to_be_bytes());
    output
}

/// Write the indexed-color PNG with the same TED palette as the emulator.
/// Arguments: path is the destination, pixels contains the full map, and
/// selected_palette maps TED indexes to RGB entries in the PNG palette.
/// Returns: success or an I/O error; the PNG has 4032x2592 pixels.
fn write_png(path: &Path, pixels: &[u8], selected_palette: Palette) -> std::io::Result<()> {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = [0u8; 13];
    header[..4].copy_from_slice(&(IMAGE_WIDTH as u32).to_be_bytes());
    header[4..8].copy_from_slice(&(IMAGE_HEIGHT as u32).to_be_bytes());
    header[8] = 8; // eight bits per palette index
    header[9] = 3; // indexed color
    append_chunk(&mut png, b"IHDR", &header);
    let mut palette = Vec::with_capacity(128 * 3);
    for index in 0..128 {
        let rgb = selected_palette.rgb(index);
        palette.extend_from_slice(&[(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]);
    }
    append_chunk(&mut png, b"PLTE", &palette);
    let mut scanlines = Vec::with_capacity(pixels.len() + IMAGE_HEIGHT);
    for row in pixels.chunks_exact(IMAGE_WIDTH) {
        scanlines.push(0); // PNG filter type 0: use the palette indices as-is
        scanlines.extend_from_slice(row);
    }
    append_chunk(&mut png, b"IDAT", &zlib_uncompressed(&scanlines));
    append_chunk(&mut png, b"IEND", &[]);
    fs::write(path, png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Verify rock border, row-major tiles, visible fire, and TED colors.
    /// Arguments: none. Returns: nothing; assertions fail on a decode error.
    fn two_stage_map_decodes_to_full_image() {
        let mut memory = vec![0u8; IMAGE_BYTES];
        memory[MAP_BASE..MAP_BASE + MAP_WIDTH * MAP_HEIGHT].fill(99);
        memory[MAP_BASE] = 0;
        memory[MAP_BASE + 1] = 0x96; // stationary fire in the next block
        memory[BACKGROUND_REGISTER] = 0x91;
        memory[EXTRA_COLOR_2_REGISTER] = 0xb2;
        memory[LARGE_TILE_BASE + 1] = 1; // second small-tile column
        memory[LARGE_TILE_BASE + 50 * LARGE_TILE_BYTES..LARGE_TILE_BASE + 51 * LARGE_TILE_BYTES]
            .fill(52);
        for (tile, code) in [(0, 2), (1, 3), (52, 0)] {
            let base = SMALL_TILE_BASE + tile * SMALL_TILE_BYTES;
            memory[base..base + 9].fill(code);
            memory[base + 9] = 0x4f;
        }
        let second_tile = SMALL_TILE_BASE + SMALL_TILE_BYTES;
        memory[second_tile + 1] = 4; // row zero, column one
        memory[second_tile + 3] = 6; // row one, column zero
        for (code, pattern) in [(2, 0xaa), (3, 0xff), (0xe0, 0xff)] {
            memory[CHARACTER_BASE + code * 8..CHARACTER_BASE + code * 8 + 8].fill(pattern);
        }
        memory[CHARACTER_BASE + 4 * 8..CHARACTER_BASE + 4 * 8 + 8].fill(0xaa);
        let pixels = decode_labyrinth(&memory).unwrap();
        let pixel = |x, y| pixels[y * IMAGE_WIDTH + x];
        assert_eq!(pixels.len(), 4032 * 2592);
        assert_eq!(pixel(0, 0), 0x32); // surrounding rock block
        assert_eq!(pixel(96, 96), 0x32); // first map block, character 2
        assert_eq!(pixel(120, 96), 0x4f); // second tile uses character 3
        assert_eq!(pixel(128, 96), 0x32); // character 4 is right of character 3
        assert_eq!(pixel(120, 104), 0x11); // character 6 is below it
        assert_eq!(pixel(256, 96), 0x3a); // fire overlays a blank tile
        assert_eq!(large_tile_index(199), 99);
    }

    #[test]
    /// Draw the game's visible fire frame when the captured glyph is blank.
    /// Arguments: none. Returns: nothing; assertions fail on incorrect pixels.
    fn fire_uses_stored_animation_frame() {
        let mut memory = vec![0u8; IMAGE_BYTES];
        memory[FIRE_FRAME_BASE..FIRE_FRAME_BASE + 8]
            .copy_from_slice(&[0x2c, 0xbf, 0xfa, 0x22, 0xbb, 0xb8, 0xce, 0x2c]);
        let mut pixels = vec![0u8; IMAGE_WIDTH * 8];
        draw_character(&memory, &mut pixels, FIRE_GLYPH_CODE, 0x5f, 0, 0, 0, 0x31);
        assert!(memory[CHARACTER_BASE + FIRE_GLYPH_CODE as usize * 8
            ..CHARACTER_BASE + FIRE_GLYPH_CODE as usize * 8 + 8]
            .iter()
            .all(|&byte| byte == 0));
        assert_eq!(&pixels[..8], &[0, 0, 0x31, 0x31, 0x5f, 0x5f, 0, 0]);
    }
}
