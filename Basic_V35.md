# Commodore BASIC V3.5 behavior reference

This is a **user-visible specification** for a future, independently written BASIC and TEDMON implementation. It records names from the supplied ROM images and paraphrases observable behavior; it contains no ROM routines, addresses of implementation routines, disassembly, or copied manual prose. A first implementation may support a subset, but the inventory below is complete for the BASIC keyword table in this ROM pair.

## Sources and scope

- The examined BASIC V3.5 ROM image is 16 KiB (SHA-256 `cbf0c4dec44e3e203beaf09690e5c7f859b0eca164e789884161c8cb0e596567`). Its high-bit-terminated keyword table begins at file offset `$018E`. Tokens `$80`–`$FD` name **126 entries**; the byte following the table is `$00`. The hex codes below were decoded directly from that table.
- The examined system ROM image is 16 KiB (SHA-256 `cfd5758d96b0d9bdbe1461990d507475774da65383c7cad2edcdfa0073fcc6b9`). It contains the text `MONITOR` at file offset `$0F37` and the register headings `PC SR AC XR YR SP` nearby. The table alone does not specify command grammar, so the command descriptions below are checked against Commodore's [Plus/4 User's Manual](https://plus4world.powweb.com/publications/Commodore_Plus4_Users_Manual), especially its BASIC reference (printed pp. 116–167) and TEDMON section (printed pp. 184–192). The [Commodore 16 User Manual](https://www.commodore.ca/wp-content/uploads/2018/11/Commodore-16-User-Manual.pdf) provides a second contemporary reference.
- The token table is the inventory authority. Some written forms, such as `GETKEY`, use more than one token. The manual describes these forms even though they have no dedicated token.
- Disk, cassette, printer, and monitor operations describe the original machine's interface. A replacement ROM can document unsupported devices or operations while retaining the same BASIC syntax.

`n`, `x`, `y`, `line`, and `addr` denote numeric expressions; `s$` and `file$` denote string expressions; `var` denotes a variable. Square brackets mark optional syntax, and `...` marks repetition. Decimal is the default in BASIC; TEDMON numbers are hexadecimal. **Statements/commands have no expression return value** unless an output or changed state is stated. Functions return the value described in their row. Errors and device status are observable results, not Rust return types. Line numbers in examples are decimal.

## BASIC token inventory: program flow and core I/O (`$80`–`$A2`)

| Token | Keyword and form | Observable behavior or result |
|---|---|---|
| `$80` | `END` | Stop the running program without deleting it; `CONT` can resume at the following statement. |
| `$81` | `FOR var=start TO limit [STEP delta]` | Initialize a numeric loop variable and save loop state. Default step is 1; a negative step counts downward. |
| `$82` | `NEXT [var[,var...]]` | Advance each named loop, or the innermost loop when omitted; repeat until its limit has been passed. |
| `$83` | `DATA item[,item...]` | Hold literal data in program text for later `READ`; execution skips the items. Quoting preserves commas, colons, and spaces. |
| `$84` | `INPUT#file,var[,var...]` | Read delimited values from an open logical file into typed variables; no screen prompt. |
| `$85` | `INPUT ["prompt";]var[,var...]` | Prompt and wait for RETURN-terminated input. Multiple values use commas; a missing value leaves that variable unchanged. Program mode only. |
| `$86` | `DIM array(max[,max...])[,array(...)]` | Allocate arrays with indexes starting at zero. Undimensioned arrays allow subscripts 0–10; redimensioning the same array is an error. |
| `$87` | `READ var[,var...]` | Take successive items from `DATA` into variables, advancing the data pointer; type and end-of-data errors are observable. |
| `$88` | `[LET] var=expression` | Assign a numeric, integer (`%`), or string (`$`) variable or array element. `LET` is optional. |
| `$89` | `GOTO line` | Continue at the named line. `GO TO line` is the two-token spelling. |
| `$8A` | `RUN [line]` | Clear variables and stacks, then execute from the first line or a specified line. Also stops automatic line numbering. |
| `$8B` | `IF expression THEN clause [:ELSE clause]` | Execute the true clause for nonzero; otherwise execute the optional `ELSE` clause on the same program line. `THEN line` branches to that line. |
| `$8C` | `RESTORE [line]` | Reset the `DATA` read pointer to the first item, or to data at/after the specified line. |
| `$8D` | `GOSUB line` | Save the return position and execute a subroutine at `line`. |
| `$8E` | `RETURN` | Resume after the most recent `GOSUB`; error if no subroutine is active. |
| `$8F` | `REM text` | Ignore the remainder of this program line as a comment. |
| `$90` | `STOP` | Interrupt execution with a break indication and line number; `CONT` can resume. |
| `$91` | `ON expression GOTO line[,line...]` or `ON expression GOSUB line[,line...]` | Choose the 1-based list entry from the integer result. Zero or an index past the list falls through; a negative index is an error. |
| `$92` | `WAIT addr,mask[,xor]` | Repeatedly read memory until `((PEEK(addr) XOR xor) AND mask) <> 0`; omitted `xor` is zero. |
| `$93` | `LOAD [file$[,device[,relocate]]]` | Load from cassette (default device 1) or another device. `relocate=0` loads at BASIC's current program start; `1` uses the file's stored address. Loading another BASIC program replaces the current one. |
| `$94` | `SAVE [file$[,device[,flag]]]` | Save the current BASIC program, defaulting to cassette. On tape, the flag controls tape end markers and related file attributes. |
| `$95` | `VERIFY [file$[,device[,relocate]]]` | Compare a tape/disk program with memory using the same placement rule as `LOAD`; report success or a verification error. |
| `$96` | `DEF FNname(var)=expression` | Define a one-argument numeric function; later `FNname(n)` evaluates the saved expression with that argument. |
| `$97` | `POKE addr,byte` | Write a byte (0–255) to address 0–65535; visible RAM, I/O, and ROM-overlaid RAM behavior depends on the memory map. |
| `$98` | `PRINT#file[,items...]` | Send formatted output to an open logical file/device. Separators affect spacing and records. |
| `$99` | `PRINT [items...]` or `? [items...]` | Output expressions to the current output channel. Comma moves to a print zone; semicolon concatenates; a trailing separator suppresses the normal line ending. |
| `$9A` | `CONT` | Resume after `STOP`, END, or STOP key if the program has not been edited and no disqualifying error occurred. |
| `$9B` | `LIST [first][-last]` | Display all or a line range, with a single number selecting one line. `LIST -last` and `LIST first-` are open ranges. |
| `$9C` | `CLR` | Erase variable, array, string, and runtime stack state while keeping program lines. |
| `$9D` | `CMD file[,items...]` | Redirect normal text output to a previously opened file/device; `PRINT#` can return output to the screen. |
| `$9E` | `SYS addr` | Enter machine code at a decimal address (0–65535); execution later returns according to the machine-code calling convention. |
| `$9F` | `OPEN file,device[,secondary[,name$]]` | Associate a logical file number with a device and optional channel/name; later `GET#`, `INPUT#`, `PRINT#`, `CMD`, and `CLOSE` use that number. |
| `$A0` | `CLOSE file` | Finish and release an open logical file/channel. |
| `$A1` | `GET var[,var...]` or `GET#file,var[,var...]` | Read one character without waiting from the keyboard, or from an open file. With no keyboard character, a string target becomes empty. Program mode only. |
| `$A2` | `NEW` | Delete the stored BASIC program and clear variables. |

## BASIC token inventory: print controls, grammar, and operators (`$A3`–`$B3`)

These entries are part of expressions or compound statements; most cannot stand alone as commands. Comparisons yield BASIC true `-1` or false `0`. Logical operations act on integer values bit by bit.

| Token | Form | Observable behavior or result |
|---|---|---|
| `$A3` | `TAB(column)` in `PRINT` | Move the following output item to a requested print column (0–255). |
| `$A4` | `TO` | Separate start and end in `FOR`, coordinate segments in `DRAW`, or source and destination in disk commands. |
| `$A5` | `FNname(n)` | Call the numeric function established by `DEF FNname`; return its numeric result. |
| `$A6` | `SPC(count)` in `PRINT` | Insert `count` spaces (0–255) in formatted output. |
| `$A7` | `THEN` | Introduce the true branch of `IF`; a bare line number after it is a branch target. |
| `$A8` | `NOT n` | Return the bitwise complement of the BASIC integer value. |
| `$A9` | `STEP delta` | Set the increment for a `FOR` loop. |
| `$AA` | `a+b` | Numeric addition, or string concatenation for string operands. |
| `$AB` | `a-b` | Numeric subtraction; prefix `-` negates. |
| `$AC` | `a*b` | Numeric multiplication. |
| `$AD` | `a/b` | Numeric division; zero divisor is an error. |
| `$AE` | `a^b` | Exponentiation. |
| `$AF` | `a AND b` | Bitwise integer AND; often combines comparisons. |
| `$B0` | `a OR b` | Bitwise integer OR. |
| `$B1` | `a>b` | Greater-than comparison. |
| `$B2` | `a=b` | Equality comparison, distinct from assignment context. |
| `$B3` | `a<b` | Less-than comparison. Adjacent relational tokens form `<=`, `>=`, and `<>`. |

Operator order is parentheses, exponentiation, multiplication/division, addition/subtraction, comparisons, then `NOT`, `AND`, and `OR`; same-priority binary operators evaluate left to right where applicable. The built-in constant `π` is approximately `3.14159265` and is not a keyword-table token. Numeric truth is nonzero; comparison results are `-1`/`0`. String comparisons follow the machine's character ordering.

## BASIC token inventory: standard functions and `GO` (`$B4`–`$CB`)

| Token | Form | Return value or behavior |
|---|---|---|
| `$B4` | `SGN(n)` | `-1`, `0`, or `1` according to the sign. |
| `$B5` | `INT(n)` | Greatest integer no larger than `n` (so `INT(-4.5)=-5`). |
| `$B6` | `ABS(n)` | Absolute value. |
| `$B7` | `USR(n)` | Call machine code through the vector at decimal 1281/1282 (`$0501/$0502`), passing and receiving a floating-point value. |
| `$B8` | `FRE(dummy)` | Number of unused BASIC bytes; argument is evaluated but not used as the requested size. |
| `$B9` | `POS(dummy)` | Current text print column; argument is a dummy. |
| `$BA` | `SQR(n)` | Square root; negative input is an error. |
| `$BB` | `RND(n)` | Pseudorandom fraction in `[0,1)`. Positive `n` advances the sequence, zero uses the clock, negative `n` seeds a repeatable sequence. |
| `$BC` | `LOG(n)` | Natural logarithm; nonpositive input is an error. |
| `$BD` | `EXP(n)` | `e` raised to `n`. |
| `$BE` | `COS(n)` | Cosine of an angle in radians. |
| `$BF` | `SIN(n)` | Sine of an angle in radians. |
| `$C0` | `TAN(n)` | Tangent of an angle in radians. |
| `$C1` | `ATN(n)` | Arctangent in radians. |
| `$C2` | `PEEK(addr)` | Byte at memory address 0–65535, returned as 0–255. |
| `$C3` | `LEN(s$)` | String length in characters/bytes. |
| `$C4` | `STR$(n)` | Printable decimal form of a numeric expression, including Commodore numeric formatting. |
| `$C5` | `VAL(s$)` | Numeric prefix parsed from a string; zero when no numeric prefix is present. |
| `$C6` | `ASC(s$)` | Character code of the first character; an empty string is an error. |
| `$C7` | `CHR$(byte)` | One-character string for character code 0–255. |
| `$C8` | `LEFT$(s$,count)` | Leftmost `count` characters. |
| `$C9` | `RIGHT$(s$,count)` | Rightmost `count` characters. |
| `$CA` | `MID$(s$,start[,count])` | Substring starting at 1-based `start`. On the assignment left side, `MID$(variable$,start[,count])=source$` replaces characters in place. |
| `$CB` | `GO TO line` | Alternate spelling of `GOTO`; `GO` combines with token `TO` rather than being a standalone jump. |

## BASIC token inventory: extra functions (`$CC`–`$D4`)

| Token | Form | Return value |
|---|---|---|
| `$CC` | `RGR(dummy)` | Current `GRAPHIC` mode number, 0–4; argument is a dummy. |
| `$CD` | `RCLR(source)` | Color number (1–16) currently assigned to source 0–4. |
| `$CE` | `RLUM(source)` | Luminance level (0–7) currently assigned to source 0–4. |
| `$CF` | `JOY(port)` | Joystick 1 or 2 state: directions are center 0, up 1, up-right 2, right 3, down-right 4, down 5, down-left 6, left 7, up-left 8; add 128 while fire is pressed. |
| `$D0` | `RDOT(part)` | For `part=0`, pixel-cursor X; `1`, Y; `2`, color source at that position. |
| `$D1` | `DEC(hex$)` | Numeric value of a hexadecimal string representing 0–`FFFF`. |
| `$D2` | `HEX$(n)` | Four-character uppercase hexadecimal representation of a 16-bit value. |
| `$D3` | `ERR$(error)` | Text of a BASIC error code, commonly `ERR$(ER)` in a trap handler. |
| `$D4` | `INSTR(haystack$,needle$[,start])` | 1-based position of the first match at/after `start` (default 1); zero if absent. |

## BASIC token inventory: extensions, graphics, disk, and editor (`$D5`–`$FD`)

Graphics commands use an invisible **pixel cursor** as the default point or as the start of a relative move. A `+` or `-` before a coordinate makes that coordinate relative to the pixel cursor. A distance and angle can also be separated by a semicolon. Unless `SCALE 1` is active, high-resolution pixels are 320×200 and multicolor pixels are 160×200. Color sources are 0 background, 1 foreground, 2 multicolor 1, 3 multicolor 2, and 4 border; drawing normally accepts 0–3. See the [original Plus/4 manual](https://plus4world.powweb.com/publications/Commodore_Plus4_Users_Manual), printed pp. 129–157.

| Token | Keyword and form | Observable behavior or result |
|---|---|---|
| `$D5` | `ELSE clause` | Execute the false branch of an `IF` on the same line. It is a clause marker, not an independent command. |
| `$D6` | `RESUME [line or NEXT]` | After `TRAP`, retry the error line, continue at the next statement, or branch to a specified line. |
| `$D7` | `TRAP [line]` | Divert most execution errors and the STOP key to a handler; populate `ER` and `EL`. Omit the line to disable trapping. An error inside the handler is not retrapped. |
| `$D8` | `TRON` | Enable execution trace; print line numbers as statements run. |
| `$D9` | `TROFF` | Disable execution trace. |
| `$DA` | `SOUND voice,frequency,duration` | Play tone voice 1 or 2, or noise voice 3; frequency control 0–1023, duration 0–65535 ticks. On a PAL machine ticks are **1/50 second** (NTSC: 1/60); zero duration stops the voice. A new sound on a busy voice waits for it, and tone 2 and noise share hardware. No expression value. |
| `$DB` | `VOL level` | Set common volume 0–8 for both tone voices and noise; zero mutes. |
| `$DC` | `AUTO [increment]` | In direct mode, begin automatic line numbering with the given spacing; `AUTO` alone turns it off. |
| `$DD` | `PUDEF "chars"` | Replace up to four `PRINT USING` formatting symbols by position: fill, comma, decimal point, dollar sign. Unspecified positions keep defaults. |
| `$DE` | `GRAPHIC mode[,clear]` or `GRAPHIC CLR` | Select 0 text, 1 high-resolution, 2 high-resolution plus bottom text, 3 multicolor, 4 multicolor plus bottom text. Modes 1–4 reserve a 10 KiB bitmap, retained when returning to mode 0. `clear=1` clears it; `GRAPHIC CLR` releases the allocation. |
| `$DF` | `PAINT [source][,x,y][,boundary-mode]` | Flood-fill from the given/default pixel cursor. Default source 1. Mode 0 stops at the selected source color; mode 1 stops at any nonbackground source. The pixel cursor remains at the starting point. |
| `$E0` | `CHAR [source,]column,row[,s$][,reverse]` | Draw character-ROM glyphs at a 40×25 cell position on the current screen; reverse flag 0/1. In bitmap mode, embedded text control codes are not interpreted. |
| `$E1` | `BOX [source,]x1,y1[,x2,y2[,angle[,fill]]]` | Draw a rectangle, optionally rotate it clockwise about its center and fill it. Missing second corner defaults to the pixel cursor; the cursor ends at that corner. |
| `$E2` | `CIRCLE [source,]x,y,x-radius[,y-radius[,start-angle[,end-angle[,rotation[,step]]]]]` | Draw an ellipse or arc with clockwise degree angles; omitted Y radius equals X radius, start/end default to 0/360, rotation to 0, angular step to 2. The cursor ends on the arc. Equal radii need not look circular because pixel aspect ratios differ. |
| `$E3` | `GSHAPE s$[,x,y[,mode]]` | Draw an image previously captured by `SSHAPE` at a point (default pixel cursor). Modes 0–4: replace, invert, OR, AND, XOR. |
| `$E4` | `SSHAPE target$,x1,y1[,x2,y2]` | Store a bitmap rectangle and dimensions in a BASIC string; the 255-character string limit bounds shape size. |
| `$E5` | `DRAW [source,][x1,y1][ TO x2,y2 ...]` | Plot one point or connected line segments; missing start uses the pixel cursor, which advances with drawing. |
| `$E6` | `LOCATE x,y` or `LOCATE distance;angle` | Move the pixel cursor without drawing, using absolute, relative, or polar coordinates. |
| `$E7` | `COLOR source,color[,luminance]` | Assign color 1–16 to source 0–4 with luminance 0–7 (default 7); the border is source 4. |
| `$E8` | `SCNCLR` | Clear the active text, graphics, or split screen. |
| `$E9` | `SCALE 0 or 1` | Use native bitmap coordinates (0), or map both coordinates from 0–1023 (1). |
| `$EA` | `HELP` | After a BASIC error, list the offending program line with the error location highlighted. |
| `$EB` | `DO [WHILE condition or UNTIL condition]` | Begin a loop; a top test may skip its body. |
| `$EC` | `LOOP [WHILE condition or UNTIL condition]` | Repeat or leave the innermost `DO` loop; without a test, loop indefinitely. |
| `$ED` | `EXIT` | Leave the innermost `DO`/`LOOP` and continue after `LOOP`. |
| `$EE` | `DIRECTORY [Ddrive][,Uunit][,"pattern"]` | List matching disk filenames without replacing the BASIC program. `*` and `?` are wildcards. |
| `$EF` | `DSAVE file$[,Ddrive][,Uunit]` | Save the current BASIC program to disk (defaults: drive 0, unit 8). |
| `$F0` | `DLOAD file$[,Ddrive][,Uunit]` | Load a BASIC program from disk; from a running program it can chain into the newly loaded program. |
| `$F1` | `HEADER disk$[,Iid][,Ddrive][,ON Uunit]` | Format/initialize a disk. A full header erases its contents; omitting the ID on a previously formatted disk performs a quick directory reset. |
| `$F2` | `SCRATCH file$[,Ddrive][,Uunit]` | Ask for confirmation and delete the named disk file. |
| `$F3` | `COLLECT [Ddrive][,ON Uunit]` | Recover blocks left allocated by improperly closed disk files and repair the directory's free-space accounting. |
| `$F4` | `COPY [Dsrc,]"name" TO [Ddst,]"newname"[,ON Uunit]` | Copy/rename a file within or between drives; a drive-to-drive form copies all files. |
| `$F5` | `RENAME [Ddrive,]"source" TO "target"[,Uunit]` | Change a disk file's directory name. |
| `$F6` | `BACKUP Dsrc TO Ddst[,ON Uunit]` | Copy an entire disk, including its format, on a dual-drive unit; destination contents are destroyed. |
| `$F7` | `DELETE [first][-last]` | Delete one program line or an inclusive line range in direct mode. |
| `$F8` | `RENUMBER [new-first[,step[,first-line]]]` | Renumber all or part of the program in direct mode, starting with defaults 10/10/first line, and update referenced line numbers in branch statements. |
| `$F9` | `KEY [key-number,string-expression]` | With no arguments, show the eight function-key definitions; otherwise redefine key 1–8. All definitions together are limited to 128 characters. |
| `$FA` | `MONITOR` | Enter TEDMON; the initial display shows CPU register state. `X` returns to BASIC. See below. |
| `$FB` | `PRINT [#file,] USING format$;items...` | Format numeric/string output. `#` reserves a character position; other formatting characters include `+`, `-`, `.`, `,`, `$`, `^^^^`, `=`, and `>`. A numeric value too wide for its field appears as asterisks. `USING` is a clause token. |
| `$FC` | `UNTIL condition` | End a `DO`/`LOOP` iteration when the condition becomes true, at whichever end contains the test. |
| `$FD` | `WHILE condition` | Continue a `DO`/`LOOP` iteration while the condition is true, at whichever end contains the test. |

The Commodore manual describes `SOUND` durations as 1/60 second. For PAL hardware, the observed unit is 1/50 second; [the Plus/4 World SOUND reference](https://plus4world.powweb.com/plus4encyclopedia/500106) distinguishes PAL and NTSC. The PAL value is the relevant target for this emulator and should be verified with a timing probe during ROM replacement.

### Composite forms and reserved variables

- `GETKEY var[,var...]` is the token pair `GET` (`$A1`) + `KEY` (`$F9`), optionally written `GET KEY`. It waits for one keystroke per variable. Unlike `GET`, it never returns immediately with an empty result. Program mode only.
- `GO TO line` is `GO` (`$CB`) + `TO` (`$A4`); `PRINT USING` is `PRINT` (`$99`) + `USING` (`$FB`); `DEF FN...`, `ON ... GOTO/GOSUB`, `FOR ... TO ... STEP ... NEXT`, `IF ... THEN ... ELSE`, and `DO ... LOOP ... WHILE/UNTIL` are likewise compound grammar, not extra tokens.
- `ST` reports I/O status; `DS` and `DS$` report disk error number/text. `TI` counts 1/60-second software ticks, and `TI$` is the six-digit `hhmmss` clock string and can be assigned to set the clock. `ER` and `EL` hold the trapped BASIC error code and line. These special variable names are not in the keyword table. They are included because programs can read them.
- BASIC variables are floating point by default; `%` selects signed 16-bit integer storage and `$` selects strings. Only the first two characters of a variable name are significant. Arrays start at subscript zero. Program statements may be separated by `:`; direct mode executes without a stored line number. `?` is shorthand for `PRINT`.

## TEDMON entered by `MONITOR`

TEDMON is the monitor, small assembler, and disassembler reached by BASIC `MONITOR` or a suitable machine-code break. Its commands use **hexadecimal** numbers without BASIC's decimal conversion. The descriptions below are the user-visible interface in Commodore's [Plus/4 User's Manual, TEDMON section](https://plus4world.powweb.com/publications/Commodore_Plus4_Users_Manual), printed pp. 184–192. A command normally returns to the monitor prompt; `G` starts code and `X` returns to BASIC.

| Command | Form | Observable behavior or output |
|---|---|---|
| `A` or `.` | `A addr mnemonic [operand]` | Assemble one 6502-family instruction into memory and offer the next instruction address. Operands use monitor notation such as `#$20` or `$2000`; a malformed line gets `?`. |
| `C` | `C start end other-start` | Compare an inclusive source range with memory at `other-start`; print addresses where bytes differ; a matching range produces no differences. |
| `D` | `D [start [end]]` | Disassemble memory as addresses, machine bytes, mnemonics, and operands. With omitted bounds, display the next page of instructions. The output can be edited and re-entered. |
| `F` | `F start end byte` | Fill each location in the inclusive range with the one- or two-digit hex byte. |
| `G` | `G [addr]` | Restore the displayed registers and start executing at `addr`, or at the current PC if omitted. A program may return to TEDMON with `BRK`. |
| `H` | `H start end byte [byte...]` or `H start end 'text` | Hunt an inclusive memory range for a sequence of hex bytes or a quoted-by-apostrophe character sequence; print matching addresses. |
| `L` | `L "filename",device` | Load a program file from tape or disk at the load address in its first two bytes. Device numbers are hex (`1` cassette, `8` disk). |
| `M` | `M [start [end]]` | Dump memory as hex bytes and characters. Omitted addresses continue the previous dump/page. A displayed row can be edited and submitted. |
| `>` | `> addr byte [byte...]` | Write one to eight hex bytes into consecutive memory addresses; also used when editing an `M` line. |
| `R` | `R` | Display PC, status register (`SR`), accumulator (`AC`), X (`XR`), Y (`YR`), and stack pointer (`SP`). |
| `;` | `; PC SR AC XR YR SP` | Replace register values from an edited `R` display; fields to the right may be left alone by omitting them. |
| `S` | `S "filename",device,start,end-plus-one` | Save memory from `start` **up to but excluding** `end-plus-one` as a load-address program file. |
| `T` | `T start end destination` | Copy an inclusive memory range to `destination`, including overlapping transfers. |
| `V` | `V "filename",device` | Compare a cassette/disk program file with memory at its stored load address; show an error if bytes differ. |
| `X` | `X` | Exit TEDMON and return to BASIC. Modified register/stack state can affect the resumed BASIC environment. |

`$07F8` is a monitor view setting: zero makes `M`/`D` inspect the ROM mapped above `$8000`; `$80` selects the RAM underneath. This selection does not change what `G` executes. The RAM/ROM distinction, monitor screen output, device interaction, and error indications are part of the external behavior to test.

## Useful behavioral probes for a replacement ROM

These are examples of **observations**, not reference implementation code:

```basic
PRINT 2+3*4                 : REM arithmetic precedence; prints 14
FOR I=1 TO 3:PRINT I;:NEXT  : REM prints 1 2 3 without separate line endings
PRINT HEX$(255),DEC("00FF") : REM four-digit hex and inverse numeric conversion
GRAPHIC 2,1:LOCATE 160,100:DRAW 1,200,100:GRAPHIC 0
VOL 8:SOUND 1,770,50         : REM approximately one second on PAL
MONITOR                       : REM R displays registers; X returns to BASIC
```

For compatibility tests, verify both the resulting text/graphics/device state and the relevant errors, cursor positions, `ER`/`EL`, and `ST`/`DS` status where applicable. The ROM token inventory specifies **what syntax exists**, while the manual and black-box probes specify **what it does**. This document does not require copying the ROM's internal algorithms, byte layout, or bugs into a new implementation.
