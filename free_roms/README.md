# Independent BASIC ROMs

`basrom.asm` and `sysrom.asm` are independently written 6502 assembly sources.
`sysrom.asm` includes the editable `font.asm`. They build into the 16 KiB
`basrom.dat` and `sysrom.dat` images required by xc16emu. The font starts with
the reference C16 glyphs in all 256 screen-code positions.
The startup banner identifies this early interpreter as **FREE BASIC V0.1**.

## ROM space used

| ROM | Capacity | Used | Free |
| --- | ---: | ---: | ---: |
| `basrom.dat` | 16,384 bytes | 2,081 bytes | 14,303 bytes |
| `sysrom.dat` | 16,384 bytes | 3,615 bytes | 12,769 bytes |

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

Edit the font with `./target/release/font-editor free_roms/font.asm` from the
repository root, then run `make -C free_roms` again to assemble changed pixels.
Hold an arrow key or use the scroll wheel to select glyphs. Left drag paints or
erases in the enlarged strip or PIXELS pane; right drag or Shift drag scrolls
the strip. The selected glyph flashes once in reverse in the 16×16 grid when
selection changes. Press `I` to invert the picture while keeping the border
blue, `H` to toggle help, and `Q` or Escape to save and quit.
Press `U` to undo the last paint stroke or restore, and `R` to restore the
selected glyph to its original shape.
Press `L` to copy edited glyphs 0–127 to glyphs 128–255 with the same original
bitmap, even when their code indices differ. A second-set glyph with no
first-set match is left unchanged. If several first-set glyphs match, the
same-index partner takes priority. The copy is one undoable edit.

Run the emulator with these images:

```sh
cargo run --release -- --roms free_roms
```

Run the headless ROM checks:

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
input line. Shift+C= switches between the uppercase/graphics and
lowercase/uppercase character sets; on macOS, Option is the C= key.

Program text occupies `$1200`–`$3fff`, a region available on every supported
RAM configuration. The boot title shows the unused bytes measured at startup
and stays on the screen until normal output scrolls it away. A stored line uses three bytes of
metadata plus its text. The Edit → Paste menu item and Command-V type clipboard
lines through the emulated keyboard and submit each line with Return. Interactive
text uses the host keyboard layout, so C16 punctuation such as `*`, quotes,
brackets, and comparison signs can be entered directly. Output scrolls up when
the 25-line screen fills.

This is still a small BASIC subset: arithmetic is unsigned 16-bit, there is
one active `FOR` loop, and other BASIC keywords and TEDMON are not
implemented.
