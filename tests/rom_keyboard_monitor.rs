use xc16emu::Machine;

fn roms() -> [(&'static str, &'static str); 2] {
    [
        ("original", concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms")),
        ("free", concat!(env!("CARGO_MANIFEST_DIR"), "/free_roms")),
    ]
}

fn boot(path: &str) -> Machine {
    let mut machine = Machine::from_rom_dir(path).unwrap();
    machine.run_frames(100).unwrap();
    machine
}

fn shifted(machine: &mut Machine, index: u8) {
    machine.set_key(15, true);
    machine.set_key(index, true);
    machine.run_frames(2).unwrap();
    machine.set_key(index, false);
    machine.set_key(15, false);
    machine.run_frames(2).unwrap();
}

#[test]
fn shifted_i_is_a_graphic_glyph_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        let cell =
            u16::from_be_bytes([machine.ted.registers[12], machine.ted.registers[13]]) as usize;
        shifted(&mut machine, 33);
        assert_eq!(machine.ram[0x0c00 + cell], 0x49, "{name}");
    }
}

#[test]
fn shifted_letter_glyphs_match_in_both_roms() {
    let indexes = [
        10, 28, 20, 18, 14, 21, 26, 29, 33, 34, 37, 42, 36, 39, 38, 41, 62, 17, 13, 22, 30, 31, 9,
        23, 25, 12,
    ];
    for (name, path) in roms() {
        let mut machine = boot(path);
        let first =
            u16::from_be_bytes([machine.ted.registers[12], machine.ted.registers[13]]) as usize;
        for index in indexes {
            shifted(&mut machine, index);
        }
        let actual = &machine.ram[0x0c00 + first..0x0c00 + first + 26];
        let expected: Vec<u8> = (0x41..=0x5a).collect();
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn quoted_shifted_graphics_remain_graphics_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("PRINT \"").unwrap();
        shifted(&mut machine, 33);
        machine.type_text("\"\n").unwrap();
        let graphics = machine.ram[0x0c00..0x0fe8]
            .iter()
            .filter(|&&code| code == 0x49)
            .count();
        assert!(graphics >= 2, "{name}: {graphics} graphic cells");
        assert!(
            !machine.capture_text_screen().contains("?SYNTAX ERROR"),
            "{name}"
        );
    }
}

#[test]
fn shifted_letter_abbreviations_work_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("10 PRINT 123\n").unwrap();
        machine.type_text("L").unwrap();
        shifted(&mut machine, 33); // L + Shift-I abbreviates LIST.
        machine.type_text("\n").unwrap();
        let screen = machine.capture_text_screen();
        assert!(
            screen
                .lines()
                .filter(|line| line.trim() == "10 PRINT 123")
                .count()
                >= 2,
            "{name}: {screen}"
        );
        machine.type_text("R").unwrap();
        shifted(&mut machine, 30); // R + Shift-U abbreviates RUN.
        machine.type_text("\n").unwrap();
        let screen = machine.capture_text_screen();
        assert!(
            screen.lines().any(|line| line.trim() == "123"),
            "{name}: {screen}"
        );
        machine.type_text("P").unwrap();
        shifted(&mut machine, 17); // PRINT# precedes PRINT in token order.
        machine.type_text(" 7+5\n").unwrap();
        assert!(
            machine
                .capture_text_screen()
                .contains("?FILE NOT OPEN ERROR"),
            "{name}"
        );
    }
}

#[test]
fn monitor_deposit_dump_and_exit_work_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("> 0400 2A 2B\n").unwrap();
        machine.type_text("M 0400 0407\n").unwrap();
        assert_eq!(&machine.ram[0x0400..0x0402], &[0x2a, 0x2b], "{name}");
        let screen = machine.capture_text_screen();
        assert!(
            screen.contains("PC") && screen.contains("2A 2B"),
            "{name}: {screen}"
        );
        machine.type_text("X\n").unwrap();
        assert!(machine.capture_text_screen().contains("READY."), "{name}");
    }
}

#[test]
fn monitor_assembler_and_disassembler_work_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("A 0400 LDA #$2A\n").unwrap();
        assert_eq!(&machine.ram[0x0400..0x0402], &[0xa9, 0x2a], "{name}");

        let mut machine = boot(path);
        machine.ram[0x0400..0x0406].copy_from_slice(&[0xa9, 0x2a, 0x8d, 0x00, 0x05, 0x60]);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("D 0400 0405\n").unwrap();
        let screen = machine.capture_text_screen();
        assert!(
            screen.contains("LDA") && screen.contains("STA") && screen.contains("RTS"),
            "{name}: {screen}"
        );
    }
}

#[test]
fn monitor_relative_branch_crosses_a_page_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("A 04FF BNE $04FE\n").unwrap();
        assert_eq!(&machine.ram[0x04ff..0x0501], &[0xd0, 0xfd], "{name}");

        let mut machine = boot(path);
        machine.ram[0x04ff..0x0501].copy_from_slice(&[0xd0, 0xfd]);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("D 04FF 04FF\n").unwrap();
        let screen = machine.capture_text_screen();
        assert!(
            screen.contains("BNE") && screen.contains("04FE"),
            "{name}: {screen}"
        );
    }
}

#[test]
fn monitor_hunt_and_transfer_work_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("> 0420 01 02 03 04\n").unwrap();
        machine.type_text("H 0420 0423 02\n").unwrap();
        assert!(machine.capture_text_screen().contains("0421"), "{name}");
        machine.type_text("T 0420 0423 0430\n").unwrap();
        assert_eq!(&machine.ram[0x0430..0x0434], &[1, 2, 3, 4], "{name}");
    }
}

#[test]
fn monitor_fill_and_compare_work_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("F 0440 0443 7E\n").unwrap();
        assert_eq!(&machine.ram[0x0440..0x0444], &[0x7e; 4], "{name}");
        machine.type_text("> 0450 7E 00 7E 7E\n").unwrap();
        machine.type_text("C 0440 0443 0450\n").unwrap();
        assert!(machine.capture_text_screen().contains("0441"), "{name}");
    }
}

#[test]
fn monitor_go_returns_on_brk_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("> 0400 00\n").unwrap();
        machine.type_text("G 0400\n").unwrap();
        machine.type_text("R\n").unwrap();
        let screen = machine.capture_text_screen();
        assert!(screen.matches("PC").count() >= 2, "{name}: {screen}");
    }
}

#[test]
fn monitor_register_edit_is_visible_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("; 1234 20 11 22 33 F0\n").unwrap();
        machine.type_text("R\n").unwrap();
        let screen = machine.capture_text_screen();
        assert!(screen.contains("1234 20 11 22 33 F0"), "{name}: {screen}");
    }
}

#[test]
fn monitor_go_uses_edited_registers_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.ram[0x0400..0x0404].copy_from_slice(&[0x8d, 0x00, 0x05, 0x00]); // STA $0500; BRK
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("; 0400 20 2A 00 00 F0\n").unwrap();
        machine.type_text("G\n").unwrap();
        assert_eq!(machine.ram[0x0500], 0x2a, "{name}");
    }
}

#[test]
fn monitor_go_uses_edited_stack_pointer_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.ram[0x0400..0x0405].copy_from_slice(&[0xba, 0x8e, 0x01, 0x05, 0x00]); // TSX; STX $0501; BRK
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("; 0400 20 00 00 00 F0\n").unwrap();
        machine.type_text("G\n").unwrap();
        assert_eq!(machine.ram[0x0501], 0xf0, "{name}");
    }
}

#[test]
fn monitor_can_view_ram_beneath_rom_in_both_roms() {
    for (name, path) in roms() {
        let mut machine = boot(path);
        machine.type_text("MONITOR\n").unwrap();
        machine.type_text("> 8000 42\n").unwrap();
        machine.type_text("> 07F8 80\n").unwrap();
        machine.type_text("M 8000 8007\n").unwrap();
        let screen = machine.capture_text_screen();
        assert!(
            screen.contains("8000") && screen.contains("42"),
            "{name}: {screen}"
        );
    }
}
