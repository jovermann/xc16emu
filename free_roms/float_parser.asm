; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; ROM-native decimal source parser for the system-ROM floating accumulator.
; The public fexpr result is FAC ($B4-$B9). The existing integer expression
; entry remains available while the rest of BASIC is moved to this path.

fac_exp EQU $b4
fac_m0 EQU $b5
fac_m1 EQU $b6
fac_m2 EQU $b7
fac_m3 EQU $b8
fac_sign EQU $b9
arg_exp EQU $ba
fp_intlo EQU $c7
fp_inthi EQU $c8
f_fraction EQU $ac
f_frac_mode EQU $ab
f_exponent EQU $ad
f_exp_sign EQU $ae
f_seen EQU $af
fprint_count EQU $ce
fprint_index EQU $cf
fprint_mode EQU $0766          ; 0 screen, 1 STR$ temporary buffer
fprint_sci EQU $0767
fprint_exp EQU $0768
fprint_sign EQU $0769

; Evaluate signed decimal addition/subtraction, leaving a normalized FAC.
; Arguments: pos at an expression. Returns: FAC and advanced pos.
fexpr
        JSR fterm
.more   JSR spaces
        CMP #43
        BEQ .plus
        CMP #45
        BEQ .minus
        RTS
.plus   INC pos
        LDA fac_exp
        PHA
        LDA fac_m0
        PHA
        LDA fac_m1
        PHA
        LDA fac_m2
        PHA
        LDA fac_m3
        PHA
        LDA fac_sign
        PHA
        JSR fterm
        JSR $d924              ; copy right FAC to ARG
        PLA
        STA fac_sign
        PLA
        STA fac_m3
        PLA
        STA fac_m2
        PLA
        STA fac_m1
        PLA
        STA fac_m0
        PLA
        STA fac_exp
        JSR $d900              ; signed FP addition
        JMP .more
.minus  INC pos
        LDA fac_exp
        PHA
        LDA fac_m0
        PHA
        LDA fac_m1
        PHA
        LDA fac_m2
        PHA
        LDA fac_m3
        PHA
        LDA fac_sign
        PHA
        JSR fterm
        JSR $d924
        PLA
        STA fac_sign
        PLA
        STA fac_m3
        PLA
        STA fac_m2
        PLA
        STA fac_m1
        PLA
        STA fac_m0
        PLA
        STA fac_exp
        JSR $d903              ; signed FP subtraction
        JMP .more

; Evaluate multiplication and division before fexpr handles addition.
; Arguments: pos at a term. Returns: FAC and advanced pos.
fterm
        JSR fprimary
.more   JSR spaces
        CMP #42
        BEQ .multiply
        CMP #47
        BEQ .divide
        RTS
.multiply
        INC pos
        LDA fac_exp
        PHA
        LDA fac_m0
        PHA
        LDA fac_m1
        PHA
        LDA fac_m2
        PHA
        LDA fac_m3
        PHA
        LDA fac_sign
        PHA
        JSR fprimary
        JSR $d924
        PLA
        STA fac_sign
        PLA
        STA fac_m3
        PLA
        STA fac_m2
        PLA
        STA fac_m1
        PLA
        STA fac_m0
        PLA
        STA fac_exp
        JSR $d912
        JMP .more
.divide
        INC pos
        LDA fac_exp
        PHA
        LDA fac_m0
        PHA
        LDA fac_m1
        PHA
        LDA fac_m2
        PHA
        LDA fac_m3
        PHA
        LDA fac_sign
        PHA
        JSR fprimary
        JSR $d924
        PLA
        STA fac_sign
        PLA
        STA fac_m3
        PLA
        STA fac_m2
        PLA
        STA fac_m1
        PLA
        STA fac_m0
        PLA
        STA fac_exp
        JSR $d915
        BCC .more
        JMP division_zero_error

; Parse a signed literal, parenthesized expression, PI, or existing integer
; variable into FAC. Typed variables and numeric functions will join here.
; Arguments: pos at a primary. Returns: FAC and advanced pos.
fprimary
        JSR spaces
        CMP #45
        BEQ .minus_jump
        CMP #43
        BEQ .plus_jump
        CMP #40
        BEQ .paren_jump
        CMP #46
        BEQ .number
        CMP #48
        BCC .name
        CMP #58
        BCC .number
        JMP .name
.plus_jump
        JMP .positive
.minus_jump
        JMP .negative
.paren_jump
        JMP .parenthesized
.number JMP fnumber
.name   LDY pos
        LDA input,Y
        CMP #80
        BNE .variable
        INY
        LDA input,Y
        CMP #73
        BNE .variable
        INY
        LDA input,Y
        CMP #65
        BCS .variable
        STY pos
        JMP $d91e              ; PI
.variable
        JSR float_function
        BCS .function_done
        JSR numeric_function    ; existing ROM integer functions
        BCS .integer_result
        LDY pos
        LDA input,Y
        CMP #68                 ; DEC("hex") uses the legacy ROM parser
        BNE .named
        INY
        LDA input,Y
        CMP #69
        BNE .named
        INY
        LDA input,Y
        CMP #67
        BNE .named
        JSR atom
        JMP .integer_result
.named  JSR number_name
        BCS .load
        JMP syntax
.load   LDA number_after
        STA pos
        JMP number_load_float
.integer_result
        LDA vallo
        STA fp_intlo
        LDA valhi
        STA fp_inthi
        JMP $d906
.function_done
        RTS
.negative
        INC pos
        JSR fprimary
        JMP $d90c
.positive
        INC pos
        JMP fprimary
.parenthesized
        INC pos
        JSR fexpr
        JSR spaces
        CMP #41
        BEQ .close
        JMP syntax
.close  INC pos
        RTS
.literal
        JMP fnumber

; Recognize floating built-ins before the legacy integer function parser.
; Arguments: pos at a name. Returns: carry set and FAC on a match.
float_function
        JSR $e600              ; user-defined FN call in system ROM
        BCC .builtin
        RTS
.builtin
        LDA #kwfre%256
        STA kwlo
        LDA #kwfre/256
        STA kwhi
        JSR keyword
        BCC .pos
        JSR float_argument
        JSR number_free_count
        LDA vallo
        STA fp_intlo
        LDA valhi
        STA fp_inthi
        JSR $d906
        LDA valhi
        BPL .fre_done
        LDX #5
.unsigned_constant
        LDA float_65536,X
        STA arg_exp,X
        DEX
        BPL .unsigned_constant
        JSR $d900
.fre_done
        SEC
        RTS
.pos   LDA #kwpos%256
        STA kwlo
        LDA #kwpos/256
        STA kwhi
        JSR keyword
        BCC .rnd
        JSR float_argument
        LDA column
        STA fp_intlo
        LDA #0
        STA fp_inthi
        JSR $d906
        SEC
        RTS
.rnd   LDA #kwrnd%256
        STA kwlo
        LDA #kwrnd/256
        STA kwhi
        JSR keyword
        BCC .atn
        JSR float_argument
        JSR float_rnd
        SEC
        RTS
.atn   LDA #kwatn%256
        STA kwlo
        LDA #kwatn/256
        STA kwhi
        JSR keyword
        BCC .val
        JSR float_argument
        JSR $de12
        SEC
        RTS
.val   LDA #kwval%256
        STA kwlo
        LDA #kwval/256
        STA kwhi
        JSR keyword
        BCC .exp
        JSR number_string_argument
        JSR float_val
        SEC
        RTS
.exp   LDA #kwexp%256
        STA kwlo
        LDA #kwexp/256
        STA kwhi
        JSR keyword
        BCC .log
        JSR float_argument
        JSR $de0f
        BCC .exp_done
        JMP quantity_error
.exp_done
        SEC
        RTS
.log   LDA #kwlog%256
        STA kwlo
        LDA #kwlog/256
        STA kwhi
        JSR keyword
        BCC .tan
        JSR float_argument
        JSR $de0c
        BCC .log_done
        JMP quantity_error
.log_done
        SEC
        RTS
.tan   LDA #kwtan%256
        STA kwlo
        LDA #kwtan/256
        STA kwhi
        JSR keyword
        BCC .sqr
        JSR float_argument
        JSR $de09
        BCC .tan_done
        JMP division_zero_error
.tan_done
        SEC
        RTS
.sqr   LDA #kwsqr%256
        STA kwlo
        LDA #kwsqr/256
        STA kwhi
        JSR keyword
        BCC .sin
        JSR float_argument
        JSR $de06              ; system-ROM square root
        BCC .sqr_done
        JMP quantity_error
.sqr_done
        SEC
        RTS
.sin    LDA #kwsin%256
        STA kwlo
        LDA #kwsin/256
        STA kwhi
        JSR keyword
        BCC .cos
        JSR float_argument
        JSR $de00              ; system-ROM sine
        SEC
        RTS
.cos    LDA #kwcos%256
        STA kwlo
        LDA #kwcos/256
        STA kwhi
        JSR keyword
        BCC .usr
        JSR float_argument
        JSR $de03              ; system-ROM cosine
        SEC
        RTS
.usr
        LDA #kwusr%256
        STA kwlo
        LDA #kwusr/256
        STA kwhi
        JSR keyword
        BCC .int
        JSR float_argument
        JSR usr_trampoline
        SEC
        RTS
.int    LDA #kwint%256
        STA kwlo
        LDA #kwint/256
        STA kwhi
        JSR keyword
        BCC .abs
        JSR float_argument
        LDX #5
.save   LDA fac_exp,X
        STA $05c8,X
        DEX
        BPL .save
        JSR $d909
        JSR $d906
        LDX #5
.arg    LDA $05c8,X
        STA arg_exp,X
        DEX
        BPL .arg
        JSR $d918
        CMP #1                 ; truncation exceeds negative fractional input
        BNE .int_done
        LDA #1
        JSR $d921
        JSR $d903
.int_done
        SEC
        RTS
.abs    LDA #kwabs%256
        STA kwlo
        LDA #kwabs/256
        STA kwhi
        JSR keyword
        BCC .sgn
        JSR float_argument
        JSR $d90f
        SEC
        RTS
.sgn    LDA #kwsgn%256
        STA kwlo
        LDA #kwsgn/256
        STA kwhi
        JSR keyword
        BCC .miss
        JSR float_argument
        LDA fac_exp
        BEQ .zero
        LDA fac_sign
        BEQ .one
        LDA #$ff
        BNE .convert
.one    LDA #1
        BNE .convert
.zero   LDA #0
.convert
        STA fp_intlo
        CMP #$80
        LDA #0
        BCC .positive
        LDA #$ff
.positive
        STA fp_inthi
        JSR $d906
        SEC
        RTS
.miss   CLC
        RTS

; Evaluate a parenthesized floating expression for a function.
; Arguments: pos after function name. Returns: FAC and pos after ')'.
float_argument
        JSR spaces
        CMP #40
        BEQ .open
        JMP syntax
.open   INC pos
        JSR fexpr
        JSR spaces
        CMP #41
        BEQ .close
        JMP syntax
.close  INC pos
        RTS

; Call user machine code at the ordinary BASIC USR vector, preserving FAC.
; Arguments: FAC and vector at $0501. Returns: the user's modified FAC.
usr_trampoline
        JMP ($0501)

kwusr   BYT "USR",0
kwsin   BYT "SIN",0
kwcos   BYT "COS",0
kwtan   BYT "TAN",0
kwlog   BYT "LOG",0
kwexp   BYT "EXP",0
kwatn   BYT "ATN",0
kwrnd   BYT "RND",0

rnd_state_lo EQU $0764
rnd_state_hi EQU $0765

; Advance a 16-bit Galois LFSR and return its upper 15 bits as a fraction.
; RND(0) seeds once from TED raster timing; RND(negative) selects a repeatable seed.
; Arguments: FAC is RND's argument. Returns: FAC in [0,1).
float_rnd
        JSR $d909
        LDA fac_sign
        BEQ .positive
        LDA fp_intlo
        STA rnd_state_lo
        LDA fp_inthi
        STA rnd_state_hi
        JMP .advance
.positive
        LDA fac_exp
        BNE .ensure_seed
        LDA rnd_state_lo
        ORA rnd_state_hi
        BNE .advance          ; do not collapse every RND(0) to a raster seed
        LDA $ff1d              ; changing TED raster phase
        STA rnd_state_lo
        LDA $ff1e
        STA rnd_state_hi
.ensure_seed
        LDA rnd_state_lo
        ORA rnd_state_hi
        BNE .advance
        LDA #$a7
        STA rnd_state_lo
        LDA #$5d
        STA rnd_state_hi
.advance
        LSR rnd_state_hi
        ROR rnd_state_lo
        BCC .fraction
        LDA rnd_state_hi
        EOR #$b4
        STA rnd_state_hi
.fraction
        LDA rnd_state_lo
        STA fp_intlo
        LDA rnd_state_hi
        AND #$7f
        STA fp_inthi
        JSR $d906
        LDX #5
.divisor
        LDA rnd_divisor,X
        STA arg_exp,X
        DEX
        BPL .divisor
        JSR $d915
        RTS

rnd_divisor BYT 144,$80,0,0,0,0
float_65536 BYT 145,$80,0,0,0,0

; Accumulate decimal digits in FAC, then divide or multiply by ten for the
; decimal point and optional E exponent. A digit is converted by 6502 code
; in the system ROM; no host floating-point operation is involved.
; Arguments: pos at a digit or dot. Returns: FAC and advanced pos.
fnumber
        LDA #0
        STA fac_exp
        STA fac_m0
        STA fac_m1
        STA fac_m2
        STA fac_m3
        STA fac_sign
        STA f_fraction
        STA f_frac_mode
        STA f_exponent
        STA f_exp_sign
        STA f_seen
.digit  LDY pos
        LDA input,Y
        CMP #48
        BCC .dot
        CMP #58
        BCS .dot
        SEC
        SBC #48
        PHA
        JSR $d91b              ; ARG = 10
        JSR $d912              ; FAC *= 10
        PLA
        JSR $d921              ; ARG = digit
        JSR $d900              ; FAC += digit
        LDA #1
        STA f_seen
        LDA f_frac_mode
        BEQ .advance
        INC f_fraction
.advance
        INC pos
        JMP .digit
.dot    CMP #46
        BNE .after_digits
        LDA f_frac_mode
        BNE .after_digits       ; a second dot is not a number byte
        LDA #1
        STA f_frac_mode
        INC pos
        JMP .digit
.after_digits
        LDA f_seen
        BNE .scientific
        JMP syntax
.scientific
        LDY pos
        LDA input,Y
        CMP #69                 ; scientific exponent E[+/-]digits
        BNE .scale
        INC pos
        LDY pos
        LDA input,Y
        CMP #45
        BNE .exp_plus
        LDA #1
        STA f_exp_sign
        INC pos
        JMP .exp_digits
.exp_plus
        CMP #43
        BNE .exp_digits
        INC pos
.exp_digits
        LDY pos
        LDA input,Y
        CMP #48
        BCC .scale
        CMP #58
        BCS .scale
        SEC
        SBC #48
        STA digit
        LDA f_exponent
        ASL
        STA f_exponent
        ASL
        ASL
        CLC
        ADC f_exponent
        CLC
        ADC digit
        STA f_exponent
        INC pos
        JMP .exp_digits
.scale
        LDA f_exp_sign
        BNE .divide_exponent
.multiply_exponent
        LDA f_exponent
        BEQ .fraction_scale
        DEC f_exponent
        JSR $d91b
        JSR $d912
        JMP .multiply_exponent
.divide_exponent
        LDA f_exponent
        BEQ .fraction_scale
        DEC f_exponent
        JSR $d91b
        JSR $d915
        JMP .divide_exponent
.fraction_scale
        LDA f_fraction
        BEQ .done
        DEC f_fraction
        JSR $d91b
        JSR $d915
        JMP .fraction_scale
.done   RTS

f_val_index EQU $059c
f_val_sign EQU $059d

; Parse the numeric prefix of a BASIC string into FAC. The string bytes are
; fetched through the RAM bank reader, including when program RAM is under ROM.
; Arguments: str_ptr/str_len from VAL(s$). Returns: FAC, or zero for no digits.
float_val
        LDA str_ptrlo
        STA srclo
        LDA str_ptrhi
        STA srchi
        LDA #0
        STA f_val_index
        STA f_val_sign
        STA f_fraction
        STA f_frac_mode
        STA f_exponent
        STA f_exp_sign
        STA f_seen
        STA fac_exp
        STA fac_m0
        STA fac_m1
        STA fac_m2
        STA fac_m3
        STA fac_sign
.spaces
        LDY f_val_index
        CPY str_len
        BNE .space_byte
        JMP .done
.space_byte
        JSR rampeek_src
        CMP #32
        BNE .sign
        INC f_val_index
        JMP .spaces
.sign   CMP #45
        BNE .plus
        LDA #1
        STA f_val_sign
        INC f_val_index
        JMP .digits
.plus   CMP #43
        BNE .digits
        INC f_val_index
.digits
        LDY f_val_index
        CPY str_len
        BNE .digit_byte
        JMP .scale
.digit_byte
        JSR rampeek_src
        CMP #48
        BCC .dot
        CMP #58
        BCS .dot
        SEC
        SBC #48
        PHA
        JSR $d91b
        JSR $d912
        PLA
        JSR $d921
        JSR $d900
        LDA #1
        STA f_seen
        LDA f_frac_mode
        BEQ .advance
        INC f_fraction
.advance
        INC f_val_index
        JMP .digits
.dot    CMP #46
        BNE .maybe_exponent
        LDA f_frac_mode
        BNE .maybe_exponent
        LDA #1
        STA f_frac_mode
        INC f_val_index
        JMP .digits
.maybe_exponent
        CMP #69
        BNE .scale
        LDA f_seen
        BEQ .scale
        INC f_val_index
        LDY f_val_index
        CPY str_len
        BEQ .scale
        JSR rampeek_src
        CMP #45
        BNE .exponent_plus
        LDA #1
        STA f_exp_sign
        INC f_val_index
        JMP .exponent_digits
.exponent_plus
        CMP #43
        BNE .exponent_digits
        INC f_val_index
.exponent_digits
        LDY f_val_index
        CPY str_len
        BEQ .scale
        JSR rampeek_src
        CMP #48
        BCC .scale
        CMP #58
        BCS .scale
        SEC
        SBC #48
        STA digit
        LDA f_exponent
        ASL
        STA f_exponent
        ASL
        ASL
        CLC
        ADC f_exponent
        CLC
        ADC digit
        STA f_exponent
        INC f_val_index
        JMP .exponent_digits
.scale
        LDA f_exp_sign
        BNE .divide_exponent
.multiply_exponent
        LDA f_exponent
        BEQ .fraction_scale
        DEC f_exponent
        JSR $d91b
        JSR $d912
        JMP .multiply_exponent
.divide_exponent
        LDA f_exponent
        BEQ .fraction_scale
        DEC f_exponent
        JSR $d91b
        JSR $d915
        JMP .divide_exponent
.fraction_scale
        LDA f_fraction
        BEQ .apply_sign
        DEC f_fraction
        JSR $d91b
        JSR $d915
        JMP .fraction_scale
.apply_sign
        LDA f_val_sign
        BEQ .done
        JSR $d90c
.done   RTS

; Print FAC with a leading sign column and up to six fractional digits.
; Rounding happens in the system ROM before the integer/fraction split, so
; finite binary approximations of decimal values print cleanly at this scale.
; Arguments: FAC is a BASIC number. Returns: screen output and trailing space.
float_print
        LDA #0
        STA fprint_sci
        LDA fac_exp
        BNE .nonzero
        JMP float_print_core
.nonzero
        CMP #145
        BCS .scientific
        CMP #110
        BCC .scientific
        JMP float_print_core
.scientific
        LDA #1
        STA fprint_sci
        LDA #0
        STA fprint_exp
        LDA fac_sign
        STA fprint_sign
        JSR $d90f
        LDA fac_exp
        CMP #145
        BCC .small
.large  JSR $d91b
        JSR $d918
        CMP #$ff
        BEQ .scaled
        JSR $d91b
        JSR $d915
        INC fprint_exp
        JMP .large
.small  LDA fac_exp
        CMP #129
        BCS .scaled
        JSR $d91b
        JSR $d912
        DEC fprint_exp
        JMP .small
.scaled
        JSR $d927              ; round before checking a carry into 10
        JSR $d900
        JSR $d91b
        JSR $d918
        CMP #$ff
        BEQ .normalized
        JSR $d91b
        JSR $d915
        INC fprint_exp
.normalized
        LDA fprint_sign
        STA fac_sign
        JSR float_print_core
        LDA #69                 ; E+00 or E-00
        JSR float_putchar
        LDA fprint_exp
        BPL .positive_exponent
        LDA #45
        JSR float_putchar
        SEC
        LDA #0
        SBC fprint_exp
        JMP .exponent_digits
.positive_exponent
        LDA #43
        JSR float_putchar
        LDA fprint_exp
.exponent_digits
        LDX #48
.tens   CMP #10
        BCC .two_digits
        SEC
        SBC #10
        INX
        JMP .tens
.two_digits
        PHA
        TXA
        JSR float_putchar
        PLA
        CLC
        ADC #48
        JSR float_putchar
        LDA #32
        JMP float_putchar

; Format a value whose integer part fits the BASIC ROM's decimal digit table.
; Arguments: FAC and fprint_sci. Returns: digits, and space if not scientific.
float_print_core
        LDA fac_exp
        BEQ .positive
        LDA fac_sign
        BEQ .positive
        LDA #45
        BNE .sign
.positive
        LDA #32
.sign   JSR float_putchar
        JSR $d90f              ; abs(FAC)
        LDA fprint_sci
        BNE .rounded
        JSR $d927              ; ARG = 0.0000005
        JSR $d900              ; round to six decimal places
.rounded
        LDX #5
.save   LDA fac_exp,X
        STA $05c8,X
        DEX
        BPL .save
        JSR $d909              ; integer part
        LDA fp_intlo
        STA vallo
        LDA fp_inthi
        STA valhi
        JSR printnum
        JSR $d906              ; integer part as FAC
        JSR $d924              ; move integer to ARG
        LDX #5
.restore
        LDA $05c8,X
        STA fac_exp,X
        DEX
        BPL .restore
        JSR $d903              ; fraction = rounded FAC - integer
        LDA fac_exp
        BEQ .space              ; common integer case needs no digit loop
        LDA #0
        STA fprint_count
.digit  JSR $d91b              ; multiply fraction by ten
        JSR $d912
        JSR $d909
        LDA fp_intlo
        CLC
        ADC #48
        LDX fprint_count
        STA $05d8,X
        LDA fp_intlo
        JSR $d921
        JSR $d903              ; remove the extracted digit
        INC fprint_count
        LDA fprint_count
        CMP #6
        BCC .digit
.trim   LDX fprint_count
        BEQ .space
        DEX
        LDA $05d8,X
        CMP #48
        BNE .fraction
        DEC fprint_count
        JMP .trim
.fraction
        LDA #46
        JSR float_putchar
        LDA #0
        STA fprint_index
.emit   LDX fprint_index
        LDA $05d8,X
        JSR float_putchar
        INC fprint_index
        LDA fprint_index
        CMP fprint_count
        BCC .emit
.space  LDA fprint_sci
        BNE .no_space
        LDA #32
        JMP float_putchar
.no_space
        RTS

; Route formatted digits to the screen or STR$'s temporary RAM buffer.
; Arguments: character in A, fprint_mode. Returns: updated screen or str_len.
float_putchar
        LDY fprint_mode
        BEQ .screen
        LDY str_len
        STA str_numberbuf,Y
        INC str_len
        RTS
.screen JMP putchar
