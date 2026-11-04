; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Independent BASIC V0.1 device and graphics extensions. Included by
; basrom.asm, so its parser state and utility routines are in scope.

soundvoice EQU $69
freqlo EQU $6a
freqhi EQU $6b
durlo EQU $6c
durhi EQU $6d
kwlo EQU $6e
kwhi EQU $6f
gfxrequest EQU $70
bitmaphi EQU $71
gxlo EQU $72
gxhi EQU $73
gy EQU $74
gsource EQU $75
gptrlo EQU $76
gptrhi EQU $77
gmask EQU $78
gtemp EQU $79
gyhi EQU $7a
cxlo EQU $0300
cxhi EQU $0301
cy EQU $0302
radius EQU $0303
circle_x EQU $0304
circle_y EQU $0305
circle_errlo EQU $0306
circle_errhi EQU $0307
oct_x EQU $0308
oct_y EQU $0309
line_target_xlo EQU $030a
line_target_xhi EQU $030b
line_target_y EQU $030c
line_dxlo EQU $030d
line_dxhi EQU $030e
line_dy EQU $030f
line_errlo EQU $0310
line_errhi EQU $0311
line_sx EQU $0312
line_sy EQU $0313
box_x1lo EQU $0314
box_x1hi EQU $0315
box_y1 EQU $0316
box_x2lo EQU $0317
box_x2hi EQU $0318
box_y2 EQU $0319
box_fill EQU $031a
box_row EQU $031b
paint_toplo EQU $031c
paint_row EQU $031d
paint_above EQU $031e
paint_below EQU $031f
paint_tophi EQU $06e0
paint_saved_srclo EQU $06e1
paint_saved_srchi EQU $06e2
paint_nextlo EQU $06e3
paint_nexthi EQU $06e4
char_column EQU $0320
char_row EQU $0321
yradius EQU $0322
circle_opt EQU $06b0          ; optional CIRCLE argument index
circle_startlo EQU $06b1
circle_starthi EQU $06b2
circle_endlo EQU $06b3
circle_endhi EQU $06b4
circle_rotation EQU $06b5
circle_step EQU $06b6
char_start EQU $033b
char_after EQU $033c
char_reverse EQU $033d
glyphlo EQU $7c
glyphhi EQU $7d

; Dispatch a fully spelled extension keyword. C=1 means handled; the caller
; then resumes after a colon, unless failed was set by a command.
extra_dispatch
        LDA #kwmonitor%256
        STA kwlo
        LDA #kwmonitor/256
        STA kwhi
        JSR keyword
        BCC .notmonitor
        JSR $c80c             ; system ROM owns interactive MONITOR input
        SEC
        RTS
.notmonitor
        LDA #kwgraphic%256
        STA kwlo
        LDA #kwgraphic/256
        STA kwhi
        JSR keyword
        BCC .notgraphic
        JSR graphicstmt
        SEC
        RTS
.notgraphic
        LDA #kwpaint%256
        STA kwlo
        LDA #kwpaint/256
        STA kwhi
        JSR keyword
        BCC .notpaint
        JSR paintstmt
        SEC
        RTS
.notpaint
        LDA #kwchar%256
        STA kwlo
        LDA #kwchar/256
        STA kwhi
        JSR keyword
        BCC .notchar
        JSR charstmt
        SEC
        RTS
.notchar
        LDA #kwscnclr%256
        STA kwlo
        LDA #kwscnclr/256
        STA kwhi
        JSR keyword
        BCC .notscnclr
        JSR scnclrstmt
        SEC
        RTS
.notscnclr
        LDA #kwscale%256
        STA kwlo
        LDA #kwscale/256
        STA kwhi
        JSR keyword
        BCC .notscale
        JSR scalestmt
        SEC
        RTS
.notscale
        LDA #kwvol%256
        STA kwlo
        LDA #kwvol/256
        STA kwhi
        JSR keyword
        BCC .notvol
        JSR volstmt
        SEC
        RTS
.notvol
        LDA #kwsound%256
        STA kwlo
        LDA #kwsound/256
        STA kwhi
        JSR keyword
        BCC .notsound
        JSR soundstmt
        SEC
        RTS
.notsound
        LDA #kwlocate%256
        STA kwlo
        LDA #kwlocate/256
        STA kwhi
        JSR keyword
        BCC .notlocate
        JSR locatestmt
        SEC
        RTS
.notlocate
        LDA #kwdraw%256
        STA kwlo
        LDA #kwdraw/256
        STA kwhi
        JSR keyword
        BCC .notdraw
        JSR drawstmt
        SEC
        RTS
.notdraw
        LDA #kwcircle%256
        STA kwlo
        LDA #kwcircle/256
        STA kwhi
        JSR keyword
        BCC .notcircle
        JSR circlestmt
        SEC
        RTS
.notcircle
        LDA #kwbox%256
        STA kwlo
        LDA #kwbox/256
        STA kwhi
        JSR keyword
        BCC .notbox
        JSR boxstmt
        SEC
        RTS
.notbox
        LDA #kwcolor%256
        STA kwlo
        LDA #kwcolor/256
        STA kwhi
        JSR keyword
        BCC .notcolor
        JSR colorstmt
        SEC
        RTS
.notcolor
        LDA #kwsshape%256
        STA kwlo
        LDA #kwsshape/256
        STA kwhi
        JSR keyword
        BCC .notsshape
        JSR sshapestmt
        SEC
        RTS
.notsshape
        LDA #kwgshape%256
        STA kwlo
        LDA #kwgshape/256
        STA kwhi
        JSR keyword
        BCC .notgshape
        JSR gshapestmt
        SEC
        RTS
.notgshape
        LDA #kwpoke%256
        STA kwlo
        LDA #kwpoke/256
        STA kwhi
        JSR keyword
        BCC .notpoke
        JSR pokestmt
        SEC
        RTS
.notpoke
        LDA #kwdef%256
        STA kwlo
        LDA #kwdef/256
        STA kwhi
        JSR keyword
        BCC .notdef
        JSR $e603             ; system-ROM DEF FN parser
        SEC
        RTS
.notdef
        LDA #kwdsave%256
        STA kwlo
        LDA #kwdsave/256
        STA kwhi
        JSR keyword
        BCC .notdsave
        JSR dsavestmt
        SEC
        RTS
.notdsave
        LDA #kwdload%256
        STA kwlo
        LDA #kwdload/256
        STA kwhi
        JSR keyword
        BCC .unknown
        JSR dloadstmt
        SEC
        RTS
.unknown
        CLC
        RTS

; Compare the zero-terminated ROM spelling at kwlo:kwhi with input[pos].
; On success pos advances past the word. A following letter rejects a prefix.
keyword
        LDY #0
        LDX pos
.letter
        LDA (kwlo),Y
        BEQ .endword
        CMP input,X
        BNE .no
        INY
        INX
        JMP .letter
.endword
        LDA input,X
        CMP #65
        BCC .yes
        CMP #91
        BCC .no
.yes    STX pos
        SEC
        RTS
.no     CLC
        RTS

; Parse a required comma followed by an integer expression.
commaexpr
        JSR spaces
        CMP #44
        BEQ .hascomma
        JMP syntax
.hascomma
        INC pos
        JMP expr
commaexpr_x
        JSR spaces
        CMP #44
        BEQ .hascomma
        JMP syntax
.hascomma
        INC pos
        JMP coord_x
commaexpr_y
        JSR spaces
        CMP #44
        BEQ .hascomma
        JMP syntax
.hascomma
        INC pos
        JMP coord_y

; Report an out-of-range command argument through BASIC's syntax path.
badargument
        JMP syntax

; Parse a drawing color source (background 0 or foreground 1..3), rejecting
; values outside the TED bitmap palette before they reach the pixel writer.
sourceexpr
        JSR expr
        LDA failed
        BNE .done
        LDA valhi
        BNE .bad
        LDA vallo
        CMP #4
        BCS .bad
        STA gsource
.done   RTS
.bad    JMP badargument

; SCALE 1 maps 0..1023 onto the native 320×200 coordinates; SCALE 0 restores
; direct pixels. Multiplication by 5/16 and 25/128 exactly performs the TED
; 1024-unit proportional mapping, with the last logical point reaching 319/199.
scalestmt
        JSR expr
        LDA valhi
        BNE .bad
        LDA vallo
        CMP #2
        BCS .bad
        STA scaleflag
        RTS
.bad    JMP badargument

coord_x JSR expr
        LDA scaleflag
        BEQ .done
        LDA #5
        STA gtemp
        JSR scale_multiply
        LDX #4
.shift  LSR valhi
        ROR vallo
        DEX
        BNE .shift
.done   RTS
coord_y JSR expr
        LDA scaleflag
        BEQ .done
        LDA #25
        STA gtemp
        JSR scale_multiply
        LDX #7
.shift  LSR valhi
        ROR vallo
        DEX
        BNE .shift
.done   RTS
scale_multiply
        LDA vallo
        STA termlo
        LDA valhi
        STA termhi
        LDA gtemp
        STA vallo
        LDA #0
        STA valhi
        JSR multiply
        LDA termlo
        STA vallo
        LDA termhi
        STA valhi
        RTS

; Reserve a real RAM bitmap, program FF12's base while retaining ROM glyphs,
; and select TED's full or raster-split picture. GRAPHIC 0 keeps the bitmap;
; GRAPHIC CLR releases it. The 16 KiB machine uses $2000, larger ones $6000.
graphicstmt
        JSR spaces
        CMP #67                 ; GRAPHIC CLR
        BNE .mode
        LDY pos
        INY
        LDA input,Y
        CMP #76
        BEQ .clr_l
        JMP badargument
.clr_l INY
        LDA input,Y
        CMP #82
        BEQ .clr_r
        JMP badargument
.clr_r INY
        LDA input,Y
        CMP #65
        BCC .clr_valid
        CMP #91
        BCS .clr_valid
        JMP badargument
.clr_valid
        LDA pos
        CLC
        ADC #3
        STA pos
        LDA memlimhi
        STA gfxlimit
        LDA #4
        STA $ff12               ; font stays in system ROM
        LDA #8
        STA $ff07
        STA $ff14
        LDA #0
        STA gfxmode
        STA gfxphase
        STA shape_valid
        STA $ff0b
        LDA #$1b
        STA $ff06
        RTS
.mode   JSR expr
        LDA failed
        BEQ .modeparsed
        RTS
.modeparsed
        LDA valhi
        BEQ .modebyte
        JMP badargument
.modebyte
        LDA vallo
        CMP #5
        BCC .modevalid
        JMP badargument
.modevalid
        STA gfxrequest
        LDA #0
        STA gtemp               ; optional clear flag
        JSR spaces
        CMP #44
        BNE .select
        JSR commaexpr
        LDA failed
        BNE .done
        LDA vallo
        STA gtemp
.select
        LDA gfxrequest
        BNE .picture
        LDA #0
        STA gfxmode
        STA gfxphase
        STA $ff0b
        LDA #8
        STA $ff07               ; text mode is never multicolor
        STA $ff14
        LDA #$1b
        STA $ff06
        RTS
.picture
        LDA memlimhi
        CMP #$40
        BNE .large
        LDA #$20
        BNE .reserve
.large  LDA #$60
.reserve
        STA bitmaphi
        SEC
        SBC #8                  ; color and screen matrices precede bitmap
        STA gfxmatrix
        LDA endhi
        CMP gfxmatrix
        BCC .room
        BNE .no_memory
        LDA endlo
        BNE .no_memory
.room   LDA gfxlimit
        CMP memlimhi
        BNE .matrix_ready
        JSR initmatrix
.matrix_ready
        LDA gfxmatrix
        STA gfxlimit
        STA $ff14
        LDA bitmaphi
        LSR
        LSR
        AND #$38                ; FF12 bits 3..5 encode bitmap base
        ORA #4                  ; character glyphs still come from ROM
        STA $ff12
        LDA gfxrequest
        CMP #3
        BCC .hires
        LDA #$18               ; 40 columns and multicolor pixels
        BNE .control2
.hires  LDA #$08
.control2
        STA $ff07
        LDA gfxrequest
        STA gfxmode
        LDA #0
        STA gfxphase
        STA $ff0b
        LDA #$3b
        STA $ff06
        LDA gtemp
        BEQ .done
        JSR clearbitmap
.done   RTS
.no_memory
        LDA #1
        STA failed
        JSR error_to_text
        LDA #memmsg%256
        STA msglo
        LDA #memmsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS
.invalid
        JMP badargument

; Clear the active screen. In graphics modes only the 8000 bitmap bytes are
; cleared; the text matrix and color attributes retain their prior contents.
scnclrstmt
        LDA gfxmode
        BNE clearbitmap
        LDA #0
        STA dstlo
        LDA #$0c
        STA dsthi
        LDY #0
.textpage
        LDA #32
        STA (dstlo),Y
        INC dstlo
        BNE .textnext
        INC dsthi
.textnext
        LDA dsthi
        CMP #$0f
        BNE .textpage
        LDA dstlo
        CMP #$e8
        BNE .textpage
        LDA #0
        STA $20                 ; home the system ROM's screen pointer
        STA $28                 ; home its 0..999 cell index
        STA $27                 ; column
        STA $2e                 ; row
        STA $ff0c
        STA $ff0d
        LDA #$0c
        STA $21
        LDA #0
        STA $29
        RTS

; Fill the 320×200 bitmap RAM with background pixels without touching the
; color matrix, program records, or system ROM overlay.
clearbitmap
        LDA #0
        STA dstlo
        LDA bitmaphi
        STA dsthi
        LDA #$40
        STA srclo
        LDA bitmaphi
        CLC
        ADC #$1f
        STA srchi
        LDY #0
.clear  LDA #0
        STA (dstlo),Y
        INC dstlo
        BNE .next
        INC dsthi
.next   LDA dsthi
        CMP srchi
        BNE .clear
        LDA dstlo
        CMP srclo
        BNE .clear
        RTS

; Initialize the dedicated graphics matrix pair. The 1000 color bytes use
; dark foreground; the corresponding code bytes select black in hires mode.
; BASIC text continues to use its separate $0800/$0C00 matrices.
initmatrix
        LDA #0
        STA dstlo
        STA srclo
        LDA gfxmatrix
        STA dsthi
        CLC
        ADC #4
        STA srchi
        LDY #0
.fill   LDA #$10
        STA (dstlo),Y
        LDA #0
        STA (srclo),Y
        INC dstlo
        INC srclo
        BNE .next
        INC dsthi
        INC srchi
.next   LDA dsthi
        SEC
        SBC gfxmatrix
        CMP #3
        BNE .fill
        LDA dstlo
        CMP #$e8
        BNE .fill
        RTS

; POKE stores a byte at a BASIC-computed RAM or hardware address.
; Arguments: pos follows POKE and names address,value. Returns: no value;
; failed reports an invalid signed address or byte outside 0..255.
pokestmt
        JSR fexpr
        LDA failed
        BNE .done
        JSR $d909              ; FAC to signed address for this ROM subset
        LDA fp_intlo
        STA dstlo
        LDA fp_inthi
        STA dsthi
        JSR spaces
        CMP #44
        BNE .bad
        INC pos
        JSR fexpr
        LDA failed
        BNE .done
        JSR $d909
        LDA fp_inthi
        BNE .bad
        LDY #0
        LDA fp_intlo
        STA (dstlo),Y
.done   RTS
.bad    JMP badargument

; VOL writes TED's shared four-bit volume, preserving voice and DAC controls.
volstmt
        JSR expr
        LDA failed
        BNE .done
        LDA valhi
        BNE .bad
        LDA vallo
        CMP #9
        BCS .bad
        STA gtemp
        LDA $ff11
        AND #$f0
        ORA gtemp
        STA $ff11
.done   RTS
.bad    JMP badargument

; SOUND voice,frequency,duration programs TED's two tone counters or noise.
; Durations live in zero page and the frame IRQ decrements them at 50 Hz.
soundstmt
        JSR expr
        LDA failed
        BEQ .voiceparsed
        RTS
.voiceparsed
        LDA valhi
        BEQ .voicebyte
        JMP badargument
.voicebyte
        LDA vallo
        CMP #1
        BCS .voicepositive
        JMP badargument
.voicepositive
        CMP #4
        BCC .voicevalid
        JMP badargument
.voicevalid
        STA soundvoice
        JSR commaexpr
        LDA failed
        BEQ .frequency_parsed
        RTS
.frequency_parsed
        LDA valhi
        CMP #4
        BCC .frequency_valid
        JMP badargument
.frequency_valid
        LDA vallo
        STA freqlo
        LDA valhi
        STA freqhi
        JSR commaexpr
        LDA failed
        BEQ .duration_parsed
        RTS
.duration_parsed
        LDA vallo
        STA durlo
        LDA valhi
        STA durhi
        LDA durlo
        ORA durhi
        BEQ .program_sound      ; duration zero stops the voice immediately
        LDA soundvoice
        CMP #1
        BNE .wait_second
.wait_first
        LDA sound1lo
        ORA sound1hi
        BNE .wait_first
        JMP .program_sound
.wait_second
        LDA sound2lo
        ORA sound2hi
        BNE .wait_second
.program_sound
        SEI                     ; frame IRQ must not see half a duration
        LDA soundvoice
        CMP #1
        BNE .second
        LDA durlo
        STA sound1lo
        LDA durhi
        STA sound1hi
        LDA freqlo
        STA $ff0e
        LDA $ff12
        AND #$fc
        ORA freqhi
        STA $ff12
        LDA $ff11
        ORA #$10
        LDX durlo
        BNE .firston
        LDX durhi
        BNE .firston
        AND #$ef
.firston
        STA $ff11
        CLI
        RTS
.second
        LDA durlo
        STA sound2lo
        LDA durhi
        STA sound2hi
        LDA freqlo
        STA $ff0f
        LDA freqhi
        STA $ff10
        LDA $ff11
        AND #$9f               ; tone two and noise share one counter
        LDX durlo
        BNE .secondon
        LDX durhi
        BEQ .secondoff
.secondon
        LDX soundvoice
        CPX #2
        BEQ .tone
        ORA #$40
        BNE .secondoff
.tone   ORA #$20
.secondoff
        STA $ff11
        CLI
.done   RTS
.bad    JMP badargument

; LOCATE moves the current pixel point. Coordinates are native bitmap pixels.
locatestmt
        JSR coord_x
        LDA failed
        BNE .done
        LDA vallo
        STA gxlo
        LDA valhi
        STA gxhi
        JSR commaexpr_y
        LDA vallo
        STA gy
        LDA valhi
        STA gyhi
.done   RTS

; DRAW source,x,y plots a point and advances the invisible pixel cursor.
; Source zero erases a pixel; sources 1..3 set it in hires mode.
drawstmt
        JSR sourceexpr
        LDA failed
        BNE .done
        JSR commaexpr_x
        LDA failed
        BNE .done
        LDA vallo
        STA gxlo
        LDA valhi
        STA gxhi
        JSR commaexpr_y
        LDA failed
        BNE .done
        LDA vallo
        STA gy
        LDA valhi
        STA gyhi
        JSR plotpixel
.next_segment
        JSR spaces
        CMP #84                 ; DRAW ... TO x,y
        BNE .done
        LDY pos
        INY
        LDA input,Y
        CMP #79
        BNE .done
        INC pos
        INC pos
        JSR coord_x
        LDA failed
        BNE .done
        LDA vallo
        STA line_target_xlo
        LDA valhi
        STA line_target_xhi
        JSR commaexpr_y
        LDA failed
        BNE .done
        LDA valhi
        BNE .bad
        LDA vallo
        STA line_target_y
        JSR drawline
        JMP .next_segment
.bad    JMP badargument
.done   RTS

; Join the current point to line_target using an axis-major integer
; Bresenham accumulator. Endpoints are included, so chained TO segments join.
drawline
        SEC
        LDA line_target_xlo
        SBC gxlo
        STA line_dxlo
        LDA line_target_xhi
        SBC gxhi
        STA line_dxhi
        BCC .xnegative
        LDA #1
        STA line_sx
        BNE .ydifference
.xnegative
        LDA #$ff
        STA line_sx
        SEC
        LDA gxlo
        SBC line_target_xlo
        STA line_dxlo
        LDA gxhi
        SBC line_target_xhi
        STA line_dxhi
.ydifference
        SEC
        LDA line_target_y
        SBC gy
        BCC .ynegative
        STA line_dy
        LDA #1
        STA line_sy
        BNE .choose_axis
.ynegative
        LDA #$ff
        STA line_sy
        SEC
        LDA gy
        SBC line_target_y
        STA line_dy
.choose_axis
        LDA line_dxhi
        BNE .xmajor
        LDA line_dxlo
        CMP line_dy
        BCS .xmajor
        LDA line_dy
        LSR
        STA line_errlo
        LDA #0
        STA line_errhi
.yloop  JSR plotpixel
        JSR line_at_target
        BCS .done
        JSR line_step_y
        SEC
        LDA line_errlo
        SBC line_dxlo
        STA line_errlo
        LDA line_errhi
        SBC line_dxhi
        STA line_errhi
        BPL .yloop
        JSR line_step_x
        CLC
        LDA line_errlo
        ADC line_dy
        STA line_errlo
        BCC .yloop
        INC line_errhi
        JMP .yloop
.xmajor
        LDA line_dxlo
        STA line_errlo
        LDA line_dxhi
        STA line_errhi
        LSR line_errhi
        ROR line_errlo
.xloop  JSR plotpixel
        JSR line_at_target
        BCS .done
        JSR line_step_x
        SEC
        LDA line_errlo
        SBC line_dy
        STA line_errlo
        LDA line_errhi
        SBC #0
        STA line_errhi
        BPL .xloop
        JSR line_step_y
        CLC
        LDA line_errlo
        ADC line_dxlo
        STA line_errlo
        LDA line_errhi
        ADC line_dxhi
        STA line_errhi
        JMP .xloop
.done   RTS

line_at_target
        LDA gxlo
        CMP line_target_xlo
        BNE .no
        LDA gxhi
        CMP line_target_xhi
        BNE .no
        LDA gy
        CMP line_target_y
        BNE .no
        SEC
        RTS
.no     CLC
        RTS
line_step_x
        LDA line_sx
        BMI .back
        INC gxlo
        BNE .done
        INC gxhi
.done   RTS
.back   LDA gxlo
        BNE .decrement
        DEC gxhi
.decrement
        DEC gxlo
        RTS
line_step_y
        LDA line_sy
        BMI .back
        INC gy
        RTS
.back   DEC gy
        RTS

; BOX source,x1,y1,x2,y2[,angle[,fill]] draws four inclusive edges or
; horizontal scan lines. This small ROM currently accepts angle zero.
boxstmt
        JSR sourceexpr
        LDA failed
        BEQ .source
        RTS
.source
        JSR commaexpr_x
        LDA failed
        BEQ .x1
        RTS
.x1    LDA vallo
        STA box_x1lo
        LDA valhi
        STA box_x1hi
        JSR commaexpr_y
        LDA failed
        BEQ .y1
        RTS
.y1    LDA vallo
        STA box_y1
        JSR commaexpr_x
        LDA failed
        BEQ .x2
        RTS
.x2    LDA vallo
        STA box_x2lo
        LDA valhi
        STA box_x2hi
        JSR commaexpr_y
        LDA failed
        BEQ .y2
        RTS
.y2    LDA vallo
        STA box_y2
        LDA #0
        STA box_fill
        JSR spaces
        CMP #44
        BNE .draw
        JSR commaexpr           ; rotation angle
        LDA vallo
        ORA valhi
        BEQ .angle_zero
        JMP badargument
.angle_zero
        JSR spaces
        CMP #44
        BNE .draw
        JSR commaexpr
        LDA vallo
        STA box_fill
.draw   LDA box_fill
        BEQ .outline
        LDA box_y1
        STA box_row
.fillrow
        LDA box_x1lo
        STA gxlo
        LDA box_x1hi
        STA gxhi
        LDA box_row
        STA gy
        LDA #0
        STA gyhi
        LDA box_x2lo
        STA line_target_xlo
        LDA box_x2hi
        STA line_target_xhi
        LDA box_row
        STA line_target_y
        JSR drawline
        LDA box_row
        CMP box_y2
        BEQ .done
        BCC .fillinc
        DEC box_row
        JMP .fillrow
.fillinc
        INC box_row
        JMP .fillrow
.outline
        LDA box_x1lo
        STA gxlo
        LDA box_x1hi
        STA gxhi
        LDA box_y1
        STA gy
        LDA #0
        STA gyhi
        LDA box_x2lo
        STA line_target_xlo
        LDA box_x2hi
        STA line_target_xhi
        LDA box_y1
        STA line_target_y
        JSR drawline
        LDA box_y2
        STA line_target_y
        JSR drawline
        LDA box_x1lo
        STA line_target_xlo
        LDA box_x1hi
        STA line_target_xhi
        JSR drawline
        LDA box_y1
        STA line_target_y
        JSR drawline
.done   RTS
.bad    JMP badargument

; Translate one 320×200 coordinate into TED's interleaved bitmap layout:
; (y/8)*320 + (x/8)*8 + (y mod 8). Out-of-range points are clipped.
plotpixel
        JSR pixeladdress
        BCC .outside
        LDA gfxmode
        CMP #3
        BCS .multicolor
        LDY #0
        LDA gsource
        BEQ .erase
        LDA (gptrlo),Y
        ORA gmask
        STA (gptrlo),Y
        RTS
.erase  LDA gmask
        EOR #$ff
        AND (gptrlo),Y
        STA (gptrlo),Y
        RTS
.multicolor
        LDY #0
        LDA gmask
        EOR #$ff
        AND (gptrlo),Y
        STA gtemp
        LDA gsource
        AND #3
        ASL
        ASL
        STA digit
        LDA gxlo
        AND #3
        ORA digit
        TAX
        LDA multibits,X
        ORA gtemp
        STA (gptrlo),Y
.outside
        RTS

; Resolve a high-resolution pixel into a bitmap byte and one bit mask.
; C=1 means that gx:gy is inside the picture; C=0 means it is clipped.
pixeladdress
        LDA gfxmode
        BEQ .reject
        LDA gyhi
        BNE .reject
        LDA gy
        CMP #200
        BCS .reject
        LDA gfxmode
        CMP #3
        BCC .hires_x
        LDA gxhi
        BNE .reject
        LDA gxlo
        CMP #160
        BCS .reject
        JMP .xvalid
.reject
        CLC
        RTS
.hires_x
        LDA gxhi
        CMP #2
        BCS .reject
        CMP #1
        BNE .xvalid
        LDA gxlo
        CMP #64                 ; $0140 = 320
        BCS .outside
.xvalid
        LDA #0
        STA gptrlo
        LDA bitmaphi
        STA gptrhi
        LDA gy
        LSR
        LSR
        LSR
        TAX
        BEQ .rowready
.row   CLC
        LDA gptrlo
        ADC #$40               ; one character row is 320 bytes
        STA gptrlo
        LDA gptrhi
        ADC #1
        STA gptrhi
        DEX
        BNE .row
.rowready
        LDA gfxmode
        CMP #3
        BCC .hires_offset
        LDA gxlo
        ASL
        STA tmplo
        LDA #0
        ROL
        STA tmphi
        LDA tmplo
        AND #$f8
        CLC
        ADC gptrlo
        STA gptrlo
        LDA tmphi
        ADC gptrhi
        STA gptrhi
        JMP .vertical_offset
.hires_offset
        LDA gxlo
        AND #$f8
        CLC
        ADC gptrlo
        STA gptrlo
        LDA gxhi
        ADC gptrhi
        STA gptrhi
.vertical_offset
        LDA gy
        AND #7
        CLC
        ADC gptrlo
        STA gptrlo
        BCC .byte
        INC gptrhi
.byte   LDA gfxmode
        CMP #3
        BCC .hires_mask
        LDA gxlo
        AND #3
        TAX
        LDA pairbits,X
        STA gmask
        SEC
        RTS
.hires_mask
        LDA gxlo
        AND #7
        TAX
        LDA pixelbits,X
        STA gmask
        SEC
        RTS
.outside
        CLC
        RTS

; Test whether a pixel can be filled. A nonzero source fills zero bits;
; source zero erases set bits. Return C=1 for a matching target pixel.
paint_target
        JSR pixeladdress
        BCC .no
        LDY #0
        LDA (gptrlo),Y
        AND gmask
        LDX gsource
        BEQ .erase_target
        CMP #0
        BEQ .yes
        CLC
        RTS
.erase_target
        CMP #0
        BNE .yes
.no     CLC
        RTS
.yes    SEC
        RTS

; PAINT source,x,y[,boundary-mode] fills one connected region. Each stack
; entry stores a three-byte seed in unused BASIC program RAM above endlo:endhi.
; The stack stops before the graphics allocation or the downward string heap.
; Scan lines push only the first pixel of an adjacent run.
paintstmt
        JSR sourceexpr
        LDA failed
        BEQ .source
        RTS
.source
        JSR commaexpr_x
        LDA failed
        BEQ .x
        RTS
.x     LDA vallo
        STA gxlo
        LDA valhi
        STA gxhi
        JSR commaexpr_y
        LDA failed
        BEQ .y
        RTS
.y     LDA vallo
        STA gy
        LDA valhi
        STA gyhi
        JSR spaces
        CMP #44
        BNE .start
        JSR commaexpr           ; accept the documented boundary argument
.start  LDA srclo
        STA paint_saved_srclo
        LDA srchi
        STA paint_saved_srchi
        LDA endlo
        STA paint_toplo
        LDA endhi
        STA paint_tophi
        JSR paint_target
        BCS .seed
        JMP paint_finish
.seed
        JSR paint_push
        LDA failed
        BEQ .popnext
        JMP paint_finish
.popnext
        LDA paint_toplo
        CMP endlo
        BNE .pop
        LDA paint_tophi
        CMP endhi
        BNE .pop
        JMP paint_finish
.pop    SEC
        LDA paint_toplo
        SBC #3
        STA paint_toplo
        LDA paint_tophi
        SBC #0
        STA paint_tophi
        LDA paint_toplo
        STA srclo
        LDA paint_tophi
        STA srchi
        LDY #0
        LDA (srclo),Y
        STA gxlo
        INY
        LDA (srclo),Y
        STA gxhi
        INY
        LDA (srclo),Y
        STA gy
        STA paint_row
        LDA #0
        STA gyhi
        JSR paint_target
        BCC .popnext
.seek_left
        JSR paint_target
        BCC .left_edge
        LDA gxlo
        BNE .left_dec
        DEC gxhi
.left_dec
        DEC gxlo
        JMP .seek_left
.left_edge
        INC gxlo
        BNE .scan_ready
        INC gxhi
.scan_ready
        LDA #0
        STA paint_above
        STA paint_below
.scan   JSR paint_target
        BCC .popnext
        JSR plotpixel
        LDA paint_row
        BEQ .below
        SEC
        SBC #1
        STA gy
        JSR paint_target
        BCC .above_clear
        LDA paint_above
        BNE .above_restore
        JSR paint_push
        LDA failed
        BEQ .above_pushed
        JMP paint_finish
.above_pushed
        LDA #1
        STA paint_above
        BNE .above_restore
.above_clear
        LDA #0
        STA paint_above
.above_restore
        LDA paint_row
        STA gy
.below  LDA paint_row
        CMP #199
        BCS .advance
        CLC
        ADC #1
        STA gy
        JSR paint_target
        BCC .below_clear
        LDA paint_below
        BNE .below_restore
        JSR paint_push
        LDA failed
        BEQ .below_pushed
        JMP paint_finish
.below_pushed
        LDA #1
        STA paint_below
        BNE .below_restore
.below_clear
        LDA #0
        STA paint_below
.below_restore
        LDA paint_row
        STA gy
.advance
        INC gxlo
        BNE .scan
        INC gxhi
        JMP .scan
.done   JMP paint_finish

; Restore the BASIC source pointer after temporary stack use.
; Arguments: paint_saved_src* hold the entry pointer. Returns: to PAINT's caller.
paint_finish
        LDA paint_saved_srclo
        STA srclo
        LDA paint_saved_srchi
        STA srchi
        RTS

; Push gx:gy above the stored program, checking graphics and string limits.
; Arguments: paint_top is the first free byte. Returns: top advances by three,
; or failed reports insufficient fill workspace.
paint_push
        CLC
        LDA paint_toplo
        ADC #3
        STA paint_nextlo
        LDA paint_tophi
        ADC #0
        STA paint_nexthi
        LDA paint_nexthi
        CMP gfxlimit
        BCC .string_limit
        BNE .overflow
        LDA paint_nextlo
        BNE .overflow
.string_limit
        LDA paint_nexthi
        CMP str_tophi
        BCC .room
        BNE .overflow
        LDA paint_nextlo
        CMP str_toplo
        BCC .room
        BNE .overflow
.room   LDA paint_toplo
        STA srclo
        LDA paint_tophi
        STA srchi
        LDY #0
        LDA gxlo
        STA (srclo),Y
        INY
        LDA gxhi
        STA (srclo),Y
        INY
        LDA gy
        STA (srclo),Y
        LDA paint_nextlo
        STA paint_toplo
        LDA paint_nexthi
        STA paint_tophi
        RTS
.overflow
        JMP paint_memory_error

paint_memory_error
        LDA #1
        STA failed
        JSR error_to_text
        LDA #memmsg%256
        STA msglo
        LDA #memmsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

; CHAR source,column,row,"text" copies eight ROM glyph bytes per character
; into the interleaved bitmap. ASCII letters use the same screen-code mapping
; as the system ROM text printer, and each new glyph advances one column.
charstmt
        JSR sourceexpr
        LDA failed
        BEQ .source
        RTS
.source
        JSR commaexpr
        LDA failed
        BEQ .column
        RTS
.column
        LDA valhi
        BEQ .column_byte
        JMP badargument
.column_byte
        LDA vallo
        CMP #40
        BCC .column_ok
        JMP badargument
.column_ok
        STA char_column
        JSR commaexpr
        LDA failed
        BEQ .row
        RTS
.row    LDA valhi
        BEQ .row_byte
        JMP badargument
.row_byte
        LDA vallo
        CMP #25
        BCC .row_ok
        JMP badargument
.row_ok
        STA char_row
        JSR spaces
        CMP #44
        BEQ .hascomma
        JMP badargument
.hascomma
        INC pos
        JSR string_parse
        BCS .string_ready
        JMP badargument
.string_ready
        LDA #0
        STA char_reverse
        JSR spaces
        CMP #44
        BNE .ready_to_draw
        JSR commaexpr
        LDA valhi
        BEQ .reverse_byte
        JMP badargument
.reverse_byte
        LDA vallo
        CMP #2
        BCC .reverse_valid
        JMP badargument
.reverse_valid
        STA char_reverse
.ready_to_draw
        LDA str_ptrlo
        STA srclo
        LDA str_ptrhi
        STA srchi
        LDA #0
        STA str_index
.nextchar
        LDY str_index
        CPY str_len
        BEQ .finished
        JSR rampeek_src
        INC str_index
        STA gtemp
        JSR char_screen_code
        STA digit
        LDA digit
        AND #31
        ASL
        ASL
        ASL
        STA glyphlo
        LDA digit
        LSR
        LSR
        LSR
        LSR
        LSR
        CLC
        ADC #$d0
        STA glyphhi
        LDA char_column
        CMP #40
        BCS .advance
        LDA gfxmode
        BEQ .textglyph
        LDA char_column
        ASL
        STA gxlo
        LDA #0
        ROL
        STA gxhi
        ASL gxlo
        ROL gxhi
        ASL gxlo
        ROL gxhi
        LDA char_row
        ASL
        ASL
        ASL
        STA gy
        LDA #0
        STA gyhi
        JSR pixeladdress
        BCC .advance
        LDY #0
.copyglyph
        LDA (glyphlo),Y
        LDX char_reverse
        BEQ .normal_glyph
        EOR #$ff
.normal_glyph
        LDX gsource
        BNE .writeglyph
        LDA #0
.writeglyph
        STA (gptrlo),Y
        INY
        CPY #8
        BNE .copyglyph
        JMP .advance
.textglyph
        JSR char_text_cell
.advance
        INC char_column
        JMP .nextchar
.finished
        RTS

; Place one native code directly in the active 40×25 text matrix.
char_text_cell
        LDA #0
        STA gptrlo
        LDA #$0c
        STA gptrhi
        LDX char_row
        BEQ .add_column
.row   CLC
        LDA gptrlo
        ADC #40
        STA gptrlo
        BCC .next
        INC gptrhi
.next   DEX
        BNE .row
.add_column
        CLC
        LDA gptrlo
        ADC char_column
        STA gptrlo
        BCC .write
        INC gptrhi
.write  LDY #0
        LDA digit
        LDX char_reverse
        BEQ .normal_code
        ORA #$80
.normal_code
        STA (gptrlo),Y
        RTS

; Convert a quoted PETSCII or ASCII byte to the ROM glyph index for CHAR.
; Arguments: A is the source byte. Returns: A is the screen-code glyph index.
char_screen_code
        CMP #$c0                ; shifted PETSCII graphics, e.g. $CD/$CE
        BCC .ordinary
        CMP #$e0
        BCS .literal
        AND #$7f                ; $CD/$CE select diagonal glyphs $4D/$4E
        RTS
.ordinary
        CMP #65
        BCC .literal
        CMP #91
        BCS .lower
        SEC
        SBC #64
        RTS
.lower  CMP #97
        BCC .literal
        CMP #123
        BCS .literal
        SEC
        SBC #96
.literal
        RTS

; CIRCLE source,x,y,radius draws eight symmetric octants using a signed
; 16-bit midpoint error. It clips naturally at the picture boundaries.
circlestmt
        JSR sourceexpr
        LDA failed
        BEQ .sourceparsed
        RTS
.sourceparsed
        JSR commaexpr_x
        LDA failed
        BEQ .centerxparsed
        RTS
.centerxparsed
        LDA vallo
        STA cxlo
        LDA valhi
        STA cxhi
        JSR commaexpr_y
        LDA failed
        BEQ .centeryparsed
        RTS
.centeryparsed
        LDA vallo
        STA cy
        JSR commaexpr_x
        LDA failed
        BEQ .radiusparsed
        RTS
.radiusparsed
        LDA valhi
        BEQ .radiusbyte
        JMP badargument
.radiusbyte
        LDA vallo
        CMP #200
        BCC .radius_valid
        JMP badargument
.radius_valid
        STA radius
        STA yradius
        BNE .nonzero_radius
        LDA cxlo
        STA gxlo
        LDA cxhi
        STA gxhi
        LDA cy
        STA gy
        LDA #0
        STA gyhi
        JMP plotpixel
.nonzero_radius
        JSR spaces
        CMP #44
        BNE .radii_ready
        JSR commaexpr_y
        LDA failed
        BEQ .y_radius_parsed
        RTS
.y_radius_parsed
        LDA valhi
        BEQ .y_radius_byte
        JMP badargument
.y_radius_byte
        LDA vallo
        CMP #200
        BCC .y_radius_valid
        JMP badargument
.y_radius_valid
        STA yradius
.radii_ready
        LDA #0
        STA circle_opt
        STA circle_startlo
        STA circle_starthi
        STA circle_rotation
        LDA #$68               ; default end angle 360 degrees
        STA circle_endlo
        LDA #1
        STA circle_endhi
        LDA #2                 ; default curve sampling parameter
        STA circle_step
.option
        JSR spaces
        CMP #44
        BNE .options_done
        INC pos
        JSR spaces
        CMP #44                ; empty optional argument
        BEQ .next_option
        CMP #58
        BEQ .next_option
        CMP #0
        BEQ .next_option
        JSR expr
        LDA failed
        BEQ .option_value
        JMP .done
.option_value
        LDX circle_opt
        CPX #0
        BNE .not_start
        LDA vallo
        STA circle_startlo
        LDA valhi
        STA circle_starthi
        JMP .next_option
.not_start
        CPX #1
        BNE .not_end
        LDA vallo
        STA circle_endlo
        LDA valhi
        STA circle_endhi
        JMP .next_option
.not_end
        LDA valhi
        BEQ .short_option
        JMP .invalid_option
.short_option
        CPX #2
        BNE .step_value
        LDA vallo
        STA circle_rotation
        JMP .next_option
.step_value
        LDA vallo
        STA circle_step
.next_option
        INC circle_opt
        LDA circle_opt
        CMP #4
        BCC .option
.options_done
        LDA circle_step
        BNE .has_step
        JMP badargument
.has_step
        CMP #3
        BCC .midpoint
        LDA radius             ; polygon offsets use signed Q6 bytes
        CMP #128
        BCC .x_radius_ok
        JMP badargument
.x_radius_ok
        LDA yradius
        CMP #128
        BCC .y_radius_ok
        JMP badargument
.y_radius_ok
        JSR $e2e6              ; stepped closed outline in system ROM
        RTS
.midpoint
        LDA circle_rotation
        BEQ .unrotated
        JSR $e2e0             ; system-ROM fixed-point rotation setup
.unrotated
        LDA radius
        STA circle_x
        LDA #0
        STA circle_y
        SEC
        LDA #1
        SBC radius
        STA circle_errlo
        LDA #0
        SBC #0
        STA circle_errhi
.loop   LDA circle_x
        CMP circle_y
        BCC .done
        STA oct_x
        LDA circle_y
        STA oct_y
        JSR circle_scale_y
        JSR circle_four_select
        LDA circle_y
        STA oct_x
        LDA circle_x
        STA oct_y
        JSR circle_scale_y
        JSR circle_four_select
        INC circle_y
        LDA circle_errhi
        BMI .errnegative
        DEC circle_x
        SEC
        LDA circle_y
        SBC circle_x
        STA tmplo
        LDA #0
        SBC #0
        STA tmphi
        JSR circle_adjust
        JMP .loop
.errnegative
        LDA circle_y
        STA tmplo
        LDA #0
        STA tmphi
        JSR circle_adjust
        JMP .loop
.bad    JMP badargument
.invalid_option
        JMP badargument
.done   RTS

; Plot the four ellipse quadrants with the optional geometric rotation.
; Arguments: oct_x/oct_y are positive offsets. Returns: plotted pixels.
circle_four_select
        LDA circle_rotation
        BEQ .plain
        JMP $e2e3             ; rotated four-point routine in system ROM
.plain  JMP circle_four

; Add 2*signed(tmphi:tmplo)+1 to the midpoint error.
circle_adjust
        ASL tmplo
        ROL tmphi
        INC tmplo
        BNE .add
        INC tmphi
.add    CLC
        LDA tmplo
        ADC circle_errlo
        STA circle_errlo
        LDA tmphi
        ADC circle_errhi
        STA circle_errhi
        RTS

; Convert a circle's vertical offset to an ellipse offset. Both radii are
; at most 199, so a 16-bit product and repeated subtraction suffice.
circle_scale_y
        LDA radius
        BEQ .zero
        LDA oct_y
        STA termlo
        LDA #0
        STA termhi
        LDA yradius
        STA vallo
        LDA #0
        STA valhi
        JSR multiply
        LDX #0
.divide
        LDA termhi
        BNE .subtract
        LDA termlo
        CMP radius
        BCC .quotient
.subtract
        SEC
        LDA termlo
        SBC radius
        STA termlo
        LDA termhi
        SBC #0
        STA termhi
        INX
        JMP .divide
.quotient
        STX oct_y
        RTS
.zero   LDA #0
        STA oct_y
        RTS

; Draw all four sign combinations of (oct_x,oct_y) around (cx,cy).
circle_four
        CLC
        LDA cxlo
        ADC oct_x
        STA gxlo
        LDA cxhi
        ADC #0
        STA gxhi
        CLC
        LDA cy
        ADC oct_y
        STA gy
        LDA #0
        ADC #0
        STA gyhi
        JSR plotpixel
        SEC
        LDA cxlo
        SBC oct_x
        STA gxlo
        LDA cxhi
        SBC #0
        STA gxhi
        JSR plotpixel
        CLC
        LDA cxlo
        ADC oct_x
        STA gxlo
        LDA cxhi
        ADC #0
        STA gxhi
        SEC
        LDA cy
        SBC oct_y
        STA gy
        LDA #0
        SBC #0
        STA gyhi
        JSR plotpixel
        SEC
        LDA cxlo
        SBC oct_x
        STA gxlo
        LDA cxhi
        SBC #0
        STA gxhi
        JMP plotpixel

pixelbits BYT $80,$40,$20,$10,$08,$04,$02,$01
pairbits BYT $c0,$30,$0c,$03
multibits BYT $00,$00,$00,$00,$40,$10,$04,$01
          BYT $80,$20,$08,$02,$c0,$30,$0c,$03

; COLOR source,hue[,luminance] updates TED palette registers. Foreground
; source one fills the 1000 color cells used by both text and bitmap modes.
colorstmt
        JSR expr
        LDA failed
        BEQ .sourceparsed
        RTS
.sourceparsed
        LDA valhi
        BEQ .sourcebyte
        JMP badargument
.sourcebyte
        LDA vallo
        CMP #5
        BCC .sourcevalid
        JMP badargument
.sourcevalid
        STA gsource
        JSR commaexpr
        LDA failed
        BEQ .hue_parsed
        RTS
.hue_parsed
        LDA valhi
        BEQ .hue_byte
        JMP badargument
.hue_byte
        LDA vallo
        BNE .hue_positive
        JMP badargument
.hue_positive
        CMP #17
        BCS .bad
        SEC
        SBC #1
        STA gtemp               ; hue encoded in low nibble
        JSR spaces
        CMP #44
        BNE .default_luminance
        JSR commaexpr
        LDA valhi
        BNE .bad
        LDA vallo
        CMP #8
        BCS .bad
        JMP .encode
.default_luminance
        LDA #7
.encode
        ASL
        ASL
        ASL
        ASL
        ORA gtemp
        STA gtemp
        LDA gsource
        BEQ .background
        CMP #1
        BEQ .foreground
        CMP #2
        BEQ .extra1
        CMP #3
        BEQ .extra2
        LDA gtemp
        STA $ff19
        RTS
.background
        LDA gtemp
        STA $ff15
        RTS
.extra1
        LDA gtemp
        STA $ff16
        RTS
.extra2
        LDA gtemp
        STA $ff17
        RTS
.foreground
        LDA #0
        STA dstlo
        LDA gfxmode
        BEQ .textmatrix
        LDA gfxmatrix
        BNE .matrixchosen
.textmatrix
        LDA #8
.matrixchosen
        STA dsthi
        CLC
        ADC #3
        STA srchi
        LDY #0
.fill   LDA gtemp
        STA (dstlo),Y
        INC dstlo
        BNE .next
        INC dsthi
.next   LDA dsthi
        CMP srchi
        BNE .fill
        LDA dstlo
        CMP #$e8
        BNE .fill
.done   RTS
.bad    JMP badargument

; The virtual drive uses an explicit byte command ($83) and read request
; ($84) at $FEF0. A secondary address starts filename, data, or close phases.
iec_send
        PHA
        LDA #$83
        STA $fef0
        PLA
        STA $fef0
        RTS
iec_secondary
        PHA
        LDA #$82
        STA $fef0
        PLA
        STA $fef0
        RTS
iec_read
        LDA #$84
        STA $fef0
        LDA $fef0
        RTS

; Send a string filename to the virtual IEC drive. The ROM parses the bytes;
; the drive root constrains the resulting relative path on the host.
iec_filename
        JSR string_parse
        BCS .parsed
        JMP badargument
.parsed
        LDA str_ptrlo
        STA srclo
        LDA str_ptrhi
        STA srchi
        LDA #0
        STA str_index
        LDA #$f0
        JSR iec_secondary
.name   LDY str_index
        CPY str_len
        BEQ .endname
        JSR rampeek_src
        JSR iec_send
        INC str_index
        JMP .name
.endname
        RTS

; DSAVE stores an independent free-ROM binary: magic "FBR1", a 16-bit byte
; count, then the packed numbered records. It never writes a host file until
; the E* secondary close command completes.
dsavestmt
        JSR iec_filename
        LDA failed
        BNE .done
        LDA #1                  ; nonzero channel selects writing
        JSR iec_secondary
        LDA #70                 ; FBR1 signature
        JSR iec_send
        LDA #66
        JSR iec_send
        LDA #82
        JSR iec_send
        LDA #49
        JSR iec_send
        SEC
        LDA endlo
        SBC #$e8
        STA newlo
        LDA endhi
        SBC #$0f
        STA newhi
        LDA newlo
        JSR iec_send
        LDA newhi
        JSR iec_send
        LDA #$e8
        STA ptrlo
        LDA #$0f
        STA ptrhi
.byte   LDA ptrhi
        CMP endhi
        BNE .send
        LDA ptrlo
        CMP endlo
        BEQ .close
.send   LDY #0
        JSR rampeek_ptr
        JSR iec_send
        INC ptrlo
        BNE .byte
        INC ptrhi
        JMP .byte
.close  LDA #$e0
        JSR iec_secondary
        LDA $fef1
        BEQ .done
        JMP disk_error
.done   RTS

; DLOAD accepts our packed FBR1 records or a standard tokenized BASIC PRG.
; Both formats are decoded here in the ROM; the virtual drive only sends bytes.
dloadstmt
        JSR iec_filename
        LDA failed
        BEQ .named
        RTS
.named
        LDA #loadingmsg%256
        STA msglo
        LDA #loadingmsg/256
        STA msghi
        JSR printz
        JSR newline
        LDA #0
        JSR iec_secondary
        LDA $fef1
        BEQ .signature
        JMP disk_error
.signature
        JSR iec_read
        CMP #$01                ; standard BASIC load address is $4001
        BNE .packed
        JMP dload_prg
.packed
        CMP #70
        BNE .signature_bad
        JSR iec_read
        CMP #66
        BNE .signature_bad
        JSR iec_read
        CMP #82
        BNE .signature_bad
        JSR iec_read
        CMP #49
        BNE .signature_bad
        JMP .signature_good
.signature_bad
        JMP dload_badfile
.signature_good
        JSR iec_read
        STA newlo
        JSR iec_read
        STA newhi
        CLC
        LDA newlo
        ADC #$e8
        STA newlo
        LDA newhi
        ADC #$0f
        STA newhi
        BCS .range_bad          ; wrapped length must never reach I/O
        LDA newhi
        CMP gfxlimit
        BCC .room
        BNE .range_bad
        LDA newlo
        BEQ .room
.range_bad
        JMP dload_badfile
.room   LDA #$e8
        STA ptrlo
        LDA #$0f
        STA ptrhi
.copy   LDA ptrhi
        CMP newhi
        BNE .readbyte
        LDA ptrlo
        CMP newlo
        BEQ .complete
.readbyte
        JSR iec_read
        LDY #0
        STA (ptrlo),Y
        INC ptrlo
        BNE .copy
        INC ptrhi
        JMP .copy
.complete
        LDA #$e8
        STA ptrlo
        LDA #$0f
        STA ptrhi
.validate
        LDA ptrhi
        CMP newhi
        BNE .record
        LDA ptrlo
        CMP newlo
        BEQ .valid
.record
        LDY #2
        JSR rampeek_ptr
        CMP #128                ; loaded source must fit below variables
        BCS dload_badfile
        CLC
        ADC #3
        CLC
        ADC ptrlo
        STA tmplo
        LDA ptrhi
        ADC #0
        STA tmphi
        CMP newhi
        BCC .advance_record
        BNE dload_badfile
        LDA tmplo
        CMP newlo
        BCC .advance_record
        BNE dload_badfile
.advance_record
        LDA tmplo
        STA ptrlo
        LDA tmphi
        STA ptrhi
        JMP .validate
.valid
        LDA newlo
        STA endlo
        LDA newhi
        STA endhi
        LDA #$e0
        JSR iec_secondary
        JSR clearvars
        LDA runmode
        BEQ .done
        LDA #$e8
        STA runlo
        LDA #$0f
        STA runhi
        LDA #2
        STA restart
.done   RTS
dload_badfile
        LDA #$e8               ; malformed input cannot remain a program
        STA endlo
        LDA #$0f
        STA endhi
        JMP disk_error

; Read a standard $4001 or $1001 BASIC PRG. Each line contains a next pointer,
; a little-endian line number, and tokenized text ending in zero. Translate
; only the tokens supported by this ROM into its editable source records.
; Arguments: IEC channel is open and its first byte ($01) was consumed.
; Returns: a replaced program and cleared variables, or a disk error.
dload_prg
        JSR iec_read
        CMP #$40
        BEQ .address_ok
        CMP #$10
        BEQ .address_ok
        JMP .bad
.address_ok
        LDA #$e8
        STA endlo
        LDA #$0f
        STA endhi
.line   JSR iec_read            ; linked-list next address, low byte
        STA tmplo
        JSR iec_read            ; high byte; $0000 ends the program
        ORA tmplo
        BEQ .done
        JSR iec_read
        STA linelo
        JSR iec_read
        STA linehi
        LDA #0
        STA count               ; source length
        STA trail               ; quoted string or REM literal state
.char   JSR iec_read
        BEQ .endline
        STA digit
        CMP #34
        BNE .notquote
        LDA trail
        EOR #1
        STA trail
        LDA digit
.notquote
        LDA trail
        BNE .literal
        LDA digit
        BPL .literal
        JSR dload_token
        BCC .bad
        JMP .char
.literal
        LDX count
        CPX #127                ; $0200-$027F precedes variable RAM
        BCS .bad
        LDA digit
        STA input,X
        INC count
        JMP .char
.endline
        LDX count
        LDA #0
        STA input,X
        STA pos
        JSR storeline
        JMP .line
.done   LDA #$e0
        JSR iec_secondary
        JSR clearvars
        LDA #0                  ; storeline reused the direct input buffer
        STA input              ; stop direct execution at the load command
        STA pos
        LDA runmode
        BEQ .return
        LDA #$e8               ; DLOAD in a program chains to the new first line
        STA runlo
        LDA #$0f
        STA runhi
        LDA #2
        STA restart
.return
        RTS
.bad    JMP dload_badfile

; Expand a token by looking it up in the ROM's compact token/text table.
; Arguments: digit is the token, count is the current source length.
; Returns: carry set on success, clear for an unsupported/oversize token.
dload_token
        LDA #dload_tokens%256
        STA srclo
        LDA #dload_tokens/256
        STA srchi
.next   LDY #0
        LDA (srclo),Y
        BEQ .unknown
        CMP digit
        BEQ .copy
.skip   INC srclo
        BNE .skipcarry
        INC srchi
.skipcarry
        LDA (srclo),Y
        BNE .skip
        JSR dload_advance
        JMP .next
.copy   JSR dload_advance
.byte   LDA (srclo),Y
        BEQ .found
        LDX count
        CPX #127
        BCS .unknown
        STA input,X
        INC count
        JSR dload_advance
        JMP .byte
.found  SEC
        RTS
.unknown
        CLC
        RTS

; Advance the source pointer in the token table across page boundaries.
; Arguments: srclo/srchi address a table byte. Returns: next address.
dload_advance
        INC srclo
        BNE .done
        INC srchi
.done   RTS

; Token words needed to decode the present PRG subset into editable source.
; Loading can preserve keywords that the interpreter cannot execute yet.
; Unknown tokens report a file error instead of storing invalid source.
dload_tokens
        BYT $81," FOR ",0
        BYT $82," NEXT ",0
        BYT $89," GOTO ",0
        BYT $8b," IF ",0
        BYT $8f," REM ",0
        BYT $96," DEF ",0
        BYT $97," POKE ",0
        BYT $99," PRINT ",0
        BYT $a4," TO ",0
        BYT $a5," FN ",0
        BYT $a7," THEN ",0
        BYT $a9," STEP ",0
        BYT $aa,"+",0
        BYT $ab,"-",0
        BYT $ac,"*",0
        BYT $ad,"/",0
        BYT $b1,">",0
        BYT $b2,"=",0
        BYT $b3,"<",0
        BYT $b5," INT ",0
        BYT $bb," RND ",0
        BYT $ba," SQR ",0
        BYT $bf," SIN ",0
        BYT $cd," RCLR ",0
        BYT $ce," RLUM ",0
        BYT $d5," ELSE ",0
        BYT $de," GRAPHIC ",0
        BYT $df," PAINT ",0
        BYT $e0," CHAR ",0
        BYT $e1," BOX ",0
        BYT $e2," CIRCLE ",0
        BYT $e5," DRAW ",0
        BYT 0

loadingmsg BYT "LOADING",0

disk_error
        LDA #1
        STA failed
        JSR error_to_text
        LDA #diskmsg%256
        STA msglo
        LDA #diskmsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

diskmsg BYT "?DISK ERROR",0

kwgraphic BYT "GRAPHIC",0
kwmonitor BYT "MONITOR",0
kwpaint BYT "PAINT",0
kwchar BYT "CHAR",0
kwscnclr BYT "SCNCLR",0
kwscale BYT "SCALE",0
kwvol BYT "VOL",0
kwsound BYT "SOUND",0
kwlocate BYT "LOCATE",0
kwdraw BYT "DRAW",0
kwcircle BYT "CIRCLE",0
kwbox BYT "BOX",0
kwcolor BYT "COLOR",0
kwdsave BYT "DSAVE",0
kwdload BYT "DLOAD",0
kwpoke BYT "POKE",0
kwdef BYT "DEF",0
