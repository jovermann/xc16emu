// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! CPU regression checks for arithmetic and indexed addressing.

use xc16emu::Machine;

/// Construct a machine executing a short RAM program at $0200.
/// Arguments: bytes is the program image and roms locates either ROM set.
/// Returns: machine ready to execute the bytes.
fn program(bytes: &[u8], roms: &str) -> Machine {
    let mut machine = Machine::from_rom_dir(roms).unwrap();
    machine.ram[0x200..0x200 + bytes.len()].copy_from_slice(bytes);
    machine.cpu.pc = 0x200;
    machine
}

#[test]
/// Check ADC sets overflow for two negative inputs.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn adc_detects_negative_plus_negative_overflow() {
    for roms in [
        concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/free_roms"),
    ] {
        let mut machine = program(&[0x18, 0xa9, 0x80, 0x69, 0x80], roms);
        for _ in 0..3 {
            machine.step().unwrap();
        }
        assert_eq!(machine.cpu.a, 0, "{roms}");
        assert_eq!(machine.cpu.p & 0x43, 0x43, "{roms}"); // carry, zero, overflow
    }
}

#[test]
/// Check indexed absolute addressing crosses a page correctly.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn absolute_indexing_carries_into_the_high_address_byte() {
    for roms in [
        concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/free_roms"),
    ] {
        let mut machine = program(&[0xa2, 0x01, 0xbd, 0xff, 0x12], roms);
        machine.ram[0x1300] = 0x42;
        assert_eq!(machine.step().unwrap(), 2, "{roms}"); // LDX #$01
        assert_eq!(machine.step().unwrap(), 5, "{roms}"); // LDA $12FF,X crosses a page
        assert_eq!(machine.cpu.a, 0x42, "{roms}");
    }
}
