; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Integer-compatible BASIC functions executed by the emulated 7501. More
; functions will share this dispatch once the ROM floating accumulator lands.

num_sign EQU $a0
instr_haylo EQU $03f4
instr_hayhi EQU $03f5
instr_haylen EQU $03f6
instr_needlo EQU $03f7
instr_needhi EQU $03f8
instr_needlen EQU $03f9
instr_position EQU $a7
instr_offset EQU $a8
instr_byte EQU $a9

; Recognize a numeric function at input[pos]. Returns carry set on a match,
; with vallo/valhi holding its result; otherwise leaves pos unchanged.
numeric_function
        LDA #kwabs%256
        STA kwlo
        LDA #kwabs/256
        STA kwhi
        JSR keyword
        BCC .sqr
        JSR number_abs
        SEC
        RTS
.sqr   LDA #kwsqr%256
        STA kwlo
        LDA #kwsqr/256
        STA kwhi
        JSR keyword
        BCC .sgn
        JSR number_sqr
        SEC
        RTS
.sgn    LDA #kwsgn%256
        STA kwlo
        LDA #kwsgn/256
        STA kwhi
        JSR keyword
        BCC .int
        JSR number_sgn
        SEC
        RTS
.int    LDA #kwint%256
        STA kwlo
        LDA #kwint/256
        STA kwhi
        JSR keyword
        BCC .fre
        JSR number_argument
        SEC
        RTS
.fre    LDA #kwfre%256
        STA kwlo
        LDA #kwfre/256
        STA kwhi
        JSR keyword
        BCC .pos
        JSR number_fre
        SEC
        RTS
.pos    LDA #kwpos%256
        STA kwlo
        LDA #kwpos/256
        STA kwhi
        JSR keyword
        BCC .peek
        JSR number_argument
        LDA column
        STA vallo
        LDA #0
        STA valhi
        SEC
        RTS
.peek   LDA #kwpeek%256
        STA kwlo
        LDA #kwpeek/256
        STA kwhi
        JSR keyword
        BCC .len
        JSR number_peek
        SEC
        RTS
.len    LDA #kwlen%256
        STA kwlo
        LDA #kwlen/256
        STA kwhi
        JSR keyword
        BCC .asc
        JSR number_string_argument
        LDA str_len
        STA vallo
        LDA #0
        STA valhi
        SEC
        RTS
.asc    LDA #kwasc%256
        STA kwlo
        LDA #kwasc/256
        STA kwhi
        JSR keyword
        BCC .val
        JSR number_asc
        SEC
        RTS
.val    LDA #kwval%256
        STA kwlo
        LDA #kwval/256
        STA kwhi
        JSR keyword
        BCC .rgr
        JSR number_val
        SEC
        RTS
.rgr    LDA #kwrgr%256
        STA kwlo
        LDA #kwrgr/256
        STA kwhi
        JSR keyword
        BCC .rclr
        JSR number_argument
        LDA gfxmode
        STA vallo
        LDA #0
        STA valhi
        SEC
        RTS
.rclr  LDA #kwrclr%256
        STA kwlo
        LDA #kwrclr/256
        STA kwhi
        JSR keyword
        BCC .rlum
        JSR number_color
        LDA vallo
        AND #15
        CLC
        ADC #1
        STA vallo
        LDA #0
        STA valhi
        SEC
        RTS
.rlum  LDA #kwrlum%256
        STA kwlo
        LDA #kwrlum/256
        STA kwhi
        JSR keyword
        BCC .joy
        JSR number_color
        LDA vallo
        LSR
        LSR
        LSR
        LSR
        AND #7
        STA vallo
        LDA #0
        STA valhi
        SEC
        RTS
.joy   LDA #kwjoy%256
        STA kwlo
        LDA #kwjoy/256
        STA kwhi
        JSR keyword
        BCC .rdot
        JSR number_joy
        SEC
        RTS
.rdot   LDA #kwrdot%256
        STA kwlo
        LDA #kwrdot/256
        STA kwhi
        JSR keyword
        BCC .instr
        JSR number_rdot
        SEC
        RTS
.instr  LDA #kwinstr%256
        STA kwlo
        LDA #kwinstr/256
        STA kwhi
        JSR keyword
        BCC .miss
        JSR number_instr
        SEC
        RTS
.miss   CLC
        RTS

; Parse a required numeric argument in parentheses using ROM expressions.
; Arguments: pos after a function word. Returns: value or failed.
number_argument
        JSR spaces
        CMP #40
        BEQ .open
        JMP syntax
.open   INC pos
        JSR expr
        LDA failed
        BNE .done
        JSR string_close
.done   RTS

; Parse a required string argument in parentheses.
; Arguments: pos after a function word. Returns: str_ptr/str_len or failed.
number_string_argument
        JSR string_first_argument
        LDA failed
        BNE .done
        JSR string_close
.done   RTS

; ABS returns the magnitude of the signed integer argument.
; Arguments: pos after ABS. Returns: vallo/valhi.
number_abs
        JSR number_argument
        LDA valhi
        BPL .done
        SEC
        LDA #0
        SBC vallo
        STA vallo
        LDA #0
        SBC valhi
        STA valhi
.done   RTS

; SQR computes the integer floor of the square root by subtracting odd
; numbers. Negative inputs receive an illegal-quantity diagnostic.
; Arguments: pos after SQR. Returns: the nonnegative integer root.
number_sqr
        JSR number_argument
        LDA valhi
        BPL .nonnegative
        JMP quantity_error
.nonnegative
        LDA #0
        STA num_sign           ; root
        LDA #1
        STA mullo              ; next odd number
        LDA #0
        STA mulhi
.step   LDA valhi
        CMP mulhi
        BCC .done
        BNE .subtract
        LDA vallo
        CMP mullo
        BCC .done
.subtract
        SEC
        LDA vallo
        SBC mullo
        STA vallo
        LDA valhi
        SBC mulhi
        STA valhi
        INC num_sign
        CLC
        LDA mullo
        ADC #2
        STA mullo
        BCC .step
        INC mulhi
        JMP .step
.done   LDA num_sign
        STA vallo
        LDA #0
        STA valhi
        RTS

; SGN returns minus one, zero, or plus one for a signed argument.
; Arguments: pos after SGN. Returns: vallo/valhi.
number_sgn
        JSR number_argument
        LDA valhi
        BMI .negative
        ORA vallo
        BEQ .zero
        LDA #1
        STA vallo
        LDA #0
        STA valhi
        RTS
.negative
        LDA #$ff
        STA vallo
        STA valhi
        RTS
.zero   LDA #0
        STA vallo
        STA valhi
        RTS

; FRE evaluates its dummy argument then reports the current free RAM bytes.
; Arguments: pos after FRE. Returns: unsigned count in vallo/valhi.
number_fre
        JSR number_argument
number_free_count
        LDA str_tophi
        CMP gfxlimit
        BCC .string_top
        LDA #0
        STA tmplo
        LDA gfxlimit
        BNE .subtract
.string_top
        LDA str_toplo
        STA tmplo
        LDA str_tophi
.subtract
        STA tmphi
        SEC
        LDA tmplo
        SBC endlo
        STA vallo
        LDA tmphi
        SBC endhi
        STA valhi
        RTS

; PEEK reads the CPU-visible byte at the evaluated 16-bit address.
; Arguments: pos after PEEK. Returns: zero-extended byte.
number_peek
        JSR number_argument
        LDA vallo
        STA srclo
        LDA valhi
        STA srchi
        LDY #0
        LDA (srclo),Y
        STA vallo
        LDA #0
        STA valhi
        RTS

; ASC returns the first byte of a nonempty BASIC string.
; Arguments: pos after ASC. Returns: zero-extended code or failed.
number_asc
        JSR number_string_argument
        LDA str_len
        BNE .nonempty
        JMP badargument
.nonempty
        LDA str_ptrlo
        STA srclo
        LDA str_ptrhi
        STA srchi
        LDY #0
        JSR rampeek_src
        STA vallo
        LDA #0
        STA valhi
        RTS

; VAL converts the leading signed decimal integer bytes of a string.
; Arguments: pos after VAL. Returns: signed integer prefix or zero.
number_val
        JSR number_string_argument
        LDA str_ptrlo
        STA srclo
        LDA str_ptrhi
        STA srchi
        LDA #0
        STA vallo
        STA valhi
        STA num_sign
        LDY #0
.spaces CPY str_len
        BEQ .done
        JSR rampeek_src
        CMP #32
        BNE .sign
        INY
        JMP .spaces
.sign   CMP #45
        BNE .plus
        LDA #1
        STA num_sign
        INY
        JMP .digits
.plus   CMP #43
        BNE .digits
        INY
.digits CPY str_len
        BEQ .signed
        JSR rampeek_src
        CMP #48
        BCC .signed
        CMP #58
        BCS .signed
        SEC
        SBC #48
        STA digit
        LDA vallo
        STA tmplo
        LDA valhi
        STA tmphi
        ASL vallo
        ROL valhi
        ASL tmplo
        ROL tmphi
        ASL tmplo
        ROL tmphi
        ASL tmplo
        ROL tmphi
        CLC
        LDA vallo
        ADC tmplo
        STA vallo
        LDA valhi
        ADC tmphi
        STA valhi
        CLC
        LDA vallo
        ADC digit
        STA vallo
        BCC .next
        INC valhi
.next   INY
        JMP .digits
.signed
        LDA num_sign
        BEQ .done
        SEC
        LDA #0
        SBC vallo
        STA vallo
        LDA #0
        SBC valhi
        STA valhi
.done   RTS

; RDOT returns the graphics cursor coordinate or its selected color source.
; Arguments: part=0/1/2. Returns: 16-bit coordinate or source value.
number_rdot
        JSR number_argument
        LDA vallo
        BEQ .x
        CMP #1
        BEQ .y
        CMP #2
        BEQ .pixel
        JMP badargument
.pixel
        LDA cxlo
        STA gxlo
        LDA cxhi
        STA gxhi
        LDA cy
        STA gy
        LDA #0
        STA gyhi
        JSR pixeladdress
        BCC .background
        LDY #0
        LDA (gptrlo),Y
        AND gmask
        STA vallo
        LDA gfxmode
        CMP #3
        BCS .multicolor
        LDA vallo
        BEQ .background
        LDA #1
        BNE .source
.multicolor
        LDA gxlo
        AND #3
        TAX
.shift  CPX #3
        BEQ .source_byte
        LSR vallo
        LSR vallo
        INX
        JMP .shift
.source_byte
        LDA vallo
        AND #3
        JMP .source
.background
        LDA #0
.source
        STA vallo
        STA valhi
        LDA #0
        STA valhi
        RTS
.x      LDA cxlo
        STA vallo
        LDA cxhi
        STA valhi
        RTS
.y      LDA cy
        STA vallo
        LDA #0
        STA valhi
        RTS

; Read the selected TED color byte for RCLR/RLUM source 0..4.
; Arguments: evaluated source in vallo. Returns: encoded color in vallo.
number_color
        JSR number_argument
        LDA vallo
        CMP #5
        BCC .valid
        JMP badargument
.valid  BEQ .background
        CMP #1
        BEQ .foreground
        CMP #2
        BEQ .extra1
        CMP #3
        BEQ .extra2
        LDA $ff19
        JMP .done
.background
        LDA $ff15
        JMP .done
.foreground
        LDA $0800
        JMP .done
.extra1
        LDA $ff16
        JMP .done
.extra2
        LDA $ff17
.done   STA vallo
        LDA #0
        STA valhi
        RTS

; Scan one virtual joystick port through FF08's active-low latch and map
; direction bits to BASIC's 0..8 compass code, with fire adding 128.
; Arguments: port 1 or 2 in the expression. Returns: BASIC JOY value.
number_joy
        JSR number_argument
        LDA vallo
        CMP #1
        BEQ .one
        CMP #2
        BEQ .two
        JMP badargument
.one   LDA #$fb
        STA $ff08
        LDA $ff08
        STA num_sign
        LDA #$40
        BNE .decode
.two   LDA #$fd
        STA $ff08
        LDA $ff08
        STA num_sign
        LDA #$80
.decode STA instr_byte
        LDA num_sign
        EOR #$ff
        AND #15
        TAX
        LDA joy_directions,X
        STA vallo
        LDA num_sign
        AND instr_byte
        BNE .no_fire
        LDA vallo
        ORA #$80
        STA vallo
.no_fire
        LDA #0
        STA valhi
        RTS

joy_directions BYT 0,1,5,0,7,8,6,7,3,2,4,3,0,1,5,0

; INSTR searches a string from a one-based starting position and returns
; the first one-based match, or zero when the needle is absent.
; Arguments: pos after INSTR. Returns: vallo/valhi.
number_instr
        JSR string_first_argument
        LDA failed
        BEQ .first_ok
        JMP .done
.first_ok
        LDA str_ptrlo
        STA instr_haylo
        LDA str_ptrhi
        STA instr_hayhi
        LDA str_len
        STA instr_haylen
        JSR spaces
        CMP #44
        BEQ .second
        JMP syntax
.second INC pos
        JSR string_parse
        BCS .needle
        JMP syntax
.needle LDA str_ptrlo
        STA instr_needlo
        LDA str_ptrhi
        STA instr_needhi
        LDA str_len
        STA instr_needlen
        LDA #0
        STA instr_position
        JSR spaces
        CMP #44
        BNE .close
        INC pos
        JSR expr
        LDA vallo
        BEQ .close
        SEC
        SBC #1
        STA instr_position
.close  JSR string_close
        LDA failed
        BNE .done
.candidate
        LDA instr_position
        CLC
        ADC instr_needlen
        BCS .not_found
        CMP instr_haylen
        BCC .try
        BEQ .try
        JMP .not_found
.try    LDA #0
        STA instr_offset
.compare
        LDA instr_offset
        CMP instr_needlen
        BEQ .found
        CLC
        ADC instr_position
        CLC
        ADC instr_haylo
        STA srclo
        LDA instr_hayhi
        ADC #0
        STA srchi
        LDY #0
        JSR rampeek_src
        STA instr_byte
        CLC
        LDA instr_needlo
        ADC instr_offset
        STA srclo
        LDA instr_needhi
        ADC #0
        STA srchi
        JSR rampeek_src
        CMP instr_byte
        BNE .next
        INC instr_offset
        JMP .compare
.next   INC instr_position
        BNE .candidate
.not_found
        LDA #0
        STA vallo
        STA valhi
        RTS
.found  LDA instr_position
        CLC
        ADC #1
        STA vallo
        LDA #0
        STA valhi
.done   RTS

kwabs   BYT "ABS",0
kwsqr   BYT "SQR",0
kwsgn   BYT "SGN",0
kwint   BYT "INT",0
kwfre   BYT "FRE",0
kwpos   BYT "POS",0
kwpeek  BYT "PEEK",0
kwlen   BYT "LEN",0
kwasc   BYT "ASC",0
kwval   BYT "VAL",0
kwrgr   BYT "RGR",0
kwrclr  BYT "RCLR",0
kwrlum  BYT "RLUM",0
kwjoy   BYT "JOY",0
kwrdot  BYT "RDOT",0
kwinstr BYT "INSTR",0
