# Independent BASIC ROMs

`basrom.asm` and `sysrom.asm` are independently written 6502 assembly sources.
They build into the 16 KiB `basrom.dat` and `sysrom.dat` images required by
xc16emu. The bold font in `sysrom.asm` is independently drawn.
The startup banner identifies this early interpreter as **FREE BASIC V0.1**.

## ROM space used

| ROM | Capacity | Used | Free |
| --- | ---: | ---: | ---: |
| `basrom.dat` | 16,384 bytes | 2,081 bytes | 14,303 bytes |
| `sysrom.dat` | 16,384 bytes | 3,531 bytes | 12,853 bytes |

These counts come from the current assembled address ranges, including the
system ROM's 2,048-byte font area and the BASIC ROM's one-byte end marker.
Assembler-filled gaps are counted as free. BASIC has 14,303 consecutive free
bytes at `$8820`–`$BFFE`. The system ROM has three free gaps; the largest is
10,233 bytes at `$D801`–`$FFF9`.

Build the sibling assembler once, then build the ROMs:

```sh
cargo build --release --manifest-path ../ass6510/Cargo.toml
make -C free_roms
```

Run the emulator with these images:

```sh
cargo run --release -- --roms free_roms
```

Run the headless ROM checks without opening an audio device:

```sh
make -C free_roms test
```

The ROMs handle the first three probes in `Basic_V35.md`: integer `PRINT`
with `+` and `*`, `FOR`/`NEXT`, `HEX$`, `DEC`, and the example `REM` notes.
Numbered lines are stored in ascending order. Reenter a line number to replace
it; enter the number alone to delete it. `LIST [first][-last]` displays source
lines, and `RUN [line]` executes the stored program. `GOTO line` jumps to an
existing line, including from the direct prompt. A `FOR`/`NEXT` loop may
span numbered lines. Entering a numbered line leaves the next screen row ready
for more input without printing `READY.` again. Pause/Break acts as RUN/STOP:
it interrupts a running program with `?BREAK`. Left and Right move across screen cells; Up and Down move
one physical screen row while retaining the cursor column. Screen-row links
identify wrapped lines, so moving to a continuation row imports the whole line
for editing. Backspace removes the
preceding character, with key repeat when held. Forward Delete clears a line's
text but keeps its number, so Return then deletes that stored program line.
**Edit → Delete Program Line** performs both actions for the current numbered
input line.

Program text occupies `$1200`–`$3fff`, a region available on every supported
RAM configuration. The boot title shows the unused bytes measured at startup
and stays on the screen until normal output scrolls it away. A stored line uses three bytes of
metadata plus its text. The Edit → Paste menu item and Command-V type clipboard
lines through the emulated keyboard and submit each line with Return. Interactive
text uses the host keyboard layout, so ASCII punctuation such as `*`, quotes,
brackets, and comparison signs can be entered directly. Output scrolls up when
the 25-line screen fills.

This is still a small BASIC subset: arithmetic is unsigned 16-bit, there is
one active `FOR` loop, and other BASIC keywords and TEDMON are not
implemented.
