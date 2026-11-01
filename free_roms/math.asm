; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Independent 6502 floating-point primitives. This module lives in the
; unused system-ROM area beginning at $D900. FAC and ARG have a binary
; exponent biased by 129, a normalized unsigned 32-bit significand in
; big-endian order, and a separate sign byte (0 or $80). Zero has exponent 0.
; The layout is also suitable for converting to the six-byte USR accumulator.

fac_exp EQU $b4
fac_m0 EQU $b5
fac_m1 EQU $b6
fac_m2 EQU $b7
fac_m3 EQU $b8
fac_sign EQU $b9
arg_exp EQU $ba
arg_m0 EQU $bb
arg_m1 EQU $bc
arg_m2 EQU $bd
arg_m3 EQU $be
arg_sign EQU $bf
fp_swap_temp EQU $c0
fp_count EQU $c6
fp_intlo EQU $c7
fp_inthi EQU $c8
fp_intsign EQU $c9
fp_quotbit EQU $ca

; Fixed entry points for the BASIC ROM. Arguments and return values are the
; FAC/ARG cells described above; no simulator-specific arithmetic is used.
fp_api_add JMP fp_add
fp_api_sub JMP fp_sub
fp_api_from_int JMP fp_from_int
fp_api_to_int JMP fp_to_int
fp_api_neg JMP fp_neg
fp_api_abs JMP fp_abs
fp_api_mul JMP fp_mul
fp_api_div JMP fp_div
fp_api_cmp JMP fp_cmp
fp_api_ten JMP fp_ten_to_arg
fp_api_pi JMP fp_pi_to_fac
fp_api_digit JMP fp_digit_to_arg
fp_api_copy_arg JMP fp_copy_fac_to_arg
fp_api_round6 JMP fp_round6_to_arg

; Normalize FAC after subtraction or integer conversion. FAC's sign remains
; unchanged; a zero significand clears its exponent and sign.
fp_normalize
        LDA fac_m0
        ORA fac_m1
        ORA fac_m2
        ORA fac_m3
        BNE .nonzero
        LDA #0
        STA fac_exp
        STA fac_sign
        RTS
.nonzero
        LDA fac_m0
        BMI .done
        LDA fac_exp
        BEQ .underflow
        DEC fac_exp
        ASL fac_m3
        ROL fac_m2
        ROL fac_m1
        ROL fac_m0
        JMP .nonzero
.underflow
        LDA #0
        STA fac_m0
        STA fac_m1
        STA fac_m2
        STA fac_m3
        STA fac_sign
.done   RTS

; Exchange FAC and ARG so FAC has the greater absolute magnitude.
; Arguments: FAC and ARG. Returns: the cells swapped, no flags promised.
fp_swap
        LDX #5
.byte   LDA fac_exp,X
        STA fp_swap_temp,X
        LDA arg_exp,X
        STA fac_exp,X
        LDA fp_swap_temp,X
        STA arg_exp,X
        DEX
        BPL .byte
        RTS

; Add two signed normalized values. The smaller significand is shifted to
; the larger exponent, then added or subtracted by sign. The result is FAC.
; Arguments: FAC and ARG. Returns: FAC sum; ARG may be shifted or swapped.
fp_add
        LDA fac_exp
        BNE .fac_present
        LDX #5
.copy   LDA arg_exp,X
        STA fac_exp,X
        DEX
        BPL .copy
        RTS
.fac_present
        LDA arg_exp
        BNE .compare
        RTS
.compare
        LDA fac_exp
        CMP arg_exp
        BCC .exchange
        BNE .align
        LDX #1
.compare_mantissa
        LDA fac_exp,X
        CMP arg_exp,X
        BCC .exchange
        BNE .align
        INX
        CPX #5
        BCC .compare_mantissa
        JMP .align
.exchange
        JSR fp_swap
.align
        SEC
        LDA fac_exp
        SBC arg_exp
        TAX
        BEQ .combine
        CPX #32
        BCC .shift
        LDA #0
        STA arg_m0
        STA arg_m1
        STA arg_m2
        STA arg_m3
        JMP .combine
.shift  LSR arg_m0
        ROR arg_m1
        ROR arg_m2
        ROR arg_m3
        DEX
        BNE .shift
.combine
        LDA fac_sign
        EOR arg_sign
        BMI .subtract
        CLC
        LDA fac_m3
        ADC arg_m3
        STA fac_m3
        LDA fac_m2
        ADC arg_m2
        STA fac_m2
        LDA fac_m1
        ADC arg_m1
        STA fac_m1
        LDA fac_m0
        ADC arg_m0
        STA fac_m0
        BCC .done
        ROR fac_m0             ; carry is the ninth significand bit
        ROR fac_m1
        ROR fac_m2
        ROR fac_m3
        INC fac_exp
.done   RTS
.subtract
        SEC
        LDA fac_m3
        SBC arg_m3
        STA fac_m3
        LDA fac_m2
        SBC arg_m2
        STA fac_m2
        LDA fac_m1
        SBC arg_m1
        STA fac_m1
        LDA fac_m0
        SBC arg_m0
        STA fac_m0
        JMP fp_normalize

; Subtract ARG from FAC by toggling ARG's sign around signed addition.
; Arguments: FAC and ARG. Returns: FAC difference, ARG otherwise consumed.
fp_sub
        LDA arg_sign
        EOR #$80
        STA arg_sign
        JSR fp_add
        LDA arg_sign
        EOR #$80
        STA arg_sign
        RTS

; Negate a nonzero FAC by toggling its sign byte.
; Arguments: FAC. Returns: FAC with the opposite sign.
fp_neg
        LDA fac_exp
        BEQ .done
        LDA fac_sign
        EOR #$80
        STA fac_sign
.done   RTS

; Take FAC's magnitude without changing the exponent or significand.
; Arguments: FAC. Returns: nonnegative FAC.
fp_abs
        LDA #0
        STA fac_sign
        RTS

; Convert a signed 16-bit integer to normalized FAC.
; Arguments: fp_intlo/fp_inthi. Returns: FAC; input bytes may be negated.
fp_from_int
        LDA #0
        STA fac_sign
        LDA fp_inthi
        BPL .magnitude
        LDA #$80
        STA fac_sign
        SEC
        LDA #0
        SBC fp_intlo
        STA fp_intlo
        LDA #0
        SBC fp_inthi
        STA fp_inthi
.magnitude
        LDA fp_intlo
        ORA fp_inthi
        BNE .nonzero
        LDA #0
        STA fac_exp
        STA fac_m0
        STA fac_m1
        STA fac_m2
        STA fac_m3
        STA fac_sign
        RTS
.nonzero
        LDA #144
        STA fac_exp
        LDA fp_inthi
        STA fac_m0
        LDA fp_intlo
        STA fac_m1
        LDA #0
        STA fac_m2
        STA fac_m3
        JMP fp_normalize

; Convert FAC to a signed 16-bit integer, truncating fractional bits toward
; zero. Values beyond the 16-bit range return the low 16 bits as zero.
; Arguments: FAC. Returns: fp_intlo/fp_inthi.
fp_to_int
        LDA #0
        STA fp_intlo
        STA fp_inthi
        LDA fac_exp
        CMP #129
        BCC .done
        CMP #145
        BCS .done
        LDA fac_m1
        STA fp_intlo
        LDA fac_m0
        STA fp_inthi
        LDA #144
        SEC
        SBC fac_exp
        TAX
        BEQ .sign
.shift  LSR fp_inthi
        ROR fp_intlo
        DEX
        BNE .shift
.sign  LDA fac_sign
        BEQ .done
        SEC
        LDA #0
        SBC fp_intlo
        STA fp_intlo
        LDA #0
        SBC fp_inthi
        STA fp_inthi
.done   RTS

; Multiply two normalized FAC/ARG values with an unsigned 32-by-32-bit
; shift/add product. The upper 32 product bits become the significand; the
; exponent is adjusted according to whether product bit 63 or 62 is set.
; Arguments: FAC and ARG. Returns: signed product in FAC.
fp_mul
        LDA fac_exp
        BNE .fac_nonzero
        JMP .zero
.fac_nonzero
        LDA arg_exp
        BNE .both_nonzero
        JMP .zero
.both_nonzero
        LDA fac_sign
        EOR arg_sign
        AND #$80
        STA fac_sign
        LDA fac_exp
        CLC
        ADC arg_exp
        SEC
        SBC #129
        STA fac_exp
        LDX #7
        LDA #0
.clear  STA $05e0,X          ; eight-byte product
        STA $05e8,X          ; eight-byte shifted multiplicand
        DEX
        BPL .clear
        LDA arg_m3
        STA $05e8
        LDA arg_m2
        STA $05e9
        LDA arg_m1
        STA $05ea
        LDA arg_m0
        STA $05eb
        LDA fac_m3
        STA $05f0
        LDA fac_m2
        STA $05f1
        LDA fac_m1
        STA $05f2
        LDA fac_m0
        STA $05f3
        LDA #32
        STA fp_count
.bit    LDA $05f0
        AND #1
        BEQ .shift
        CLC
        LDA $05e0
        ADC $05e8
        STA $05e0
        LDA $05e1
        ADC $05e9
        STA $05e1
        LDA $05e2
        ADC $05ea
        STA $05e2
        LDA $05e3
        ADC $05eb
        STA $05e3
        LDA $05e4
        ADC $05ec
        STA $05e4
        LDA $05e5
        ADC $05ed
        STA $05e5
        LDA $05e6
        ADC $05ee
        STA $05e6
        LDA $05e7
        ADC $05ef
        STA $05e7
.shift  CLC
        ROL $05e8
        ROL $05e9
        ROL $05ea
        ROL $05eb
        ROL $05ec
        ROL $05ed
        ROL $05ee
        ROL $05ef
        LSR $05f3
        ROR $05f2
        ROR $05f1
        ROR $05f0
        DEC fp_count
        BNE .bit
        LDA $05e7
        STA fac_m0
        LDA $05e6
        STA fac_m1
        LDA $05e5
        STA fac_m2
        LDA $05e4
        STA fac_m3
        LDA fac_m0
        BMI .high_product
        LDA $05e3
        ASL
        ROL fac_m3
        ROL fac_m2
        ROL fac_m1
        ROL fac_m0
        RTS
.high_product
        INC fac_exp
        RTS
.zero   LDA #0
        STA fac_exp
        STA fac_m0
        STA fac_m1
        STA fac_m2
        STA fac_m3
        STA fac_sign
        RTS

; Divide two normalized values with a 64-bit restoring division. A zero
; divisor clears FAC and returns carry set; otherwise FAC receives the
; normalized quotient and carry is clear. The quotient and scratch bytes are
; RAM owned by the ROM, never host arithmetic.
; Arguments: FAC numerator, ARG denominator. Returns: FAC quotient and C.
fp_div
        LDA arg_exp
        BNE .divisor_present
        LDA #0
        STA fac_exp
        SEC
        RTS
.divisor_present
        LDA fac_exp
        BNE .both_present
        CLC
        RTS

.both_present
        LDA fac_sign
        EOR arg_sign
        AND #$80
        STA fac_sign
        LDA fac_exp
        SEC
        SBC arg_exp
        CLC
        ADC #129
        STA fac_exp
        LDX #7
        LDA #0
.clear  STA $05e0,X          ; five-byte remainder and spare bytes
        STA $05e8,X          ; eight-byte shifted numerator
        DEX
        BPL .clear
        STA $05d0
        STA $05d1
        STA $05d2
        STA $05d3
        LDA fac_m3
        STA $05ec
        LDA fac_m2
        STA $05ed
        LDA fac_m1
        STA $05ee
        LDA fac_m0
        STA $05ef
        LDA arg_m3
        STA $05f0
        LDA arg_m2
        STA $05f1
        LDA arg_m1
        STA $05f2
        LDA arg_m0
        STA $05f3
        ; If numerator significand is at least denominator, one fewer
        ; binary shift is needed. Otherwise halve the exponent and use 32.
        LDA fac_m0
        CMP arg_m0
        BCC .shift32
        BNE .shift31
        LDA fac_m1
        CMP arg_m1
        BCC .shift32
        BNE .shift31
        LDA fac_m2
        CMP arg_m2
        BCC .shift32
        BNE .shift31
        LDA fac_m3
        CMP arg_m3
        BCC .shift32
.shift31
        LSR $05ef
        ROR $05ee
        ROR $05ed
        ROR $05ec
        ROR $05eb
        ROR $05ea
        ROR $05e9
        ROR $05e8
        JMP .begin
.shift32
        DEC fac_exp
.begin  LDA #64
        STA fp_count
.bit    ASL $05e8
        ROL $05e9
        ROL $05ea
        ROL $05eb
        ROL $05ec
        ROL $05ed
        ROL $05ee
        ROL $05ef
        ROL $05e0
        ROL $05e1
        ROL $05e2
        ROL $05e3
        ROL $05e4
        LDA #0
        STA fp_quotbit
        LDA $05e4
        BNE .subtract
        LDA $05e3
        CMP $05f3
        BCC .quotient
        BNE .subtract
        LDA $05e2
        CMP $05f2
        BCC .quotient
        BNE .subtract
        LDA $05e1
        CMP $05f1
        BCC .quotient
        BNE .subtract
        LDA $05e0
        CMP $05f0
        BCC .quotient
.subtract
        SEC
        LDA $05e0
        SBC $05f0
        STA $05e0
        LDA $05e1
        SBC $05f1
        STA $05e1
        LDA $05e2
        SBC $05f2
        STA $05e2
        LDA $05e3
        SBC $05f3
        STA $05e3
        LDA $05e4
        SBC #0
        STA $05e4
        LDA #1
        STA fp_quotbit
.quotient
        ASL $05d0
        ROL $05d1
        ROL $05d2
        ROL $05d3
        LDA $05d0
        ORA fp_quotbit
        STA $05d0
        DEC fp_count
        BEQ .complete
        JMP .bit
.complete
        LDA $05d3
        STA fac_m0
        LDA $05d2
        STA fac_m1
        LDA $05d1
        STA fac_m2
        LDA $05d0
        STA fac_m3
        CLC
        RTS

; Compare signed FAC to ARG without modifying either operand.
; Arguments: FAC and ARG. Returns: A=$FF for less, 0 equal, 1 greater.
fp_cmp
        LDA fac_sign
        CMP arg_sign
        BEQ .same_sign
        BMI .less
        BPL .greater
.same_sign
        LDA fac_exp
        CMP arg_exp
        BCC .magnitude_less
        BNE .magnitude_greater
        LDX #1
.mantissa
        LDA fac_exp,X
        CMP arg_exp,X
        BCC .magnitude_less
        BNE .magnitude_greater
        INX
        CPX #5
        BCC .mantissa
        LDA #0
        RTS
.magnitude_less
        LDA fac_sign
        BEQ .less
        BNE .greater
.magnitude_greater
        LDA fac_sign
        BEQ .greater
.less   LDA #$ff
        RTS
.greater
        LDA #1
        RTS

; Load 10 into ARG without changing FAC.
; Arguments: none. Returns: ARG=10.
fp_ten_to_arg
        LDX #5
.copy   LDA fp_ten_constant,X
        STA arg_exp,X
        DEX
        BPL .copy
        RTS

; Load an approximately 32-bit-significand value of PI into FAC.
; Arguments: none. Returns: FAC=3.141592653589793 to its own precision.
fp_pi_to_fac
        LDX #5
.copy   LDA fp_pi_constant,X
        STA fac_exp,X
        DEX
        BPL .copy
        RTS

; Convert the digit in A to ARG while preserving FAC in RAM. This supports
; decimal source parsing without a host-side conversion service.
; Arguments: A is 0..9. Returns: ARG holds the digit; FAC is unchanged.
fp_digit_to_arg
        STA fp_intlo
        LDX #5
.save   LDA fac_exp,X
        STA $05c0,X
        DEX
        BPL .save
        LDA #0
        STA fp_inthi
        JSR fp_from_int
        LDX #5
.copy   LDA fac_exp,X
        STA arg_exp,X
        LDA $05c0,X
        STA fac_exp,X
        DEX
        BPL .copy
        RTS

; Copy FAC into ARG without altering FAC, for an expression's right operand.
; Arguments: FAC. Returns: ARG equal to FAC.
fp_copy_fac_to_arg
        LDX #5
.copy   LDA fac_exp,X
        STA arg_exp,X
        DEX
        BPL .copy
        RTS

; Load half a millionth into ARG for six-digit decimal PRINT rounding.
; Arguments: none. Returns: ARG=0.0000005 to FAC precision.
fp_round6_to_arg
        LDX #5
.copy   LDA fp_round6_constant,X
        STA arg_exp,X
        DEX
        BPL .copy
        RTS

fp_ten_constant BYT 132,$a0,0,0,0,0
fp_pi_constant BYT 130,$c9,$0f,$da,$a2,0
fp_round6_constant BYT 108,$86,$37,$bd,$06,0
