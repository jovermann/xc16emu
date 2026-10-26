; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Independently written minimal Plus/4 system ROM. The fixed entry points at
; $C800 are the private interface used by basrom.asm. No original ROM machine
; code is used here.

        ASS $C000

screenlo EQU $20                 ; next screen cell, absolute address
screenhi EQU $21
msglo   EQU $22                 ; zero-terminated message pointer
msghi   EQU $23
keybits EQU $24                 ; pressed bits in the selected keyboard row
shifted EQU $25                 ; whether the Shift matrix position is held
keybit  EQU $26                 ; selected bit within a matrix row
column  EQU $27                 ; next text column, 0..39
celllo  EQU $28                 ; next matrix cell, 0..999
cellhi  EQU $29
linelen EQU $2a                 ; bytes in the current input line
lastkey EQU $2b                 ; edge detector for a held key
letter  EQU $2c                 ; character currently being printed
editpos EQU $2d                 ; cursor offset within the current input line
rowidx  EQU $2e                 ; physical cursor row, 0..24
srclo   EQU $1a                 ; screen-row pointer during line import
srchi   EQU $1b
startrow EQU $1c                ; first physical row of the logical line
copyrowidx EQU $1d             ; row being copied into the input buffer
textlen EQU $1e                 ; number of copied input bytes
rowcount EQU $1f                ; characters used by one screen row
input   EQU $1000               ; one direct-mode line, zero terminated
rowlink EQU $1140               ; one byte per row: continuation from above
storedline EQU $60               ; BASIC accepted a numbered source line

reset   SEI
        CLD
        LDX #$ff
        TXS
        LDA #$00
        STA $ff0a               ; no TED interrupts in this small ROM
        STA $ff0c               ; cursor position is configured below
        STA $ff0d
        STA column
        STA celllo
        STA cellhi
        STA linelen
        STA editpos
        STA rowidx
        LDX #24
.clearlinks
        STA rowlink,X
        DEX
        BPL .clearlinks
        JSR $8003               ; reset numbered program and variables
        LDA #$ff
        STA lastkey
        LDA #$00
        STA screenlo
        LDA #$0c
        STA screenhi

        ; Match the observed Plus/4 text colors and matrix layout.
        LDA #$1b
        STA $ff06               ; screen on, 25 text rows
        LDA #$88
        STA $ff07               ; 40 columns, all 256 glyph indices visible
        LDA #$08
        STA $ff14               ; color at $0800, text at $0c00
        LDA #$04
        STA $ff12               ; glyphs come from the system ROM
        LDA #$d0
        STA $ff13               ; independent glyphs begin at $d000
        LDA #$71
        STA $ff15               ; white background
        LDA #$5b
        STA $ff16
        LDA #$75
        STA $ff17
        LDA #$77
        STA $ff18
        LDA #$6e
        STA $ff19               ; original blue border
        LDX #$00
.clear   LDA #$10                ; dark foreground on white
        STA $0800,X
        STA $0900,X
        STA $0a00,X
        STA $0b00,X
        LDA #$20                ; screen-code space
        STA $0c00,X
        STA $0d00,X
        STA $0e00,X
        STA $0f00,X
        INX
        BNE .clear

        JSR newline             ; first screen row is blank
        JSR draw_title
        JSR newline
        JSR newline
        JSR ready
        JMP inputloop

; Print the boot banner using the BASIC ROM's available-capacity calculation.
draw_title
        LDA #banner%256
        STA msglo
        LDA #banner/256
        STA msghi
        JSR printz
        JSR $8006               ; compute free bytes from program pointers
        LDA #bannersuffix%256
        STA msglo
        LDA #bannersuffix/256
        STA msghi
        JSR printz
        RTS

; Poll one keyboard transition, echo it, and send complete lines to BASIC.
inputloop
        JSR scan
        CMP #$ff
        BNE .havekey
        STA lastkey
        JMP inputloop
.havekey CMP lastkey
        BNE .newkey
        JMP inputloop
.newkey  STA lastkey
        TAX
        CPX #48                 ; cursor left
        BNE .notleft
        JMP cursorleft
.notleft
        CPX #51                 ; cursor right
        BNE .notright
        JMP cursorright
.notright
        CPX #43                 ; cursor up moves to the prior screen row
        BNE .notup
        JMP cursorup
.notup
        CPX #40                 ; cursor down moves to the next screen row
        BNE .notdown
        JMP cursordown
.notdown
        CPX #3                  ; DEL clears the body, retaining line number
        BNE .notdelete
        JMP clearbody
.notdelete
        JSR readshift           ; key press may begin halfway through a scan
        LDY shifted
        BEQ .keyunshifted
        LDA shiftedchars,X
        BNE .keyplain
.keyunshifted
        LDA keychars,X
        BEQ inputloop
.keyplain
        CMP #13
        BEQ .entered
        CMP #8
        BEQ .erasekey
        STA letter
        LDA editpos
        CMP #79
        BCS inputloop
        LDX linelen
.pad_to_cursor
        CPX editpos
        BCS .padded
        LDA #32
        STA input,X
        INX
        STX linelen
        JMP .pad_to_cursor
.padded
        LDX linelen
        CPX #79
        BCS inputloop
.insertshift
        CPX editpos
        BEQ .insertchar
        DEX
        LDA input,X
        STA input+1,X
        JMP .insertshift
.insertchar
        LDX editpos
        LDA letter
        STA input,X
        INC linelen
        INC editpos
        JSR redraw_insert
        JMP inputloop
.erasekey
        LDA editpos
        BEQ .delete_under
        JSR retreat
        DEC editpos
.delete_under
        LDA editpos
        CMP linelen
        BCC .delete_now
        JMP inputloop
.delete_now
        LDX editpos
.deleteshift
        INX
        LDA input,X
        DEX
        STA input,X
        INX
        CPX linelen
        BNE .deleteshift
        DEC linelen
        JSR redraw_delete
        JMP inputloop
.entered LDX linelen
        LDA #$00
        STA input,X
        JSR endcursor
        JSR newline
        JSR $8000               ; one direct-mode BASIC statement list
        LDA storedline
        BNE .entered_done        ; storing a line leaves a blank input row
        JSR ready
.entered_done
        LDA #$00
        STA linelen
        STA editpos
        JMP inputloop

; Move the input cursor left without changing the character underneath.
cursorleft
        LDA editpos
        BNE .leftmove
        JMP inputloop
.leftmove
        DEC editpos
        JSR retreat
        JMP inputloop

; Move right only over characters already present in the input buffer.
cursorright
        LDA editpos
        CMP #79
        BCC .rightmove
        JMP inputloop
.rightmove
        INC editpos
        JSR advance
        JMP inputloop

; Move one physical row up and import that row's linked logical line.
cursorup
        LDA rowidx
        BNE .upmove
        JMP inputloop
.upmove
        DEC rowidx
        SEC
        LDA celllo
        SBC #40
        STA celllo
        LDA cellhi
        SBC #0
        STA cellhi
        SEC
        LDA screenlo
        SBC #40
        STA screenlo
        LDA screenhi
        SBC #0
        STA screenhi
        JSR sync_cursor
        JSR load_line
        JMP inputloop

; Move one physical row down and import that row's linked logical line.
cursordown
        LDA rowidx
        CMP #24
        BCC .downmove
        JMP inputloop
.downmove
        INC rowidx
        CLC
        LDA celllo
        ADC #40
        STA celllo
        LDA cellhi
        ADC #0
        STA cellhi
        CLC
        LDA screenlo
        ADC #40
        STA screenlo
        LDA screenhi
        ADC #0
        STA screenhi
        JSR sync_cursor
        JSR load_line
        JMP inputloop

; Clear the line body. Keeping its number lets Return delete that program line.
clearbody
        JSR endcursor
        LDX #0
.bodydigits
        LDA input,X
        CMP #48
        BCC .bodyclear
        CMP #58
        BCS .bodyclear
        INX
        CPX linelen
        BNE .bodydigits
.bodyclear
        STX keybit              ; temporary count of preserved digits
.bodymore
        LDA linelen
        CMP keybit
        BEQ .bodydone
        JSR retreat
        DEC linelen
        DEC editpos
        LDY #0
        LDA #32
        STA (screenlo),Y
        JMP .bodymore
.bodydone
        LDX linelen
        LDA #0
        STA input,X
        JMP inputloop

; Move to the end of the current line before submitting it.
endcursor
        LDA editpos
        CMP linelen
        BEQ .enddone
        BCC .endforward
        SEC
        SBC linelen
        TAX
        JSR retreat_count
        LDA linelen
        STA editpos
        RTS
.endforward
        INC editpos
        JSR advance
        JMP endcursor
.enddone
        RTS

; Follow screen-row continuation links and import the selected logical line.
; This lets an old LIST row be edited in place and submitted with Return.
load_line
        LDA rowidx
        STA startrow
.line_start
        LDX startrow
        BEQ .start_found
        LDA rowlink,X
        BEQ .start_found
        DEC startrow
        JMP .line_start
.start_found
        LDA #0
        STA textlen
        LDA startrow
        STA copyrowidx
.import_row
        LDX copyrowidx
        LDA rowlo,X
        STA srclo
        LDA rowhi,X
        STA srchi
        LDA #40
        STA rowcount
        INX
        CPX #25
        BCS .trim_row
        LDA rowlink,X
        BNE .import_cells
.trim_row
        LDY #39
.find_tail
        LDA (srclo),Y
        AND #$7f
        CMP #32
        BNE .tail_found
        DEY
        BPL .find_tail
        LDA #0
        STA rowcount
        BEQ .import_cells
.tail_found
        TYA
        CLC
        ADC #1
        STA rowcount
.import_cells
        LDY #0
.import_byte
        CPY rowcount
        BEQ .row_imported
        LDA (srclo),Y
        AND #$7f
        CMP #32
        BCS .imported_ascii
        CLC
        ADC #64                 ; TED screen codes 0..31 to ASCII
.imported_ascii
        LDX textlen
        STA input,X
        INC textlen
        LDA textlen
        CMP #79
        BEQ .import_done
        INY
        JMP .import_byte
.row_imported
        LDX copyrowidx
        INX
        CPX #25
        BCS .import_done
        LDA rowlink,X
        BEQ .import_done
        STX copyrowidx
        JMP .import_row
.import_done
        LDX textlen
        STX linelen
        LDA #0
        STA input,X
        LDA rowidx
        SEC
        SBC startrow
        BEQ .same_row
        CMP #1
        BNE .beyond_row
        LDA column
        CLC
        ADC #40
        JMP .clamp_edit
.same_row
        LDA column
        JMP .clamp_edit
.beyond_row
        LDA #79
.clamp_edit
        STA editpos
        RTS

; Redraw the inserted character and the tail, then restore the edit cursor.
redraw_insert
        LDX editpos
        DEX
        JSR redraw_tail
        LDA linelen
        SEC
        SBC editpos
        TAX
        JMP retreat_count

; Redraw the remaining tail and blank the vacated rightmost cell.
redraw_delete
        LDX editpos
        JSR redraw_tail
        LDA #32
        JSR putchar
        LDA linelen
        SEC
        SBC editpos
        CLC
        ADC #1
        TAX
        JMP retreat_count

; Print input bytes from X through the end without changing editpos.
redraw_tail
        CPX linelen
        BEQ .taildone
        LDA input,X
        JSR putchar
        INX
        JMP redraw_tail
.taildone
        RTS

; Move the display cursor back X cells while preserving the input buffer.
retreat_count
        CPX #0
        BEQ .retreat_done
.retreat_next
        JSR retreat
        DEX
        BNE .retreat_next
.retreat_done
        RTS

; Decrement screen pointer, cell index, and column without erasing a glyph.
retreat
        LDA screenlo
        BNE .backscreen
        DEC screenhi
.backscreen
        DEC screenlo
        LDA celllo
        BNE .backcell
        DEC cellhi
.backcell
        DEC celllo
        LDA column
        BNE .backcolumn
        DEC rowidx
        LDA #40
        STA column
.backcolumn
        DEC column
        JMP sync_cursor

; Scan the active-low 8x8 keyboard matrix and return one key index or $ff.
; The Shift key is remembered separately and is not returned as a character.
scan    LDA #$00
        STA shifted
        LDX #$00
.scanrow LDA rows,X
        STA $fd30
        LDA #$ff
        STA $ff08
        LDA $ff08
        EOR #$ff
        STA keybits
        CPX #$01
        BNE .scanbits
        AND #$80
        BEQ .scanbits
        LDA #$01
        STA shifted
        LDA keybits
        AND #$7f
        STA keybits
.scanbits
        LDA keybits
        BEQ .nextrow
        LDY #$00
.findbit LSR
        BCS .foundbit
        INY
        CPY #$08
        BNE .findbit
.nextrow INX
        CPX #$08
        BNE .scanrow
        LDA #$ff
        RTS
.foundbit
        STY keybit
        TXA
        ASL
        ASL
        ASL
        CLC
        ADC keybit
        RTS

; Sample Shift at the moment a new key is decoded, independent of scan phase.
readshift
        LDA #$fd
        STA $fd30
        LDA #$ff
        STA $ff08
        LDA $ff08
        AND #$80
        BNE .unshifted
        LDA #$01
        STA shifted
        RTS
.unshifted
        LDA #$00
        STA shifted
        RTS

; Return to a fresh prompt. A pending semicolon PRINT result gets a newline.
ready   LDA column
        BEQ .readyl
        JSR newline
.readyl  LDA #readymsg%256
        STA msglo
        LDA #readymsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

; Output ASCII A as a TED screen code and return the original byte in A.
; Capitals use native codes 1..26 and lowercase uses the second set.
; Extra ASCII punctuation uses duplicate glyph slots in that set.
; The public entry at $C800 preserves X.
putchar STA letter
        CMP #13
        BEQ .putnewline
        CMP #65
        BCC .other_ascii
        CMP #91
        BCS .after_capital
        SEC
        SBC #64
        JMP .writecode
.after_capital
        CMP #97
        BCC .other_ascii
        CMP #123
        BCS .other_ascii
        SEC
        SBC #96
        ORA #$80                ; second-set lowercase at $81..$9a
        JMP .writecode
.other_ascii
        CMP #94
        BEQ .caret
        CMP #95
        BEQ .underscore
        CMP #64
        BCC .writecode
        ORA #$80
        JMP .writecode
.caret
        LDA #$9e
        JMP .writecode
.underscore
        LDA #$9f
.writecode
        LDY #$00
        STA (screenlo),Y
        JSR advance
        LDA letter
        RTS
.putnewline
        JSR newline
        LDA letter
        RTS

; Output the zero-terminated text at msglo:msghi.
printz  LDY #$00
.printzlp
        LDA (msglo),Y
        BEQ .printdone
        JSR putchar
        INC msglo
        BNE .printzlp
        INC msghi
        JMP .printzlp
.printdone
        RTS

; Move to the next text row, scrolling when the bottom row is full.
newline LDA #40
        SEC
        SBC column
        TAX
.newlinelp
        JSR advance
        DEX
        BNE .newlinelp
        TXA
        PHA
        LDX rowidx
        LDA #0
        STA rowlink,X           ; explicit newline starts a new logical line
        PLA
        TAX
        RTS

; Advance one matrix cell and update TED's blinking text cursor.
advance INC screenlo
        BNE .advcell
        INC screenhi
.advcell INC celllo
        BNE .advcol
        INC cellhi
.advcol  INC column
        LDA column
        CMP #40
        BNE .advwrap
        LDA #$00
        STA column
        INC rowidx
        TXA
        PHA
        LDX rowidx
        CPX #25
        BCS .adv_nomark
        LDA #1
        STA rowlink,X           ; automatic wrap links adjacent screen rows
.adv_nomark
        PLA
        TAX
.advwrap LDA cellhi
        CMP #$03
        BCC cursor
        BNE .wrap
        LDA celllo
        CMP #$e8
        BCC cursor
.wrap    TXA
        PHA
        JSR scrollscreen
        PLA
        TAX
        LDA #$c0               ; row 24 starts at cell 960
        STA celllo
        STA screenlo
        LDA #$03
        STA cellhi
        LDA #$0f
        STA screenhi
        LDA #24
        STA rowidx
sync_cursor
cursor  LDA cellhi
        STA $ff0c
        LDA celllo
        STA $ff0d
        RTS

; Move rows 1..24 over rows 0..23, including their color attributes.
; Copying forward is safe because the destination precedes the source.
scrollscreen
        LDX #0
.scrollpages
        LDA $0c28,X
        STA $0c00,X
        LDA $0d28,X
        STA $0d00,X
        LDA $0e28,X
        STA $0e00,X
        LDA $0828,X
        STA $0800,X
        LDA $0928,X
        STA $0900,X
        LDA $0a28,X
        STA $0a00,X
        INX
        BNE .scrollpages
        LDX #0
.scrolltail
        LDA $0f28,X
        STA $0f00,X
        LDA $0b28,X
        STA $0b00,X
        INX
        CPX #$c0               ; 192 cells after the first 768
        BNE .scrolltail
        LDX #0
.scrollblank
        LDA #32
        STA $0fc0,X
        LDA #$10
        STA $0bc0,X
        INX
        CPX #40
        BNE .scrollblank
        LDX #0
.scrolllinks
        LDA rowlink+1,X
        STA rowlink,X
        INX
        CPX #24
        BNE .scrolllinks
        LDA #1
        STA rowlink+24
        RTS

banner  BYT "FREE BASIC V0.1 ",0
bannersuffix BYT " BYTES FREE",0
readymsg BYT "READY.",0
rows    BYT $fe,$fd,$fb,$f7,$ef,$df,$bf,$7f
rowlo   BYT $00,$28,$50,$78,$a0,$c8,$f0,$18,$40,$68,$90,$b8,$e0
        BYT $08,$30,$58,$80,$a8,$d0,$f8,$20,$48,$70,$98,$c0
rowhi   BYT $0c,$0c,$0c,$0c,$0c,$0c,$0c,$0d,$0d,$0d,$0d,$0d,$0d
        BYT $0e,$0e,$0e,$0e,$0e,$0e,$0e,$0f,$0f,$0f,$0f,$0f

; Host key indexes are a hardware interface, not copied ROM key tables.
; Zero means the key has no printable meaning in this initial subset.
keychars
        BYT 8,13,0,0,0,0,0,0
        BYT $33,$57,$41,$34,$5a,$53,$45,0
        BYT $35,$52,$44,$36,$43,$46,$54,$58
        BYT $37,$59,$47,$38,$42,$48,$55,$56
        BYT $39,$49,$4a,$30,$4d,$4b,$4f,$4e
        BYT 0,$50,$4c,0,$2e,$3a,$2d,$2c
        BYT 0,$2a,$3b,0,0,$3d,$2b,$2f
        BYT $31,0,0,$32,$20,0,$51,0

; Printable shifted symbols are expressed as character codes, independent
; of a host keyboard layout. Zero falls back to the unshifted table above.
shiftedchars
        BYT 0,0,0,0,0,0,0,0
        BYT 35,123,0,36,0,0,125,0
        BYT 37,124,0,38,0,0,126,0
        BYT 39,0,0,40,0,0,0,0
        BYT 41,0,0,64,0,0,0,0
        BYT 0,0,0,0,62,91,95,60
        BYT 0,96,93,0,0,92,94,63
        BYT 33,0,0,34,0,0,0,0

; The BASIC ROM calls only these stable, three-byte entry points.
        ORG $C800
api_char JMP putchar
api_text JMP printz
api_line JMP newline

; The following 2 KiB holds angular, bold letterforms and C16 graphic symbols.
; It covers both original 128-glyph sets. Native line and block graphics keep
; the geometry needed to join across character cells. Duplicate slots in the
; second set also show extended ASCII punctuation accepted by this editor.
; Each quoted pixel row contains eight positions (# is set, . is clear).
; The original column shows the corresponding reference-system-ROM byte.
        ORG $D000
glyphs
; Screen code $00 (@)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##.###." ; original %".##.###."
        BYT %".##.#.#." ; original %".##.###."
        BYT %".##.###." ; original %".##....."
        BYT %".##....." ; original %".##...#."
        BYT %"..#####." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $01 (A)
        BYT %"..####.." ; original %"...##..."
        BYT %".##..##." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".######." ; original %".######."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $02 (B)
        BYT %".#####.." ; original %".#####.."
        BYT %".##...#." ; original %".##..##."
        BYT %".##...#." ; original %".##..##."
        BYT %".#####.." ; original %".#####.."
        BYT %".##...#." ; original %".##..##."
        BYT %".##...#." ; original %".##..##."
        BYT %".#####.." ; original %".#####.."
        BYT %"........" ; original %"........"
; Screen code $03 (C)
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##..##."
        BYT %"##......" ; original %".##....."
        BYT %"##......" ; original %".##....."
        BYT %"##......" ; original %".##....."
        BYT %".##....." ; original %".##..##."
        BYT %"..#####." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $04 (D)
        BYT %".#####.." ; original %".####..."
        BYT %".##..##." ; original %".##.##.."
        BYT %".##...#." ; original %".##..##."
        BYT %".##...#." ; original %".##..##."
        BYT %".##...#." ; original %".##..##."
        BYT %".##..##." ; original %".##.##.."
        BYT %".#####.." ; original %".####..."
        BYT %"........" ; original %"........"
; Screen code $05 (E)
        BYT %".######." ; original %".######."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".#####.." ; original %".####..."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $06 (F)
        BYT %".######." ; original %".######."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".#####.." ; original %".####..."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %"........" ; original %"........"
; Screen code $07 (G)
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##..##."
        BYT %"##......" ; original %".##....."
        BYT %"##..###." ; original %".##.###."
        BYT %"##...##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $08 (H)
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".######." ; original %".######."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $09 (I)
        BYT %".######." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %".######." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $0A (J)
        BYT %"..#####." ; original %"...####."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %".##.##.." ; original %"....##.."
        BYT %".##.##.." ; original %".##.##.."
        BYT %"..###..." ; original %"..###..."
        BYT %"........" ; original %"........"
; Screen code $0B (K)
        BYT %".##..##." ; original %".##..##."
        BYT %".##.##.." ; original %".##.##.."
        BYT %".####..." ; original %".####..."
        BYT %".###...." ; original %".###...."
        BYT %".####..." ; original %".####..."
        BYT %".##.##.." ; original %".##.##.."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $0C (L)
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $0D (M)
        BYT %"##....##" ; original %".##...##"
        BYT %"###..###" ; original %".###.###"
        BYT %"########" ; original %".#######"
        BYT %"##.##.##" ; original %".##.#.##"
        BYT %"##....##" ; original %".##...##"
        BYT %"##....##" ; original %".##...##"
        BYT %"##....##" ; original %".##...##"
        BYT %"........" ; original %"........"
; Screen code $0E (N)
        BYT %"##....##" ; original %".##..##."
        BYT %"###...##" ; original %".###.##."
        BYT %"####..##" ; original %".######."
        BYT %"##.##.##" ; original %".######."
        BYT %"##..####" ; original %".##.###."
        BYT %"##...###" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $0F (O)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $10 (P)
        BYT %".#####.." ; original %".#####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".#####.." ; original %".#####.."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %"........" ; original %"........"
; Screen code $11 (Q)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##..#.##" ; original %".##..##."
        BYT %".##..##." ; original %"..####.."
        BYT %"..#####." ; original %"....###."
        BYT %"........" ; original %"........"
; Screen code $12 (R)
        BYT %".#####.." ; original %".#####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".#####.." ; original %".#####.."
        BYT %".####..." ; original %".####..."
        BYT %".##.##.." ; original %".##.##.."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $13 (S)
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##..##."
        BYT %"##......" ; original %".##....."
        BYT %"..####.." ; original %"..####.."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".##..##."
        BYT %".#####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $14 (T)
        BYT %"########" ; original %".######."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $15 (U)
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $16 (V)
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $17 (W)
        BYT %"##....##" ; original %".##...##"
        BYT %"##....##" ; original %".##...##"
        BYT %"##....##" ; original %".##...##"
        BYT %"##.##.##" ; original %".##.#.##"
        BYT %"########" ; original %".#######"
        BYT %"###..###" ; original %".###.###"
        BYT %"##....##" ; original %".##...##"
        BYT %"........" ; original %"........"
; Screen code $18 (X)
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $19 (Y)
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %".##..##."
        BYT %"...##..." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $1A (Z)
        BYT %".######." ; original %".######."
        BYT %".....##." ; original %".....##."
        BYT %"....##.." ; original %"....##.."
        BYT %"...##..." ; original %"...##..."
        BYT %"..##...." ; original %"..##...."
        BYT %".##....." ; original %".##....."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $1B ([)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %"..##...."
        BYT %".##....." ; original %"..##...."
        BYT %".#####.." ; original %"..##...."
        BYT %".##....." ; original %"..##...."
        BYT %".##..##." ; original %"..##...."
        BYT %".######." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $1C (\)
        BYT %"...##..." ; original %"....##.."
        BYT %"..####.." ; original %"...#..#."
        BYT %".##..##." ; original %"..##...."
        BYT %"...##..." ; original %".#####.."
        BYT %"...##..." ; original %"..##...."
        BYT %"...##..." ; original %".##...#."
        BYT %"...##..." ; original %"######.."
        BYT %"........" ; original %"........"
; Screen code $1D (])
        BYT %"...##..." ; original %"..####.."
        BYT %"..##...." ; original %"....##.."
        BYT %".##....." ; original %"....##.."
        BYT %"########" ; original %"....##.."
        BYT %".##....." ; original %"....##.."
        BYT %"..##...." ; original %"....##.."
        BYT %"...##..." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $1E (^)
        BYT %"..####.." ; original %"........"
        BYT %".##..##." ; original %"...##..."
        BYT %".....##." ; original %"..####.."
        BYT %"...###.." ; original %".######."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"...##..."
; Screen code $1F (_)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"...#...."
        BYT %"........" ; original %"..##...."
        BYT %"........" ; original %".#######"
        BYT %"........" ; original %".#######"
        BYT %"........" ; original %"..##...."
        BYT %"########" ; original %"...#...."
        BYT %"........" ; original %"........"
; Screen code $20 (space)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $21 (!)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $22 (")
        BYT %"..#.#..." ; original %".##..##."
        BYT %"..#.#..." ; original %".##..##."
        BYT %"..#.#..." ; original %".##..##."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $23 (#)
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"########" ; original %"########"
        BYT %".##..##." ; original %".##..##."
        BYT %"########" ; original %"########"
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $24 ($)
        BYT %"...#...." ; original %"...##..."
        BYT %"..####.." ; original %"..#####."
        BYT %".#.#...." ; original %".##....."
        BYT %"..###..." ; original %"..####.."
        BYT %"...#.#.." ; original %".....##."
        BYT %".####..." ; original %".#####.."
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $25 (%)
        BYT %".##..#.." ; original %".##...#."
        BYT %".##.#..." ; original %".##..##."
        BYT %"....#..." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %"..#.##.." ; original %".##..##."
        BYT %".#..##.." ; original %".#...##."
        BYT %"........" ; original %"........"
; Screen code $26 (&)
        BYT %"..##...." ; original %"..####.."
        BYT %".#..#..." ; original %".##..##."
        BYT %".#.#...." ; original %"..####.."
        BYT %"..#....." ; original %"..###..."
        BYT %".#.#.#.." ; original %".##..###"
        BYT %".#..#..." ; original %".##..##."
        BYT %"..##.#.." ; original %"..######"
        BYT %"........" ; original %"........"
; Screen code $27 (')
        BYT %"...#...." ; original %".....##."
        BYT %"...#...." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $28 (()
        BYT %"....#..." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %"..#....." ; original %"..##...."
        BYT %"..#....." ; original %"..##...."
        BYT %"...#...." ; original %"...##..."
        BYT %"....#..." ; original %"....##.."
        BYT %"........" ; original %"........"
; Screen code $29 ())
        BYT %"..#....." ; original %"..##...."
        BYT %"...#...." ; original %"...##..."
        BYT %"....#..." ; original %"....##.."
        BYT %"....#..." ; original %"....##.."
        BYT %"....#..." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %"........" ; original %"........"
; Screen code $2A (*)
        BYT %"........" ; original %"........"
        BYT %".#.#.#.." ; original %".##..##."
        BYT %"..###..." ; original %"..####.."
        BYT %".#####.." ; original %"########"
        BYT %"..###..." ; original %"..####.."
        BYT %".#.#.#.." ; original %".##..##."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $2B (+)
        BYT %"........" ; original %"........"
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %".######." ; original %".######."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $2C (,)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"...##..."
        BYT %"........" ; original %"..##...."
; Screen code $2D (-)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $2E (.)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"..##...." ; original %"...##..."
        BYT %"..##...." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $2F (/)
        BYT %".....#.." ; original %"........"
        BYT %"....#..." ; original %"......##"
        BYT %"....#..." ; original %".....##."
        BYT %"...#...." ; original %"....##.."
        BYT %"..#....." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %".#......" ; original %".##....."
        BYT %"........" ; original %"........"
; Screen code $30 (0)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".#....#." ; original %".##.###."
        BYT %".#..#.#." ; original %".###.##."
        BYT %".#.#..#." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $31 (1)
        BYT %"...##..." ; original %"...##..."
        BYT %"..###..." ; original %"...##..."
        BYT %"...##..." ; original %"..###..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $32 (2)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".....##." ; original %".....##."
        BYT %"...###.." ; original %"....##.."
        BYT %"..##...." ; original %"..##...."
        BYT %".##....." ; original %".##....."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $33 (3)
        BYT %".#####.." ; original %"..####.."
        BYT %".....##." ; original %".##..##."
        BYT %".....##." ; original %".....##."
        BYT %"...####." ; original %"...###.."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".##..##."
        BYT %".#####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $34 (4)
        BYT %"....##.." ; original %".....##."
        BYT %"...###.." ; original %"....###."
        BYT %"..####.." ; original %"...####."
        BYT %".##.##.." ; original %".##..##."
        BYT %"########" ; original %".#######"
        BYT %"....##.." ; original %".....##."
        BYT %"....##.." ; original %".....##."
        BYT %"........" ; original %"........"
; Screen code $35 (5)
        BYT %".######." ; original %".######."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".#####.."
        BYT %".#####.." ; original %".....##."
        BYT %".....##." ; original %".....##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $36 (6)
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##..##."
        BYT %"##......" ; original %".##....."
        BYT %"######.." ; original %".#####.."
        BYT %"##...##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $37 (7)
        BYT %".######." ; original %".######."
        BYT %".....##." ; original %".##..##."
        BYT %"....##.." ; original %"....##.."
        BYT %"...##..." ; original %"...##..."
        BYT %"..##...." ; original %"...##..."
        BYT %"..##...." ; original %"...##..."
        BYT %"..##...." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $38 (8)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $39 (9)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##...##" ; original %".##..##."
        BYT %"..######" ; original %"..#####."
        BYT %"......##" ; original %".....##."
        BYT %".....##." ; original %".##..##."
        BYT %".#####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $3A (:)
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $3B (;)
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"...##..."
        BYT %"........" ; original %"..##...."
; Screen code $3C (<)
        BYT %"....#..." ; original %"....###."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %".#......" ; original %".##....."
        BYT %"..#....." ; original %"..##...."
        BYT %"...#...." ; original %"...##..."
        BYT %"....#..." ; original %"....###."
        BYT %"........" ; original %"........"
; Screen code $3D (=)
        BYT %"........" ; original %"........"
        BYT %".#####.." ; original %"........"
        BYT %"........" ; original %".######."
        BYT %".#####.." ; original %"........"
        BYT %"........" ; original %".######."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $3E (>)
        BYT %"..#....." ; original %".###...."
        BYT %"...#...." ; original %"...##..."
        BYT %"....#..." ; original %"....##.."
        BYT %".....#.." ; original %".....##."
        BYT %"....#..." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %".###...."
        BYT %"........" ; original %"........"
; Screen code $3F (?)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".....##." ; original %".....##."
        BYT %"...###.." ; original %"....##.."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $40 (@)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $41 (A)
        BYT %"...##..." ; original %"....#..."
        BYT %"..####.." ; original %"...###.."
        BYT %".######." ; original %"..#####."
        BYT %"########" ; original %".#######"
        BYT %"########" ; original %".#######"
        BYT %"..####.." ; original %"...###.."
        BYT %".######." ; original %"..#####."
        BYT %"........" ; original %"........"
; Screen code $42 (B)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $43 (C)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $44 (D)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $45 (E)
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $46 (F)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $47 (G)
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
; Screen code $48 (H)
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
; Screen code $49 (I)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"###....." ; original %"###....."
        BYT %"####...." ; original %"####...."
        BYT %"..###..." ; original %"..###..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $4A (J)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...###.." ; original %"...###.."
        BYT %"....####" ; original %"....####"
        BYT %".....###" ; original %".....###"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $4B (K)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"..###..." ; original %"..###..."
        BYT %"####...." ; original %"####...."
        BYT %"###....." ; original %"###....."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $4C (L)
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
; Screen code $4D (M)
        BYT %"##......" ; original %"##......"
        BYT %"###....." ; original %"###....."
        BYT %".###...." ; original %".###...."
        BYT %"..###..." ; original %"..###..."
        BYT %"...###.." ; original %"...###.."
        BYT %"....###." ; original %"....###."
        BYT %".....###" ; original %".....###"
        BYT %"......##" ; original %"......##"
; Screen code $4E (N)
        BYT %"......##" ; original %"......##"
        BYT %".....###" ; original %".....###"
        BYT %"....###." ; original %"....###."
        BYT %"...###.." ; original %"...###.."
        BYT %"..###..." ; original %"..###..."
        BYT %".###...." ; original %".###...."
        BYT %"###....." ; original %"###....."
        BYT %"##......" ; original %"##......"
; Screen code $4F (O)
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
; Screen code $50 (P)
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
; Screen code $51 (Q)
        BYT %"........" ; original %"........"
        BYT %"..####.." ; original %"..####.."
        BYT %".######." ; original %".######."
        BYT %".######." ; original %".######."
        BYT %".######." ; original %".######."
        BYT %"..####.." ; original %".######."
        BYT %"........" ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $52 (R)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
; Screen code $53 (S)
        BYT %"........" ; original %"..##.##."
        BYT %".##.##.." ; original %".#######"
        BYT %"########" ; original %".#######"
        BYT %"########" ; original %".#######"
        BYT %".######." ; original %"..#####."
        BYT %"..####.." ; original %"...###.."
        BYT %"...##..." ; original %"....#..."
        BYT %"........" ; original %"........"
; Screen code $54 (T)
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
; Screen code $55 (U)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %".....###" ; original %".....###"
        BYT %"....####" ; original %"....####"
        BYT %"...###.." ; original %"...###.."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $56 (V)
        BYT %"##....##" ; original %"##....##"
        BYT %"###..###" ; original %"###..###"
        BYT %".######." ; original %".######."
        BYT %"..####.." ; original %"..####.."
        BYT %"..####.." ; original %"..####.."
        BYT %".######." ; original %".######."
        BYT %"###..###" ; original %"###..###"
        BYT %"........" ; original %"##....##"
; Screen code $57 (W)
        BYT %"........" ; original %"........"
        BYT %"..####.." ; original %"..####.."
        BYT %".######." ; original %".######."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".######." ; original %".######."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $58 (X)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $59 (Y)
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".....##."
; Screen code $5A (Z)
        BYT %"...##..." ; original %"....#..."
        BYT %"..####.." ; original %"...###.."
        BYT %".######." ; original %"..#####."
        BYT %"########" ; original %".#######"
        BYT %".######." ; original %"..#####."
        BYT %"..####.." ; original %"...###.."
        BYT %"...##..." ; original %"....#..."
        BYT %"........" ; original %"........"
; Screen code $5B ([)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $5C (\)
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
; Screen code $5D (])
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $5E (^)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"......##" ; original %"......##"
        BYT %"..#####." ; original %"..#####."
        BYT %".###.##." ; original %".###.##."
        BYT %"..##.##." ; original %"..##.##."
        BYT %"..##.##." ; original %"..##.##."
        BYT %"........" ; original %"........"
; Screen code $5F (_)
        BYT %"########" ; original %"########"
        BYT %".#######" ; original %".#######"
        BYT %"..######" ; original %"..######"
        BYT %"...#####" ; original %"...#####"
        BYT %"....####" ; original %"....####"
        BYT %".....###" ; original %".....###"
        BYT %"......##" ; original %"......##"
        BYT %".......#" ; original %".......#"
; Screen code $60 (`)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $61 (a)
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
; Screen code $62 (b)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
; Screen code $63 (c)
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $64 (d)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
; Screen code $65 (e)
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
; Screen code $66 (f)
        BYT %"##..##.." ; original %"##..##.."
        BYT %"##..##.." ; original %"##..##.."
        BYT %"..##..##" ; original %"..##..##"
        BYT %"..##..##" ; original %"..##..##"
        BYT %"##..##.." ; original %"##..##.."
        BYT %"##..##.." ; original %"##..##.."
        BYT %"..##..##" ; original %"..##..##"
        BYT %"..##..##" ; original %"..##..##"
; Screen code $67 (g)
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
; Screen code $68 (h)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"##..##.." ; original %"##..##.."
        BYT %"##..##.." ; original %"##..##.."
        BYT %"..##..##" ; original %"..##..##"
        BYT %"..##..##" ; original %"..##..##"
; Screen code $69 (i)
        BYT %"########" ; original %"########"
        BYT %"#######." ; original %"#######."
        BYT %"######.." ; original %"######.."
        BYT %"#####..." ; original %"#####..."
        BYT %"####...." ; original %"####...."
        BYT %"###....." ; original %"###....."
        BYT %"##......" ; original %"##......"
        BYT %"#......." ; original %"#......."
; Screen code $6A (j)
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
; Screen code $6B (k)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...#####" ; original %"...#####"
        BYT %"...#####" ; original %"...#####"
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $6C (l)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
; Screen code $6D (m)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...#####" ; original %"...#####"
        BYT %"...#####" ; original %"...#####"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $6E (n)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"#####..." ; original %"#####..."
        BYT %"#####..." ; original %"#####..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $6F (o)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
; Screen code $70 (p)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"...#####" ; original %"...#####"
        BYT %"...#####" ; original %"...#####"
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $71 (q)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $72 (r)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $73 (s)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"#####..." ; original %"#####..."
        BYT %"#####..." ; original %"#####..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $74 (t)
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
; Screen code $75 (u)
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
; Screen code $76 (v)
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
; Screen code $77 (w)
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $78 (x)
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $79 (y)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
; Screen code $7A (z)
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
; Screen code $7B ({)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
; Screen code $7C (|)
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $7D (})
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"#####..." ; original %"#####..."
        BYT %"#####..." ; original %"#####..."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $7E (~)
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $7F (blank in free ROM)
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
; Screen code $80 (blank in free ROM)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##.###." ; original %".##.###."
        BYT %".##.#.#." ; original %".##.###."
        BYT %".##.###." ; original %".##....."
        BYT %".##....." ; original %".##...#."
        BYT %"..#####." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $81 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"..####.." ; original %"..####.."
        BYT %".....##." ; original %".....##."
        BYT %"..#####." ; original %"..#####."
        BYT %".##..##." ; original %".##..##."
        BYT %"..#####." ; original %"..#####."
        BYT %"........" ; original %"........"
; Screen code $82 (blank in free ROM)
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".#####.." ; original %".#####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".#####.." ; original %".#####.."
        BYT %"........" ; original %"........"
; Screen code $83 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##..##."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##..##."
        BYT %"..#####." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $84 (blank in free ROM)
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".....##."
        BYT %"..#####." ; original %"..#####."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..#####." ; original %"..#####."
        BYT %"........" ; original %"........"
; Screen code $85 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".######." ; original %".######."
        BYT %".##....." ; original %".##....."
        BYT %"..#####." ; original %"..#####."
        BYT %"........" ; original %"........"
; Screen code $86 (blank in free ROM)
        BYT %"...####." ; original %"...###.."
        BYT %"..##...." ; original %"..##.##."
        BYT %"..##...." ; original %"..##...."
        BYT %".#####.." ; original %".####..."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"........" ; original %"........"
; Screen code $87 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"..#####." ; original %"........"
        BYT %".##..##." ; original %"..#####."
        BYT %".##..##." ; original %".##..##."
        BYT %"..#####." ; original %".##..##."
        BYT %".....##." ; original %"..#####."
        BYT %".#####.." ; original %".....##."
        BYT %"........" ; original %".#####.."
; Screen code $88 (blank in free ROM)
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".#####.." ; original %".#####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $89 (blank in free ROM)
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"..###..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"..####.." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $8A (blank in free ROM)
        BYT %"....##.." ; original %".....##."
        BYT %"........" ; original %"........"
        BYT %"...###.." ; original %".....##."
        BYT %"....##.." ; original %".....##."
        BYT %"....##.." ; original %".....##."
        BYT %".##.##.." ; original %".....##."
        BYT %"..###..." ; original %".##..##."
        BYT %"........" ; original %"..####.."
; Screen code $8B (blank in free ROM)
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##..##." ; original %".##..##."
        BYT %".##.##.." ; original %".##.##.."
        BYT %".####..." ; original %".####..."
        BYT %".##.##.." ; original %".#####.."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $8C (blank in free ROM)
        BYT %"..###..." ; original %"..###..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $8D (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %".##.##.." ; original %".##.#.##"
        BYT %"########" ; original %".#######"
        BYT %"##.##.##" ; original %".#######"
        BYT %"##.##.##" ; original %".##...##"
        BYT %"##.##.##" ; original %".##...##"
        BYT %"........" ; original %"........"
; Screen code $8E (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %".#####.." ; original %".#####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $8F (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $90 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %".#####.." ; original %"........"
        BYT %".##..##." ; original %".#####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".#####.." ; original %".##..##."
        BYT %".##....." ; original %".#####.."
        BYT %".##....." ; original %".##....."
        BYT %"........" ; original %".##....."
; Screen code $91 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"..#####." ; original %"........"
        BYT %".##..##." ; original %"..#####."
        BYT %".##..##." ; original %".##..##."
        BYT %"..#####." ; original %".##..##."
        BYT %".....##." ; original %"..#####."
        BYT %".....##." ; original %".....##."
        BYT %"........" ; original %".....##."
; Screen code $92 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %".##.###." ; original %".#####.."
        BYT %".###...." ; original %".##..##."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %"........" ; original %"........"
; Screen code $93 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##....."
        BYT %"..####.." ; original %"..####.."
        BYT %".....##." ; original %".....##."
        BYT %".#####.." ; original %".#####.."
        BYT %"........" ; original %"........"
; Screen code $94 (blank in free ROM)
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %".#####.." ; original %"######.."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##...." ; original %"..##...."
        BYT %"..##.##." ; original %"..##.##."
        BYT %"...###.." ; original %"...###.."
        BYT %"........" ; original %"........"
; Screen code $95 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..#####." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $96 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $97 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"##....##" ; original %".##...##"
        BYT %"##.##.##" ; original %".##.#.##"
        BYT %"##.##.##" ; original %".#######"
        BYT %"########" ; original %"..##.##."
        BYT %".##..##." ; original %"..#...#."
        BYT %"........" ; original %"........"
; Screen code $98 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $99 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %".##..##." ; original %"........"
        BYT %".##..##." ; original %".##..##."
        BYT %"..#####." ; original %".##..##."
        BYT %".....##." ; original %".##..##."
        BYT %".##..##." ; original %"..#####."
        BYT %"..####.." ; original %".....##."
        BYT %"........" ; original %".#####.."
; Screen code $9A (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %".######." ; original %".######."
        BYT %"....##.." ; original %"....##.."
        BYT %"...##..." ; original %"...##..."
        BYT %"..##...." ; original %"..##...."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $9B (blank in free ROM)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %"..##...."
        BYT %".##....." ; original %"..##...."
        BYT %".#####.." ; original %"..##...."
        BYT %".##....." ; original %"..##...."
        BYT %".##..##." ; original %"..##...."
        BYT %".######." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $9C (blank in free ROM)
        BYT %"...##..." ; original %"....##.."
        BYT %"..####.." ; original %"...#..#."
        BYT %".##..##." ; original %"..##...."
        BYT %"...##..." ; original %".#####.."
        BYT %"...##..." ; original %"..##...."
        BYT %"...##..." ; original %".##...#."
        BYT %"...##..." ; original %"######.."
        BYT %"........" ; original %"........"
; Screen code $9D (blank in free ROM)
        BYT %"...##..." ; original %"..####.."
        BYT %"..##...." ; original %"....##.."
        BYT %".##....." ; original %"....##.."
        BYT %"########" ; original %"....##.."
        BYT %".##....." ; original %"....##.."
        BYT %"..##...." ; original %"....##.."
        BYT %"...##..." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $9E (blank in free ROM)
        BYT %"...##..." ; original %"........"
        BYT %"..####.." ; original %"...##..."
        BYT %".##..##." ; original %"..####.."
        BYT %"........" ; original %".######."
        BYT %"........" ; original %"...##..."
        BYT %"........" ; original %"...##..."
        BYT %"........" ; original %"...##..."
        BYT %"........" ; original %"...##..."
; Screen code $9F (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"...#...."
        BYT %"........" ; original %"..##...."
        BYT %"........" ; original %".#######"
        BYT %"........" ; original %".#######"
        BYT %"........" ; original %"..##...."
        BYT %"########" ; original %"...#...."
        BYT %"........" ; original %"........"
; Screen code $A0 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $A1 (blank in free ROM)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $A2 (blank in free ROM)
        BYT %"..#.#..." ; original %".##..##."
        BYT %"..#.#..." ; original %".##..##."
        BYT %"..#.#..." ; original %".##..##."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $A3 (blank in free ROM)
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"########" ; original %"########"
        BYT %".##..##." ; original %".##..##."
        BYT %"########" ; original %"########"
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $A4 (blank in free ROM)
        BYT %"...#...." ; original %"...##..."
        BYT %"..####.." ; original %"..#####."
        BYT %".#.#...." ; original %".##....."
        BYT %"..###..." ; original %"..####.."
        BYT %"...#.#.." ; original %".....##."
        BYT %".####..." ; original %".#####.."
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $A5 (blank in free ROM)
        BYT %".##..#.." ; original %".##...#."
        BYT %".##.#..." ; original %".##..##."
        BYT %"....#..." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %"..#.##.." ; original %".##..##."
        BYT %".#..##.." ; original %".#...##."
        BYT %"........" ; original %"........"
; Screen code $A6 (blank in free ROM)
        BYT %"..##...." ; original %"..####.."
        BYT %".#..#..." ; original %".##..##."
        BYT %".#.#...." ; original %"..####.."
        BYT %"..#....." ; original %"..###..."
        BYT %".#.#.#.." ; original %".##..###"
        BYT %".#..#..." ; original %".##..##."
        BYT %"..##.#.." ; original %"..######"
        BYT %"........" ; original %"........"
; Screen code $A7 (blank in free ROM)
        BYT %"...#...." ; original %".....##."
        BYT %"...#...." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $A8 (blank in free ROM)
        BYT %"....#..." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %"..#....." ; original %"..##...."
        BYT %"..#....." ; original %"..##...."
        BYT %"...#...." ; original %"...##..."
        BYT %"....#..." ; original %"....##.."
        BYT %"........" ; original %"........"
; Screen code $A9 (blank in free ROM)
        BYT %"..#....." ; original %"..##...."
        BYT %"...#...." ; original %"...##..."
        BYT %"....#..." ; original %"....##.."
        BYT %"....#..." ; original %"....##.."
        BYT %"....#..." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %"........" ; original %"........"
; Screen code $AA (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %".#.#.#.." ; original %".##..##."
        BYT %"..###..." ; original %"..####.."
        BYT %".#####.." ; original %"########"
        BYT %"..###..." ; original %"..####.."
        BYT %".#.#.#.." ; original %".##..##."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $AB (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %".######." ; original %".######."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $AC (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"...##..."
        BYT %"........" ; original %"..##...."
; Screen code $AD (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $AE (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"..##...." ; original %"...##..."
        BYT %"..##...." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $AF (blank in free ROM)
        BYT %".....#.." ; original %"........"
        BYT %"....#..." ; original %"......##"
        BYT %"....#..." ; original %".....##."
        BYT %"...#...." ; original %"....##.."
        BYT %"..#....." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %".#......" ; original %".##....."
        BYT %"........" ; original %"........"
; Screen code $B0 (blank in free ROM)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %"##....##" ; original %".##.###."
        BYT %"##..#.##" ; original %".###.##."
        BYT %"##.#..##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $B1 (blank in free ROM)
        BYT %"...##..." ; original %"...##..."
        BYT %"..###..." ; original %"...##..."
        BYT %"...##..." ; original %"..###..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $B2 (blank in free ROM)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".....##." ; original %".....##."
        BYT %"...###.." ; original %"....##.."
        BYT %"..##...." ; original %"..##...."
        BYT %".##....." ; original %".##....."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $B3 (blank in free ROM)
        BYT %".#####.." ; original %"..####.."
        BYT %".....##." ; original %".##..##."
        BYT %".....##." ; original %".....##."
        BYT %"...####." ; original %"...###.."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".##..##."
        BYT %".#####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $B4 (blank in free ROM)
        BYT %"....##.." ; original %".....##."
        BYT %"...###.." ; original %"....###."
        BYT %"..####.." ; original %"...####."
        BYT %".##.##.." ; original %".##..##."
        BYT %"########" ; original %".#######"
        BYT %"....##.." ; original %".....##."
        BYT %"....##.." ; original %".....##."
        BYT %"........" ; original %"........"
; Screen code $B5 (blank in free ROM)
        BYT %".######." ; original %".######."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".#####.."
        BYT %".#####.." ; original %".....##."
        BYT %".....##." ; original %".....##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $B6 (blank in free ROM)
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##..##."
        BYT %"##......" ; original %".##....."
        BYT %"######.." ; original %".#####.."
        BYT %"##...##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $B7 (blank in free ROM)
        BYT %".######." ; original %".######."
        BYT %".....##." ; original %".##..##."
        BYT %"....##.." ; original %"....##.."
        BYT %"...##..." ; original %"...##..."
        BYT %"..##...." ; original %"...##..."
        BYT %"..##...." ; original %"...##..."
        BYT %"..##...." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $B8 (blank in free ROM)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $B9 (blank in free ROM)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##...##" ; original %".##..##."
        BYT %"..######" ; original %"..#####."
        BYT %"......##" ; original %".....##."
        BYT %".....##." ; original %".##..##."
        BYT %".#####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $BA (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $BB (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"...##..."
        BYT %"........" ; original %"..##...."
; Screen code $BC (blank in free ROM)
        BYT %"....#..." ; original %"....###."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %"..##...."
        BYT %".#......" ; original %".##....."
        BYT %"..#....." ; original %"..##...."
        BYT %"...#...." ; original %"...##..."
        BYT %"....#..." ; original %"....###."
        BYT %"........" ; original %"........"
; Screen code $BD (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %".#####.." ; original %"........"
        BYT %"........" ; original %".######."
        BYT %".#####.." ; original %"........"
        BYT %"........" ; original %".######."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $BE (blank in free ROM)
        BYT %"..#....." ; original %".###...."
        BYT %"...#...." ; original %"...##..."
        BYT %"....#..." ; original %"....##.."
        BYT %".....#.." ; original %".....##."
        BYT %"....#..." ; original %"....##.."
        BYT %"...#...." ; original %"...##..."
        BYT %"..#....." ; original %".###...."
        BYT %"........" ; original %"........"
; Screen code $BF (blank in free ROM)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".....##." ; original %".....##."
        BYT %"...###.." ; original %"....##.."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $C0 (blank in free ROM)
        BYT %"..####.." ; original %"........"
        BYT %".##..##." ; original %"........"
        BYT %".##.###." ; original %"........"
        BYT %".##.#.#." ; original %"########"
        BYT %".##.###." ; original %"########"
        BYT %".##....." ; original %"........"
        BYT %"..#####." ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $C1 (blank in free ROM)
        BYT %"..####.." ; original %"...##..."
        BYT %".##..##." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".######." ; original %".######."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $C2 (blank in free ROM)
        BYT %".#####.." ; original %".#####.."
        BYT %".##...#." ; original %".##..##."
        BYT %".##...#." ; original %".##..##."
        BYT %".#####.." ; original %".#####.."
        BYT %".##...#." ; original %".##..##."
        BYT %".##...#." ; original %".##..##."
        BYT %".#####.." ; original %".#####.."
        BYT %"........" ; original %"........"
; Screen code $C3 (blank in free ROM)
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##..##."
        BYT %"##......" ; original %".##....."
        BYT %"##......" ; original %".##....."
        BYT %"##......" ; original %".##....."
        BYT %".##....." ; original %".##..##."
        BYT %"..#####." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $C4 (blank in free ROM)
        BYT %".#####.." ; original %".####..."
        BYT %".##..##." ; original %".##.##.."
        BYT %".##...#." ; original %".##..##."
        BYT %".##...#." ; original %".##..##."
        BYT %".##...#." ; original %".##..##."
        BYT %".##..##." ; original %".##.##.."
        BYT %".#####.." ; original %".####..."
        BYT %"........" ; original %"........"
; Screen code $C5 (blank in free ROM)
        BYT %".######." ; original %".######."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".#####.." ; original %".####..."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $C6 (blank in free ROM)
        BYT %".######." ; original %".######."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".#####.." ; original %".####..."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %"........" ; original %"........"
; Screen code $C7 (blank in free ROM)
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##..##."
        BYT %"##......" ; original %".##....."
        BYT %"##..###." ; original %".##.###."
        BYT %"##...##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $C8 (blank in free ROM)
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".######." ; original %".######."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $C9 (blank in free ROM)
        BYT %".######." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %".######." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $CA (blank in free ROM)
        BYT %"..#####." ; original %"...####."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %"....##.." ; original %"....##.."
        BYT %".##.##.." ; original %"....##.."
        BYT %".##.##.." ; original %".##.##.."
        BYT %"..###..." ; original %"..###..."
        BYT %"........" ; original %"........"
; Screen code $CB (blank in free ROM)
        BYT %".##..##." ; original %".##..##."
        BYT %".##.##.." ; original %".##.##.."
        BYT %".####..." ; original %".####..."
        BYT %".###...." ; original %".###...."
        BYT %".####..." ; original %".####..."
        BYT %".##.##.." ; original %".##.##.."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $CC (blank in free ROM)
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $CD (blank in free ROM)
        BYT %"##....##" ; original %".##...##"
        BYT %"###..###" ; original %".###.###"
        BYT %"########" ; original %".#######"
        BYT %"##.##.##" ; original %".##.#.##"
        BYT %"##....##" ; original %".##...##"
        BYT %"##....##" ; original %".##...##"
        BYT %"##....##" ; original %".##...##"
        BYT %"........" ; original %"........"
; Screen code $CE (blank in free ROM)
        BYT %"##....##" ; original %".##..##."
        BYT %"###...##" ; original %".###.##."
        BYT %"####..##" ; original %".######."
        BYT %"##.##.##" ; original %".######."
        BYT %"##..####" ; original %".##.###."
        BYT %"##...###" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $CF (blank in free ROM)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $D0 (blank in free ROM)
        BYT %".#####.." ; original %".#####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".#####.." ; original %".#####.."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %".##....." ; original %".##....."
        BYT %"........" ; original %"........"
; Screen code $D1 (blank in free ROM)
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##..#.##" ; original %".##..##."
        BYT %".##..##." ; original %"..####.."
        BYT %"..#####." ; original %"....###."
        BYT %"........" ; original %"........"
; Screen code $D2 (blank in free ROM)
        BYT %".#####.." ; original %".#####.."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".#####.." ; original %".#####.."
        BYT %".####..." ; original %".####..."
        BYT %".##.##.." ; original %".##.##.."
        BYT %".##..##." ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $D3 (blank in free ROM)
        BYT %"..#####." ; original %"..####.."
        BYT %".##....." ; original %".##..##."
        BYT %"##......" ; original %".##....."
        BYT %"..####.." ; original %"..####.."
        BYT %".....##." ; original %".....##."
        BYT %".....##." ; original %".##..##."
        BYT %".#####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $D4 (blank in free ROM)
        BYT %"########" ; original %".######."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $D5 (blank in free ROM)
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"........" ; original %"........"
; Screen code $D6 (blank in free ROM)
        BYT %"##....##" ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $D7 (blank in free ROM)
        BYT %"##....##" ; original %".##...##"
        BYT %"##....##" ; original %".##...##"
        BYT %"##....##" ; original %".##...##"
        BYT %"##.##.##" ; original %".##.#.##"
        BYT %"########" ; original %".#######"
        BYT %"###..###" ; original %".###.###"
        BYT %"##....##" ; original %".##...##"
        BYT %"........" ; original %"........"
; Screen code $D8 (blank in free ROM)
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"..####.." ; original %"..####.."
        BYT %".##..##." ; original %".##..##."
        BYT %"##....##" ; original %".##..##."
        BYT %"........" ; original %"........"
; Screen code $D9 (blank in free ROM)
        BYT %"##....##" ; original %".##..##."
        BYT %".##..##." ; original %".##..##."
        BYT %"..####.." ; original %".##..##."
        BYT %"...##..." ; original %"..####.."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"........" ; original %"........"
; Screen code $DA (blank in free ROM)
        BYT %".######." ; original %".######."
        BYT %".....##." ; original %".....##."
        BYT %"....##.." ; original %"....##.."
        BYT %"...##..." ; original %"...##..."
        BYT %"..##...." ; original %"..##...."
        BYT %".##....." ; original %".##....."
        BYT %".######." ; original %".######."
        BYT %"........" ; original %"........"
; Screen code $DB (blank in free ROM)
        BYT %"..###..." ; original %"...##..."
        BYT %"..#....." ; original %"...##..."
        BYT %"..#....." ; original %"...##..."
        BYT %"..#....." ; original %"########"
        BYT %"..#....." ; original %"########"
        BYT %"..#....." ; original %"...##..."
        BYT %"..###..." ; original %"...##..."
        BYT %"........" ; original %"...##..."
; Screen code $DC (blank in free ROM)
        BYT %".#......" ; original %"##......"
        BYT %"..#....." ; original %"##......"
        BYT %"..#....." ; original %"..##...."
        BYT %"...#...." ; original %"..##...."
        BYT %"....#..." ; original %"##......"
        BYT %"....#..." ; original %"##......"
        BYT %".....#.." ; original %"..##...."
        BYT %"........" ; original %"..##...."
; Screen code $DD (blank in free ROM)
        BYT %"..###..." ; original %"...##..."
        BYT %"....#..." ; original %"...##..."
        BYT %"....#..." ; original %"...##..."
        BYT %"....#..." ; original %"...##..."
        BYT %"....#..." ; original %"...##..."
        BYT %"....#..." ; original %"...##..."
        BYT %"..###..." ; original %"...##..."
        BYT %"........" ; original %"...##..."
; Screen code $DE (blank in free ROM)
        BYT %"..##..##" ; original %"..##..##"
        BYT %"..##..##" ; original %"..##..##"
        BYT %"##..##.." ; original %"##..##.."
        BYT %"##..##.." ; original %"##..##.."
        BYT %"..##..##" ; original %"..##..##"
        BYT %"..##..##" ; original %"..##..##"
        BYT %"##..##.." ; original %"##..##.."
        BYT %"##..##.." ; original %"##..##.."
; Screen code $DF (blank in free ROM)
        BYT %"..##..##" ; original %"..##..##"
        BYT %"#..##..#" ; original %"#..##..#"
        BYT %"##..##.." ; original %"##..##.."
        BYT %".##..##." ; original %".##..##."
        BYT %"..##..##" ; original %"..##..##"
        BYT %"#..##..#" ; original %"#..##..#"
        BYT %"##..##.." ; original %"##..##.."
        BYT %".##..##." ; original %".##..##."
; Screen code $E0 (blank in free ROM)
        BYT %"..#....." ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"....#..." ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $E1 (blank in free ROM)
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
; Screen code $E2 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
; Screen code $E3 (blank in free ROM)
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $E4 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
; Screen code $E5 (blank in free ROM)
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
; Screen code $E6 (blank in free ROM)
        BYT %"##..##.." ; original %"##..##.."
        BYT %"##..##.." ; original %"##..##.."
        BYT %"..##..##" ; original %"..##..##"
        BYT %"..##..##" ; original %"..##..##"
        BYT %"##..##.." ; original %"##..##.."
        BYT %"##..##.." ; original %"##..##.."
        BYT %"..##..##" ; original %"..##..##"
        BYT %"..##..##" ; original %"..##..##"
; Screen code $E7 (blank in free ROM)
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
; Screen code $E8 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"##..##.." ; original %"##..##.."
        BYT %"##..##.." ; original %"##..##.."
        BYT %"..##..##" ; original %"..##..##"
        BYT %"..##..##" ; original %"..##..##"
; Screen code $E9 (blank in free ROM)
        BYT %"##..##.." ; original %"##..##.."
        BYT %"#..##..#" ; original %"#..##..#"
        BYT %"..##..##" ; original %"..##..##"
        BYT %".##..##." ; original %".##..##."
        BYT %"##..##.." ; original %"##..##.."
        BYT %"#..##..#" ; original %"#..##..#"
        BYT %"..##..##" ; original %"..##..##"
        BYT %".##..##." ; original %".##..##."
; Screen code $EA (blank in free ROM)
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
        BYT %"......##" ; original %"......##"
; Screen code $EB (blank in free ROM)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...#####" ; original %"...#####"
        BYT %"...#####" ; original %"...#####"
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $EC (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
; Screen code $ED (blank in free ROM)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...#####" ; original %"...#####"
        BYT %"...#####" ; original %"...#####"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $EE (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"#####..." ; original %"#####..."
        BYT %"#####..." ; original %"#####..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $EF (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
; Screen code $F0 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"...#####" ; original %"...#####"
        BYT %"...#####" ; original %"...#####"
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $F1 (blank in free ROM)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $F2 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $F3 (blank in free ROM)
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"#####..." ; original %"#####..."
        BYT %"#####..." ; original %"#####..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
        BYT %"...##..." ; original %"...##..."
; Screen code $F4 (blank in free ROM)
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
        BYT %"##......" ; original %"##......"
; Screen code $F5 (blank in free ROM)
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
        BYT %"###....." ; original %"###....."
; Screen code $F6 (blank in free ROM)
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
        BYT %".....###" ; original %".....###"
; Screen code $F7 (blank in free ROM)
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $F8 (blank in free ROM)
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $F9 (blank in free ROM)
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
        BYT %"########" ; original %"########"
; Screen code $FA (blank in free ROM)
        BYT %".......#" ; original %".......#"
        BYT %"......##" ; original %"......##"
        BYT %".....##." ; original %".....##."
        BYT %".##.##.." ; original %".##.##.."
        BYT %".####..." ; original %".####..."
        BYT %".###...." ; original %".###...."
        BYT %".##....." ; original %".##....."
        BYT %"........" ; original %"........"
; Screen code $FB (blank in free ROM)
        BYT %"....##.." ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %".##....." ; original %"........"
        BYT %"...#...." ; original %"####...."
        BYT %"...#...." ; original %"####...."
        BYT %"....##.." ; original %"####...."
        BYT %"........" ; original %"####...."
; Screen code $FC (blank in free ROM)
        BYT %"...#...." ; original %"....####"
        BYT %"...#...." ; original %"....####"
        BYT %"...#...." ; original %"....####"
        BYT %"...#...." ; original %"....####"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"...#...." ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $FD (blank in free ROM)
        BYT %".##....." ; original %"...##..."
        BYT %"...#...." ; original %"...##..."
        BYT %"...#...." ; original %"...##..."
        BYT %"....##.." ; original %"#####..."
        BYT %"...#...." ; original %"#####..."
        BYT %"...#...." ; original %"........"
        BYT %".##....." ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $FE (blank in free ROM)
        BYT %"........" ; original %"####...."
        BYT %"........" ; original %"####...."
        BYT %"..#..#.." ; original %"####...."
        BYT %".#.##..." ; original %"####...."
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
        BYT %"........" ; original %"........"
; Screen code $FF (blank in free ROM)
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"####...." ; original %"####...."
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
        BYT %"....####" ; original %"....####"
irq     RTI
        ORG $FFFA
        ADR irq,reset,irq
        END
