# Independent BASIC ROMs

`basrom.asm` and `sysrom.asm` are independently written 6502 assembly sources.
`sysrom.asm` includes the editable `font.asm`. They build into the 16 KiB
`basrom.dat` and `sysrom.dat` images required by xc16emu. The font starts with
the reference C16 glyphs in all 256 screen-code positions.
The startup banner identifies this early interpreter as **FREE BASIC V0.1**.

## BASIC program memory

Numbered programs begin immediately after the 40×25 screen matrix at `$0FE8`.
The ROM copies a small byte reader to `$0700` so it can briefly switch out the
ROM overlays when reading program
records. Storage stops before the hardware I/O page at `$FD00` on 64 KiB
machines. The boot banner measures the available bytes for each RAM size:

| RAM | Program area | Bytes free at boot |
| --- | --- | ---: |
| 16 KiB | `$0FE8`–`$3FFF` | 12,312 |
| 32 KiB | `$0FE8`–`$7FFF` | 28,696 |
| 64 KiB | `$0FE8`–`$FCFF` | 60,696 |

## ROM space used

| ROM | Capacity | Used | Free |
| --- | ---: | ---: | ---: |
| `basrom.dat` | 16,384 bytes | 15,179 bytes | 1,205 bytes |
| `sysrom.dat` | 16,384 bytes | 11,898 bytes | 4,486 bytes |

These counts come from the current assembled address ranges, including the
system ROM's 2,048-byte font area and the BASIC ROM's one-byte end marker.
Assembler-filled gaps are counted as free. BASIC has 1,205 consecutive free
bytes at `$BB4A`–`$BFFE`. The largest system-ROM gap has 2,030 bytes at
`$C812`–`$CFFF`.

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

The ROMs handle the first three probes in `Basic_V35.md`: numeric `PRINT`
with `+` and `*`, `FOR`/`NEXT`, `HEX$`, `DEC`, and the example `REM` notes.
Numbered lines are stored in ascending order. Reenter a line number to replace
it; enter the number alone to delete it. `LIST [first][-last]` displays source
lines, and `RUN [line]` executes the stored program. `GOTO line` jumps to an
existing line, including from the direct prompt. Nested `FOR`/`NEXT` loops and
`GOSUB`/`RETURN` calls share a bounded stack below the screen matrix.
`IF`/`THEN`/`ELSE` chooses a clause. A loop may span numbered lines, and `NEXT J,I`
advances two nested loops. Entering a numbered line leaves the next screen row ready
for more input without printing `READY.` again. Pause/Break (F15 on some
macOS keyboards) and **Machine → Run/Stop** act as RUN/STOP:
it interrupts a running program with `?BREAK`. The blinking text cursor is
hidden while a program runs and restored at the prompt. Left and Right move
across screen cells; Up and Down move
one physical screen row while retaining the cursor column. Screen-row links
identify wrapped lines, so moving to a continuation row imports the whole line
for editing. Backspace removes the
preceding character, with key repeat when held. Forward Delete clears a line's
text but keeps its number, so Return then deletes that stored program line.
**Edit → Delete Program Line** performs both actions for the current numbered
input line. Shift+C= switches between the uppercase/graphics and
lowercase/uppercase character sets; on macOS, Option is the C= key.
Shift plus a letter enters its C16 graphic glyph. BASIC keyword abbreviations
use the same shifted final letter as the original keyboard: for example,
`L` then Shift+`I` executes `LIST`. Abbreviations are expanded by the ROM
before a line is stored or executed; quoted graphic characters remain graphics.

## Machine-language monitor

`MONITOR` enters the system ROM's hexadecimal monitor. `X` returns to BASIC.
It accepts `A`/`.` (assemble), `C` (compare), `D` (disassemble), `F` (fill),
`G` (execute), `H` (hunt bytes or apostrophe-prefixed text), `L` (load PRG),
`M` (memory dump), `>` (deposit bytes), `R` (registers), `S` (save PRG),
`T` (transfer), `V` (verify PRG), and `;` (edit saved registers). Assembly and
disassembly cover the documented 7501 opcodes. `M` and `D` inspect RAM beneath
ROM when bit 7 of `$07F8` is set. Disk commands use device `8` and the
directory selected with `--drive`. A target started with `G` can return to the
monitor with `RTS` or `BRK`.

## Graphics, sound, and disk commands

- `GRAPHIC mode[,clear]` selects modes 0–4. Modes 2 and 4 switch from bitmap
  to text for the bottom five character rows on each raster frame. `GRAPHIC CLR`
  releases the bitmap allocation. `SCNCLR` clears the active picture.
- `DRAW source,x,y[ TO x,y ...]`, `LOCATE x,y`, `CIRCLE source,x,y,xradius[,yradius]`,
  and `BOX source,x1,y1,x2,y2[,0[,fill]]` draw in native bitmap coordinates.
  `PAINT source,x,y[,boundary]` fills a connected area. Its temporary seed
  stack uses free BASIC program RAM and stops before graphics or string data.
  `CHAR source,column,row,"text"[,reverse]` writes text cells or draws ROM glyphs
  in a bitmap.
  `CIRCLE` also accepts rotation and angular step arguments. A step of 120
  degrees draws the rotated triangles used by the TOLL example.
- `COLOR source,hue[,luminance]` changes TED colors. `SCALE 1` maps 0–1023
  coordinates onto the native picture; `SCALE 0` uses native coordinates.
  Modes 1–2 use 320×200 pixels and modes 3–4 use 160×200 pixels.
- `SSHAPE A$,x1,y1,x2,y2` captures a packed high-resolution shape, and
  `GSHAPE A$[,x,y[,mode]]` draws it with modes 0–4 (replace, invert, OR, AND,
  XOR). This small interpreter keeps one named shape slot of at most 255 bytes.
- Numeric expressions use a ROM-owned floating accumulator with `+`, `-`,
  `*`, `/`, parentheses, unary signs, decimal fractions, and `E` exponents.
  Default numeric variables retain floating values; `%` variables truncate
  to signed 16-bit integers. Twelve names with two significant letters share
  a typed descriptor table. `FOR`/`NEXT` supports fractional and signed steps.
  The ROM implements `ABS`, `SGN`, `INT`, `SQR`, `RND`,
  `LOG`, `EXP`, `COS`, `SIN`, `TAN`, `ATN`, `USR`, `FRE`, `POS`, `PEEK`, `LEN`,
  `ASC`, `VAL`, `RGR`, `RCLR`, `RLUM`, `JOY`, `RDOT`, `INSTR`, and `DEC`.
  Numbered programs can define a one-argument function with `DEF FN A(X)=...`
  and call it with `FN A(value)`.
- Sixteen string names can hold up to 255 bytes each, subject to available
  program memory. `PRINT`, `CHAR`, `DSAVE`, and `DLOAD` accept string values.
  String functions include `LEFT$`, `RIGHT$`, `MID$`, `CHR$`, `HEX$`, and
  floating `STR$`; `MID$(A$,start[,count])=text$` replaces existing bytes.
  Strings concatenate with `+`. The current heap does not reclaim replaced
  strings until variables are cleared. Decimal output currently prints at most
  six fractional digits and uses scientific notation outside its fixed range.
- `VOL level` accepts 0–8. `SOUND voice,frequency,duration` uses TED tone
  voices 1–2 or noise voice 3, with a frequency control of 0–1023 and a
  duration in PAL 1/50-second ticks. Duration zero stops a voice. Reusing a
  busy voice waits for its current duration to expire.
- `POKE address,value` writes a byte of machine memory. `DSAVE "file"` and
  `DLOAD "file"` write and read numbered programs through
  the emulator's virtual IEC drive. Pass `--drive DIR` to select its host
  directory. Files use the free ROM's `FBR1` binary format: four signature
  bytes, a 16-bit record length, then packed program records. `DLOAD` also
  reads standard tokenized BASIC PRG files at `$1001` or `$4001` when their tokens are
  supported by this ROM. Unsupported tokens report a disk error.

Bitmap mode reserves 2 KiB of color/screen matrices and 8 KiB of pixel RAM
below `$8000`. The available contiguous BASIC program area then ends at
`$1800` on 16 KiB machines or `$5800` on 32/64 KiB machines. `SSHAPE` reserves
one more page, moving those limits to `$1700` and `$5700`. `GRAPHIC CLR`
restores the full program limit. Programs that already extend into a required
graphics area receive `?OUT OF MEMORY` instead of being overwritten.
An error returns the display to text mode while retaining the allocated bitmap;
`GRAPHIC CLR` releases it.

Program text uses the area for the installed RAM size shown above. The boot
title shows the unused bytes measured at startup
and stays on the screen until normal output scrolls it away. A stored line uses three bytes of
metadata plus its text. The Edit → Paste menu item and Command-V type clipboard
lines through the emulated keyboard and submit each line with Return. Interactive
keys use the same C16 keyboard-matrix positions with every ROM; the ROM decides
the resulting character. Shift+3 sends the physical 3 key with Shift, producing
`#` in this ROM. Output scrolls up when
the 25-line screen fills.

This is still a small BASIC subset; many BASIC keywords and TEDMON are not
implemented. BASIC parsing and
execution occur entirely in the 6502 ROM code;
the Rust emulator supplies CPU, memory, TED, keyboard, sound, and virtual IEC
hardware only. Graphics currently accept absolute Cartesian coordinates;
relative and polar forms, rotated boxes, circle arcs, and multicolor shape
capture are not yet supported.
