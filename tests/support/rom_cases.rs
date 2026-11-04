// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
// Shared end-to-end BASIC and hardware checks. Included once for each ROM set.

use std::process::Command;
use xc16emu::{Machine, COMPACT_BORDER, COMPACT_HEIGHT, COMPACT_WIDTH, HEIGHT, WIDTH};

const BASIC_EXAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/basic");

/// Boot a fresh machine and let BASIC finish its banner.
/// Arguments: none. Returns: booted machine.
fn booted() -> Machine {
    let mut machine = Machine::from_rom_dir(ROM_DIR).unwrap();
    machine.run_frames(100).unwrap();
    machine
}

#[test]
/// Check ROM startup text and nonblank video.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn basic_rom_reaches_ready_screen() {
    let machine = booted();
    let text = machine.capture_text_screen();
    assert_eq!(text.lines().count(), 25);
    assert!(text.contains("BASIC V"), "{text}");
    assert!(text.contains("READY."), "{text}");
    let pixels = machine.visible_pixels();
    assert!(
        pixels.iter().any(|pixel| *pixel != pixels[0]),
        "visible image is blank"
    );
}

#[test]
/// Check the assembled cube demo draws and rotates across PAL frames.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn rotating_cube_demo_changes_its_wireframe() {
    let mut machine = booted();
    machine
        .load_prg(include_bytes!("../../examples/rotating_cube.prg"))
        .unwrap();
    machine.cpu.pc = 0x6000;
    machine.run_frames(20).unwrap();
    assert_eq!(machine.read(0x7001), 0x40, "row offset was not generated");
    assert_eq!(machine.read(0x7104), 0x05, "row carry was lost");
    assert_eq!(machine.read(0x7200), 0x80, "pixel masks were not generated");
    assert_eq!(machine.read(0x7207), 0x01, "last pixel mask is wrong");
    let first_vertices: Vec<_> = (0x7300..0x7308).map(|addr| machine.read(addr)).collect();
    let rear_width = first_vertices[1].abs_diff(first_vertices[0]);
    let near_width = first_vertices[5].abs_diff(first_vertices[4]);
    assert!(
        near_width > rear_width,
        "perspective did not enlarge the near face: {near_width} <= {rear_width}"
    );
    let first = machine.capture_graphics_screen();
    assert!(first.contains('1'), "cube did not appear");
    machine.run_frames(15).unwrap();
    let second = machine.capture_graphics_screen();
    assert!(second.contains('1'), "cube vanished");
    assert_ne!(first, second, "cube did not rotate");
    let second_vertices: Vec<_> = (0x7300..0x7308).map(|addr| machine.read(addr)).collect();
    assert_ne!(
        first_vertices, second_vertices,
        "cube corners did not rotate"
    );
    for addr in 0x7400..0x7408 {
        assert!(machine.read(addr) < 200, "cube corner left the display");
    }
    for _ in 0..360 {
        machine.run_frames(1).unwrap();
        for addr in 0x7300..0x7308 {
            let x = machine.read(addr);
            assert!((40..=250).contains(&x), "cube corner X wrapped: {x}");
        }
        for addr in 0x7400..0x7408 {
            let y = machine.read(addr);
            assert!(y < 200, "cube corner Y wrapped: {y}");
        }
    }
}

#[test]
/// Check the Mandelbrot demo renders distinct columns at a 1:1 aspect ratio.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn mandelbrot_demo_draws_black_inside_and_white_outside() {
    let mut machine = booted();
    let boot_border = machine.read(0xff19);
    assert_eq!(boot_border & 0x0f, 14, "BASIC boot border is not blue");
    machine
        .load_prg(include_bytes!("../../examples/mandelbrot.prg"))
        .unwrap();
    machine.cpu.pc = 0x4000;
    machine.run_frames(8_200).unwrap();
    assert_eq!(machine.read(0xff19), boot_border);
    let pixels = machine.capture_graphics_screen();
    let rows: Vec<_> = pixels.lines().collect();
    assert_eq!(rows.len(), 200);
    assert!(rows.iter().all(|row| row.len() == 320));
    assert_eq!(rows[0].as_bytes()[0], b'1');
    assert_eq!(rows[99].as_bytes()[160], b' ');
    assert_eq!(rows[99].as_bytes()[319], b'1');
    assert_eq!(rows[10].as_bytes()[238], b'1'); // escapes near iteration 15
    assert_ne!(rows[75].as_bytes()[133], rows[75].as_bytes()[134]);
    let black: Vec<_> = rows
        .iter()
        .enumerate()
        .flat_map(|(y, row)| {
            row.bytes()
                .enumerate()
                .filter(|&(_, pixel)| pixel == b' ')
                .map(move |(x, _)| (x, y))
        })
        .collect();
    let width = black.iter().map(|&(x, _)| x).max().unwrap()
        - black.iter().map(|&(x, _)| x).min().unwrap()
        + 1;
    let height = black.iter().map(|&(_, y)| y).max().unwrap()
        - black.iter().map(|&(_, y)| y).min().unwrap()
        + 1;
    assert!(width * 4 <= height * 5 && height * 4 <= width * 5);
    for y in 0..100 {
        assert_eq!(rows[y], rows[199 - y], "asymmetric row {y}");
    }
}

#[test]
/// Check host keys affect isolated joystick reads on each port.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn keyboard_equivalents_drive_both_joystick_ports() {
    let mut machine =
        Machine::from_rom_dir(ROM_DIR).unwrap();
    // Isolate joystick input from ordinary keyboard row scanning.
    machine.write(0xfd30, 0xff);
    for (port_select, keys, expected) in [
        (0xfb, [16, 17, 18, 19, 22], 0x4f), // 5 R D 6 T: port 1
        (0xfd, [8, 9, 10, 11, 15], 0x8f),   // 3 W A 4 Shift: port 2
    ] {
        for key in keys {
            machine.set_key(key, true);
        }
        machine.write(0xff08, port_select);
        assert_eq!(!machine.read(0xff08), expected);
        machine.write(0xff08, 0xff);
        assert_eq!(machine.read(0xff08), 0xff);
        for key in keys {
            machine.set_key(key, false);
        }
    }
}

#[test]
/// Check a PRG with a separate load address and entry point through the CLI.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn galaxy_prg_loads_after_boot_and_starts_at_4000() {
    if !SUPPORTS_GALAXY_PRG {
        // Pending: this PRG calls routines supplied by the original ROM.
        return;
    }
    let output = Command::new(env!("CARGO_BIN_EXE_xc16emu"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--roms", ROM_DIR])
        .args([
            "--prg",
            "old/galaxy_0200-4007_module_start_with_g4000",
            "--go",
            "4000",
            "--headless",
            "--frames",
            "100",
            "--text",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let screen = String::from_utf8(output.stdout).unwrap();
    assert!(screen.contains("1984"), "{screen}");
    assert!(!screen.contains("READY."), "{screen}");
}

#[test]
/// Check both PRG load times and automatic execution at the file's load address.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn prg_cli_load_and_run_options_draw_the_cube() {
    for (option, needs_go) in [
        ("--prg", true),
        ("--preload-prg", true),
        ("--run-prg", false),
        ("--run-preload-prg", false),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_xc16emu"));
        command.current_dir(env!("CARGO_MANIFEST_DIR")).args(["--roms", ROM_DIR]).args([
            option,
            "examples/rotating_cube.prg",
            "--headless",
            "--frames",
            "20",
            "--graphics",
        ]);
        if needs_go {
            command.args(["--go", "6000"]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{option}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let screen = String::from_utf8(output.stdout).unwrap();
        assert!(screen.contains('1'), "{option} did not draw the cube");
    }
}

#[test]
/// Check CLI help lists every option in aligned groups and ends with version and copyright.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn help_and_version_follow_the_command_line_layout() {
    let binary = env!("CARGO_BIN_EXE_xc16emu");
    let help_output = Command::new(binary).arg("--help").output().unwrap();
    assert!(help_output.status.success());
    let help = String::from_utf8(help_output.stdout).unwrap();
    assert!(help.starts_with("xc16emu: C16/C116/Plus/4 emulator for macOS.\n\nUsage:"));
    for option in [
        "  --roms DIR ",
        "  --ram 16|32|64 ",
        "  --prg FILE ",
        "  --run-prg FILE ",
        "  --preload-prg FILE ",
        "  --run-preload-prg FILE ",
        "  --go HEX ",
        "  --frozen-image FILE ",
        "  --drive DIR ",
        "  --dry-run ",
        "  -v --verbose ",
        "  --type TEXT ",
        "  --headless ",
        "  --frames N ",
        "  --text ",
        "  --graphics ",
        "  --zoom 1|2|4 ",
        "  --speed FACTOR ",
        "  --small-border ",
        "  -h --help ",
        "     --version ",
    ] {
        assert!(
            help.lines().any(|line| line.starts_with(option)),
            "missing {option}"
        );
    }
    let version_output = Command::new(binary).arg("--version").output().unwrap();
    assert!(version_output.status.success());
    let version = String::from_utf8(version_output.stdout).unwrap();
    assert_eq!(help.lines().last(), Some(version.trim_end()));
    assert!(version.contains(env!("CARGO_PKG_VERSION")));
    assert!(version.contains("Copyright (c) 2026 Johannes Overmann"));
}

#[test]
/// Check TOM keeps its raster split and character set stable.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn tom_raster_irq_keeps_the_character_set_stable_each_frame() {
    if !SUPPORTS_TOM_FROZEN_IMAGE {
        // Pending: the free ROM does not yet resume this original-ROM image.
        return;
    }
    let mut machine = Machine::from_rom_dir(ROM_DIR).unwrap();
    machine
        .load_frozen_image(include_bytes!("../../old/tom_working_frozen_image_64k"))
        .unwrap();
    // The game sets a second raster interrupt during the current line. If
    // that interrupt is lost, it returns to the ROM character set only on
    // alternate frames and both the picture and music run at half speed.
    for frame in 0..40 {
        machine.run_frames(1).unwrap();
        assert_eq!(machine.ted.registers[0x13], 0xd5);
        assert_eq!(machine.ted.irq_line, 33);
        if frame >= 16 {
            // TOM rewrites FF1D at the end of a raster line. Applying that
            // store at instruction start made this entire last picture line
            // alternate between its graphics and the black border.
            let pixels = machine.visible_pixels();
            let bottom_line = &pixels[181 * 320..182 * 320];
            assert!(
                bottom_line.iter().any(|&pixel| pixel != 0),
                "TOM's bottom picture line vanished on frame {}",
                machine.ted.frames
            );
        }
    }
    assert!(machine.capture_text_screen().contains("TOM 16"));
}

#[test]
/// Check all border settings surround every side of the picture.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn compact_window_has_border_on_all_four_sides() {
    let machine = booted();
    let border = machine.ted.registers[0x19] & 0x7f;
    assert_eq!(
        machine.compact_pixels().len(),
        COMPACT_WIDTH * COMPACT_HEIGHT
    );
    for border_pixels in [COMPACT_BORDER, 16, 32] {
        let pixels = machine.pixels_with_border(border_pixels);
        let width = 320 + 2 * border_pixels;
        let height = 200 + 2 * border_pixels;
        assert_eq!(pixels.len(), width * height);
        assert!(pixels[..width * border_pixels]
            .iter()
            .all(|&color| color == border));
        assert!(pixels[width * (height - border_pixels)..]
            .iter()
            .all(|&color| color == border));
        for row in pixels.chunks_exact(width) {
            assert!(row[..border_pixels].iter().all(|&color| color == border));
            assert!(row[width - border_pixels..]
                .iter()
                .all(|&color| color == border));
        }
        assert!(pixels.iter().any(|&color| color != border));
    }
}

#[test]
/// Check BASIC arithmetic and counted loops.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn arithmetic_and_for_loop_run_in_rom_basic() {
    let mut machine = booted();
    machine.type_text("PRINT 2+3*4\n").unwrap();
    machine
        .type_text("FOR I=1 TO 5:PRINT I*I:NEXT I\n")
        .unwrap();
    machine.run_frames(25).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains(" 14 "), "{text}");
    for square in [1, 4, 9, 16, 25] {
        assert!(text.contains(&format!(" {square} ")), "{text}");
    }
    assert_eq!(text.matches("READY.").count(), 3, "{text}");
}

#[test]
/// Check BASIC SOUND creates TED samples in memory.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn basic_sound_command_drives_ted_audio() {
    let mut machine = booted();
    machine.set_audio_sample_rate(48_000);
    machine.type_text("VOL 8\n").unwrap();
    machine.drain_audio_samples();
    machine.type_text("SOUND 1,770,50\n").unwrap();
    machine.run_frames(2).unwrap();
    let samples = machine.drain_audio_samples();
    assert!(samples.iter().any(|&sample| sample > 0.1));
    assert!(samples.iter().any(|&sample| sample < -0.1));
    machine.reset();
    assert!(machine.drain_audio_samples().is_empty());
}

#[test]
/// Check BASIC circle drawing and graphics/text split.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn circle_draws_pixels_and_basic_selects_split_mode() {
    let mut machine = booted();
    machine.type_text("GRAPHIC 2,1\n").unwrap();
    machine.run_frames(10).unwrap();
    machine.type_text("CIRCLE 1,160,100,50\n").unwrap();
    machine.run_frames(80).unwrap();
    let graphics = machine.capture_graphics_screen();
    assert_eq!(graphics.lines().count(), 200);
    assert!(graphics.chars().filter(|c| *c == ' ').count() > 200);
    assert_eq!(machine.visible_pixels().len(), 320 * 200);
    assert_eq!(machine.pixels.len(), WIDTH * HEIGHT);
    // BASIC 3.5 changes FF06 at a raster interrupt so the bottom lines
    // remain text while the upper screen uses bitmap graphics.
    assert!(
        machine.line_modes[70] & 0x20 != 0,
        "upper raster is not bitmap"
    );
    assert!(
        machine.line_modes[230] & 0x20 == 0,
        "lower raster is not text"
    );
}

#[test]
/// Check a BASIC sine plot changes graphics output.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn basic_sine_plot_changes_graphics_pixels() {
    let mut machine = booted();
    machine.type_text("GRAPHIC 2,1\n").unwrap();
    machine
        .type_text("FOR X=0 TO 319 STEP 4:DRAW 1,X,100+50*SIN(X/20):NEXT\n")
        .unwrap();
    machine.run_frames(100).unwrap();
    let graphics = machine.capture_graphics_screen();
    assert!(graphics.chars().filter(|c| *c == ' ').count() > 50);
}

#[test]
/// Check PAINT loads through IEC and finishes a mixed diagonal-glyph fill.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn supplied_paint_program_loads_from_virtual_drive() {
    let mut machine = booted();
    machine.set_drive_root(BASIC_EXAMPLE_DIR);
    machine.type_text("DLOAD\"PAINT.BAS\"\n").unwrap();
    machine.run_frames(50).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("LOADING"), "{text}");
    assert_eq!(text.matches("READY.").count(), 2, "{text}");
    machine.type_text("RUN\n").unwrap();
    machine.run_frames(8_000).unwrap();
    assert!(machine.line_modes[70] & 0x20 != 0);
    assert_ne!(machine.read(0xff06) & 0x20, 0, "PAINT left graphics mode");
    let filled = machine.capture_graphics_screen().matches(' ').count();
    assert!(
        (45_000..55_000).contains(&filled),
        "PAINT filled {filled} pixels; expected a diagonal-glyph pattern"
    );
}

#[test]
/// Load and list a standard tokenized disk program with either BASIC ROM.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn standard_disk_program_loads_and_lists() {
    let mut machine = booted();
    machine.set_drive_root(BASIC_EXAMPLE_DIR);
    machine.type_text("DLOAD\"PAINT.BAS\"\n").unwrap();
    machine.run_frames(50).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("LOADING"), "{text}");
    assert_eq!(text.matches("READY.").count(), 2, "{text}");
    machine.type_text("LIST\n").unwrap();
    machine.run_frames(20).unwrap();
    let text = machine.capture_text_screen();
    assert!(
        text.lines()
            .any(|line| line.trim_start().starts_with("10") && line.contains("GRAPHIC")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|line| line.trim_start().starts_with("70") && line.contains("PAINT")),
        "{text}"
    );
}

/// Load a supplied tokenized BASIC example through the emulated IEC drive.
/// Arguments: program filename. Returns: a machine ready to run the program.
fn loaded_basic_example(program: &str) -> Machine {
    let mut machine = booted();
    machine.set_drive_root(BASIC_EXAMPLE_DIR);
    machine
        .type_text(&format!("DLOAD\"{program}\"\n"))
        .unwrap();
    machine.run_frames(50).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("LOADING") && text.contains("READY."), "{text}");
    machine
}

#[test]
/// BUNT uses FOR, arithmetic, RND, and POKE to fill screen and color memory.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn supplied_bunt_program_writes_screen_and_colors() {
    let mut machine = loaded_basic_example("BUNT.BAS");
    let before: Vec<_> = (0x0c00..0x0d00).map(|address| machine.read(address)).collect();
    machine.type_text("RUN\n").unwrap();
    machine.run_frames(150).unwrap();
    let changed = (0x0c00..0x0d00)
        .filter(|address| machine.read(*address) != before[*address as usize - 0x0c00])
        .count();
    assert!(changed > 8, "BUNT did not update the screen: {changed} cells");
    let running_cursor = u16::from_be_bytes([machine.read(0xff0c), machine.read(0xff0d)]);
    assert_eq!(running_cursor, 0xffff, "BUNT left the cursor visible");
    machine.set_key(63, true); // RUN/STOP
    machine.run_frames(3).unwrap();
    machine.set_key(63, false);
    machine.run_frames(20).unwrap();
    let prompt_cursor = u16::from_be_bytes([machine.read(0xff0c), machine.read(0xff0d)]);
    assert!(prompt_cursor < 1_000, "cursor was not restored: ${prompt_cursor:04x}");
}

#[test]
/// PRINT during RUN must not reactivate TED's blinking text cursor.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn printing_program_keeps_cursor_hidden() {
    let mut machine = booted();
    machine.type_text("10 PRINT \"X\":GOTO 10\nRUN\n").unwrap();
    machine.run_frames(50).unwrap();
    let cursor = u16::from_be_bytes([machine.read(0xff0c), machine.read(0xff0d)]);
    assert_eq!(cursor, 0xffff);
}

#[test]
/// TRIANGLE uses CIRCLE's rotation and angular step to draw overlapping polygons.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn supplied_triangle_program_draws_rotated_polygons() {
    let mut machine = loaded_basic_example("TRIANGLE.BAS");
    machine.type_text("RUN\n").unwrap();
    machine.run_frames(1_000).unwrap();
    assert_ne!(machine.read(0xff06) & 0x20, 0);
    let drawn = machine
        .capture_graphics_screen()
        .chars()
        .filter(|pixel| *pixel == ' ')
        .count();
    assert!(drawn > 5_000, "TRIANGLE drew only {drawn} pixels");
}

#[test]
/// FUNC combines DEF FN, nested descending loops, and function plotting.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn supplied_func_program_starts_plotting() {
    let mut machine = loaded_basic_example("FUNC.BAS");
    machine.type_text("RUN\n").unwrap();
    machine.run_frames(1_000).unwrap();
    assert_ne!(machine.read(0xff06) & 0x20, 0);
    let drawn = machine
        .capture_graphics_screen()
        .chars()
        .filter(|pixel| *pixel == ' ')
        .count();
    assert!(drawn > 20, "FUNC drew only {drawn} pixels");
}

#[test]
/// Startup typing completes DLOAD before submitting RUN for the FUNC example.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn func_example_runs_from_command_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_xc16emu"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["--roms", ROM_DIR, "--drive", BASIC_EXAMPLE_DIR])
        .args([
            "--type",
            "DLOAD\"FUNC.BAS\"",
            "--type",
            "RUN",
            "--headless",
            "--frames",
            "1000",
            "--graphics",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let picture = String::from_utf8(output.stdout).unwrap();
    assert_eq!(picture.lines().count(), 200);
    assert!(picture.matches(' ').count() > 20, "FUNC did not plot");
}

#[test]
/// A FOR start expression must assign its result to the loop index each time.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn nested_for_start_expression_resets_inner_index() {
    let mut machine = booted();
    machine
        .type_text("10 D=95:FOR X=0 TO -2 STEP -1\n20 FOR Y=D TO -D STEP -5\n30 NEXT Y:PRINT X,Y:NEXT X\nRUN\n")
        .unwrap();
    machine.run_frames(100).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("-2        -100"), "{text}");
}

#[test]
/// Check line-accurate TED video-mode switching.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn changing_ted_mode_mid_frame_changes_only_later_lines() {
    let mut machine = booted();
    machine.write(0xff06, 0x3b);
    while machine.ted.raster < 100 {
        machine.step().unwrap();
    }
    machine.write(0xff06, 0x1b);
    machine.run_frames(1).unwrap();
    assert_ne!(machine.line_modes[80] & 0x20, 0);
    assert_eq!(machine.line_modes[180] & 0x20, 0);
}

#[test]
/// Program storage, LIST, and RUN must work through either BASIC ROM.
/// Arguments: none. Returns: nothing; assertions fail on user-visible output.
fn numbered_program_lists_and_runs() {
    let mut machine = booted();
    machine.type_text("10 PRINT 41\n20 PRINT 42\nLIST\nRUN\n").unwrap();
    machine.run_frames(10).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("10 PRINT 41"), "{text}");
    assert!(text.contains("20 PRINT 42"), "{text}");
    let output = text.split("RUN").last().unwrap();
    assert!(output.contains("41"), "{text}");
    assert!(output.contains("42"), "{text}");
}

#[test]
/// Nested GOSUB calls return to the following statement at each level.
/// Arguments: none. Returns: nothing; assertions fail on control-flow errors.
fn nested_gosub_returns_to_each_caller() {
    let mut machine = booted();
    machine
        .type_text("10 A=0:GOSUB 100:PRINT A\n20 GOTO 500\n100 A=A+1:GOSUB 200:RETURN\n200 A=A+10:RETURN\n500 REM DONE\nRUN\n")
        .unwrap();
    machine.run_frames(35).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.lines().any(|line| line.trim() == "11"), "{text}");
    assert!(!text.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// GOSUB entered at the prompt returns there after its numbered subroutine.
/// Arguments: none. Returns: nothing; assertions fail on direct-mode calls.
fn direct_gosub_returns_to_prompt() {
    let mut machine = booted();
    machine.type_text("100 PRINT 7:RETURN\nGOSUB 100\n").unwrap();
    machine.run_frames(20).unwrap();
    let text = machine.capture_text_screen();
    let output = text.split("GOSUB 100").last().unwrap();
    assert!(output.lines().any(|line| line.trim() == "7"), "{text}");
    assert!(output.contains("READY."), "{text}");
    assert!(!output.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// Nested FOR loops retain independent variables, bounds, and body positions.
/// Arguments: none. Returns: nothing; assertions fail on control-flow errors.
fn nested_for_loops_keep_each_level() {
    let mut machine = booted();
    machine
        .type_text("10 S=0\n20 FOR I=1 TO 3\n30 FOR J=1 TO 2\n40 S=S+I*10+J\n50 NEXT J\n60 NEXT I\n70 PRINT S\nRUN\n")
        .unwrap();
    machine.run_frames(35).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.lines().any(|line| line.trim() == "129"), "{text}");
    assert!(!text.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// IF chooses THEN and ELSE, including an implicit numbered-line branch.
/// Arguments: none. Returns: nothing; assertions fail on control-flow errors.
fn if_then_else_selects_one_branch() {
    let mut machine = booted();
    machine
        .type_text("10 A=0\n20 IF 2>1 THEN A=A+1:ELSE A=99\n30 IF 2<1 THEN A=99:ELSE A=A+10\n40 IF A=11 THEN 70 ELSE 50\n50 PRINT 999:GOTO 80\n70 PRINT A\n80 REM DONE\nRUN\n")
        .unwrap();
    machine.run_frames(35).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.lines().any(|line| line.trim() == "11"), "{text}");
    assert!(!text.lines().any(|line| line.trim() == "999"), "{text}");
    assert!(!text.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// FOR frames survive GOSUB, while loops inside the subroutine are popped.
/// Arguments: none. Returns: nothing; assertions fail on stack corruption.
fn for_loops_and_gosub_share_control_stack() {
    let mut machine = booted();
    machine
        .type_text("10 S=0:FOR I=1 TO 2:GOSUB 100:NEXT I\n20 PRINT S:GOTO 200\n100 FOR J=1 TO 2:S=S+I+J:NEXT J:RETURN\n200 REM DONE\nRUN\n")
        .unwrap();
    machine.run_frames(35).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.lines().any(|line| line.trim() == "12"), "{text}");
    assert!(!text.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// A comma-separated NEXT list advances inner and outer loops in order.
/// Arguments: none. Returns: nothing; assertions fail on loop-state errors.
fn next_accepts_multiple_nested_variables() {
    let mut machine = booted();
    machine
        .type_text("10 S=0\n20 FOR I=1 TO 2:FOR J=1 TO 2:S=S+1:NEXT J,I\n30 PRINT S\nRUN\n")
        .unwrap();
    machine.run_frames(30).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.lines().any(|line| line.trim() == "4"), "{text}");
    assert!(!text.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// IF accepts all three comparison signs and both compound comparisons.
/// Arguments: none. Returns: nothing; assertions fail on clause selection.
fn if_comparisons_choose_the_expected_clause() {
    let mut machine = booted();
    machine
        .type_text("10 IF 3>=3 THEN PRINT \"GE\"\n20 IF 3<=4 THEN PRINT \"LE\"\n30 IF 3<>4 THEN PRINT \"NE\"\n40 IF 0 THEN PRINT \"BAD\":ELSE PRINT \"ZERO\"\nRUN\n")
        .unwrap();
    machine.run_frames(25).unwrap();
    let text = machine.capture_text_screen();
    let output = text.split("RUN").last().unwrap();
    for expected in ["GE", "LE", "NE", "ZERO"] {
        assert!(output.contains(expected), "missing {expected}: {text}");
    }
    assert!(!output.contains("BAD"), "{text}");
    assert!(!output.contains("?SYNTAX ERROR"), "{text}");
}

#[test]
/// A false IF uses the first following ELSE, matching BASIC V3.5.
/// Arguments: none. Returns: nothing; assertions fail on clause matching.
fn nested_if_uses_the_first_else() {
    let mut machine = booted();
    machine
        .type_text("10 IF 0 THEN IF 1 THEN PRINT \"NO\":ELSE PRINT \"NO\":ELSE PRINT \"YES\"\nRUN\n")
        .unwrap();
    machine.run_frames(20).unwrap();
    let text = machine.capture_text_screen();
    let output = text.split("RUN").last().unwrap();
    assert!(output.lines().any(|line| line.trim() == "NO"), "{text}");
    assert!(!output.lines().any(|line| line.trim() == "YES"), "{text}");
}

#[test]
/// Common numeric and string values survive assignment and PRINT.
/// Arguments: none. Returns: nothing; assertions fail on BASIC behavior.
fn numeric_and_string_variables_print() {
    let mut machine = booted();
    machine
        .type_text("A=1.25\nB$=\"HELLO\"\nPRINT A;B$\n")
        .unwrap();
    machine.run_frames(5).unwrap();
    let text = machine.capture_text_screen();
    let output = text.split("PRINT A;B$").last().unwrap();
    assert!(output.contains("1.25"), "{text}");
    assert!(output.contains("HELLO"), "{text}");
}

#[test]
/// RUN/STOP must interrupt an endless numbered GOTO loop on either ROM.
/// Arguments: none. Returns: nothing; assertions fail when execution persists.
fn run_stop_interrupts_basic_loop() {
    let mut machine = booted();
    machine.type_text("10 GOTO 10\nRUN\n").unwrap();
    machine.set_key(63, true);
    machine.run_frames(4).unwrap();
    machine.set_key(63, false);
    machine.run_frames(4).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("BREAK"), "{text}");
    machine.type_text("PRINT 7\n").unwrap();
    machine.run_frames(5).unwrap();
    assert!(machine.capture_text_screen().contains(" 7 "));
}

/// Evaluate a short BASIC expression and parse its printed numeric result.
/// Arguments: expression is BASIC source. Returns: the first printed number.
fn printed_number(expression: &str) -> f64 {
    let mut machine = booted();
    let command = format!("PRINT {expression}");
    machine.type_text(&format!("{command}\n")).unwrap();
    machine.run_frames(20).unwrap();
    let text = machine.capture_text_screen();
    let output = text
        .lines()
        .skip_while(|line| !line.contains(&command))
        .skip(1)
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("missing output for {expression}: {text}"));
    output
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap_or_else(|_| panic!("non-numeric output for {expression}: {text}"))
}

#[test]
/// Numeric functions and conversion return the same user-visible values.
/// Arguments: none. Returns: nothing; assertions fail on BASIC differences.
fn common_numeric_functions() {
    for (expression, expected) in [
        ("ABS(-3.5)", 3.5),
        ("SGN(-3.5)", -1.0),
        ("INT(-1.2)", -2.0),
        ("SQR(9)", 3.0),
        ("SIN(0)", 0.0),
        ("COS(0)", 1.0),
        ("TAN(0)", 0.0),
        ("ATN(0)", 0.0),
        ("EXP(LOG(1))", 1.0),
        ("VAL(\"12.5X\")", 12.5),
    ] {
        let actual = printed_number(expression);
        assert!((actual - expected).abs() < 0.001, "{expression}: {actual}");
    }
    assert!(printed_number("FRE(0)") > 60_000.0);
    let random = printed_number("RND(1)");
    assert!((0.0..1.0).contains(&random), "RND(1): {random}");
}

#[test]
/// A fractional FOR step visits the same three values under both ROMs.
/// Arguments: none. Returns: nothing; assertions fail on BASIC loop behavior.
fn fractional_for_loop() {
    let mut machine = booted();
    machine
        .type_text("FOR X=0 TO 1 STEP .5:PRINT X;:NEXT\n")
        .unwrap();
    machine.run_frames(20).unwrap();
    let text = machine.capture_text_screen();
    let output = text
        .split("FOR X=0 TO 1 STEP .5:PRINT X;:NEXT")
        .last()
        .unwrap();
    let values: Vec<f64> = output
        .split_whitespace()
        .filter_map(|part| part.parse().ok())
        .collect();
    assert_eq!(values, [0.0, 0.5, 1.0], "{text}");
}
