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
input   EQU $0200               ; one direct-mode line, zero terminated
rowlink EQU $02c0               ; one byte per row: continuation from above
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
        LDA #$08
        STA $ff07               ; 40 columns, native reverse-video text mode
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
        CPX #61                 ; C= and Shift toggle the two native charsets
        BNE .notcharset
        JSR readshift
        LDA shifted
        BEQ .charsetdone       ; C= alone has no text-editor action
        LDA $ff13
        EOR #$04               ; $D0 graphics/uppercase, $D4 lowercase/upper
        STA $ff13
.charsetdone
        JMP inputloop
.notcharset
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
        BCC .insertshift
        JMP inputloop
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

; Output ASCII A as a native C16 screen code and return its original byte.
; Letters use codes 1..26; @ and the C16 bracket/arrow shapes have own codes.
; Unsupported host punctuation appears as a question mark, never as graphics.
; The public entry at $C800 preserves X.
putchar STA letter
        CMP #13
        BEQ .putnewline
        CMP #64
        BEQ .at_sign
        BCC .writecode
        CMP #91
        BCC .capital
        CMP #91
        BEQ .left_bracket
        CMP #93
        BEQ .right_bracket
        CMP #94
        BEQ .up_arrow
        CMP #97
        BCC .unsupported
        CMP #128
        BCS .writecode          ; already a native screen-code byte
        CMP #123
        BCS .unsupported
        SEC
        SBC #96                 ; lowercase PETSCII uses this glyph set too
        JMP .writecode
.capital
        SEC
        SBC #64
        JMP .writecode
.at_sign
        LDA #0
        JMP .writecode
.left_bracket
        LDA #27
        JMP .writecode
.right_bracket
        LDA #29
        JMP .writecode
.up_arrow
        LDA #30
        JMP .writecode
.unsupported
        LDA #63                 ; native question-mark glyph
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
        BYT 35,0,0,36,0,0,0,0
        BYT 37,0,0,38,0,0,0,0
        BYT 39,0,0,40,0,0,0,0
        BYT 41,0,0,64,0,0,0,0
        BYT 0,0,0,0,62,91,0,60
        BYT 0,0,93,0,0,0,94,63
        BYT 33,0,0,34,0,0,0,0

; The BASIC ROM calls only these stable, three-byte entry points.
        ORG $C800
api_char JMP putchar
api_text JMP printz
api_line JMP newline

        INCLUDE "font.asm"

irq     RTI
        ORG $FFFA
        ADR irq,reset,irq
        END
