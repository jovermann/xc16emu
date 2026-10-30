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

/// CPU-visible memory map and device register numbers implemented by xc16emu.
///
/// `$0000-$7FFF` is RAM (mirrored on 16/32 KiB models). The 7501's `$0000`
/// and `$0001` port registers are currently treated as RAM by this core.
/// The free ROM uses
/// `$0800-$0BFF` for colors, `$0C00-$0FE7` for text, and `$0FE8` onward for
/// BASIC; those RAM assignments are software conventions, not fixed hardware.
/// ROM can overlay `$8000-$BFFF` and `$C000-$FBFF`. System ROM supplies
/// `$FC00-$FCFF` and `$FF40-$FFFF`. In the I/O gap, `$FD00-$FDFF` contains
/// FD device/port/bank groups, `$FE00-$FEFF` contains virtual IEC registers,
/// and `$FF00-$FF3F` contains TED registers. Unlisted FD groups return a
/// fixed value; unlisted FE registers read as zero; unlisted TED registers
/// retain their last written byte.
pub mod memory_map {
    /// Full 16-bit CPU address space.
    pub const ADDRESS_SPACE_BYTES: usize = 0x10000;
    /// 7501 CPU direction and data ports; currently backed by ordinary RAM.
    pub const CPU_IO_DIRECTION: u16 = 0x0000;
    pub const CPU_IO_DATA: u16 = 0x0001;
    /// CPU stack page begins at `$0100`.
    pub const STACK_PAGE_START: usize = 0x0100;
    /// Free ROM's default color matrix at `$0800`.
    pub const DEFAULT_COLOR_RAM: usize = 0x0800;
    /// Free ROM's default screen matrix at `$0C00`.
    pub const DEFAULT_SCREEN_RAM: usize = 0x0c00;
    /// First byte after the 40×25 screen matrix, used by free BASIC.
    pub const FREE_BASIC_RAM_START: usize = 0x0fe8;
    /// A TED matrix places screen codes `$0400` bytes after color bytes.
    pub const SCREEN_CODE_OFFSET: usize = 0x0400;

    /// First address overlaid by the lower 16 KiB ROM bank.
    pub const LOWER_ROM_START: u16 = 0x8000;
    /// First address overlaid by the upper 16 KiB ROM bank.
    pub const UPPER_ROM_START: u16 = 0xc000;
    /// System ROM takes precedence over optional upper ROM banks here.
    pub const SYSTEM_ROM_START: u16 = 0xfc00;
    /// Hardware I/O begins here and takes precedence over RAM/ROM reads.
    pub const IO_START: u16 = 0xfd00;
    /// Virtual IEC register page begins here.
    pub const IEC_PAGE_START: u16 = 0xfe00;
    /// Low eight bits select a register within an I/O page.
    pub const IO_PAGE_OFFSET_MASK: u16 = 0x00ff;
    /// TED register page begins here.
    pub const TED_PAGE_START: u16 = 0xff00;
    /// I/O ends before the system ROM resumes at `$FF40`.
    pub const IO_END: u16 = 0xff40;
    /// One ROM image occupies 16 KiB.
    pub const ROM_BANK_BYTES: usize = 0x4000;
    /// Offset mask for a byte within a ROM image.
    pub const ROM_BANK_MASK: usize = ROM_BANK_BYTES - 1;
    /// Each optional ROM bank has lower and upper images.
    pub const ROM_IMAGES_PER_BANK: usize = 2;
    pub const UPPER_ROM_IMAGE_OFFSET: usize = 1;
    /// Four pairs of lower/upper ROM images are addressable.
    pub const ROM_IMAGE_COUNT: usize = 8;
    /// The fixed system ROM is image index one.
    pub const SYSTEM_ROM_IMAGE: usize = 1;
    /// Physical RAM address masks for 16, 32, and 64 KiB machines.
    pub const RAM_MASK_16K: u16 = 0x3fff;
    pub const RAM_MASK_32K: u16 = 0x7fff;
    pub const RAM_MASK_64K: u16 = 0xffff;

    /// CPU NMI, reset, and IRQ/BRK vector addresses in system ROM.
    pub const NMI_VECTOR: u16 = 0xfffa;
    pub const RESET_VECTOR: u16 = 0xfffc;
    pub const IRQ_VECTOR: u16 = 0xfffe;
    /// Reset vector offset within the 16 KiB system ROM image.
    pub const SYSTEM_RESET_VECTOR_OFFSET: usize = RESET_VECTOR as usize - UPPER_ROM_START as usize;
    /// IRQ vector offset in the fixed 16 KiB system ROM image.
    pub const SYSTEM_IRQ_VECTOR_OFFSET: usize = IRQ_VECTOR as usize - UPPER_ROM_START as usize;

    /// FD-page decode preserves 16-byte register groups.
    pub const FD_GROUP_MASK: u16 = 0xfff0;
    /// `$FD00-$FD0F`: legacy fixed device response.
    pub const FD_DEVICE_GROUP: u16 = 0xfd00;
    /// `$FD10-$FD1F`: user-port latch.
    pub const FD_USER_PORT_GROUP: u16 = 0xfd10;
    /// `$FD30-$FD3F`: keyboard row select latch.
    pub const FD_KEYBOARD_GROUP: u16 = 0xfd30;
    /// `$FDD0-$FDDF`: lower/upper ROM bank selection by address bits.
    pub const FD_ROM_BANK_GROUP: u16 = 0xfdd0;
    /// Fixed replies from the emulated FD device and unimplemented FD groups.
    pub const FD_DEVICE_RESPONSE: u8 = 0xac;
    pub const FD_UNMAPPED_RESPONSE: u8 = 0x66;
    /// Low two FDDx address bits select the lower ROM bank.
    pub const LOWER_BANK_SELECT_MASK: u16 = 0x0003;
    /// FDDx address bits 2-3 select the upper ROM bank.
    pub const UPPER_BANK_SELECT_SHIFT: u16 = 2;

    /// `$FEF0`: virtual IEC command/data register.
    pub const IEC_DATA_REGISTER: u8 = 0xf0;
    /// `$FEF1`: virtual IEC transfer status register.
    pub const IEC_STATUS_REGISTER: u8 = 0xf1;
    /// `$FEF2`: virtual IEC handshake register.
    pub const IEC_HANDSHAKE_REGISTER: u8 = 0xf2;
    /// Indices of the three virtual IEC register latches.
    pub const IEC_DATA_INDEX: usize = 0;
    pub const IEC_STATUS_INDEX: usize = 1;
    pub const IEC_HANDSHAKE_INDEX: usize = 2;
    pub const IEC_REGISTER_COUNT: usize = 3;
    /// Virtual IEC command prefixes and one-byte read request.
    pub const IEC_FRAMING_COMMAND: u8 = 0x81;
    pub const IEC_SECONDARY_COMMAND: u8 = 0x82;
    pub const IEC_SEND_BYTE_COMMAND: u8 = 0x83;
    pub const IEC_READ_BYTE_COMMAND: u8 = 0x84;
    /// Secondary-address high nibbles for naming and closing a file.
    pub const IEC_NAME_SECONDARY: u8 = 0xf0;
    pub const IEC_CLOSE_SECONDARY: u8 = 0xe0;
    pub const IEC_SECONDARY_MASK: u8 = 0xf0;
    /// Secondary-address low nibble selects read (zero) or write (nonzero).
    pub const IEC_CHANNEL_MASK: u8 = 0x0f;
    /// Virtual drive status bytes read through `$FEF1`.
    pub const IEC_STATUS_OK: u8 = 0;
    pub const IEC_STATUS_SAVE_ERROR: u8 = 1;
    pub const IEC_STATUS_LOAD_ERROR: u8 = 2;
    pub const IEC_STATUS_LAST_BYTE: u8 = 3;
    /// Writing zero to `$FEF2` raises the emulated handshake flag to `$FF`.
    pub const IEC_HANDSHAKE_ACK: u8 = 0xff;

    /// TED register offsets relative to `$FF00`.
    pub const TED_TIMER_FIRST: u8 = 0x00;
    pub const TED_TIMER_LAST: u8 = 0x05;
    pub const TED_CONTROL_1: u8 = 0x06;
    pub const TED_CONTROL_2: u8 = 0x07;
    pub const TED_KEYBOARD_LATCH: u8 = 0x08;
    pub const TED_IRQ_STATUS: u8 = 0x09;
    pub const TED_IRQ_CONTROL: u8 = 0x0a;
    pub const TED_IRQ_LINE: u8 = 0x0b;
    pub const TED_CURSOR_HIGH: u8 = 0x0c;
    pub const TED_CURSOR_LOW: u8 = 0x0d;
    pub const TED_TONE_1_LOW: u8 = 0x0e;
    pub const TED_TONE_2_LOW: u8 = 0x0f;
    pub const TED_TONE_2_HIGH: u8 = 0x10;
    pub const TED_SOUND_CONTROL: u8 = 0x11;
    pub const TED_BITMAP_AND_TONE_1_HIGH: u8 = 0x12;
    pub const TED_CHARACTER_BASE: u8 = 0x13;
    pub const TED_MATRIX_BASE: u8 = 0x14;
    pub const TED_BACKGROUND_COLOR: u8 = 0x15;
    pub const TED_EXTRA_COLOR_1: u8 = 0x16;
    pub const TED_EXTRA_COLOR_2: u8 = 0x17;
    pub const TED_EXTRA_COLOR_3: u8 = 0x18;
    pub const TED_BORDER_COLOR: u8 = 0x19;
    /// The frozen-image loader reapplies control through border registers.
    pub const TED_SNAPSHOT_END: u16 = TED_PAGE_START + TED_BORDER_COLOR as u16 + 1;
    pub const TED_RASTER_HIGH: u8 = 0x1c;
    pub const TED_RASTER_LOW: u8 = 0x1d;
    pub const TED_LINE_POSITION: u8 = 0x1e;
    pub const TED_SHOW_ROM: u8 = 0x3e;
    pub const TED_SHOW_RAM: u8 = 0x3f;
    /// TED has 64 memory-mapped registers at `$FF00-$FF3F`.
    pub const TED_REGISTER_MASK: u16 = 0x003f;
    pub const TED_REGISTER_COUNT: usize = 64;
    /// Selected TED control bits used by the renderer and CPU scheduler.
    pub const TED_SCREEN_ENABLE_BIT: u8 = 0x10;
    pub const TED_BITMAP_MODE_BIT: u8 = 0x20;
    pub const TED_EXTENDED_MODE_BIT: u8 = 0x40;
    pub const TED_FULL_GLYPH_SET_BIT: u8 = 0x80;
    pub const TED_ROM_PATTERN_BIT: u8 = 0x04;
    pub const TED_COLOR_VALUE_MASK: u8 = 0x7f;
    /// TED interrupt flag and raster compare masks.
    pub const TED_IRQ_PENDING_MASK: u8 = 0x7f;
    pub const TED_IRQ_MASTER_BIT: u8 = 0x80;
    pub const TED_IRQ_ENABLE_MASK: u8 = 0x5e;
    pub const TED_RASTER_HIGH_BIT: u16 = 0x0100;
    pub const TED_RASTER_LOW_MASK: u16 = 0x00ff;
    /// Eight rows with eight physical keys each form the keyboard matrix.
    pub const KEYBOARD_MATRIX_ROWS: usize = 8;
    pub const KEYBOARD_MATRIX_KEYS: u8 = 64;
    pub const KEYBOARD_IDLE_VALUE: u8 = 0xff;
    /// FF08 active-low selection bits for joystick substitutes.
    pub const JOYSTICK_1_SELECT_BIT: u8 = 0x04;
    pub const JOYSTICK_2_SELECT_BIT: u8 = 0x02;
    pub const JOYSTICK_1_MATRIX_ROW: usize = 2;
    pub const JOYSTICK_2_MATRIX_ROW: usize = 1;
    pub const JOYSTICK_DIRECTION_BITS: u8 = 0x0f;
    pub const JOYSTICK_1_FIRE_BIT: u8 = 0x40;
    pub const JOYSTICK_2_BITS: u8 = 0x8f;

    /// RAM entry and saved stack pointer of the legacy frozen-image format.
    pub const FROZEN_TRAMPOLINE: usize = 0x7200;
    pub const FROZEN_STACK_POINTER: usize = 0x7ef3;
}

use memory_map::*;

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
    pub registers: [u8; TED_REGISTER_COUNT],
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
            registers: [0; TED_REGISTER_COUNT],
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
            key_latch: KEYBOARD_IDLE_VALUE,
        }
    }
}

/// A complete machine with ROM, RAM, CPU, TED, keyboard, and a virtual drive.
pub struct Machine {
    /// Physical 64 KiB RAM; smaller models use `ram_mask` mirroring.
    pub ram: Box<[u8; ADDRESS_SPACE_BYTES]>,
    /// CPU registers.
    pub cpu: Cpu,
    /// Number of successfully executed CPU instructions, for the GUI MIPS meter.
    pub instructions_executed: u64,
    /// TED state.
    pub ted: Ted,
    /// TED audio oscillator and samples waiting for the desktop audio device.
    sound: sound::Sound,
    /// Four pairs of 16 KiB ROMs, with missing optional banks zero-filled.
    roms: [Vec<u8>; ROM_IMAGE_COUNT],
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
    pub key_matrix: [u8; KEYBOARD_MATRIX_ROWS],
    /// Last rendered full PAL frame as TED color indices.
    pub pixels: Box<[u8; WIDTH * HEIGHT]>,
    /// Mode observed while drawing each raster line.
    pub line_modes: [u8; HEIGHT],
    /// Video matrix base latched at the start of each eight-line character row.
    video_matrix_latch: usize,
    /// Virtual IEC registers: data at FEF0, status at FEF1, handshake at FEF2.
    iec: [u8; IEC_REGISTER_COUNT],
    /// Pending command prefix from a FEF0 write (81, 82, or 83).
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
        let mut roms: [Vec<u8>; ROM_IMAGE_COUNT] = std::array::from_fn(|_| vec![0; ROM_BANK_BYTES]);
        for (i, name) in names.iter().enumerate() {
            match fs::read(path.as_ref().join(name)) {
                Ok(data) if data.len() == ROM_BANK_BYTES => roms[i] = data,
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("{name} must be {ROM_BANK_BYTES} bytes"),
                    ))
                }
                Err(e) if i < 2 => return Err(e),
                Err(_) => {}
            }
        }
        let mut m = Self {
            ram: Box::new([0; ADDRESS_SPACE_BYTES]),
            cpu: Cpu::default(),
            instructions_executed: 0,
            ted: Ted::default(),
            sound: sound::Sound::default(),
            roms,
            low_bank: 0,
            high_bank: 0,
            ram_enabled: false,
            ram_mask: RAM_MASK_64K,
            user_port: 0,
            key_port: KEYBOARD_IDLE_VALUE,
            key_matrix: [0; KEYBOARD_MATRIX_ROWS],
            pixels: Box::new([0; WIDTH * HEIGHT]),
            line_modes: [0; HEIGHT],
            video_matrix_latch: 0,
            iec: [0; IEC_REGISTER_COUNT],
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
        self.ted.key_latch = KEYBOARD_IDLE_VALUE;
        self.cpu = Cpu::default();
        self.cpu.pc = u16::from_le_bytes([
            self.roms[SYSTEM_ROM_IMAGE][SYSTEM_RESET_VECTOR_OFFSET],
            self.roms[SYSTEM_ROM_IMAGE][SYSTEM_RESET_VECTOR_OFFSET + 1],
        ]);
        self.low_bank = 0;
        self.high_bank = 0;
        self.ram_enabled = false;
        self.key_matrix.fill(0);
        self.key_port = KEYBOARD_IDLE_VALUE;
        self.video_matrix_latch = 0;
    }

    /// Set a 16, 32, or 64 KiB model, clear physical RAM, and reset.
    /// Arguments: kib is 16, 32, or 64 KiB. Returns: success or an invalid-size error.
    pub fn set_ram_size(&mut self, kib: u16) -> Result<(), &'static str> {
        self.ram_mask = match kib {
            16 => RAM_MASK_16K,
            32 => RAM_MASK_32K,
            64 => RAM_MASK_64K,
            _ => return Err("RAM must be 16, 32, or 64 KiB"),
        };
        self.ram.fill(0);
        self.reset();
        Ok(())
    }

    /// Read the CPU's memory map. FD00-FF3F accesses hardware even when RAM
    /// is selected; lower addresses read RAM, and the upper addresses read
    /// either RAM or the selected ROM bank. FC00-FFFF outside I/O uses system
    /// ROM while ROM is enabled, keeping its vectors and entry points visible.
    /// Arguments: address is the 16-bit CPU address. Returns: the visible byte.
    pub fn read(&mut self, address: u16) -> u8 {
        if (IO_START..IO_END).contains(&address) {
            return self.read_io(address);
        }
        if address < LOWER_ROM_START || self.ram_enabled {
            return self.ram[(address & self.ram_mask) as usize];
        }
        if address < UPPER_ROM_START {
            return self.roms[self.low_bank * ROM_IMAGES_PER_BANK]
                [address as usize & ROM_BANK_MASK];
        }
        if address < SYSTEM_ROM_START {
            return self.roms[self.high_bank * ROM_IMAGES_PER_BANK + UPPER_ROM_IMAGE_OFFSET]
                [address as usize & ROM_BANK_MASK];
        }
        self.roms[SYSTEM_ROM_IMAGE][address as usize & ROM_BANK_MASK]
    }

    /// Write the CPU's memory map. FD00-FF3F reaches hardware; every other
    /// address writes physical RAM, including RAM hidden beneath a ROM bank.
    /// Arguments: address is the 16-bit CPU address and value is the byte.
    /// Returns: nothing; RAM or the addressed device changes immediately.
    pub fn write(&mut self, address: u16, value: u8) {
        if (IO_START..IO_END).contains(&address) {
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
        if start + data.len() - 2 > ADDRESS_SPACE_BYTES {
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
        if data.len() != ADDRESS_SPACE_BYTES {
            return Err("frozen image must be exactly 65536 bytes");
        }
        self.reset();
        self.ram[2..ROM_BANK_BYTES].copy_from_slice(&data[2..ROM_BANK_BYTES]);
        for address in TED_PAGE_START + TED_CONTROL_1 as u16..TED_SNAPSHOT_END {
            self.write(address, data[address as usize]);
        }
        self.write(FD_KEYBOARD_GROUP, 0);
        self.ram[FROZEN_TRAMPOLINE..FROZEN_TRAMPOLINE + 7]
            .copy_from_slice(&[0x68, 0xa8, 0x68, 0xaa, 0x68, 0x28, 0x60]);
        self.cpu.sp = data[FROZEN_STACK_POINTER];
        self.cpu.pc = FROZEN_TRAMPOLINE as u16;
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
        if index >= KEYBOARD_MATRIX_KEYS {
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
        let base = ((self.ted.registers[TED_MATRIX_BASE as usize] & 0xf8) as usize) << 8;
        let full_glyph_set =
            self.ted.registers[TED_CONTROL_2 as usize] & TED_FULL_GLYPH_SET_BIT != 0;
        let mut out = String::with_capacity(1024);
        for row in 0..25 {
            if row != 0 {
                out.push('\n');
            }
            for col in 0..40 {
                let code =
                    self.ram[(base + SCREEN_CODE_OFFSET + row * 40 + col) & self.ram_mask as usize];
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

    /// Write all 128 screen codes into the $0C00 matrix as a 16 by 8 grid.
    /// Arguments: self receives only screen-memory writes. Returns: nothing.
    /// The current TED character-set selection determines which 128 glyphs appear.
    pub fn write_glyph_table_to_screen(&mut self) {
        let screen = &mut self.ram[DEFAULT_SCREEN_RAM..FREE_BASIC_RAM_START];
        screen.fill(0x20);

        let title = b"ROM GLYPHS 00-7F";
        for (offset, &letter) in title.iter().enumerate() {
            screen[3 * 40 + 12 + offset] = if letter.is_ascii_uppercase() {
                letter - b'A' + 1
            } else {
                letter
            };
        }
        // Labels sit outside the touching 16 by 8 block of glyph cells.
        for column in 0..16 {
            let nibble = column as u8;
            screen[6 * 40 + 12 + column] = if nibble < 10 {
                b'0' + nibble
            } else {
                nibble - 9
            };
        }
        for row in 0..8 {
            let screen_row = 8 + row;
            screen[screen_row * 40 + 9] = b'0' + row as u8;
            screen[screen_row * 40 + 10] = b'0';
            for column in 0..16 {
                screen[screen_row * 40 + 12 + column] = (row * 16 + column) as u8;
            }
        }
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
            let visible = self.ted.registers[TED_CONTROL_1 as usize] & TED_SCREEN_ENABLE_BIT != 0
                && (3..203).contains(&self.ted.raster);
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
            self.ted.irq |= TED_IRQ_MASTER_BIT;
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
        for row in 0..KEYBOARD_MATRIX_ROWS {
            if self.key_port & (1 << row) == 0 {
                keys |= self.key_matrix[row];
            }
        }
        // The port signals share keyboard-matrix bit positions. Joy 1 uses
        // 5/R/D/6/T; Joy 2 uses 3/W/A/4/Shift. Bit 2 means left and bit 3
        // means right, regardless of the old README's swapped directions.
        if joystick_select & JOYSTICK_1_SELECT_BIT == 0 {
            let row = self.key_matrix[JOYSTICK_1_MATRIX_ROW];
            keys |= row & JOYSTICK_DIRECTION_BITS; // up, down, left, right
            keys |= row & JOYSTICK_1_FIRE_BIT; // fire
        }
        if joystick_select & JOYSTICK_2_SELECT_BIT == 0 {
            let row = self.key_matrix[JOYSTICK_2_MATRIX_ROW];
            keys |= row & JOYSTICK_2_BITS; // four directions and Shift/fire
        }
        self.ted.key_latch = !keys;
    }

    /// Dispatch a CPU read in FD00-FF3F to the addressed device. IEC lives
    /// at FEF0-FEF2 and TED at FF00-FF3F.
    /// Arguments: address is the full mapped CPU address. Returns: its device
    /// value, or the fixed legacy value for unimplemented FD/FE registers.
    fn read_io(&mut self, address: u16) -> u8 {
        if address >= TED_PAGE_START {
            return self.read_ted((address & TED_REGISTER_MASK) as u8);
        }
        if address >= IEC_PAGE_START {
            let reg = (address & IO_PAGE_OFFSET_MASK) as u8;
            return self.read_iec(reg);
        }
        match address & FD_GROUP_MASK {
            FD_DEVICE_GROUP => FD_DEVICE_RESPONSE,
            FD_USER_PORT_GROUP => self.user_port,
            FD_KEYBOARD_GROUP => self.key_port,
            _ => FD_UNMAPPED_RESPONSE,
        }
    }

    /// Dispatch a CPU write in FD00-FF3F to TED, virtual IEC, keyboard,
    /// user port, or ROM-bank selection. FDD0-FDDF selects the two ROM banks.
    /// Arguments: address is the full mapped CPU address and value is written.
    /// Returns: nothing; the selected device state changes immediately.
    fn write_io(&mut self, address: u16, value: u8) {
        if address >= TED_PAGE_START {
            self.write_ted((address & TED_REGISTER_MASK) as u8, value);
            return;
        }
        if address >= IEC_PAGE_START {
            let reg = (address & IO_PAGE_OFFSET_MASK) as u8;
            self.write_iec(reg, value);
            return;
        }
        match address & FD_GROUP_MASK {
            FD_USER_PORT_GROUP => self.user_port = value,
            FD_KEYBOARD_GROUP => self.key_port = value,
            FD_ROM_BANK_GROUP => {
                // FDD0-FDDF encodes independent 16 KiB lower/upper ROM banks.
                self.low_bank = (address & LOWER_BANK_SELECT_MASK) as usize;
                self.high_bank =
                    ((address >> UPPER_BANK_SELECT_SHIFT) & LOWER_BANK_SELECT_MASK) as usize;
            }
            _ => {}
        }
    }

    /// Read a TED register through FF00-FF3F. Timers, keyboard, interrupts,
    /// and the raster counter have live values; other addresses return their
    /// most recently written register byte.
    /// Arguments: reg is the six-bit TED register index. Returns: its byte.
    fn read_ted(&self, reg: u8) -> u8 {
        let r = reg as usize;
        match reg {
            // Each pair is the low/high byte of one of the three timers.
            TED_TIMER_FIRST..=TED_TIMER_LAST => (self.ted.timer[r / 2] >> ((r % 2) * 8)) as u8,
            TED_KEYBOARD_LATCH => self.ted.key_latch,
            TED_IRQ_STATUS => self.ted.irq,
            TED_IRQ_CONTROL => self.ted.irq_enable | (self.ted.irq_line >> 8) as u8,
            TED_IRQ_LINE => self.ted.irq_line as u8,
            TED_RASTER_HIGH => (self.ted.raster >> 8) as u8,
            TED_RASTER_LOW => self.ted.raster as u8,
            TED_LINE_POSITION => (self.ted.line_cycles / 2) as u8,
            _ => self.ted.registers[r],
        }
    }

    /// Apply a TED register write at FF00-FF3F. Mode and color writes affect
    /// subsequent raster lines, timers reload, keyboard scans latch results,
    /// and FF3E/FF3F switch ROM visibility for CPU memory reads.
    /// Arguments: reg is the six-bit TED register index and value is written.
    /// Returns: nothing; device state changes before the next CPU instruction.
    fn write_ted(&mut self, reg: u8, value: u8) {
        let r = reg as usize;
        self.ted.registers[r] = value;
        match reg {
            TED_TIMER_FIRST..=TED_TIMER_LAST => {
                // Each timer has low and high registers. Writing either half
                // changes its reload value and restarts its current count.
                let timer = &mut self.ted.reload[r / 2];
                if r & 1 == 0 {
                    *timer = (*timer & 0xff00) | value as u16;
                } else {
                    *timer = (*timer & TED_RASTER_LOW_MASK) | ((value as u16) << 8);
                }
                self.ted.timer[r / 2] = *timer;
            }
            TED_KEYBOARD_LATCH => self.scan_keyboard(value),
            TED_IRQ_STATUS => {
                // Writing one bits clears the corresponding pending IRQs.
                self.ted.irq &= !(value & TED_IRQ_PENDING_MASK);
                if self.ted.irq & TED_IRQ_PENDING_MASK == 0 {
                    self.ted.irq &= TED_IRQ_PENDING_MASK;
                }
            }
            TED_IRQ_CONTROL => {
                // IRQ enable bits share a register with raster line bit 8.
                self.ted.irq_enable = value & TED_IRQ_ENABLE_MASK;
                self.ted.irq_line =
                    (self.ted.irq_line & TED_RASTER_LOW_MASK) | (((value & 1) as u16) << 8);
            }
            TED_IRQ_LINE => {
                // Raster line bits 0..7 select the next IRQ compare line.
                self.ted.irq_line = (self.ted.irq_line & TED_RASTER_HIGH_BIT) | value as u16;
                // TED compares against the current line as well as at line
                // boundaries. TOM arms the next raster IRQ after entering
                // that line; a boundary-only check misses every second IRQ.
                if self.ted.raster == self.ted.irq_line {
                    self.ted.irq |= 2;
                }
            }
            TED_RASTER_HIGH => {
                self.ted.raster =
                    (self.ted.raster & TED_RASTER_LOW_MASK) | (((value & 1) as u16) << 8)
            }
            TED_RASTER_LOW => {
                self.ted.raster = (self.ted.raster & TED_RASTER_HIGH_BIT) | value as u16
            }
            TED_SHOW_ROM => self.ram_enabled = false,
            TED_SHOW_RAM => self.ram_enabled = true,
            _ => {}
        }
    }

    /// Read a virtual IEC register after a ROM command. FEF0 exposes the byte
    /// prepared by an 84 read request during DLOAD; FEF1 reports transfer or
    /// file status (0 success, 1 save error, 2 load error, 3 final read byte);
    /// FEF2 is the ROM's handshake flag. Other FE registers read as zero.
    /// Arguments: reg is the low byte of the FE-page address. Returns: its byte.
    fn read_iec(&self, reg: u8) -> u8 {
        match reg {
            IEC_DATA_REGISTER => self.iec[IEC_DATA_INDEX],
            IEC_STATUS_REGISTER => self.iec[IEC_STATUS_INDEX],
            IEC_HANDSHAKE_REGISTER => self.iec[IEC_HANDSHAKE_INDEX],
            _ => 0,
        }
    }

    /// Accept the ROM's virtual IEC command/data writes at FEF0 and FEF2.
    /// This is the original emulator's register-level shortcut for DLOAD and
    /// DSAVE, rather than a bit-timed serial IEC bus. Prefix 82 selects an
    /// OPEN/CLOSE phase, 83 sends one filename or save-data byte, 84 requests
    /// the next load byte, and 81 consumes a ROM framing byte.
    /// Arguments: reg is the low byte of the FE-page address; value is the ROM
    /// command or payload byte. Returns: nothing; IEC data/status may change.
    fn write_iec(&mut self, reg: u8, value: u8) {
        if reg == IEC_HANDSHAKE_REGISTER {
            // The ROM writes zero to acknowledge a transfer and reads FEF2
            // back as FF; a nonzero write clears that acknowledgement.
            self.iec[IEC_HANDSHAKE_INDEX] = if value == 0 { IEC_HANDSHAKE_ACK } else { 0 };
            return;
        }
        if reg != IEC_DATA_REGISTER {
            // Only FEF0 carries commands/data; other FE-page writes are idle.
            return;
        }
        // Every FEF0 write clears the handshake until the ROM acknowledges it.
        self.iec[IEC_HANDSHAKE_INDEX] = 0;
        match self.iec_command {
            IEC_SECONDARY_COMMAND => {
                if value & IEC_SECONDARY_MASK == IEC_NAME_SECONDARY {
                    // Secondary address F*: begin a new filename. Subsequent
                    // 83-prefixed bytes form the DLOAD/DSAVE path.
                    self.iec_name.clear();
                    self.iec_naming = true;
                } else if value & IEC_SECONDARY_MASK == IEC_CLOSE_SECONDARY {
                    // Secondary address E*: close the file. DSAVE writes its
                    // accumulated bytes here, so a failed host write appears
                    // as status 1; dry-run reports success without a write.
                    self.iec_naming = false;
                    if self.iec_writing {
                        if let Some(path) = self.iec_path() {
                            self.iec[IEC_STATUS_INDEX] =
                                if self.iec_dry_run || fs::write(path, &self.iec_output).is_ok() {
                                    IEC_STATUS_OK
                                } else {
                                    IEC_STATUS_SAVE_ERROR
                                };
                        } else {
                            self.iec[IEC_STATUS_INDEX] = IEC_STATUS_SAVE_ERROR;
                        }
                    }
                    self.iec_writing = false;
                } else {
                    // Any other secondary address opens the named file. Low
                    // nibble zero means DLOAD/read; nonzero means DSAVE/write.
                    self.iec_naming = false;
                    if value & IEC_CHANNEL_MASK == 0 {
                        self.iec_writing = false;
                        if self.verbose {
                            eprintln!("IEC load {:?}", self.iec_path());
                        }
                        match self.iec_path().map(fs::read) {
                            Some(Ok(data)) => {
                                // Keep bytes in memory for later 84 read
                                // requests; the ROM interprets their format.
                                self.iec_input = data;
                                self.iec_position = 0;
                                self.iec[IEC_STATUS_INDEX] = IEC_STATUS_OK;
                            }
                            _ => self.iec[IEC_STATUS_INDEX] = IEC_STATUS_LOAD_ERROR,
                        }
                    } else {
                        // A DSAVE starts a fresh output buffer. Nothing is
                        // written to the host until the E* close command.
                        self.iec_writing = true;
                        if self.verbose {
                            eprintln!("IEC save {:?}", self.iec_path());
                        }
                        self.iec_output.clear();
                        self.iec[IEC_STATUS_INDEX] = IEC_STATUS_OK;
                    }
                }
                self.iec_command = 0;
            }
            IEC_SEND_BYTE_COMMAND => {
                // Each 83 prefix contributes exactly one byte. During the
                // F* phase it extends the name; during DSAVE it extends data.
                if self.iec_naming {
                    self.iec_name.push(value);
                } else {
                    self.iec_output.push(value);
                }
                // The ROM sends a new 0x83 prefix before every data byte.
                self.iec_command = 0;
            }
            IEC_FRAMING_COMMAND => self.iec_command = 0,
            _ => match value {
                // Arm a prefix; the following FEF0 byte is its argument.
                IEC_FRAMING_COMMAND..=IEC_SEND_BYTE_COMMAND => self.iec_command = value,
                IEC_READ_BYTE_COMMAND => {
                    // DLOAD asks for one byte. FEF0 returns it, while FEF1
                    // becomes 3 on the last byte and 0 before that.
                    self.iec[IEC_DATA_INDEX] = *self.iec_input.get(self.iec_position).unwrap_or(&0);
                    self.iec_position += 1;
                    self.iec[IEC_STATUS_INDEX] = if self.iec_position >= self.iec_input.len() {
                        IEC_STATUS_LAST_BYTE
                    } else {
                        IEC_STATUS_OK
                    };
                }
                _ => {
                    // Other control bytes remain visible to the ROM as data;
                    // the virtual drive reports no transfer error for them.
                    self.iec[IEC_DATA_INDEX] = value;
                    self.iec[IEC_STATUS_INDEX] = IEC_STATUS_OK;
                }
            },
        }
    }

    /// Resolve the DLOAD/DSAVE filename in the configured host directory.
    /// The part after the last colon is the host-relative name; absolute or
    /// parent components are rejected so a ROM command cannot escape it.
    /// Arguments: the name accumulated through 83 writes is read from self.
    /// Returns: a confined host path, or None for an invalid name.
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
        28 => '£',
        29 => ']',
        30 => '↑',
        31 => '←',
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
        '^' => 54,
        '[' => 45,
        ']' => 50,
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
                | '^'
                | '['
                | ']'
        ),
    ))
}
