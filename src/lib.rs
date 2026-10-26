// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! Portable C16/Plus/4 hardware core, independent of the desktop interface.

mod cpu;
mod sound;
mod video;
pub use cpu::Cpu;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
pub use video::{COMPACT_BORDER, COMPACT_HEIGHT, COMPACT_WIDTH, HEIGHT, WIDTH};

/// PAL raster lines per frame.
pub const LINES_PER_FRAME: u16 = 312;
/// Nominal PAL TED timing ticks per wall-clock second.
pub const PAL_TED_TICKS_PER_SECOND: u64 = 114 * LINES_PER_FRAME as u64 * 50;
/// Half-speed TED ticks in one PAL raster line. The CPU can execute one
/// cycle per tick in the border, or one cycle per two ticks during display.
const TICKS_PER_LINE: u32 = 114;

/// TED registers and timing state.
pub struct Ted {
    /// Last written register values.
    pub registers: [u8; 64],
    /// Current raster line, including the border.
    pub raster: u16,
    /// Output scanline, separate from the software-writable raster counter.
    display_line: u16,
    /// TED timing ticks into this raster line (114 ticks per PAL line).
    pub line_cycles: u32,
    /// Unpaired TED tick for the three timers, which run at the slow clock.
    timer_tick_remainder: u8,
    /// Number of frames completed.
    pub frames: u64,
    /// Total elapsed TED ticks, unaffected by raster-counter writes.
    pub ticks: u64,
    /// Programmed interrupt line.
    pub irq_line: u16,
    /// Pending interrupt sources.
    pub irq: u8,
    /// Enabled interrupt sources.
    pub irq_enable: u8,
    /// Timer reload values.
    pub reload: [u16; 3],
    /// Current timer values.
    pub timer: [u16; 3],
    /// Last scanned keyboard value.
    pub key_latch: u8,
}

impl Default for Ted {
    /// Initialize TED registers, counters, and frame state.
    /// Arguments: none. Returns: a fresh TED state.
    fn default() -> Self {
        Self {
            registers: [0; 64],
            raster: 0,
            display_line: 0,
            line_cycles: 0,
            timer_tick_remainder: 0,
            frames: 0,
            ticks: 0,
            irq_line: 0,
            irq: 0,
            irq_enable: 0,
            reload: [0; 3],
            timer: [0; 3],
            key_latch: 0xff,
        }
    }
}

/// A complete machine with ROM, RAM, CPU, TED, keyboard, and a virtual drive.
pub struct Machine {
    /// Physical 64 KiB RAM; smaller models use `ram_mask` mirroring.
    pub ram: Box<[u8; 65536]>,
    /// CPU registers.
    pub cpu: Cpu,
    /// Number of successfully executed CPU instructions, for the GUI MIPS meter.
    pub instructions_executed: u64,
    /// TED state.
    pub ted: Ted,
    /// TED audio oscillator and samples waiting for the desktop audio device.
    sound: sound::Sound,
    /// Four pairs of 16 KiB ROMs, with missing optional banks zero-filled.
    roms: [Vec<u8>; 8],
    /// Lower ROM bank number.
    low_bank: usize,
    /// Upper ROM bank number.
    high_bank: usize,
    /// CPU reads RAM beneath the ROMs when true.
    ram_enabled: bool,
    /// Physical RAM mirroring mask.
    ram_mask: u16,
    /// FD10 user port.
    user_port: u8,
    /// FD30 keyboard row select.
    key_port: u8,
    /// Pressed keys in the eight by eight hardware matrix.
    pub key_matrix: [u8; 8],
    /// Last rendered full PAL frame as TED color indices.
    pub pixels: Box<[u8; WIDTH * HEIGHT]>,
    /// Mode observed while drawing each raster line.
    pub line_modes: [u8; HEIGHT],
    /// Video matrix base latched at the start of each eight-line character row.
    video_matrix_latch: usize,
    /// Virtual drive IEC port, status, and handshake flag.
    iec: [u8; 3],
    /// IEC command currently being handled.
    iec_command: u8,
    /// Host filename transmitted by the ROM.
    iec_name: Vec<u8>,
    /// Current file contents for read operations.
    iec_input: Vec<u8>,
    /// Read offset within `iec_input`.
    iec_position: usize,
    /// Current file contents for write operations.
    iec_output: Vec<u8>,
    /// True while IEC filename bytes are arriving.
    iec_naming: bool,
    /// Host directory exposed as IEC device 8.
    drive_root: PathBuf,
    /// True while the virtual drive has a file open for writing.
    iec_writing: bool,
    /// Whether IEC file writes are simulated without touching the host disk.
    iec_dry_run: bool,
    /// Print virtual-drive operations for troubleshooting when enabled.
    verbose: bool,
}

impl Machine {
    /// Load required BASIC and system ROMs and any available optional ROMs.
    /// Arguments: path locates the ROM directory. Returns: a ready machine or an I/O/ROM-size error.
    pub fn from_rom_dir(path: impl AsRef<Path>) -> io::Result<Self> {
        let names = [
            "basrom.dat",
            "sysrom.dat",
            "solrom.dat",
            "sohrom.dat",
            "e1lrom.dat",
            "e1hrom.dat",
            "e2lrom.dat",
            "e2hrom.dat",
        ];
        let mut roms: [Vec<u8>; 8] = std::array::from_fn(|_| vec![0; 16384]);
        for (i, name) in names.iter().enumerate() {
            match fs::read(path.as_ref().join(name)) {
                Ok(data) if data.len() == 16384 => roms[i] = data,
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{name} must be 16384 bytes"),
                    ))
                }
                Err(e) if i < 2 => return Err(e),
                Err(_) => {}
            }
        }
        let mut m = Self {
            ram: Box::new([0; 65536]),
            cpu: Cpu::default(),
            instructions_executed: 0,
            ted: Ted::default(),
            sound: sound::Sound::default(),
            roms,
            low_bank: 0,
            high_bank: 0,
            ram_enabled: false,
            ram_mask: 0xffff,
            user_port: 0,
            key_port: 0xff,
            key_matrix: [0; 8],
            pixels: Box::new([0; WIDTH * HEIGHT]),
            line_modes: [0; HEIGHT],
            video_matrix_latch: 0,
            iec: [0; 3],
            iec_command: 0,
            iec_name: Vec::new(),
            iec_input: Vec::new(),
            iec_position: 0,
            iec_output: Vec::new(),
            iec_naming: false,
            drive_root: PathBuf::from("."),
            iec_writing: false,
            iec_dry_run: false,
            verbose: false,
        };
        m.reset();
        Ok(m)
    }

    /// Restart the machine without clearing RAM, as the original reset did.
    /// Arguments: self is the mutable machine. Returns: nothing.
    pub fn reset(&mut self) {
        self.ted = Ted::default();
        self.sound.reset();
        self.ted.key_latch = 0xff;
        self.cpu = Cpu::default();
        self.cpu.pc = u16::from_le_bytes([self.roms[1][0x3ffc], self.roms[1][0x3ffd]]);
        self.low_bank = 0;
        self.high_bank = 0;
        self.ram_enabled = false;
        self.key_matrix.fill(0);
        self.key_port = 0xff;
        self.video_matrix_latch = 0;
    }

    /// Set a 16, 32, or 64 KiB model, clear physical RAM, and reset.
    /// Arguments: kib is 16, 32, or 64 KiB. Returns: success or an invalid-size error.
    pub fn set_ram_size(&mut self, kib: u16) -> Result<(), &'static str> {
        self.ram_mask = match kib {
            16 => 0x3fff,
            32 => 0x7fff,
            64 => 0xffff,
            _ => return Err("RAM must be 16, 32, or 64 KiB"),
        };
        self.ram.fill(0);
        self.reset();
        Ok(())
    }

    /// Read CPU-visible memory, honoring ROM banking and I/O.
    /// Arguments: address is the 16-bit CPU address. Returns: the selected byte.
    pub fn read(&mut self, address: u16) -> u8 {
        if (0xfd00..0xff40).contains(&address) {
            return self.read_io(address);
        }
        if address < 0x8000 || self.ram_enabled {
            return self.ram[(address & self.ram_mask) as usize];
        }
        if address < 0xc000 {
            return self.roms[self.low_bank * 2][(address & 0x3fff) as usize];
        }
        if address < 0xfc00 {
            return self.roms[self.high_bank * 2 + 1][(address & 0x3fff) as usize];
        }
        self.roms[1][(address & 0x3fff) as usize]
    }

    /// Write RAM or a hardware register. Writes beneath ROM remain in RAM.
    /// Arguments: address is the 16-bit CPU address; value is the byte to store. Returns: nothing.
    pub fn write(&mut self, address: u16, value: u8) {
        if (0xfd00..0xff40).contains(&address) {
            self.write_io(address, value);
        } else {
            self.ram[(address & self.ram_mask) as usize] = value;
        }
    }

    /// Load a Commodore PRG with its two-byte address prefix.
    /// Arguments: data contains load address followed by bytes. Returns: load address or a malformed/oversize error.
    pub fn load_prg(&mut self, data: &[u8]) -> Result<u16, &'static str> {
        if data.len() < 2 {
            return Err("missing PRG address");
        }
        let start = u16::from_le_bytes([data[0], data[1]]) as usize;
        if start + data.len() - 2 > 65536 {
            return Err("PRG exceeds memory");
        }
        self.ram[start..start + data.len() - 2].copy_from_slice(&data[2..]);
        Ok(start as u16)
    }

    /// Restore the 64 KiB frozen-image format used by the old emulator.
    /// The snapshot stores a prepared stack frame; a short RAM trampoline
    /// restores A, X, Y and P, then returns to its saved program counter.
    /// Arguments: data is the complete old-format snapshot. Returns: success or a size error.
    pub fn load_frozen_image(&mut self, data: &[u8]) -> Result<(), &'static str> {
        if data.len() != 65536 {
            return Err("frozen image must be exactly 65536 bytes");
        }
        self.reset();
        self.ram[2..16384].copy_from_slice(&data[2..16384]);
        for address in 0xff06..0xff1a {
            self.write(address, data[address as usize]);
        }
        self.write(0xfd30, 0);
        self.ram[0x7200..0x7207].copy_from_slice(&[0x68, 0xa8, 0x68, 0xaa, 0x68, 0x28, 0x60]);
        self.cpu.sp = data[0x7ef3];
        self.cpu.pc = 0x7200;
        Ok(())
    }

    /// Choose the directory exposed to BASIC's DLOAD and DSAVE commands.
    /// Arguments: path is the directory to expose. Returns: nothing.
    pub fn set_drive_root(&mut self, path: impl Into<PathBuf>) {
        self.drive_root = path.into();
    }

    /// Set a virtual-drive dry run that acknowledges saves without writing.
    /// Arguments: enabled selects acknowledgement without writing. Returns: nothing.
    pub fn set_dry_run(&mut self, enabled: bool) {
        self.iec_dry_run = enabled;
    }

    /// Enable or disable virtual-drive diagnostic messages.
    /// Arguments: enabled turns diagnostic messages on. Returns: nothing.
    pub fn set_verbose(&mut self, enabled: bool) {
        self.verbose = enabled;
    }

    /// Press or release one C16 matrix key, numbered 0 to 63.
    /// Arguments: index is a key number from 0 to 63; pressed sets or clears it. Returns: nothing; out-of-range indices are ignored.
    pub fn set_key(&mut self, index: u8, pressed: bool) {
        if index >= 64 {
            return;
        }
        let row = (index / 8) as usize;
        let bit = 1 << (index % 8);
        if pressed {
            self.key_matrix[row] |= bit;
        } else {
            self.key_matrix[row] &= !bit;
        }
    }

    /// Type text through the emulated keyboard matrix. Each character is held
    /// and released for two PAL frames so the ROM's own scan sees both edges.
    /// This keeps BASIC integration tests independent of GUI event handling.
    /// Arguments: text is the BASIC input to type. Returns: success or an unsupported-character/emulation error.
    pub fn type_text(&mut self, text: &str) -> Result<(), String> {
        for character in text.chars() {
            let (key, shift) = matrix_for_char(character)
                .ok_or_else(|| format!("cannot type character {character:?}"))?;
            if shift {
                self.set_key(15, true);
            }
            self.set_key(key, true);
            self.run_frames(2)?;
            self.set_key(key, false);
            if shift {
                self.set_key(15, false);
            }
            self.run_frames(2)?;
        }
        Ok(())
    }

    /// Execute one instruction and advance video and timers by its cycles.
    /// Arguments: self is the mutable machine. Returns: consumed CPU cycles or an execution error.
    pub fn step(&mut self) -> Result<u8, String> {
        let cycles = self.step_cpu()?;
        self.instructions_executed += 1;
        self.advance_hardware(cycles);
        Ok(cycles)
    }

    /// Run complete PAL frames.
    /// Arguments: count is the frame increment. Returns: success or an execution error.
    pub fn run_frames(&mut self, count: u64) -> Result<(), String> {
        let target = self.ted.frames + count;
        while self.ted.frames < target {
            self.step()?;
        }
        Ok(())
    }

    /// Enable TED audio capture at the output device's sample rate. A rate of
    /// zero disables capture, which keeps headless runs inexpensive by default.
    /// Arguments: sample_rate is samples per second; zero disables capture. Returns: nothing.
    pub fn set_audio_sample_rate(&mut self, sample_rate: u32) {
        self.sound.set_sample_rate(sample_rate);
    }

    /// Take the mono, floating-point audio generated since the previous call.
    /// Samples lie in the range -1 to 1 and can be tested without a GUI.
    /// Arguments: self is the mutable machine. Returns: samples accumulated since the previous drain.
    pub fn drain_audio_samples(&mut self) -> Vec<f32> {
        self.sound.drain()
    }

    /// Capture 40 by 25 screen codes as stable, human-readable ASCII text.
    /// Arguments: self is the machine. Returns: 25 newline-separated 40-character rows.
    pub fn capture_text_screen(&self) -> String {
        let base = ((self.ted.registers[0x14] & 0xf8) as usize) << 8;
        let full_glyph_set = self.ted.registers[0x07] & 0x80 != 0;
        let mut out = String::with_capacity(1024);
        for row in 0..25 {
            if row != 0 {
                out.push('\n');
            }
            for col in 0..40 {
                let code = self.ram[(base + 0x400 + row * 40 + col) & self.ram_mask as usize];
                out.push(screen_code_to_char(code, full_glyph_set));
            }
        }
        out
    }

    /// Capture 320 by 200 visible pixels as hexadecimal TED hue digits.
    /// A zero hue becomes a space, giving readable diffs for graphics tests.
    /// Arguments: self is the machine. Returns: 200 newline-separated 320-character rows.
    pub fn capture_graphics_screen(&self) -> String {
        let mut out = String::with_capacity(64200);
        for y in 0..200 {
            if y != 0 {
                out.push('\n');
            }
            for x in 0..320 {
                let color = self.pixels[(y + video::VISIBLE_Y) * WIDTH + x + video::VISIBLE_X] & 15;
                out.push(if color == 0 {
                    ' '
                } else {
                    char::from_digit(color as u32, 16).unwrap()
                });
            }
        }
        out
    }

    /// Return the 320 by 200 visible region as color indices.
    /// Arguments: self is the machine. Returns: 320 by 200 TED color indices.
    pub fn visible_pixels(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64000);
        for y in video::VISIBLE_Y..video::VISIBLE_Y + 200 {
            out.extend_from_slice(
                &self.pixels[y * WIDTH + video::VISIBLE_X..y * WIDTH + video::VISIBLE_X + 320],
            );
        }
        out
    }

    /// Return the active picture with an even 8-pixel TED border on every
    /// side. This is the buffer used by the `--small-border` desktop mode.
    /// Arguments: self is the machine. Returns: 336 by 216 TED color indices.
    pub fn compact_pixels(&self) -> Vec<u8> {
        self.pixels_with_border(COMPACT_BORDER)
    }

    /// Return the 320 by 200 picture with an even border on all four sides.
    /// The full PAL buffer permits at most 56 pixels on each side.
    /// Arguments: border is pixels per side and must fit the PAL buffer. Returns: cropped TED color indices.
    pub fn pixels_with_border(&self, border: usize) -> Vec<u8> {
        assert!(border <= video::VISIBLE_X && border <= video::VISIBLE_Y);
        let width = 320 + 2 * border;
        let height = 200 + 2 * border;
        let left = video::VISIBLE_X - border;
        let top = video::VISIBLE_Y - border;
        let mut out = Vec::with_capacity(width * height);
        for y in top..top + height {
            out.extend_from_slice(&self.pixels[y * WIDTH + left..y * WIDTH + left + width]);
        }
        out
    }

    /// Convert a TED color number into 24-bit RGB for GUI display.
    /// Arguments: color is the TED palette index. Returns: packed 24-bit RGB.
    pub fn rgb(color: u8) -> u32 {
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

    /// Advance TED time at the CPU's fast border and slow display rates.
    /// Arguments: cycles is elapsed CPU cycles. Returns: nothing.
    fn advance_hardware(&mut self, cycles: u8) {
        let mut elapsed_ticks = 0u8;
        for _ in 0..cycles {
            // TED normally lets the CPU run at double clock speed outside
            // the picture. The short slow region in a border line accounts
            // for its fixed bus use; the longer one models display fetches.
            let visible = self.ted.registers[6] & 0x10 != 0 && (3..203).contains(&self.ted.raster);
            let slow = if visible {
                (8..106).contains(&self.ted.line_cycles)
            } else {
                (52..62).contains(&self.ted.line_cycles)
            };
            let ticks = if slow { 2 } else { 1 };
            elapsed_ticks += ticks;
            self.ted.line_cycles += u32::from(ticks);
            while self.ted.line_cycles >= TICKS_PER_LINE {
                self.ted.line_cycles -= TICKS_PER_LINE;
                self.render_line(self.ted.raster, self.ted.display_line);
                self.ted.raster = (self.ted.raster + 1) % LINES_PER_FRAME;
                self.ted.display_line += 1;
                // Reprogramming the raster counter can shorten a frame.
                // Keep output rows sequential and start a fresh output frame
                // when the counter wraps, as the old emulator did.
                if self.ted.raster == 0 || self.ted.display_line == LINES_PER_FRAME {
                    self.ted.display_line = 0;
                    self.ted.frames += 1;
                }
                if self.ted.raster == self.ted.irq_line {
                    self.ted.irq |= 2;
                }
            }
        }
        self.ted.ticks += u64::from(elapsed_ticks);
        self.sound.advance(elapsed_ticks, &self.ted.registers);
        let timer_ticks = u16::from(self.ted.timer_tick_remainder) + u16::from(elapsed_ticks);
        self.ted.timer_tick_remainder = (timer_ticks & 1) as u8;
        for i in 0..3 {
            let reload = self.ted.reload[i];
            if reload == 0 {
                continue;
            }
            let mut left = timer_ticks / 2;
            while left > 0 {
                let timer = if self.ted.timer[i] == 0 {
                    reload
                } else {
                    self.ted.timer[i]
                };
                let elapsed = left.min(timer);
                self.ted.timer[i] = timer - elapsed;
                left -= elapsed;
                if self.ted.timer[i] == 0 {
                    self.ted.irq |= [8, 16, 64][i];
                }
            }
        }
        if self.ted.irq & self.ted.irq_enable != 0 {
            self.ted.irq |= 0x80;
            if self.cpu.p & 4 == 0 {
                self.irq_cpu();
            }
        }
    }

    /// Latch keyboard rows selected at FD30 and virtual joystick ports
    /// selected by the value written to FF08. This lets host keys substitute
    /// for either joystick even when a game isolates keyboard and joystick
    /// reads using separate selectors.
    /// Arguments: joystick_select has active-low port selection bits. Returns: nothing; FF08 latch changes.
    fn scan_keyboard(&mut self, joystick_select: u8) {
        let mut keys = 0;
        for row in 0..8 {
            if self.key_port & (1 << row) == 0 {
                keys |= self.key_matrix[row];
            }
        }
        // The port signals share keyboard-matrix bit positions. Joy 1 uses
        // 5/R/D/6/T; Joy 2 uses 3/W/A/4/Shift. Bit 2 means left and bit 3
        // means right, regardless of the old README's swapped directions.
        if joystick_select & 0x04 == 0 {
            let row = self.key_matrix[2];
            keys |= row & 0x0f; // up, down, left, right
            keys |= row & 0x40; // fire
        }
        if joystick_select & 0x02 == 0 {
            let row = self.key_matrix[1];
            keys |= row & 0x8f; // four directions and Shift/fire
        }
        self.ted.key_latch = !keys;
    }

    /// Read a memory-mapped device register.
    /// Arguments: address is a mapped CPU address. Returns: register value.
    fn read_io(&mut self, address: u16) -> u8 {
        if address >= 0xff00 {
            return self.read_ted((address & 63) as u8);
        }
        if address >= 0xfe00 {
            return match address & 255 {
                0xf0 => self.iec[0],
                0xf1 => self.iec[1],
                0xf2 => self.iec[2],
                _ => 0,
            };
        }
        match address & 0xf0 {
            0x00 => 0xac,
            0x10 => self.user_port,
            0x30 => self.key_port,
            _ => 0x66,
        }
    }

    /// Write a memory-mapped device register.
    /// Arguments: address is a mapped CPU address; value is the written byte. Returns: nothing.
    fn write_io(&mut self, address: u16, value: u8) {
        if address >= 0xff00 {
            self.write_ted((address & 63) as u8, value);
            return;
        }
        if address >= 0xfe00 {
            self.write_iec((address & 255) as u8, value);
            return;
        }
        match address & 0xf0 {
            0x10 => self.user_port = value,
            0x30 => self.key_port = value,
            0xd0 => {
                self.low_bank = (address & 3) as usize;
                self.high_bank = ((address >> 2) & 3) as usize;
            }
            _ => {}
        }
    }

    /// Read an implemented TED register or its write-back value.
    /// Arguments: reg is the TED register index. Returns: register value.
    fn read_ted(&self, reg: u8) -> u8 {
        let r = reg as usize;
        match reg {
            0x00..=0x05 => (self.ted.timer[r / 2] >> ((r % 2) * 8)) as u8,
            0x08 => self.ted.key_latch,
            0x09 => self.ted.irq,
            0x0a => self.ted.irq_enable | (self.ted.irq_line >> 8) as u8,
            0x0b => self.ted.irq_line as u8,
            0x1c => (self.ted.raster >> 8) as u8,
            0x1d => self.ted.raster as u8,
            0x1e => (self.ted.line_cycles / 2) as u8,
            _ => self.ted.registers[r],
        }
    }

    /// Apply a TED write immediately. The next completed raster line sees
    /// the new mode, supporting BASIC's bitmap/text split screen.
    /// Arguments: reg is the TED register index; value is the written byte. Returns: nothing.
    fn write_ted(&mut self, reg: u8, value: u8) {
        let r = reg as usize;
        self.ted.registers[r] = value;
        match reg {
            0x00..=0x05 => {
                let timer = &mut self.ted.reload[r / 2];
                if r & 1 == 0 {
                    *timer = (*timer & 0xff00) | value as u16;
                } else {
                    *timer = (*timer & 255) | ((value as u16) << 8);
                }
                self.ted.timer[r / 2] = *timer;
            }
            0x08 => self.scan_keyboard(value),
            0x09 => {
                self.ted.irq &= !(value & 0x7f);
                if self.ted.irq & 0x7f == 0 {
                    self.ted.irq &= 0x7f;
                }
            }
            0x0a => {
                self.ted.irq_enable = value & 0x5e;
                self.ted.irq_line = (self.ted.irq_line & 255) | (((value & 1) as u16) << 8);
            }
            0x0b => {
                self.ted.irq_line = (self.ted.irq_line & 0x100) | value as u16;
                // TED compares against the current line as well as at line
                // boundaries. TOM arms the next raster IRQ after entering
                // that line; a boundary-only check misses every second IRQ.
                if self.ted.raster == self.ted.irq_line {
                    self.ted.irq |= 2;
                }
            }
            0x1c => self.ted.raster = (self.ted.raster & 255) | (((value & 1) as u16) << 8),
            0x1d => self.ted.raster = (self.ted.raster & 0x100) | value as u16,
            0x3e => self.ram_enabled = false,
            0x3f => self.ram_enabled = true,
            _ => {}
        }
    }

    /// Implement the original virtual drive's IEC command protocol.
    /// Arguments: reg is the IEC register index; value is the command/data byte. Returns: nothing.
    fn write_iec(&mut self, reg: u8, value: u8) {
        if reg == 0xf2 {
            self.iec[2] = if value == 0 { 255 } else { 0 };
            return;
        }
        if reg != 0xf0 {
            return;
        }
        self.iec[2] = 0;
        match self.iec_command {
            0x82 => {
                if value & 0xf0 == 0xf0 {
                    self.iec_name.clear();
                    self.iec_naming = true;
                } else if value & 0xf0 == 0xe0 {
                    self.iec_naming = false;
                    if self.iec_writing {
                        if let Some(path) = self.iec_path() {
                            self.iec[1] =
                                if self.iec_dry_run || fs::write(path, &self.iec_output).is_ok() {
                                    0
                                } else {
                                    1
                                };
                        } else {
                            self.iec[1] = 1;
                        }
                    }
                    self.iec_writing = false;
                } else {
                    self.iec_naming = false;
                    if value & 15 == 0 {
                        self.iec_writing = false;
                        if self.verbose {
                            eprintln!("IEC load {:?}", self.iec_path());
                        }
                        match self.iec_path().map(fs::read) {
                            Some(Ok(data)) => {
                                self.iec_input = data;
                                self.iec_position = 0;
                                self.iec[1] = 0;
                            }
                            _ => self.iec[1] = 2,
                        }
                    } else {
                        self.iec_writing = true;
                        if self.verbose {
                            eprintln!("IEC save {:?}", self.iec_path());
                        }
                        self.iec_output.clear();
                        self.iec[1] = 0;
                    }
                }
                self.iec_command = 0;
            }
            0x83 => {
                if self.iec_naming {
                    self.iec_name.push(value);
                } else {
                    self.iec_output.push(value);
                }
                // The ROM sends a new 0x83 prefix before every data byte.
                self.iec_command = 0;
            }
            0x81 => self.iec_command = 0,
            _ => match value {
                0x81..=0x83 => self.iec_command = value,
                0x84 => {
                    self.iec[0] = *self.iec_input.get(self.iec_position).unwrap_or(&0);
                    self.iec_position += 1;
                    self.iec[1] = if self.iec_position >= self.iec_input.len() {
                        3
                    } else {
                        0
                    };
                }
                _ => {
                    self.iec[0] = value;
                    self.iec[1] = 0;
                }
            },
        }
    }

    /// Resolve an IEC filename inside the configured virtual drive directory.
    /// Arguments: self contains the IEC filename and drive directory. Returns: safe host path or None for an invalid name.
    fn iec_path(&self) -> Option<PathBuf> {
        let name = String::from_utf8_lossy(&self.iec_name);
        let name = name.rsplit(':').next().unwrap_or(&name);
        let relative = Path::new(name);
        if relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return None;
        }
        Some(self.drive_root.join(relative))
    }
}

/// Turn Commodore screen codes into ASCII for deterministic assertions.
/// Arguments: code is the screen byte; full_glyph_set selects 256 real glyphs
/// instead of 128 glyphs plus reverse video. Returns: one printable character.
fn screen_code_to_char(code: u8, full_glyph_set: bool) -> char {
    if full_glyph_set && (0x81..=0x9a).contains(&code) {
        return (b'a' + code - 0x81) as char;
    }
    match code & 0x7f {
        0 => '@',
        1..=26 => (b'A' + (code & 0x7f) - 1) as char,
        27 => '[',
        28 => '\\',
        29 => ']',
        30 => '^',
        31 => '_',
        32..=126 => (code & 0x7f) as char,
        _ => '.',
    }
}

/// Map printable BASIC source text to a C16 matrix key and Shift state.
/// Arguments: character is the desired typed symbol. Returns: key index and Shift state, or None if unsupported.
pub fn matrix_for_char(character: char) -> Option<(u8, bool)> {
    let key = match character.to_ascii_uppercase() {
        'A' => 10,
        'B' => 28,
        'C' => 20,
        'D' => 18,
        'E' => 14,
        'F' => 21,
        'G' => 26,
        'H' => 29,
        'I' => 33,
        'J' => 34,
        'K' => 37,
        'L' => 42,
        'M' => 36,
        'N' => 39,
        'O' => 38,
        'P' => 41,
        'Q' => 62,
        'R' => 17,
        'S' => 13,
        'T' => 22,
        'U' => 30,
        'V' => 31,
        'W' => 9,
        'X' => 23,
        'Y' => 25,
        'Z' => 12,
        '0' => 35,
        '1' => 56,
        '2' => 59,
        '3' => 8,
        '4' => 11,
        '$' => 11,
        '!' => 56,
        '"' => 59,
        '#' => 8,
        '%' => 16,
        '&' => 19,
        '\'' => 24,
        '(' => 27,
        ')' => 32,
        '@' => 35,
        '<' => 47,
        '>' => 44,
        '?' => 55,
        '_' => 46,
        '^' => 54,
        '[' => 45,
        ']' => 50,
        '\\' => 53,
        '`' => 49,
        '{' => 9,
        '}' => 14,
        '|' => 17,
        '~' => 22,
        '5' => 16,
        '6' => 19,
        '7' => 24,
        '8' => 27,
        '9' => 32,
        ' ' => 60,
        '+' => 54,
        '-' => 46,
        '*' => 49,
        '/' => 55,
        '=' => 53,
        ':' => 45,
        ';' => 50,
        ',' => 47,
        '.' => 44,
        '\u{8}' => 0,
        '\n' | '\r' => 1,
        _ => return None,
    };
    Some((
        key,
        matches!(
            character,
            '!' | '"'
                | '#'
                | '$'
                | '%'
                | '&'
                | '\''
                | '('
                | ')'
                | '@'
                | '<'
                | '>'
                | '?'
                | '_'
                | '^'
                | '['
                | ']'
                | '\\'
                | '`'
                | '{'
                | '}'
                | '|'
                | '~'
        ),
    ))
}
