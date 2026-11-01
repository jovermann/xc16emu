; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; One compact bitmap shape slot for SSHAPE/GSHAPE. This BASIC subset does not
; yet have general string variables, so the slot remembers its A$..Z$ name.
; Pixels are packed eight to a byte, with a 255-byte maximum payload.

shape_name EQU $0323
shape_width EQU $0324
shape_height EQU $0325
shape_valid EQU $0326
shape_data_hi EQU $0327
shape_x1lo EQU $0328
shape_x1hi EQU $0329
shape_y1 EQU $032a
shape_x2lo EQU $032b
shape_x2hi EQU $032c
shape_y2 EQU $032d
shape_byte EQU $032e
shape_mask EQU $032f
shape_count EQU $0330
shape_col EQU $0331
shape_row EQU $0332
shape_request EQU $0333
shape_mode EQU $0334
shape_bit EQU $0335
shape_old EQU $0336
shape_saved_xlo EQU $0337
shape_saved_xhi EQU $0338
shape_saved_y EQU $0339
shape_saved_yhi EQU $033a

; Read a single-letter shape string name, e.g. A$. Store its letter in
; shape_request and leave pos just after the dollar sign.
shape_variable
        JSR spaces
        CMP #65
        BCC .bad
        CMP #91
        BCS .bad
        STA shape_request
        INC pos
        LDY pos
        LDA input,Y
        CMP #36
        BNE .bad
        INC pos
        RTS
.bad    JMP badargument

; SSHAPE A$,x1,y1,x2,y2 captures a rectangular monochrome region. The shape
; slot sits in the page before the dedicated graphics color matrix, so an
; existing BASIC program that reaches that page causes an out-of-memory error.
sshapestmt
        LDA gfxmode
        CMP #1
        BCS .hires_min
        JMP badargument
.hires_min
        CMP #3
        BCC .hires_mode
        JMP badargument        ; multicolor would need two-bit shape data
.hires_mode
        JSR shape_variable
        LDA failed
        BEQ .name
        RTS
.name   JSR commaexpr_x
        LDA failed
        BEQ .x1
        RTS
.x1    LDA vallo
        STA shape_x1lo
        LDA valhi
        STA shape_x1hi
        JSR commaexpr_y
        LDA failed
        BEQ .y1
        RTS
.y1    LDA valhi
        BEQ .y1byte
        JMP badargument
.y1byte
        LDA vallo
        STA shape_y1
        JSR commaexpr_x
        LDA failed
        BEQ .x2
        RTS
.x2    LDA vallo
        STA shape_x2lo
        LDA valhi
        STA shape_x2hi
        JSR commaexpr_y
        LDA failed
        BEQ .y2
        RTS
.y2    LDA valhi
        BEQ .y2byte
        JMP badargument
.y2byte
        LDA vallo
        STA shape_y2
        CMP #200
        BCC .yrange
        JMP badargument
.yrange
        SEC
        SBC shape_y1
        BCS .height
        JMP badargument
.height
        CLC
        ADC #1
        STA shape_height
        SEC
        LDA shape_x2lo
        SBC shape_x1lo
        STA shape_width
        LDA shape_x2hi
        SBC shape_x1hi
        BEQ .widthhigh
        JMP badargument
.widthhigh
        INC shape_width
        LDA shape_width
        BNE .width
        JMP badargument
.width   LDA shape_x2hi
        CMP #2
        BCC .xrange
        JMP badargument
.xrange
        CMP #1
        BNE .reserve
        LDA shape_x2lo
        CMP #64
        BCC .reserve
        JMP badargument
.reserve
        LDA gfxmatrix
        SEC
        SBC #1
        STA shape_data_hi
        LDA endhi
        CMP shape_data_hi
        BCC .room
        BNE .memory_error
        LDA endlo
        BEQ .room
.memory_error
        JMP shape_memory_error
.room   LDA shape_data_hi
        STA gfxlimit
        STA dsthi
        LDA gxlo
        STA shape_saved_xlo
        LDA gxhi
        STA shape_saved_xhi
        LDA gy
        STA shape_saved_y
        LDA gyhi
        STA shape_saved_yhi
        LDA #0
        STA dstlo
        STA shape_count
        STA shape_byte
        STA shape_valid
        STA shape_row
        LDA #$80
        STA shape_mask
        LDA shape_y1
        STA gy
        LDA #0
        STA gyhi
.rowloop
        LDA shape_x1lo
        STA gxlo
        LDA shape_x1hi
        STA gxhi
        LDA #0
        STA shape_col
.pixel
        JSR pixeladdress
        BCC .capture_zero
        LDY #0
        LDA (gptrlo),Y
        AND gmask
        BEQ .capture_zero
        LDA shape_byte
        ORA shape_mask
        STA shape_byte
.capture_zero
        LSR shape_mask
        BNE .advance_pixel
        JSR shape_flush
        LDA failed
        BEQ .advance_pixel
        JMP shape_restore_cursor
.advance_pixel
        INC gxlo
        BNE .xadvanced
        INC gxhi
.xadvanced
        INC shape_col
        LDA shape_col
        CMP shape_width
        BNE .pixel
        INC gy
        INC shape_row
        LDA shape_row
        CMP shape_height
        BNE .rowloop
        LDA shape_mask
        CMP #$80
        BEQ .complete
        JSR shape_flush
        LDA failed
        BEQ .complete
        JMP shape_restore_cursor
.complete
        LDA shape_request
        STA shape_name
        LDA #1
        STA shape_valid
        JMP shape_restore_cursor
.bad    JMP badargument

shape_restore_cursor
        LDA shape_saved_xlo
        STA gxlo
        LDA shape_saved_xhi
        STA gxhi
        LDA shape_saved_y
        STA gy
        LDA shape_saved_yhi
        STA gyhi
        RTS

; Write one packed byte, reserving the final byte for a maximum of 255.
shape_flush
        LDA shape_count
        CMP #255
        BCS shape_memory_error
        LDY #0
        LDA shape_byte
        STA (dstlo),Y
        INC dstlo
        INC shape_count
        LDA #0
        STA shape_byte
        LDA #$80
        STA shape_mask
        RTS

shape_memory_error
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

; GSHAPE A$[,x,y[,mode]] draws the saved shape at the pixel cursor or the
; supplied point. Modes 0..4 replace, invert, OR, AND, and XOR the bits.
gshapestmt
        LDA gfxmode
        CMP #1
        BCS .hires_min
        JMP badargument
.hires_min
        CMP #3
        BCC .hires_mode
        JMP badargument
.hires_mode
        JSR shape_variable
        LDA failed
        BEQ .named
        RTS
.named  LDA shape_valid
        BNE .valid
        JMP badargument
.valid
        LDA shape_request
        CMP shape_name
        BEQ .matched
        JMP badargument
.matched
        LDA #0
        STA shape_mode
        JSR spaces
        CMP #44
        BNE .start
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
        JSR commaexpr
        LDA valhi
        BEQ .modebyte
        JMP badargument
.modebyte
        LDA vallo
        CMP #5
        BCC .modevalid
        JMP badargument
.modevalid
        STA shape_mode
.start  LDA gxlo
        STA shape_x1lo
        LDA gxhi
        STA shape_x1hi
        LDA gy
        STA shape_y1
        LDA #0
        STA dstlo
        STA shape_row
        LDA shape_data_hi
        STA dsthi
        LDA #$80
        STA shape_mask
.rowloop
        LDA shape_x1lo
        STA gxlo
        LDA shape_x1hi
        STA gxhi
        LDA shape_y1
        CLC
        ADC shape_row
        STA gy
        LDA #0
        ADC #0
        STA gyhi
        LDA #0
        STA shape_col
.pixel
        LDY #0
        LDA (dstlo),Y
        AND shape_mask
        BEQ .shape_zero
        LDA #1
        BNE .shape_ready
.shape_zero
        LDA #0
.shape_ready
        STA shape_bit
        LDA #0
        STA shape_old
        JSR pixeladdress
        BCC .combine
        LDY #0
        LDA (gptrlo),Y
        AND gmask
        BEQ .combine
        LDA #1
        STA shape_old
.combine
        LDA shape_mode
        BEQ .replace
        CMP #1
        BEQ .invert
        CMP #2
        BEQ .or_mode
        CMP #3
        BEQ .and_mode
        LDA shape_bit
        EOR shape_old
        JMP .write
.invert
        LDA shape_bit
        EOR #1
        JMP .write
.or_mode
        LDA shape_bit
        ORA shape_old
        JMP .write
.and_mode
        LDA shape_bit
        AND shape_old
        JMP .write
.replace
        LDA shape_bit
.write  STA gsource
        JSR plotpixel
        LSR shape_mask
        BNE .next
        LDA #$80
        STA shape_mask
        INC dstlo
.next   INC gxlo
        BNE .xadvanced
        INC gxhi
.xadvanced
        INC shape_col
        LDA shape_col
        CMP shape_width
        BEQ .rowadvance
        JMP .pixel
.rowadvance
        INC shape_row
        LDA shape_row
        CMP shape_height
        BEQ .done
        JMP .rowloop
.done
        RTS
.bad    JMP badargument

kwsshape BYT "SSHAPE",0
kwgshape BYT "GSHAPE",0
