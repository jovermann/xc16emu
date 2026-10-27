// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! Visual editor for the 256 eight-by-eight glyphs in a C16 font ASM file.

use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Scale, Window, WindowOptions};
use std::{
    env, fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use xc16emu::Machine;

const COLUMNS: usize = 40;
const ROWS: usize = 25;
const CELL: usize = 8;
const BORDER: usize = 32;
const ZOOM: usize = 2;
const PICTURE_WIDTH: usize = COLUMNS * CELL;
const PICTURE_HEIGHT: usize = ROWS * CELL;
const WIDTH: usize = (PICTURE_WIDTH + 2 * BORDER) * ZOOM;
const HEIGHT: usize = (PICTURE_HEIGHT + 2 * BORDER) * ZOOM;
const PIXELS_X: usize = (17 + 10) * CELL;
const PIXELS_Y: usize = (9 + 2) * CELL;

/// The source text and byte positions of its 2,048 editable bitmap rows.
struct FontSource {
    path: PathBuf,
    text: String,
    spans: Vec<(usize, usize)>,
    pixels: [u8; 2048],
}

impl FontSource {
    /// Read the named ASM file and parse exactly 256 eight-row glyphs.
    /// Arguments: path is the source file. Returns: parsed font or a diagnostic.
    fn load(path: PathBuf) -> Result<Self, String> {
        let path = fs::canonicalize(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(path, text)
    }

    /// Find the active BYT pixel strings in the editable font.
    /// Arguments: path names the source; text is its contents. Returns: parsed font or an error.
    fn parse(path: PathBuf, text: String) -> Result<Self, String> {
        let mut spans = Vec::with_capacity(2048);
        let mut pixels = [0u8; 2048];
        let marker = "BYT %\"";
        let mut offset = 0;
        for line in text.split_inclusive('\n') {
            if let Some(marker_at) = line.find(marker) {
                let begin = offset + marker_at + marker.len();
                let end = begin + 8;
                let bytes = text.as_bytes();
                if end >= text.len() || bytes[end] != b'"' {
                    return Err(format!(
                        "{}: malformed pixel row near byte {begin}",
                        path.display()
                    ));
                }
                let mut row = 0u8;
                for (column, &pixel) in bytes[begin..end].iter().enumerate() {
                    match pixel {
                        b'#' => row |= 0x80 >> column,
                        b'.' => {}
                        _ => {
                            return Err(format!(
                                "{}: pixel row contains a character other than # or .",
                                path.display()
                            ))
                        }
                    }
                }
                if spans.len() == pixels.len() {
                    return Err(format!(
                        "{}: expected 2,048 pixel rows, found more",
                        path.display()
                    ));
                }
                pixels[spans.len()] = row;
                spans.push((begin, end));
            }
            offset += line.len();
        }
        if spans.len() != 2048 {
            return Err(format!(
                "{}: expected 2,048 pixel rows, found {}",
                path.display(),
                spans.len()
            ));
        }
        Ok(Self {
            path,
            text,
            spans,
            pixels,
        })
    }

    /// Read one glyph pixel.
    /// Arguments: glyph is 0..255; row/column are 0..7. Returns: true for ink.
    fn pixel(&self, glyph: usize, row: usize, column: usize) -> bool {
        self.pixels[glyph * 8 + row] & (0x80 >> column) != 0
    }

    /// Set one glyph pixel to ink or paper without repeatedly toggling it.
    /// Arguments: glyph and row/column locate the bit; ink is its target. Returns: true if it changed.
    fn set_pixel(&mut self, glyph: usize, row: usize, column: usize, ink: bool) -> bool {
        let index = glyph * 8 + row;
        let mask = 0x80 >> column;
        let before = self.pixels[index];
        if ink {
            self.pixels[index] |= mask;
        } else {
            self.pixels[index] &= !mask;
        }
        self.pixels[index] != before
    }

    /// Replace only active pixel strings and atomically save the same ASM path.
    /// Arguments: self contains current pixels. Returns: success or an I/O error.
    fn save(&mut self) -> Result<(), String> {
        let mut output = String::with_capacity(self.text.len());
        let mut previous = 0;
        for (index, &(begin, end)) in self.spans.iter().enumerate() {
            output.push_str(&self.text[previous..begin]);
            let bits = self.pixels[index];
            for column in 0..8 {
                output.push(if bits & (0x80 >> column) != 0 {
                    '#'
                } else {
                    '.'
                });
            }
            previous = end;
        }
        output.push_str(&self.text[previous..]);
        let filename = self
            .path
            .file_name()
            .ok_or("font path has no filename")?
            .to_string_lossy();
        let temporary = self.path.with_file_name(format!(".{filename}.tmp"));
        fs::write(&temporary, &output).map_err(|e| format!("{}: {e}", temporary.display()))?;
        if let Ok(metadata) = fs::metadata(&self.path) {
            fs::set_permissions(&temporary, metadata.permissions())
                .map_err(|e| format!("{}: {e}", temporary.display()))?;
        }
        fs::rename(&temporary, &self.path).map_err(|e| format!("{}: {e}", self.path.display()))?;
        self.text = output;
        Ok(())
    }
}

/// A pointer drag that pans the five-glyph strip horizontally.
struct PanDrag {
    start_x: f32,
    start_first: usize,
    right_button: bool,
}

/// A left-button stroke paints or erases every pixel crossed.
struct PaintStroke {
    ink: bool,
    last_x: f32,
    last_y: f32,
}

/// Interactive selection, autosave clock, and RGB frame buffer.
struct Editor {
    font: FontSource,
    reference_pixels: [u8; 2048],
    label_pixels: [u8; 2048],
    undo_stack: Vec<[u8; 2048]>,
    first: usize,
    selected: usize,
    show_help: bool,
    inverted: bool,
    flash_until: Option<Instant>,
    wheel_remainder: f32,
    save_due: Option<Instant>,
    save_message: &'static str,
    pan: Option<PanDrag>,
    paint: Option<PaintStroke>,
    left_was_down: bool,
    right_was_down: bool,
    buffer: Vec<u32>,
}

impl Editor {
    /// Build an editor showing the first five glyphs initially.
    /// Arguments: font is editable ASM; reference_pixels come from font_orig.asm.
    /// Returns: interactive editor state.
    fn new(font: FontSource, reference_pixels: [u8; 2048]) -> Self {
        let label_pixels = font.pixels;
        Self {
            font,
            reference_pixels,
            label_pixels,
            undo_stack: Vec::new(),
            first: 0,
            selected: 0,
            show_help: false,
            inverted: false,
            flash_until: None,
            wheel_remainder: 0.0,
            save_due: None,
            save_message: "",
            pan: None,
            paint: None,
            left_was_down: false,
            right_was_down: false,
            buffer: vec![Machine::rgb(0x6e); WIDTH * HEIGHT],
        }
    }

    /// Center the selected glyph among the five glyphs in the zoom strip.
    /// Arguments: self contains the selected code. Returns: nothing.
    fn center_selected(&mut self) {
        self.first = self.selected.saturating_sub(2).min(251);
    }

    /// Select a glyph and briefly reverse only its cell in the grid.
    /// Arguments: code is the new screen-code index. Returns: nothing.
    fn select_glyph(&mut self, code: usize) {
        if self.selected != code {
            self.selected = code;
            self.flash_until = Some(Instant::now() + Duration::from_millis(180));
        }
    }

    /// Move the selection by a signed number of glyphs and center it.
    /// Arguments: delta is usually 1 or 16. Returns: nothing.
    fn move_selected(&mut self, delta: i32) {
        self.select_glyph((self.selected as i32 + delta).clamp(0, 255) as usize);
        self.center_selected();
    }

    /// Accumulate vertical wheel motion, selecting one glyph per full step.
    /// Arguments: vertical is positive for upward scrolling. Returns: nothing.
    fn scroll_wheel(&mut self, vertical: f32) {
        self.wheel_remainder -= vertical;
        let steps = self.wheel_remainder.trunc() as i32;
        if steps != 0 {
            self.wheel_remainder -= steps as f32;
            self.move_selected(steps);
        }
    }

    /// Resolve a zoom-strip or PIXELS-pane coordinate to a glyph bit.
    /// Arguments: x/y are base pixels inside the white 320x200 picture.
    /// Returns: glyph, row, column, and whether the hit belongs to the top strip.
    fn pixel_hit(&self, x: f32, y: f32) -> Option<(usize, usize, usize, bool)> {
        if (0.0..PICTURE_WIDTH as f32).contains(&x) && (0.0..64.0).contains(&y) {
            return Some((
                self.first + x as usize / 64,
                y as usize / 8,
                (x as usize % 64) / 8,
                true,
            ));
        }
        if !self.show_help
            && (PIXELS_X as f32..(PIXELS_X + 64) as f32).contains(&x)
            && (PIXELS_Y as f32..(PIXELS_Y + 64) as f32).contains(&y)
        {
            return Some((
                self.selected,
                (y as usize - PIXELS_Y) / 8,
                (x as usize - PIXELS_X) / 8,
                false,
            ));
        }
        None
    }

    /// Set the pixel under a pointer sample and schedule autosave on change.
    /// Arguments: x/y are picture coordinates; ink is the stroke's target bit.
    /// Returns: nothing; the selected glyph and source pixels may change.
    fn paint_at(&mut self, x: f32, y: f32, ink: bool) {
        if let Some((glyph, row, column, top)) = self.pixel_hit(x, y) {
            if self.font.set_pixel(glyph, row, column, ink) {
                self.mark_dirty();
            }
            if top {
                self.select_glyph(glyph);
            }
        }
    }

    /// Begin a click or continuous paint stroke, choosing ink or eraser from the first bit.
    /// Arguments: x/y are picture coordinates. Returns: nothing if no editable pixel is hit.
    fn begin_paint(&mut self, x: f32, y: f32) {
        if let Some((glyph, row, column, _)) = self.pixel_hit(x, y) {
            let ink = !self.font.pixel(glyph, row, column);
            self.undo_stack.push(self.font.pixels);
            self.paint = Some(PaintStroke {
                ink,
                last_x: x,
                last_y: y,
            });
            self.paint_at(x, y, ink);
        }
    }

    /// Mark the edited font for the next one-second autosave.
    /// Arguments: none. Returns: nothing; save state changes.
    fn mark_dirty(&mut self) {
        self.save_due
            .get_or_insert_with(|| Instant::now() + Duration::from_secs(1));
        self.save_message = "UNSAVED";
    }

    /// Undo the last paint stroke or character restore as one edit.
    /// Arguments: none. Returns: nothing; restored pixels are scheduled for saving.
    fn undo(&mut self) {
        self.paint = None;
        if let Some(previous) = self.undo_stack.pop() {
            self.font.pixels = previous;
            self.mark_dirty();
        }
    }

    /// Restore the selected glyph from font_orig.asm and make that edit undoable.
    /// Arguments: none. Returns: nothing; only the eight selected rows may change.
    fn restore_selected(&mut self) {
        self.paint = None;
        let start = self.selected * 8;
        let end = start + 8;
        if self.font.pixels[start..end] != self.reference_pixels[start..end] {
            self.undo_stack.push(self.font.pixels);
            self.font.pixels[start..end].copy_from_slice(&self.reference_pixels[start..end]);
            self.mark_dirty();
        }
    }

    /// Find a first-set glyph with the same original bitmap as a second-set glyph.
    /// Arguments: target is 128..255; same-index pairs take priority if duplicates exist.
    /// Returns: a source code in 0..127, or none when the shape is absent there.
    fn matching_reference_source(&self, target: usize) -> Option<usize> {
        let reference = &self.reference_pixels[target * 8..target * 8 + 8];
        let preferred = target - 128;
        if self.reference_pixels[preferred * 8..preferred * 8 + 8] == *reference {
            return Some(preferred);
        }
        (0..128).find(|&source| self.reference_pixels[source * 8..source * 8 + 8] == *reference)
    }

    /// Copy edited first-set glyphs to every matching original shape in the second set.
    /// Arguments: none. Returns: nothing; all changed partners form one undoable edit.
    fn link_matching_glyphs(&mut self) {
        self.paint = None;
        let before = self.font.pixels;
        for target in 128..256 {
            if let Some(source) = self.matching_reference_source(target) {
                self.font
                    .pixels
                    .copy_within(source * 8..source * 8 + 8, target * 8);
            }
        }
        if self.font.pixels != before {
            self.undo_stack.push(before);
            self.mark_dirty();
        }
    }

    /// Paint every cell crossed since the last mouse sample, avoiding holes in fast drags.
    /// Arguments: x/y are current picture coordinates. Returns: nothing.
    fn continue_paint(&mut self, x: f32, y: f32) {
        let Some(stroke) = self.paint.as_ref() else {
            return;
        };
        let (start_x, start_y, ink) = (stroke.last_x, stroke.last_y, stroke.ink);
        let steps = ((x - start_x).abs().max((y - start_y).abs()) / 4.0)
            .ceil()
            .max(1.0) as usize;
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            self.paint_at(
                start_x + (x - start_x) * t,
                start_y + (y - start_y) * t,
                ink,
            );
        }
        if let Some(stroke) = self.paint.as_mut() {
            stroke.last_x = x;
            stroke.last_y = y;
        }
    }

    /// Handle painting, grid selection, and right-button or Shift-left panning.
    /// Arguments: window supplies pointer and modifier states. Returns: nothing.
    fn handle_mouse(&mut self, window: &Window) {
        let left = window.get_mouse_down(MouseButton::Left);
        let right = window.get_mouse_down(MouseButton::Right);
        let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
        let position = window.get_mouse_pos(MouseMode::Discard).map(|(x, y)| {
            (
                x / ZOOM as f32 - BORDER as f32,
                y / ZOOM as f32 - BORDER as f32,
            )
        });
        if right && !self.right_was_down {
            if let Some((x, _)) = position.filter(|&(x, y)| {
                (0.0..PICTURE_WIDTH as f32).contains(&x) && (0.0..64.0).contains(&y)
            }) {
                self.pan = Some(PanDrag {
                    start_x: x,
                    start_first: self.first,
                    right_button: true,
                });
            }
        }
        if left && !self.left_was_down {
            if let Some((x, y)) = position {
                if shift && (0.0..PICTURE_WIDTH as f32).contains(&x) && (0.0..64.0).contains(&y) {
                    self.pan = Some(PanDrag {
                        start_x: x,
                        start_first: self.first,
                        right_button: false,
                    });
                } else if self.pixel_hit(x, y).is_some() {
                    self.begin_paint(x, y);
                } else if (0.0..128.0).contains(&x) && (72.0..200.0).contains(&y) {
                    self.select_glyph(((y as usize - 72) / 8) * 16 + x as usize / 8);
                    self.center_selected();
                }
            }
        }
        if left {
            if let Some((x, y)) = position {
                self.continue_paint(x, y);
            }
        } else {
            self.paint = None;
        }
        if let Some(pan) = self.pan.as_ref() {
            if (pan.right_button && right) || (!pan.right_button && left) {
                if let Some((x, _)) = position {
                    let shift = ((x - pan.start_x) / 64.0).round() as i32;
                    self.first = (pan.start_first as i32 - shift).clamp(0, 251) as usize;
                    self.select_glyph(self.first + 2);
                }
            } else {
                self.pan = None;
            }
        }
        self.left_was_down = left;
        self.right_was_down = right;
    }

    /// Flush one dirty font no more often than once per second.
    /// Arguments: force also saves just before exit. Returns: success or an I/O error.
    fn save_if_due(&mut self, force: bool) -> Result<(), String> {
        if self
            .save_due
            .is_some_and(|due| force || Instant::now() >= due)
        {
            self.font.save()?;
            self.save_due = None;
            self.save_message = "SAVED";
        }
        Ok(())
    }

    /// Fill a rectangle in base 384x264 coordinates, enlarged to window pixels.
    /// Arguments: x, y, width, height locate the rectangle; color is packed RGB. Returns: nothing.
    fn rectangle(&mut self, x: usize, y: usize, width: usize, height: usize, color: u32) {
        let x0 = x * ZOOM;
        let y0 = y * ZOOM;
        for py in y0..(y0 + height * ZOOM) {
            let start = py * WIDTH + x0;
            self.buffer[start..start + width * ZOOM].fill(color);
        }
    }

    /// Choose paper and ink colors for the picture, leaving the border blue.
    /// Arguments: self holds the inversion state. Returns: background and foreground RGB.
    fn picture_colors(&self) -> (u32, u32) {
        if self.inverted {
            (Machine::rgb(0x10), Machine::rgb(0x71))
        } else {
            (Machine::rgb(0x71), Machine::rgb(0x10))
        }
    }

    /// Draw one glyph at a cell position from live pixels or fixed UI pixels.
    /// Arguments: code is 0..255; x/y are base coordinates; fixed selects UI labels; reverse swaps paper and ink. Returns: nothing.
    fn glyph(&mut self, code: usize, x: usize, y: usize, fixed: bool, reverse: bool) {
        let (paper, ink) = self.picture_colors();
        if reverse {
            self.rectangle(x, y, 8, 8, ink);
        }
        for row in 0..8 {
            let bits = if fixed {
                self.label_pixels[code * 8 + row]
            } else {
                self.font.pixels[code * 8 + row]
            };
            for column in 0..8 {
                if bits & (0x80 >> column) != 0 {
                    self.rectangle(x + column, y + row, 1, 1, if reverse { paper } else { ink });
                }
            }
        }
    }

    /// Map one ASCII label character to the C16 screen-code glyph.
    /// Arguments: character is a UI label byte. Returns: screen code or space.
    fn label_code(character: u8) -> usize {
        match character {
            b'A'..=b'Z' => (character - b'A' + 1) as usize,
            b'a'..=b'z' => (character - b'a' + 1) as usize,
            b'0'..=b'9' | b' ' | b'$' | b'%' | b'#' | b':' | b'.' | b'/' | b'-' | b'+' => {
                character as usize
            }
            _ => 32,
        }
    }

    /// Draw an ASCII label in 8x8 cells, using font glyphs for its ink.
    /// Arguments: x and y are base pixel origins; text is the label. Returns: nothing.
    fn text(&mut self, x: usize, y: usize, text: &str) {
        for (index, byte) in text.bytes().enumerate() {
            self.glyph(Self::label_code(byte), x + index * 8, y, true, false);
        }
    }

    /// Render the complete border, zoom strip, glyph matrix, and information pane.
    /// Arguments: self holds current font and selection. Returns: nothing; buffer changes.
    fn render(&mut self) {
        let (paper, ink) = self.picture_colors();
        self.buffer.fill(Machine::rgb(0x6e));
        self.rectangle(BORDER, BORDER, PICTURE_WIDTH, PICTURE_HEIGHT, paper);
        for row in 0..8 {
            for column in 0..40 {
                let glyph = self.first + column / 8;
                let bit = self.font.pixels[glyph * 8 + row] & (0x80 >> (column % 8)) != 0;
                if bit {
                    self.rectangle(BORDER + column * 8, BORDER + row * 8, 8, 8, ink);
                }
            }
        }
        let strip_label = format!("GLYPHS ${:02X}-${:02X}", self.first, self.first + 4);
        self.text(BORDER, BORDER + 8 * 8, &strip_label);
        let flash = self.flash_until.is_some_and(|until| Instant::now() < until);
        for code in 0..256 {
            let x = BORDER + (code % 16) * 8;
            let y = BORDER + (9 + code / 16) * 8;
            self.glyph(code, x, y, false, flash && code == self.selected);
        }
        let pane_x = BORDER + 17 * 8;
        let pane_y = BORDER + 9 * 8;
        if self.show_help {
            for (row, line) in [
                "KEY HELP",
                "WHEEL SELECT CHAR",
                "UP DOWN 16 CHARS",
                "RIGHT DRAG SCROLL",
                "SHIFT DRAG SCROLL",
                "CLICK GRID SELECT",
                "I INVERT PICTURE",
                "U UNDO LAST EDIT",
                "R RESTORE CHAR",
                "L LINK SAME CHARS",
                "H CLOSE HELP",
                "Q OR ESC QUIT",
                "AUTOSAVE 1 SEC",
            ]
            .iter()
            .enumerate()
            {
                self.text(pane_x, pane_y + row * 8, line);
            }
        } else {
            self.text(pane_x, pane_y, &format!("CHAR ${:02X}", self.selected));
            self.text(pane_x, pane_y + 8, "ROW HEX   PIXELS");
            for row in 0..8 {
                let byte = self.font.pixels[self.selected * 8 + row];
                let pixels: String = (0..8)
                    .map(|column| {
                        if byte & (0x80 >> column) != 0 {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect();
                self.text(
                    pane_x,
                    pane_y + (row + 2) * 8,
                    &format!("{}   ${:02X}   {}", row, byte, pixels),
                );
            }
            self.text(pane_x, pane_y + 10 * 8, "L LINK SAME CHARS");
            self.text(pane_x, pane_y + 11 * 8, "H KEY HELP");
            self.text(pane_x, pane_y + 12 * 8, "I INVERT  Q QUIT");
            self.text(pane_x, pane_y + 13 * 8, "U UNDO  R RESTORE");
            self.text(pane_x, pane_y + 14 * 8, self.save_message);
        }
    }
}

/// Print the executable version and copyright information.
/// Arguments: none. Returns: nothing; writes to stdout.
fn print_version() {
    println!(
        "font-editor version {} *** Copyright (c) 2026 Johannes Overmann *** https://github.com/jovermann/xc16emu",
        env!("CARGO_PKG_VERSION")
    );
}

/// Print command-line usage and the editor's keyboard and mouse controls.
/// Arguments: none. Returns: nothing; writes to stdout.
fn print_help() {
    print_version();
    println!(
        "\nUsage: font-editor FONT.ASM\n\nOptions:\n  -h, --help       Show this help.\n      --version    Show version and copyright.\n\nControls:\n  Left/Right       Select a glyph; hold to repeat.\n  Up/Down          Jump by 16 glyphs; hold to repeat.\n  Scroll wheel     Select previous or next glyph.\n  Left drag        Paint or erase pixels in the top strip or PIXELS pane.\n  Right/Shift drag Scroll the top strip sideways.\n  Click grid glyph Center it in the top strip.\n  U                Undo the last paint stroke or restore.\n  R                Restore the selected glyph from font_orig.asm.\n  L                Copy edited 0-127 glyphs to matching 128-255 shapes.\n  I                Invert the picture (border stays blue).\n  H                Toggle key help.\n  Q or Escape      Save and exit.\n\nL matches original bitmaps in font_orig.asm, even at different code indices.\nThe selected glyph briefly flashes in reverse when selection changes.\nChanged pixels are saved to FONT.ASM once per second."
    );
}

/// Parse CLI arguments and run the editor until Q, Escape, or window close.
/// Arguments: none; reads process arguments. Returns: success or an error.
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let path = match args.next().as_deref() {
        Some("--help" | "-h") => {
            print_help();
            return Ok(());
        }
        Some("--version") => {
            print_version();
            return Ok(());
        }
        Some(value) => PathBuf::from(value),
        None => return Err("usage: font-editor FONT.ASM".into()),
    };
    if args.next().is_some() {
        return Err("usage: font-editor FONT.ASM".into());
    }
    let font = FontSource::load(path)?;
    let reference = FontSource::load(font.path.with_file_name("font_orig.asm"))?;
    let mut editor = Editor::new(font, reference.pixels);
    let mut window = Window::new(
        "Font Editor",
        WIDTH,
        HEIGHT,
        WindowOptions {
            scale: Scale::X1,
            resize: false,
            ..WindowOptions::default()
        },
    )
    .map_err(|e| e.to_string())?;
    window.set_target_fps(60);
    window.set_key_repeat_delay(0.25);
    window.set_key_repeat_rate(0.05);
    while window.is_open() {
        let quit = window.is_key_pressed(Key::Q, KeyRepeat::No)
            || window.is_key_pressed(Key::Escape, KeyRepeat::No);
        if window.is_key_pressed(Key::H, KeyRepeat::No) {
            editor.show_help = !editor.show_help;
        }
        if window.is_key_pressed(Key::I, KeyRepeat::No) {
            editor.inverted = !editor.inverted;
        }
        if window.is_key_pressed(Key::U, KeyRepeat::No) {
            editor.undo();
        }
        if window.is_key_pressed(Key::R, KeyRepeat::No) {
            editor.restore_selected();
        }
        if window.is_key_pressed(Key::L, KeyRepeat::No) {
            editor.link_matching_glyphs();
        }
        for key in window.get_keys_pressed(KeyRepeat::Yes) {
            match key {
                Key::Left => editor.move_selected(-1),
                Key::Right => editor.move_selected(1),
                Key::Up => editor.move_selected(-16),
                Key::Down => editor.move_selected(16),
                _ => {}
            }
        }
        if let Some((_, vertical)) = window.get_scroll_wheel() {
            editor.scroll_wheel(vertical);
        }
        editor.handle_mouse(&window);
        editor.save_if_due(false)?;
        editor.render();
        window
            .update_with_buffer(&editor.buffer, WIDTH, HEIGHT)
            .map_err(|e| e.to_string())?;
        if quit {
            break;
        }
    }
    editor.save_if_due(true)
}

/// Report a startup, parsing, window, or save error and exit with failure.
/// Arguments: none. Returns: nothing; may exit the process.
fn main() {
    if let Err(error) = run() {
        eprintln!("font-editor: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construct an editor with the separate reference font for tests.
    /// Arguments: font is editable source. Returns: ready editor state.
    fn test_editor(font: FontSource) -> Editor {
        let reference = FontSource::parse(
            PathBuf::from("font_orig.asm"),
            include_str!("../../free_roms/font_orig.asm").to_string(),
        )
        .unwrap();
        Editor::new(font, reference.pixels)
    }

    #[test]
    /// Parse the editable font, change one bit, and preserve its other text.
    /// Arguments: none. Returns: nothing; assertions report a regression.
    fn source_round_trip_changes_only_active_pixels() {
        let original = include_str!("../../free_roms/font.asm").to_string();
        let path =
            env::temp_dir().join(format!("xc16_font_editor_test_{}.asm", std::process::id()));
        let mut font = FontSource::parse(path.clone(), original.clone()).unwrap();
        let old = font.pixels[8];
        font.set_pixel(1, 0, 0, !font.pixel(1, 0, 0));
        assert_eq!(font.pixels[8], old ^ 0x80);
        font.save().unwrap();
        let changed = fs::read_to_string(&path).unwrap();
        assert!(changed.contains("; Screen code $01"));
        assert!(!changed.contains("; original %\""));
        assert_eq!(changed.len(), original.len());
        assert_eq!(
            FontSource::parse(path.clone(), changed).unwrap().pixels[8],
            old ^ 0x80
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    /// Confirm the border, zoom pixels, and complete lower glyph grid render.
    /// Arguments: none. Returns: nothing; assertions report layout errors.
    fn renders_border_zoom_and_grid() {
        let font = FontSource::parse(
            PathBuf::from("font.asm"),
            include_str!("../../free_roms/font.asm").to_string(),
        )
        .unwrap();
        let first_row = font.pixels[0];
        let mut editor = test_editor(font);
        editor.render();
        assert_eq!(editor.buffer[0], Machine::rgb(0x6e));
        let zoom_x = BORDER + 2 * 8;
        let zoom_y = BORDER;
        let zoom_pixel = editor.buffer[(zoom_y * ZOOM) * WIDTH + zoom_x * ZOOM];
        assert_eq!(
            zoom_pixel,
            if first_row & 0x20 != 0 {
                Machine::rgb(0x10)
            } else {
                Machine::rgb(0x71)
            }
        );
        assert_eq!(
            editor.buffer[(BORDER * ZOOM) * WIDTH + (BORDER + 16 * 8) * ZOOM],
            Machine::rgb(0x71)
        );
        let label_row = (BORDER + 8 * 8) * ZOOM;
        let labels_before =
            editor.buffer[label_row * WIDTH..(label_row + 8 * ZOOM) * WIDTH].to_vec();
        let new_ink = !editor.font.pixel(7, 0, 0);
        editor.font.set_pixel(7, 0, 0, new_ink); // The G in "GLYPHS" must remain readable.
        editor.render();
        assert_eq!(
            &editor.buffer[label_row * WIDTH..(label_row + 8 * ZOOM) * WIDTH],
            &labels_before
        );
        let last_cell = (BORDER + 15 * 8, BORDER + 9 * 8 + 15 * 8);
        assert!(last_cell.1 + 8 <= BORDER + PICTURE_HEIGHT);
        if let Ok(path) = env::var("XC16_FONT_EDITOR_PREVIEW") {
            let mut image = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
            for &rgb in &editor.buffer {
                image.extend_from_slice(&[
                    ((rgb >> 16) & 255) as u8,
                    ((rgb >> 8) & 255) as u8,
                    (rgb & 255) as u8,
                ]);
            }
            fs::write(path, image).unwrap();
        }
    }

    #[test]
    /// Delay autosave until the one-second deadline and clear dirty state on save.
    /// Arguments: none. Returns: nothing; assertions report an autosave regression.
    fn autosave_waits_for_deadline() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "xc16_font_editor_autosave_{}_{}.asm",
            std::process::id(),
            unique
        ));
        let source = include_str!("../../free_roms/font.asm").to_string();
        let font = FontSource::parse(path.clone(), source).unwrap();
        let mut editor = test_editor(font);
        editor.begin_paint(0.0, 0.0);
        editor.save_if_due(false).unwrap();
        assert!(!path.exists());
        editor.save_due = Some(Instant::now() - Duration::from_millis(1));
        editor.save_if_due(false).unwrap();
        assert!(path.exists());
        assert!(editor.save_due.is_none());
        fs::remove_file(path).unwrap();
    }

    #[test]
    /// Verify a held stroke fills crossed cells and the PIXELS pane edits the selected glyph.
    /// Arguments: none. Returns: nothing; assertions report pointer editing regressions.
    fn paints_dragged_pixels_in_both_views() {
        let font = FontSource::parse(
            PathBuf::from("font.asm"),
            include_str!("../../free_roms/font.asm").to_string(),
        )
        .unwrap();
        let mut editor = test_editor(font);
        let ink = !editor.font.pixel(0, 0, 0);
        editor.begin_paint(0.0, 0.0);
        editor.continue_paint(31.0, 0.0);
        for column in 0..4 {
            assert_eq!(editor.font.pixel(0, 0, column), ink);
        }
        editor.paint = None;
        editor.selected = 5;
        let ink = !editor.font.pixel(5, 0, 0);
        editor.begin_paint(PIXELS_X as f32, PIXELS_Y as f32);
        editor.continue_paint((PIXELS_X + 15) as f32, PIXELS_Y as f32);
        assert_eq!(editor.font.pixel(5, 0, 0), ink);
        assert_eq!(editor.font.pixel(5, 0, 1), ink);
        assert!(editor.save_due.is_some());
    }

    #[test]
    /// Check only the selected grid cell flashes and inversion leaves the border alone.
    /// Arguments: none. Returns: nothing; assertions report drawing regressions.
    fn flashes_selected_glyph_and_inverts_picture_only() {
        let font = FontSource::parse(
            PathBuf::from("font.asm"),
            include_str!("../../free_roms/font.asm").to_string(),
        )
        .unwrap();
        let mut editor = test_editor(font);
        let sample = |buffer: &[u32], x: usize, y: usize| {
            buffer[((BORDER + y) * ZOOM) * WIDTH + (BORDER + x) * ZOOM]
        };
        editor.select_glyph(2);
        editor.flash_until = Some(Instant::now() + Duration::from_secs(1));
        editor.render();
        let highlighted = sample(&editor.buffer, 2 * CELL, 9 * CELL);
        let adjacent = sample(&editor.buffer, 3 * CELL, 9 * CELL);
        editor.flash_until = Some(Instant::now() - Duration::from_millis(1));
        editor.render();
        assert_ne!(highlighted, sample(&editor.buffer, 2 * CELL, 9 * CELL));
        assert_eq!(adjacent, sample(&editor.buffer, 3 * CELL, 9 * CELL));
        assert!(editor.flash_until.is_some()); // Expiry never restarts a periodic flash.
        editor.inverted = true;
        editor.render();
        assert_eq!(editor.buffer[0], Machine::rgb(0x6e));
        assert_eq!(sample(&editor.buffer, 0, 8 * CELL), Machine::rgb(0x10));
    }

    #[test]
    /// Confirm wheel ticks and fractional trackpad motion select adjacent glyphs.
    /// Arguments: none. Returns: nothing; assertions report selection regressions.
    fn wheel_moves_selection_and_restarts_one_flash() {
        let font = FontSource::parse(
            PathBuf::from("font.asm"),
            include_str!("../../free_roms/font.asm").to_string(),
        )
        .unwrap();
        let mut editor = test_editor(font);
        assert!(editor.flash_until.is_none());
        editor.scroll_wheel(-0.4);
        assert_eq!(editor.selected, 0);
        assert!(editor.flash_until.is_none());
        editor.scroll_wheel(-0.6);
        assert_eq!(editor.selected, 1);
        assert!(editor.flash_until.is_some());
        editor.flash_until = None;
        editor.scroll_wheel(1.0);
        assert_eq!(editor.selected, 0);
        assert!(editor.flash_until.is_some());
        editor.flash_until = None;
        editor.move_selected(-1);
        assert!(editor.flash_until.is_none());
    }

    #[test]
    /// A whole drag and one reference restore each occupy one undo step.
    /// Arguments: none. Returns: nothing; assertions report undo or restore regressions.
    fn undo_reverses_strokes_and_reference_restore() {
        let font = FontSource::parse(
            PathBuf::from("font.asm"),
            include_str!("../../free_roms/font.asm").to_string(),
        )
        .unwrap();
        let mut editor = test_editor(font);
        editor.font.pixels[7] = editor.reference_pixels[7] ^ 0x80;
        let original = editor.font.pixels;
        editor.begin_paint(0.0, 0.0);
        editor.continue_paint(31.0, 0.0);
        editor.paint = None;
        let painted = editor.font.pixels;
        assert_ne!(painted, original);
        assert_eq!(editor.undo_stack.len(), 1);

        editor.restore_selected();
        assert_eq!(editor.font.pixels[..8], editor.reference_pixels[..8]);
        assert_eq!(editor.font.pixels[8..], original[8..]);
        assert_eq!(editor.undo_stack.len(), 2);
        editor.restore_selected(); // An unchanged glyph creates no new undo step.
        assert_eq!(editor.undo_stack.len(), 2);
        editor.undo();
        assert_eq!(editor.font.pixels, painted);
        editor.undo();
        assert_eq!(editor.font.pixels, original);
        assert!(editor.save_due.is_some());
    }

    #[test]
    /// Match original shapes across code indices and undo the whole copy at once.
    /// Arguments: none. Returns: nothing; assertions report glyph-copy regressions.
    fn link_copies_matching_shapes_at_any_index() {
        let font = FontSource::parse(
            PathBuf::from("font.asm"),
            include_str!("../../free_roms/font.asm").to_string(),
        )
        .unwrap();
        let mut editor = test_editor(font);
        let same = (128..256)
            .find(|&target| editor.matching_reference_source(target) == Some(target - 128))
            .unwrap();
        let shifted = (128..256)
            .find(|&target| {
                editor
                    .matching_reference_source(target)
                    .is_some_and(|source| source != target - 128)
            })
            .unwrap();
        let absent = (128..256)
            .find(|&target| editor.matching_reference_source(target).is_none())
            .unwrap();
        let duplicate = (128..256)
            .find(|&target| {
                let shape = &editor.reference_pixels[target * 8..target * 8 + 8];
                (0..128)
                    .filter(|&source| editor.reference_pixels[source * 8..source * 8 + 8] == *shape)
                    .count()
                    > 1
                    && editor.matching_reference_source(target) == Some(target - 128)
            })
            .unwrap();
        assert_eq!(
            editor.matching_reference_source(duplicate),
            Some(duplicate - 128)
        );
        let shifted_source = editor.matching_reference_source(shifted).unwrap();
        editor.font.pixels[(same - 128) * 8] ^= 0x80;
        editor.font.pixels[same * 8] = editor.font.pixels[(same - 128) * 8] ^ 0x40;
        editor.font.pixels[shifted_source * 8] ^= 0x80;
        editor.font.pixels[shifted * 8] = editor.font.pixels[shifted_source * 8] ^ 0x40;
        editor.font.pixels[absent * 8] ^= 0x40;
        let before = editor.font.pixels;

        editor.link_matching_glyphs();
        assert_eq!(
            editor.font.pixels[same * 8..same * 8 + 8],
            editor.font.pixels[(same - 128) * 8..(same - 128) * 8 + 8]
        );
        assert_eq!(
            editor.font.pixels[shifted * 8..shifted * 8 + 8],
            editor.font.pixels[shifted_source * 8..shifted_source * 8 + 8]
        );
        assert_eq!(
            editor.font.pixels[absent * 8..absent * 8 + 8],
            before[absent * 8..absent * 8 + 8]
        );
        assert_eq!(editor.undo_stack.len(), 1);
        editor.link_matching_glyphs();
        assert_eq!(editor.undo_stack.len(), 1); // A no-op adds no undo entry.
        editor.undo();
        assert_eq!(editor.font.pixels, before);
    }
}
