// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! 6502-compatible instruction engine used by the C16's 7501/8501 CPU.

use crate::{memory_map::*, Machine};

const C: u8 = 1;
const Z: u8 = 2;
const I: u8 = 4;
const D: u8 = 8;
const B: u8 = 16;
const U: u8 = 32;
const V: u8 = 64;
const N: u8 = 128;

/// CPU registers and processor status.
#[derive(Clone, Copy)]
pub struct Cpu {
    /// Accumulator.
    pub a: u8,
    /// First index register.
    pub x: u8,
    /// Second index register.
    pub y: u8,
    /// Stack pointer within page one.
    pub sp: u8,
    /// Status register containing carry, zero, interrupt, decimal, overflow, negative.
    pub p: u8,
    /// Program counter.
    pub pc: u16,
}

impl Default for Cpu {
    /// Initialize power-on CPU register values.
    /// Arguments: none. Returns: a CPU with interrupts disabled and the stack initialized.
    fn default() -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xfd,
            p: U | I,
            pc: 0,
        }
    }
}

impl Machine {
    /// Fetch a byte and wrap the program counter at 64 KiB.
    /// Arguments: the mutable machine state. Returns: the fetched byte.
    fn fetch(&mut self) -> u8 {
        let value = self.read(self.cpu.pc);
        self.cpu.pc = self.cpu.pc.wrapping_add(1);
        value
    }

    /// Fetch a little-endian word.
    /// Arguments: the mutable machine state. Returns: the fetched 16-bit word.
    fn fetch_word(&mut self) -> u16 {
        let lo = self.fetch();
        u16::from_le_bytes([lo, self.fetch()])
    }

    /// Push a byte onto the hardware stack.
    /// Arguments: value is the byte to push. Returns: nothing.
    fn push(&mut self, value: u8) {
        self.ram[STACK_PAGE_START + self.cpu.sp as usize] = value;
        self.cpu.sp = self.cpu.sp.wrapping_sub(1);
    }

    /// Pull a byte from the hardware stack.
    /// Arguments: the mutable machine state. Returns: the popped byte.
    fn pull(&mut self) -> u8 {
        self.cpu.sp = self.cpu.sp.wrapping_add(1);
        self.ram[STACK_PAGE_START + self.cpu.sp as usize]
    }

    /// Update zero and negative flags from an 8-bit result.
    /// Arguments: value is the arithmetic or load result. Returns: nothing.
    fn zn(&mut self, value: u8) {
        self.cpu.p = (self.cpu.p & !(Z | N)) | if value == 0 { Z } else { 0 } | (value & N);
    }

    /// Set or clear a single processor flag.
    /// Arguments: flag is the bit mask; state selects set or clear. Returns: nothing.
    fn flag(&mut self, flag: u8, state: bool) {
        if state {
            self.cpu.p |= flag;
        } else {
            self.cpu.p &= !flag;
        }
    }

    /// Resolve a group-one addressing mode and return address and base cycles.
    /// Arguments: mode is the three-bit mode selector. Returns: effective address, base cycles, and page-crossing flag.
    fn group_one_address(&mut self, mode: u8) -> (u16, u8, bool) {
        match mode {
            0 => {
                // Indexed indirect (zero page,X), including pointer wrap.
                let ptr = self.fetch().wrapping_add(self.cpu.x);
                let lo = self.ram[ptr as usize];
                let hi = self.ram[ptr.wrapping_add(1) as usize];
                (u16::from_le_bytes([lo, hi]), 6, false)
            }
            1 => (self.fetch() as u16, 3, false), // Zero page.
            2 => {
                // Immediate operand at the current PC.
                let addr = self.cpu.pc;
                self.cpu.pc = self.cpu.pc.wrapping_add(1);
                (addr, 2, false)
            }
            3 => (self.fetch_word(), 4, false), // Absolute.
            4 => {
                // Indirect indexed (zero page),Y.
                let ptr = self.fetch();
                let lo = self.ram[ptr as usize];
                let hi = self.ram[ptr.wrapping_add(1) as usize];
                let base = u16::from_le_bytes([lo, hi]);
                let addr = base.wrapping_add(self.cpu.y as u16);
                (addr, 5, base & 0xff00 != addr & 0xff00)
            }
            5 => (self.fetch().wrapping_add(self.cpu.x) as u16, 4, false), // Zero page,X.
            6 => {
                // Absolute,Y with a possible page-crossing cycle.
                let base = self.fetch_word();
                let addr = base.wrapping_add(self.cpu.y as u16);
                (addr, 4, base & 0xff00 != addr & 0xff00)
            }
            _ => {
                // Absolute,X with a possible page-crossing cycle.
                let base = self.fetch_word();
                let addr = base.wrapping_add(self.cpu.x as u16);
                (addr, 4, base & 0xff00 != addr & 0xff00)
            }
        }
    }

    /// Add a byte with carry, including 6502 decimal mode.
    /// Arguments: operand is the byte added with carry. Returns: nothing; accumulator and flags change.
    fn adc(&mut self, operand: u8) {
        let a = self.cpu.a;
        let carry = u8::from(self.cpu.p & C != 0);
        let binary = a as u16 + operand as u16 + carry as u16;
        self.flag(V, (!(a ^ operand) & (a ^ binary as u8) & 0x80) != 0);
        if self.cpu.p & D != 0 {
            let mut lo = (a & 15) + (operand & 15) + carry;
            let mut hi = (a >> 4) + (operand >> 4);
            if lo > 9 {
                lo = lo.wrapping_add(6);
                hi += 1;
            }
            if hi > 9 {
                hi = hi.wrapping_add(6);
            }
            self.flag(C, hi > 15);
            self.cpu.a = (hi << 4) | (lo & 15);
            // NMOS 6502 computes N and Z from the unadjusted binary value.
            self.zn(binary as u8);
        } else {
            self.flag(C, binary > 255);
            self.cpu.a = binary as u8;
            self.zn(self.cpu.a);
        }
    }

    /// Subtract with borrow and 6502 decimal adjustment.
    /// Arguments: operand is the byte subtracted with borrow. Returns: nothing; accumulator and flags change.
    fn sbc(&mut self, operand: u8) {
        let a = self.cpu.a;
        let carry = u8::from(self.cpu.p & C != 0);
        let difference = a as i16 - operand as i16 - (1 - carry) as i16;
        let binary = difference as u8;
        self.flag(V, ((a ^ binary) & (a ^ operand) & 0x80) != 0);
        self.flag(C, difference >= 0);
        if self.cpu.p & D != 0 {
            let mut lo = (a & 15) as i16 - (operand & 15) as i16 - (1 - carry) as i16;
            let mut hi = (a >> 4) as i16 - (operand >> 4) as i16;
            if lo < 0 {
                lo -= 6;
                hi -= 1;
            }
            if hi < 0 {
                hi -= 6;
            }
            self.cpu.a = ((hi << 4) | (lo & 15)) as u8;
            self.zn(binary);
        } else {
            self.cpu.a = binary;
            self.zn(binary);
        }
    }

    /// Compare one register with an operand without changing the register.
    /// Arguments: register and operand are compared as unsigned bytes. Returns: nothing.
    fn compare(&mut self, register: u8, operand: u8) {
        self.flag(C, register >= operand);
        self.zn(register.wrapping_sub(operand));
    }

    /// Perform an arithmetic/logic shift or rotate on a byte.
    /// Arguments: operation selects the shift; value is its input byte. Returns: the shifted byte.
    fn shift(&mut self, operation: u8, value: u8) -> u8 {
        let carry_in = u8::from(self.cpu.p & C != 0);
        let result = match operation {
            0 => {
                // ASL: move each bit left; old bit 7 becomes carry.
                self.flag(C, value & 128 != 0);
                value << 1
            }
            1 => {
                // ROL: rotate left through carry.
                self.flag(C, value & 128 != 0);
                (value << 1) | carry_in
            }
            2 => {
                // LSR: move each bit right; old bit 0 becomes carry.
                self.flag(C, value & 1 != 0);
                value >> 1
            }
            _ => {
                // ROR: rotate right through carry.
                self.flag(C, value & 1 != 0);
                (value >> 1) | (carry_in << 7)
            }
        };
        self.zn(result);
        result
    }

    /// Take a mask-selected branch and report additional cycles.
    /// Arguments: take selects whether to branch. Returns: CPU cycles including any page-crossing penalty.
    fn branch(&mut self, take: bool) -> u8 {
        let relative = self.fetch() as i8;
        if !take {
            return 2;
        }
        let old = self.cpu.pc;
        self.cpu.pc = self.cpu.pc.wrapping_add_signed(relative as i16);
        3 + u8::from(old & 0xff00 != self.cpu.pc & 0xff00)
    }

    /// Enter the system ROM's IRQ handler if interrupts are enabled.
    /// Arguments: the mutable machine state. Returns: nothing.
    pub(crate) fn irq_cpu(&mut self) {
        if self.cpu.p & I != 0 {
            return;
        }
        self.push((self.cpu.pc >> 8) as u8);
        self.push(self.cpu.pc as u8);
        self.push((self.cpu.p & !B) | U);
        self.cpu.p |= I;
        let lo = self.roms[SYSTEM_ROM_IMAGE][SYSTEM_IRQ_VECTOR_OFFSET];
        let hi = self.roms[SYSTEM_ROM_IMAGE][SYSTEM_IRQ_VECTOR_OFFSET + 1];
        self.cpu.pc = u16::from_le_bytes([lo, hi]);
    }

    /// Execute one documented 6502 instruction and return elapsed cycles.
    /// Unsupported opcodes are errors with their exact address for diagnosis.
    /// Arguments: the mutable machine state. Returns: elapsed CPU cycles or an opcode error.
    pub(crate) fn step_cpu(&mut self) -> Result<u8, String> {
        let pc = self.cpu.pc;
        let opcode = self.fetch();
        // Group one decodes ORA, AND, EOR, ADC, STA, LDA, CMP, SBC.
        if opcode & 3 == 1 && opcode != 0x89 {
            let operation = opcode >> 5;
            let mode = (opcode >> 2) & 7;
            if operation == 4 && mode == 2 {
                return Err(format!("invalid STA immediate at ${pc:04X}"));
            }
            let (address, mut cycles, crossed) = self.group_one_address(mode);
            if operation == 4 {
                // STA: store the accumulator through the selected address mode.
                self.write(address, self.cpu.a);
                if matches!(mode, 4 | 6 | 7) {
                    cycles += 1;
                }
            } else {
                let value = self.read(address);
                match operation {
                    0 => {
                        // ORA: bitwise OR into the accumulator.
                        self.cpu.a |= value;
                        self.zn(self.cpu.a);
                    }
                    1 => {
                        // AND: bitwise AND into the accumulator.
                        self.cpu.a &= value;
                        self.zn(self.cpu.a);
                    }
                    2 => {
                        // EOR: bitwise exclusive OR into the accumulator.
                        self.cpu.a ^= value;
                        self.zn(self.cpu.a);
                    }
                    3 => self.adc(value), // ADC: add with carry.
                    5 => {
                        // LDA: load the accumulator.
                        self.cpu.a = value;
                        self.zn(value);
                    }
                    6 => self.compare(self.cpu.a, value), // CMP: compare accumulator.
                    7 => self.sbc(value),                 // SBC: subtract with borrow.
                    _ => unreachable!(),
                }
                if crossed {
                    cycles += 1;
                }
            }
            return Ok(cycles);
        }

        // Group two decodes ASL, ROL, LSR, ROR, STX, LDX, DEC, INC.
        if opcode & 3 == 2 && !matches!(opcode, 0x8a | 0x9a | 0xaa | 0xba | 0xca | 0xea) {
            let operation = opcode >> 5;
            let mode = (opcode >> 2) & 7;
            let (address, mut cycles, crossed) = match mode {
                0 if operation == 5 => {
                    // LDX immediate.
                    let address = self.cpu.pc;
                    self.cpu.pc = self.cpu.pc.wrapping_add(1);
                    (address, 2, false)
                }
                1 => (self.fetch() as u16, 3, false), // Zero page.
                2 if operation < 4 => (0, 2, false),  // Accumulator shift/rotate.
                3 => (self.fetch_word(), 4, false),   // Absolute.
                5 => {
                    // Zero-page indexed: Y for STX/LDX, X for other operations.
                    let index = if operation == 4 || operation == 5 {
                        self.cpu.y
                    } else {
                        self.cpu.x
                    };
                    (self.fetch().wrapping_add(index) as u16, 4, false)
                }
                7 => {
                    // Absolute indexed: Y for LDX, X for the others.
                    let base = self.fetch_word();
                    let index = if operation == 5 {
                        self.cpu.y
                    } else {
                        self.cpu.x
                    };
                    let address = base.wrapping_add(index as u16);
                    let cycles = if operation == 5 { 4 } else { 5 };
                    (address, cycles, address & 0xff00 != base & 0xff00)
                }
                _ => return Err(format!("illegal opcode ${opcode:02X} at ${pc:04X}")),
            };
            match operation {
                0..=3 => {
                    // ASL, ROL, LSR, ROR: shift or rotate accumulator/memory.
                    let value = if mode == 2 {
                        self.cpu.a
                    } else {
                        self.read(address)
                    };
                    let result = self.shift(operation, value);
                    if mode == 2 {
                        self.cpu.a = result;
                    } else {
                        self.write(address, result);
                        cycles += 2;
                    }
                }
                4 => {
                    // STX: store X.
                    if mode == 0 || mode == 2 || mode == 7 {
                        return Err(format!("illegal STX at ${pc:04X}"));
                    }
                    self.write(address, self.cpu.x);
                    if mode == 5 {
                        cycles = 4;
                    }
                }
                5 => {
                    // LDX: load X.
                    if mode == 2 {
                        return Err(format!("illegal LDX at ${pc:04X}"));
                    }
                    self.cpu.x = self.read(address);
                    self.zn(self.cpu.x);
                    if crossed {
                        cycles += 1;
                    }
                }
                6 | 7 => {
                    // DEC/INC: decrement or increment memory.
                    if mode == 0 || mode == 2 {
                        return Err(format!("illegal INC/DEC at ${pc:04X}"));
                    }
                    let value = self.read(address);
                    let result = if operation == 6 {
                        value.wrapping_sub(1)
                    } else {
                        value.wrapping_add(1)
                    };
                    self.write(address, result);
                    self.zn(result);
                    cycles += 2;
                }
                _ => unreachable!(),
            }
            return Ok(cycles);
        }

        let cycles = match opcode {
            0x00 => {
                // BRK: enter the interrupt handler after a padding byte.
                self.cpu.pc = self.cpu.pc.wrapping_add(1);
                self.push((self.cpu.pc >> 8) as u8);
                self.push(self.cpu.pc as u8);
                self.push(self.cpu.p | B | U);
                self.cpu.p |= I;
                let lo = self.read(IRQ_VECTOR);
                let hi = self.read(IRQ_VECTOR + 1);
                self.cpu.pc = u16::from_le_bytes([lo, hi]);
                7
            }
            0x08 => {
                // PHP: push processor status.
                self.push(self.cpu.p | B | U);
                3
            }
            0x10 => self.branch(self.cpu.p & N == 0), // BPL: branch if positive.
            0x18 => {
                // CLC: clear carry.
                self.flag(C, false);
                2
            }
            0x20 => {
                // JSR: call a subroutine.
                let address = self.fetch_word();
                let return_pc = self.cpu.pc.wrapping_sub(1);
                self.push((return_pc >> 8) as u8);
                self.push(return_pc as u8);
                self.cpu.pc = address;
                6
            }
            0x24 | 0x2c => {
                // BIT: test accumulator bits against zero-page/absolute memory.
                let address = if opcode == 0x24 {
                    self.fetch() as u16
                } else {
                    self.fetch_word()
                };
                let value = self.read(address);
                self.flag(Z, self.cpu.a & value == 0);
                self.cpu.p = (self.cpu.p & !(N | V)) | (value & (N | V));
                if opcode == 0x24 {
                    3
                } else {
                    4
                }
            }
            0x28 => {
                // PLP: pull processor status.
                self.cpu.p = (self.pull() | U) & !B;
                4
            }
            0x30 => self.branch(self.cpu.p & N != 0), // BMI: branch if negative.
            0x38 => {
                // SEC: set carry.
                self.flag(C, true);
                2
            }
            0x40 => {
                // RTI: restore status and PC after interrupt.
                self.cpu.p = (self.pull() | U) & !B;
                let lo = self.pull();
                let hi = self.pull();
                self.cpu.pc = u16::from_le_bytes([lo, hi]);
                6
            }
            0x48 => {
                // PHA: push accumulator.
                self.push(self.cpu.a);
                3
            }
            0x4c => {
                // JMP absolute: load the target address into PC.
                self.cpu.pc = self.fetch_word();
                3
            }
            0x50 => self.branch(self.cpu.p & V == 0), // BVC: branch if overflow clear.
            0x58 => {
                // CLI: clear interrupt disable.
                self.flag(I, false);
                2
            }
            0x60 => {
                // RTS: return from subroutine.
                let lo = self.pull();
                let hi = self.pull();
                self.cpu.pc = u16::from_le_bytes([lo, hi]).wrapping_add(1);
                6
            }
            0x68 => {
                // PLA: pull accumulator.
                self.cpu.a = self.pull();
                self.zn(self.cpu.a);
                4
            }
            0x6c => {
                // JMP indirect: read the target through a page-wrapped pointer.
                let ptr = self.fetch_word();
                let lo = self.read(ptr);
                // The 6502 JMP-indirect page-wrap behavior is intentional.
                let hi = self.read((ptr & 0xff00) | (ptr.wrapping_add(1) & 255));
                self.cpu.pc = u16::from_le_bytes([lo, hi]);
                5
            }
            0x70 => self.branch(self.cpu.p & V != 0), // BVS: branch if overflow set.
            0x78 => {
                // SEI: set interrupt disable.
                self.flag(I, true);
                2
            }
            0x84 | 0x94 | 0x8c => {
                // STY: store Y using zero-page, indexed, or absolute addressing.
                let address = match opcode {
                    0x84 => self.fetch() as u16,
                    0x94 => self.fetch().wrapping_add(self.cpu.x) as u16,
                    _ => self.fetch_word(),
                };
                self.write(address, self.cpu.y);
                match opcode {
                    0x84 => 3,
                    0x94 => 4,
                    _ => 4,
                }
            }
            0x88 => {
                // DEY: decrement Y.
                self.cpu.y = self.cpu.y.wrapping_sub(1);
                self.zn(self.cpu.y);
                2
            }
            0x8a => {
                // TXA: transfer X to accumulator.
                self.cpu.a = self.cpu.x;
                self.zn(self.cpu.a);
                2
            }
            0x90 => self.branch(self.cpu.p & C == 0), // BCC: branch if carry clear.
            0x98 => {
                // TYA: transfer Y to accumulator.
                self.cpu.a = self.cpu.y;
                self.zn(self.cpu.a);
                2
            }
            0x9a => {
                // TXS: transfer X to stack pointer.
                self.cpu.sp = self.cpu.x;
                2
            }
            0xa0 | 0xa4 | 0xb4 | 0xac | 0xbc => {
                // LDY: load Y from immediate or addressed memory.
                let (address, cycles) = match opcode {
                    0xa0 => {
                        let a = self.cpu.pc;
                        self.cpu.pc = a.wrapping_add(1);
                        (a, 2)
                    }
                    0xa4 => (self.fetch() as u16, 3),
                    0xb4 => (self.fetch().wrapping_add(self.cpu.x) as u16, 4),
                    0xac => (self.fetch_word(), 4),
                    _ => (self.fetch_word().wrapping_add(self.cpu.x as u16), 4),
                };
                self.cpu.y = self.read(address);
                self.zn(self.cpu.y);
                cycles
            }
            0xa8 => {
                // TAY: transfer accumulator to Y.
                self.cpu.y = self.cpu.a;
                self.zn(self.cpu.y);
                2
            }
            0xaa => {
                // TAX: transfer accumulator to X.
                self.cpu.x = self.cpu.a;
                self.zn(self.cpu.x);
                2
            }
            0xb0 => self.branch(self.cpu.p & C != 0), // BCS: branch if carry set.
            0xb8 => {
                // CLV: clear overflow.
                self.flag(V, false);
                2
            }
            0xba => {
                // TSX: transfer stack pointer to X.
                self.cpu.x = self.cpu.sp;
                self.zn(self.cpu.x);
                2
            }
            0xc0 | 0xc4 | 0xcc => {
                // CPY: compare Y with immediate, zero-page, or absolute data.
                let (value, cycles) = match opcode {
                    0xc0 => (self.fetch(), 2),
                    0xc4 => {
                        let a = self.fetch() as u16;
                        (self.read(a), 3)
                    }
                    _ => {
                        let a = self.fetch_word();
                        (self.read(a), 4)
                    }
                };
                self.compare(self.cpu.y, value);
                cycles
            }
            0xc8 => {
                // INY: increment Y.
                self.cpu.y = self.cpu.y.wrapping_add(1);
                self.zn(self.cpu.y);
                2
            }
            0xca => {
                // DEX: decrement X.
                self.cpu.x = self.cpu.x.wrapping_sub(1);
                self.zn(self.cpu.x);
                2
            }
            0xd0 => self.branch(self.cpu.p & Z == 0), // BNE: branch if nonzero.
            0xd8 => {
                // CLD: clear decimal mode.
                self.flag(D, false);
                2
            }
            0xe0 | 0xe4 | 0xec => {
                // CPX: compare X with immediate, zero-page, or absolute data.
                let (value, cycles) = match opcode {
                    0xe0 => (self.fetch(), 2),
                    0xe4 => {
                        let a = self.fetch() as u16;
                        (self.read(a), 3)
                    }
                    _ => {
                        let a = self.fetch_word();
                        (self.read(a), 4)
                    }
                };
                self.compare(self.cpu.x, value);
                cycles
            }
            0xe8 => {
                // INX: increment X.
                self.cpu.x = self.cpu.x.wrapping_add(1);
                self.zn(self.cpu.x);
                2
            }
            0xea => 2, // NOP: consume one two-cycle instruction.
            0xf0 => self.branch(self.cpu.p & Z != 0), // BEQ: branch if zero.
            0xf8 => {
                // SED: set decimal mode.
                self.flag(D, true);
                2
            }
            _ => return Err(format!("illegal opcode ${opcode:02X} at ${pc:04X}")),
        };
        Ok(cycles)
    }
}
