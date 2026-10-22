// Copyright (C) 2026 Johannes Overmann and contributors.
// SPDX-License-Identifier: GPL-2.0-or-later
//! End-to-end checks against the ROM files supplied in old/roms.

use xc16emu::{Machine, COMPACT_BORDER, COMPACT_HEIGHT, COMPACT_WIDTH, HEIGHT, WIDTH};

/// Boot a fresh machine and let BASIC finish its banner.
fn booted() -> Machine {
    let roms = concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms");
    let mut machine = Machine::from_rom_dir(roms).unwrap();
    machine.run_frames(100).unwrap();
    machine
}

#[test]
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
