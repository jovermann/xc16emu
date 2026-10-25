// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! End-to-end checks against the ROM files supplied in old/roms.

use std::process::Command;
use xc16emu::{Machine, COMPACT_BORDER, COMPACT_HEIGHT, COMPACT_WIDTH, HEIGHT, WIDTH};

/// Boot a fresh machine and let BASIC finish its banner.
/// Arguments: none. Returns: booted machine.
fn booted() -> Machine {
    let roms = concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms");
    let mut machine = Machine::from_rom_dir(roms).unwrap();
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
    assert!(text.contains("COMMODORE BASIC V3.5"), "{text}");
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
        .load_prg(include_bytes!("../examples/rotating_cube.prg"))
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
        .load_prg(include_bytes!("../examples/mandelbrot.prg"))
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
        Machine::from_rom_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms")).unwrap();
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
    let output = Command::new(env!("CARGO_BIN_EXE_xc16emu"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
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
        command.current_dir(env!("CARGO_MANIFEST_DIR")).args([
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
/// Check TOM keeps its raster split and character set stable.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn tom_raster_irq_keeps_the_character_set_stable_each_frame() {
    let mut machine =
        Machine::from_rom_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms")).unwrap();
    machine
        .load_frozen_image(include_bytes!("../old/tom_working_frozen_image_64k"))
        .unwrap();
    // The game sets a second raster interrupt during the current line. If
    // that interrupt is lost, it returns to the ROM character set only on
    // alternate frames and both the picture and music run at half speed.
    for _ in 0..20 {
        machine.run_frames(1).unwrap();
        assert_eq!(machine.ted.registers[0x13], 0xd5);
        assert_eq!(machine.ted.irq_line, 33);
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
/// Check the bundled PAINT program loads through IEC.
/// Arguments: none. Returns: nothing; assertions fail on regression.
fn supplied_paint_program_loads_from_virtual_drive() {
    let mut machine = booted();
    machine.set_drive_root(concat!(env!("CARGO_MANIFEST_DIR"), "/old"));
    machine.type_text("DLOAD\"BASIC/PAINT.BAS\"\n").unwrap();
    machine.run_frames(50).unwrap();
    let text = machine.capture_text_screen();
    assert!(text.contains("LOADING"), "{text}");
    assert_eq!(text.matches("READY.").count(), 2, "{text}");
    machine.type_text("RUN\n").unwrap();
    machine.run_frames(50).unwrap();
    assert!(machine.line_modes[70] & 0x20 != 0);
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
