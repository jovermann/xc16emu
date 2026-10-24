xc16emu - C16/C116/plus/4 emulator
==================================

This is a C16/C116/Plus/4 emulator for macOS written in Rust.
It was originally written 1996 in C++ for Qt/X11/Linux.

Features
--------

- 7501/8501 CPU emulation with 16, 32, or 64 KiB RAM configurations. (Same ISA at 6502/6510.)
- Minimalistic PAL TED 7360/8360 emulation video with character, high-resolution bitmap, and multicolor modes.
- Scan-line timing, raster interrupts, and mid-frame mode changes for split graphics and text screens.
- TED sound with two voices, noise, volume control, and digital sample playback.
- Keyboard input and keyboard-controlled substitutes for both joystick ports.
- Virtual IEC device 8 for BASIC `DLOAD` and `DSAVE` using a host directory.
- Loading of PRG files, modules with a chosen start address, and 64 kB frozen images.
- Headless 40×25 text and 320×200 graphics capture for automated testing.
- macOS window with adjustable zoom and borders, status display, reset, Free Run, and joystick key help.
- Persistent desktop settings in `~/.xc16emu.toml`.


Build on macOS
--------------

Install Rust, then from this directory:

```sh
cargo build --release
```

Run on macOS
------------

The ROM images are not included in this repository. Put `basrom.dat` and
`sysrom.dat` in a directory, then run the built executable directly:

```sh
./target/release/xc16emu --roms /path/to/roms
```


Loading programs
----------------

`--prg FILE` copies a two-byte-address Commodore PRG directly into RAM.
`--module FILE` loads the same format **after** BASIC has booted, so startup
does not overwrite a module in low RAM. `--go HEX` starts executing at a
hexadecimal address (bare `4000`, `$4000`, or `0x4000`). For a compatible
module whose entry point is `$4000`:

```sh
./target/release/xc16emu --roms /path/to/roms --module path/to/module.prg --go 4000
```

For BASIC's `DLOAD` and `DSAVE`, `--drive DIR` exposes a host directory as
device 8. Filenames are confined to that directory. `--dry-run` acknowledges
drive saves without writing a file. `--verbose` reports drive file opens.

`--frozen-image FILE` accepts the old emulator's 64 KiB frozen-image format.

Running tests
-------------

```sh
cargo test
```


Files
-----

- `src/cpu.rs`: 6502-family instruction execution and flags.
- `src/video.rs`: TED scan-line renderer.
- `src/sound.rs`: TED audio synthesis and sample timing.
- `src/audio_output.rs`: desktop audio output.
- `src/lib.rs`: memory map, TED timing, keyboard, virtual drive, captures.
- `src/main.rs`: macOS-compatible Rust window and command-line interface.


Headless capture for testing
----------------------------

```sh
./target/release/xc16emu --roms /path/to/roms --text --frames 250
./target/release/xc16emu --roms /path/to/roms --type 'PRINT 2+3*4' --text --frames 30
./target/release/xc16emu --roms /path/to/roms --type 'GRAPHIC 2,1' --type 'CIRCLE 1,160,100,50' --graphics --frames 80
```

`--text` prints exactly 25 lines of 40 characters from the active screen
matrix. `--graphics` prints 200 lines of 320 characters from the last rendered
frame: space for hue 0 and hex digits `1`–`f` for other hues. Both captures
work without a window. `--type` sends a BASIC line through the emulated
keyboard matrix after ROM startup; it can be repeated. `--frames` specifies
additional frames after the 100-frame startup and any typed lines.


Limitations
-----------

The TED advances in PAL raster ticks, allowing the CPU to run faster in the
border than during display. It renders at scan-line boundaries and checks
raster interrupts when a compare line is programmed during the current line.
The renderer tracks output rows separately from the writable raster counter,
and its character mode can use all 256 glyphs when reverse mode is disabled.
It does not yet model every TED bus-steal phase or color subcycle. Sound is
sampled at the output device rate; the high-frequency tone edges and noise
shift-register taps are approximations rather than transistor-level emulation.
The desktop window uses a native macOS menu bar for machine and view settings.
