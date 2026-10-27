// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! Headless checks for the independently assembled first-stage ROMs.

use xc16emu::Machine;

/// Boot the free ROMs through the actual CPU and TED. Arguments: none.
/// Returns: a machine at its direct-mode prompt.
fn booted() -> Machine {
    let roms = concat!(env!("CARGO_MANIFEST_DIR"), "/free_roms");
    let mut machine = Machine::from_rom_dir(roms).unwrap();
    machine.run_frames(5).unwrap();
    machine
}

/// Press and release one matrix key for two PAL frames in each state.
/// Arguments: machine receives the key; index is its C16 matrix position. Returns: nothing.
fn press_key(machine: &mut Machine, index: u8) {
    machine.set_key(index, true);
    machine.run_frames(2).unwrap();
    machine.set_key(index, false);
    machine.run_frames(2).unwrap();
}

#[test]
/// Check ROM sizes, the requested banner, TED colors, and visible font pixels.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn boots_with_independent_font_and_original_colors() {
    let mut machine = booted();
    let text = machine.capture_text_screen();
    assert!(text.contains("FREE BASIC V0.1 11776 BYTES FREE"), "{text}");
    assert!(text.contains("READY."), "{text}");
    assert_eq!(machine.read(0xff15), 0x71);
    assert_eq!(machine.read(0xff19), 0x6e);
    let pixels = machine.visible_pixels();
    // The second text row contains the banner; its raster must differ from
    // the deliberately blank first row, proving glyph data is actually used.
    assert_ne!(&pixels[0..320 * 8], &pixels[320 * 8..320 * 16]);
}

#[test]
/// Keep the reference zero distinct from capital O.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn zero_glyph_is_open_and_distinct_from_o() {
    let mut machine = booted();
    let glyph = |machine: &mut Machine, code: u16| -> Vec<u8> {
        (0..8)
            .map(|row| machine.read(0xd000 + code * 8 + row))
            .collect()
    };
    let zero = glyph(&mut machine, 48);
    let letter_o = glyph(&mut machine, 15);
    assert_ne!(zero, letter_o);
}

#[test]
/// Keep graphics, lowercase, and second-set capitals present in the full font.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn font_covers_both_reference_character_sets() {
    let mut machine = booted();
    assert_eq!(machine.read(0xff07), 0x08);
    for code in 0_u16..256 {
        if [0x20, 0x60, 0xa0, 0xe0].contains(&code) {
            continue; // Spaces in the corresponding reference character sets.
        }
        assert!(
            (0..8).any(|row| machine.read(0xd000 + code * 8 + row) != 0),
            "glyph ${code:02X} is blank"
        );
    }
    let first_capital: Vec<_> = (0..8).map(|row| machine.read(0xd000 + 8 + row)).collect();
    let lower_a: Vec<_> = (0..8)
        .map(|row| machine.read(0xd000 + 0x81 * 8 + row))
        .collect();
    assert_ne!(first_capital, lower_a);
}

#[test]
/// Native C16 text symbols use their original screen-code positions.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn c16_symbols_use_native_screen_codes() {
    let mut machine = booted();
    let cell = u16::from_le_bytes([machine.ram[0x28], machine.ram[0x29]]) as usize;
    machine.type_text("@[]^").unwrap();
    assert_eq!(
        &machine.ram[0x0c00 + cell..0x0c00 + cell + 4],
        &[0, 27, 29, 30]
    );
    assert!(xc16emu::matrix_for_char('~').is_none());
}

#[test]
/// Shift+C= changes between the two 128-character ROM sets once per press.
/// Arguments: none. Returns: nothing; assertions report a keyboard regression.
fn shift_commodore_toggles_character_set() {
    let mut machine = booted();
    assert_eq!(machine.read(0xff13), 0xd0);
    machine.set_key(15, true); // Shift
    machine.set_key(61, true); // Commodore
    machine.run_frames(5).unwrap();
    assert_eq!(machine.read(0xff13), 0xd4);
    machine.run_frames(5).unwrap();
    assert_eq!(machine.read(0xff13), 0xd4); // Holding must not toggle again.
    machine.set_key(61, false);
    machine.set_key(15, false);
    machine.run_frames(2).unwrap();
    machine.set_key(15, true);
    machine.set_key(61, true);
    machine.run_frames(5).unwrap();
    assert_eq!(machine.read(0xff13), 0xd0);
}

#[test]
/// Guard the reference C16 punctuation and graphics while letters are edited.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn non_alphanumeric_glyphs_keep_reference_shapes() {
    let mut machine = booted();
    let mut checksum = 0xcbf2_9ce4_8422_2325_u64;
    for code in 0_u16..256 {
        let is_letter_or_digit = (1..=26).contains(&code)
            || (0x81..=0x9a).contains(&code)
            || (0xc1..=0xda).contains(&code)
            || (48..=57).contains(&code)
            || (0xb0..=0xb9).contains(&code);
        if !is_letter_or_digit {
            for row in 0..8 {
                checksum ^= machine.read(0xd000 + code * 8 + row) as u64;
                checksum = checksum.wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    assert_eq!(checksum, 0xa948_0d52_feb3_7c91);
}

#[test]
/// Write 128 touching screen codes without changing anything outside $0C00.
/// Arguments: none. Returns: nothing; assertions guard the live machine state.
fn glyph_table_writes_only_the_screen_matrix() {
    let mut machine = booted();
    let before_ram = machine.ram.clone();
    let before_registers = machine.ted.registers;
    let before_pixels = machine.pixels.clone();
    let before_keys = machine.key_matrix;
    let before_pc = machine.cpu.pc;
    let before_ticks = machine.ted.ticks;
    machine.write_glyph_table_to_screen();
    for index in 0_u8..128 {
        let cell_x = 12 + index as usize % 16;
        let cell_y = 8 + index as usize / 16;
        assert_eq!(machine.ram[0x0c00 + cell_y * 40 + cell_x], index);
        if cell_x < 27 {
            assert_eq!(machine.ram[0x0c00 + cell_y * 40 + cell_x + 1], index + 1);
        }
    }
    assert_eq!(&machine.ram[..0x0c00], &before_ram[..0x0c00]);
    assert_eq!(&machine.ram[0x0fe8..], &before_ram[0x0fe8..]);
    assert_eq!(machine.ted.registers, before_registers);
    assert_eq!(machine.pixels, before_pixels);
    assert_eq!(machine.key_matrix, before_keys);
    assert_eq!(machine.cpu.pc, before_pc);
    assert_eq!(machine.ted.ticks, before_ticks);
}

#[test]
/// Text capture follows TED's lowercase-set versus reverse-video selection.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn text_capture_decodes_second_set_lowercase() {
    let mut machine = booted();
    machine.write(0xff07, 0x88);
    let row = machine.ram[0x2e] as usize;
    let col = machine.ram[0x27] as usize;
    machine.ram[0x0c00 + row * 40 + col] = 0x81;
    let text = machine.capture_text_screen();
    assert_eq!(text.lines().nth(row).unwrap().as_bytes()[col], b'a');
    machine.write(0xff07, 0x08);
    let text = machine.capture_text_screen();
    assert_eq!(text.lines().nth(row).unwrap().as_bytes()[col], b'A');
}

#[test]
/// Type the three documented first-stage examples via the keyboard matrix.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn runs_first_three_basic_examples() {
    let mut machine = booted();
    machine
        .type_text("PRINT 2+3*4                 : REM arithmetic precedence; prints 14\n")
        .unwrap();
    machine
        .type_text("FOR I=1 TO 3:PRINT I;:NEXT  : REM prints 1 2 3 without separate line endings\n")
        .unwrap();
    machine
        .type_text(
            "PRINT HEX$(255),DEC(\"00FF\") : REM four-digit hex and inverse numeric conversion\n",
        )
        .unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains(" 14 "), "{text}");
    assert!(text.contains(" 1  2  3 "), "{text}");
    assert!(text.contains("00FF"), "{text}");
    assert!(text.contains(" 255 "), "{text}");
    assert!(!text.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// Report an unsupported keyword and return to the prompt.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn unsupported_keyword_returns_to_prompt() {
    let mut machine = booted();
    machine.type_text("MONITOR\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("?SYNTAX ERROR"), "{text}");
    assert!(text.matches("READY.").count() >= 2, "{text}");
}

#[test]
/// Store sorted lines, replace and delete them, and run the edited result.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn numbered_program_can_be_listed_edited_and_run() {
    let mut machine = booted();
    machine.type_text("20 PRINT 2\n").unwrap();
    machine.type_text("10 PRINT 1\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("FREE BASIC V0.1 11776 BYTES FREE"), "{text}");

    machine.type_text("LIST\n").unwrap();
    let text = machine.capture_text_screen();
    let listing = text.split("LIST").last().unwrap();
    let first = listing.find("10 PRINT 1").expect("line 10 in LIST output");
    let second = listing.find("20 PRINT 2").expect("line 20 in LIST output");
    assert!(first < second, "{text}");

    machine.type_text("RUN 20\n").unwrap();
    let text = machine.capture_text_screen();
    let run_output = text.split("RUN 20").last().unwrap();
    assert!(run_output.contains(" 2 "), "{text}");
    assert!(!run_output.contains(" 1 "), "{text}");

    machine.type_text("10 PRINT 42\n").unwrap();
    machine.type_text("20\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("FREE BASIC V0.1 11776 BYTES FREE"), "{text}");
    machine.type_text("LIST 10\n").unwrap();
    let text = machine.capture_text_screen();
    let listing = text.split("LIST 10").last().unwrap();
    assert!(listing.contains("10 PRINT 42"), "{text}");
    assert!(!listing.contains("20 PRINT 2"), "{text}");

    machine.type_text("RUN\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains(" 42 "), "{text}");
    assert!(!text.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// Entering numbered source leaves the next row ready for more source text.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn numbered_lines_do_not_print_ready_again() {
    let mut machine = booted();
    machine.type_text("10 PRINT 1\n20 PRINT 2\n").unwrap();
    let text = machine.capture_text_screen();
    assert_eq!(text.matches("READY.").count(), 1, "{text}");
    let rows: Vec<_> = text.lines().map(str::trim_end).collect();
    assert!(
        rows.windows(2)
            .any(|pair| pair == ["10 PRINT 1", "20 PRINT 2"]),
        "{text}"
    );
    machine.type_text("LIST\n").unwrap();
    assert_eq!(machine.capture_text_screen().matches("READY.").count(), 2);
}

#[test]
/// RUN/STOP interrupts a tight numbered GOTO loop and returns to the prompt.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn run_stop_interrupts_program() {
    let mut machine = booted();
    machine.type_text("10 GOTO 10\nRUN\n").unwrap();
    machine.set_key(52, true);
    machine.run_frames(2).unwrap();
    machine.set_key(52, false);
    machine.run_frames(2).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("?BREAK"), "{text}");
    assert_eq!(machine.ram[0x52], 0); // RUN mode has ended.
    assert!(text.matches("READY.").count() >= 2, "{text}");
}

#[test]
/// Run a loop that crosses stored-line boundaries and use LIST/RUN ranges.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn numbered_loop_and_line_ranges_work() {
    let mut machine = booted();
    machine.type_text("10 FOR I=1 TO 3\n").unwrap();
    machine.type_text("20 PRINT I;\n").unwrap();
    machine.type_text("30 NEXT\n").unwrap();
    machine.type_text("LIST 20-30\n").unwrap();
    let text = machine.capture_text_screen();
    let listing = text.split("LIST 20-30").last().unwrap();
    assert!(listing.contains("20 PRINT I;"), "{text}");
    assert!(listing.contains("30 NEXT"), "{text}");
    assert!(!listing.contains("10 FOR"), "{text}");
    machine.type_text("RUN\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains(" 1  2  3 "), "{text}");
}

#[test]
/// GOTO changes the next RUN record and works from the direct prompt.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn goto_skips_lines_and_accepts_a_target_expression() {
    let mut machine = booted();
    machine.type_text("10 PRINT 1\n").unwrap();
    machine.type_text("20 GOTO 30+10\n").unwrap();
    machine.type_text("30 PRINT 99\n").unwrap();
    machine.type_text("40 PRINT 2\n").unwrap();
    machine.type_text("RUN\n").unwrap();
    let text = machine.capture_text_screen();
    let output = text.split("RUN").last().unwrap();
    assert!(output.contains(" 1 "), "{text}");
    assert!(output.contains(" 2 "), "{text}");
    assert!(!output.contains(" 99 "), "{text}");

    machine.type_text("GOTO 40\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(
        text.split("GOTO 40").last().unwrap().contains(" 2 "),
        "{text}"
    );
}

#[test]
/// A GOTO inside a FOR loop must leave NEXT's return point intact.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn goto_inside_numbered_loop() {
    let mut machine = booted();
    machine.type_text("10 FOR I=1 TO 3\n").unwrap();
    machine.type_text("20 GOTO 40\n").unwrap();
    machine.type_text("30 PRINT 99\n").unwrap();
    machine.type_text("40 PRINT I;:NEXT\n").unwrap();
    machine.type_text("RUN\n").unwrap();
    let text = machine.capture_text_screen();
    let output = text.split("RUN").last().unwrap();
    assert!(output.contains(" 1  2  3 "), "{text}");
    assert!(!output.contains(" 99 "), "{text}");
    assert!(!output.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// A missing GOTO destination gives a BASIC error and returns to READY.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn goto_reports_missing_line() {
    let mut machine = booted();
    machine.type_text("GOTO 999\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("?UNDEF'D STATEMENT"), "{text}");
    assert!(text.matches("READY.").count() >= 2, "{text}");
}

#[test]
/// Backspace edits the current input line before it enters the program store.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn backspace_edits_numbered_input() {
    let mut machine = booted();
    machine.type_text("10 PRINT 1\u{8}2\n").unwrap();
    machine.type_text("RUN\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("10 PRINT 2"), "{text}");
    assert!(text.contains(" 2 "), "{text}");
}

#[test]
/// Deleting a middle record keeps later packed records intact.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn deleting_middle_line_preserves_following_lines() {
    let mut machine = booted();
    machine.type_text("10 PRINT 1\n").unwrap();
    machine.type_text("20 PRINT 2\n").unwrap();
    machine.type_text("30 PRINT 3\n").unwrap();
    machine.type_text("20\n").unwrap();
    machine.type_text("LIST\n").unwrap();
    let text = machine.capture_text_screen();
    let listing = text.split("LIST").last().unwrap();
    assert!(listing.contains("10 PRINT 1"), "{text}");
    assert!(listing.contains("30 PRINT 3"), "{text}");
    assert!(!listing.contains("20 PRINT 2"), "{text}");
    assert!(text.contains("FREE BASIC V0.1 11776 BYTES FREE"), "{text}");
}

#[test]
/// Verify native C16 punctuation can be entered and listed.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn accepts_special_characters() {
    let mut machine = booted();
    let symbols = "!\"#$%&'()*+,-./:;<=>?@[]";
    machine.type_text(&format!("10 REM {symbols}\n")).unwrap();
    machine.type_text("LIST\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains(&format!("10 REM {symbols}")), "{text}");
}

#[test]
/// Keep the newest output visible by moving the screen up at row 25.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn screen_scrolls_instead_of_wrapping_to_top() {
    let mut machine = booted();
    machine.type_text("PRINT \"FIRST\"\n").unwrap();
    for _ in 0..12 {
        machine.type_text("PRINT \"LAST\"\n").unwrap();
    }
    let text = machine.capture_text_screen();
    assert!(!text.contains("FIRST"), "{text}");
    assert!(!text.contains("FREE BASIC"), "{text}");
    assert!(text.contains("LAST"), "{text}");
    assert!(text.lines().last().unwrap().trim().is_empty(), "{text}");
}

#[test]
/// Move within a line, insert text, and delete at the edit position.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn cursor_left_right_and_delete_edit_input() {
    let mut machine = booted();
    machine.type_text("10 PRINT 12").unwrap();
    press_key(&mut machine, 48); // Left: before the 2.
    machine.type_text("3").unwrap();
    press_key(&mut machine, 51); // Right: after the 2.
    press_key(&mut machine, 0); // DEL removes the preceding 2.
    machine.type_text("\nLIST\nRUN\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("10 PRINT 13"), "{text}");
    assert!(text.contains(" 13 "), "{text}");
}

#[test]
/// Move onto an earlier numbered screen line and replace it in place.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn cursor_up_moves_to_prior_screen_line_for_editing() {
    let mut machine = booted();
    machine.type_text("10 PRINT 1\n").unwrap();
    let prompt_row = machine.ram[0x2e];
    press_key(&mut machine, 43); // Up to the numbered source line.
    assert_eq!(machine.ram[0x2e], prompt_row - 1);
    press_key(&mut machine, 3); // Keep the number, clear its text.
    machine.type_text(" PRINT 2\n").unwrap();
    machine.type_text("RUN\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("10 PRINT 2"), "{text}");
    assert!(text.contains(" 2 "), "{text}");
    assert!(!text.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// DEL leaves a selected screen line's number so Return deletes its record.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn delete_key_clears_program_line_body() {
    let mut machine = booted();
    machine.type_text("10 PRINT 7\n").unwrap();
    press_key(&mut machine, 43); // Up to line 10.
    press_key(&mut machine, 3); // DEL leaves only the number.
    machine.type_text("\nLIST\n").unwrap();
    let text = machine.capture_text_screen();
    let listing = text.split("LIST").last().unwrap();
    assert!(!listing.contains("10 PRINT 7"), "{text}");
    assert!(text.contains("FREE BASIC V0.1 11776 BYTES FREE"), "{text}");
}

#[test]
/// Up and Down move by a 40-column screen row inside a wrapped input line.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn vertical_cursor_keys_move_inside_wrapped_input() {
    let mut machine = booted();
    let source = format!("10 REM {}", "A".repeat(42));
    machine.type_text(&source).unwrap();
    let end = source.len() as u8;
    assert_eq!(machine.ram[0x2d], end);
    press_key(&mut machine, 43); // Up, one visible row.
    assert_eq!(machine.ram[0x2d], end - 40);
    machine.type_text("*").unwrap();
    press_key(&mut machine, 40); // Down to the original position.
    assert_eq!(machine.ram[0x2d], end + 1);
    press_key(&mut machine, 40); // Down onto the next, empty screen row.
    assert_eq!(machine.ram[0x2a], 0);
    press_key(&mut machine, 43); // Back to the wrapped source line.
    assert_eq!(machine.ram[0x2d], end + 1);
    machine.type_text("\nLIST\n").unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("10 REM AA*"), "{text}");
}

#[test]
/// A LIST-style wrapped screen row links back to its numbered source line.
/// Arguments: none. Returns: nothing; assertions report a regression.
fn wrapped_numbered_line_can_be_edited_from_its_second_screen_row() {
    let mut machine = booted();
    let source = format!("10 REM {}", "A".repeat(60));
    machine.type_text(&format!("{source}\n")).unwrap();
    press_key(&mut machine, 43); // Continuation row of line 10.
    assert_eq!(machine.ram[0x2a], source.len() as u8);
    assert_eq!(machine.ram[0x2d], 40);
    let row = machine.ram[0x2e] as usize;
    assert_eq!(machine.ram[0x1140 + row], 1);
    machine.type_text("*\n").unwrap();
    let expected = format!("{}*{}", &source[..40], &source[40..]);
    let stored_len = machine.ram[0x1202] as usize;
    assert_eq!(
        &machine.ram[0x1203..0x1203 + stored_len],
        &expected.as_bytes()[3..]
    );
}
