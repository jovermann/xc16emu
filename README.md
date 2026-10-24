# XC16EMU Rust

A Rust port of the C16/C116/Plus/4 emulator in [`old/`](old). The old source
and ROM images are left untouched. The emulator core is independent of the
desktop window, so BASIC and video behavior can be tested headlessly.

## Build and run on macOS

Install Rust, then from this directory:

```sh
cargo run --release
```

The program uses the ROM images in `old/roms` by default. Set `--roms DIR` to
use another directory. BASIC and system ROMs must each be 16 KiB. Optional
software and expansion banks are loaded when present. No ROM is copied into
the Rust binary.

The desktop window uses a pixel display suitable for emulator video. It starts
at 2× zoom with a 32-pixel border. The View menu selects 1×, 2×, or 4×
zoom; big (32 pixels), medium (16 pixels), or compact (8 pixels, one character
cell) border; and
whether to show the bottom status line. Its font and one-pixel outline stay at
screen resolution as the video zoom changes. It shows measured millions of
emulated CPU instructions per second (MIPS) and a realtime factor based on
elapsed PAL TED ticks (1.00X means realtime). `--small-border` starts in
compact mode and `--zoom 1|2|4` overrides the saved zoom for that launch. The
menu choices for zoom, border, status bar, joystick help, Free Run, and Sound are saved in
`~/.xc16emu.toml`. `--speed 0.5` and
`--speed 2` change the target emulation speed from the default 50 PAL frames
per second. The Machine menu can reset the computer, toggle Free Run (which
removes the frame rate limit), or toggle Sound. The status line shows a white
speaker when sound is on and a gray crossed-out speaker when muted. macOS
window commands are under Window. F12 also
resets; Escape exits. Letters, digits, cursor keys,
Return, Shift, Control, Alt, F1–F3, Home and Tab map to C16 keys.

The View menu or Break/Pause (F15 on macOS) shows and hides an in-window guide to the keyboard keys
for both joystick ports. Port 1 uses 5/R/D/6 for up/down/left/right and T for
fire. Port 2 uses 3/W/A/4 for up/down/left/right and Shift for fire. The guide
stays visible until toggled off, and its setting is saved between launches.

Sound plays through the default system output device. TED's two square-wave
voices, voice-two noise mode, volume control, and direct volume DAC mode are
generated from emulated TED time. Try `VOL 8` followed by
`SOUND 1,770,50` in BASIC. If no audio device is available, the emulator still
runs and prints a warning. Headless programs can call
`set_audio_sample_rate` and `drain_audio_samples` to inspect mono audio
without opening a window.
If sound is unexpectedly quiet, check the macOS output volume and device.
Launching with `XC16EMU_AUDIO_DEBUG=1 cargo run --release` prints the selected
device and a count of nonzero samples sent to it.

## Headless capture

```sh
cargo run --release -- --text --frames 250
cargo run --release -- --type 'PRINT 2+3*4' --text --frames 30
cargo run --release -- --type 'GRAPHIC 2,1' --type 'CIRCLE 1,160,100,50' --graphics --frames 80
```

`--text` prints exactly 25 lines of 40 characters from the active screen
matrix. `--graphics` prints 200 lines of 320 characters from the last rendered
frame: space for hue 0 and hex digits `1`–`f` for other hues. Both captures
work without a window. `--type` sends a BASIC line through the emulated
keyboard matrix after ROM startup; it can be repeated. `--frames` specifies
additional frames after the 100-frame startup and any typed lines.

## Programs and the virtual drive

`--prg FILE` copies a two-byte-address Commodore PRG directly into RAM.
`--module FILE` loads the same format **after** BASIC has booted, so startup
does not overwrite a module in low RAM. `--go HEX` starts executing at a
hexadecimal address (bare `4000`, `$4000`, or `0x4000`). For the supplied
Galaxy image, which loads at `$0200` and starts at `$4000`:

```sh
cargo run --release -- --module old/galaxy_0200-4007_module_start_with_g4000 --go 4000
```

For BASIC's `DLOAD` and `DSAVE`, `--drive DIR` exposes a host directory as
device 8. Filenames are confined to that directory. `--dry-run` acknowledges
drive saves without writing a file. `--verbose` reports drive file opens.

For example, the original PAINT program can be loaded with:

```sh
cargo run --release -- --drive old --type 'DLOAD"BASIC/PAINT.BAS"' --type RUN --headless --frames 100
```

`--frozen-image FILE` accepts the old emulator's 64 KiB frozen-image format.

## Tests and structure

```sh
cargo test
```

The integration tests boot the supplied BASIC ROM, execute arithmetic and
loop commands, draw a circle and sine plot, load `PAINT.BAS`, and verify that
`GRAPHIC 2` switches from bitmap to text on later raster lines. The CPU
tests cover overflow and indexed page crossing.

- `src/cpu.rs`: 6502-family instruction execution and flags.
- `src/video.rs`: TED scan-line renderer.
- `src/sound.rs`: TED audio synthesis and sample timing.
- `src/audio_output.rs`: desktop audio output.
- `src/lib.rs`: memory map, TED timing, keyboard, virtual drive, captures.
- `src/main.rs`: macOS-compatible Rust window and command-line interface.
- `BugsInTheOldCode.txt`: confirmed defects corrected during the port.

## Current limits

The TED advances in PAL raster ticks, allowing the CPU to run faster in the
border than during display. It renders at scan-line boundaries and checks
raster interrupts when a compare line is programmed during the current line.
The renderer tracks output rows separately from the writable raster counter,
and its character mode can use all 256 glyphs when reverse mode is disabled.
It does not yet model every TED bus-steal phase or color subcycle. Sound is
sampled at the output device rate; the high-frequency tone edges and noise
shift-register taps are approximations rather than transistor-level emulation.
The desktop window uses a native macOS menu bar for machine and view settings.
