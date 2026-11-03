; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Trigonometric functions built only from the system ROM's floating operations.
; The calculation uses a recurrence for odd Taylor terms through x^15/15!.

        ORG $DE00
math_sin JMP advanced_sin      ; $DE00, FAC := sin(FAC)
math_cos JMP advanced_cos      ; $DE03, FAC := cos(FAC)
math_sqr JMP advanced_sqr      ; $DE06, FAC := sqrt(FAC), carry on domain error
math_tan JMP advanced_tan      ; $DE09, FAC := tan(FAC), carry on pole
math_log JMP advanced_log      ; $DE0C, FAC := ln(FAC), carry on domain error
math_exp JMP advanced_exp      ; $DE0F, FAC := exp(FAC), carry on overflow
math_atn JMP advanced_atn      ; $DE12, FAC := atan(FAC)

trig_x2 EQU $05a0             ; six-byte floating scratch values
trig_term EQU $05a6
trig_sum EQU $05ac
trig_counter EQU $05b2
trig_negative EQU $05b3

; Compute sine, reducing a moderate argument into [-PI,PI] first.
; Arguments: FAC is radians. Returns: FAC is their sine.
advanced_sin
        LDA fac_exp
        BNE .nonzero
        LDA #0
        STA fac_sign
        RTS
.nonzero
        LDA fac_sign
        STA trig_negative
        JSR fp_abs
.reduce
        JSR trig_two_pi_arg
        JSR fp_cmp
        CMP #$ff
        BEQ .half
        JSR trig_two_pi_arg
        JSR fp_sub
        JMP .reduce
.half   JSR fp_pi_to_fac_arg
        JSR fp_cmp
        CMP #1
        BNE .series
        JSR trig_two_pi_arg
        JSR fp_sub
.series
        LDX #5
.save_x
        LDA fac_exp,X
        STA trig_term,X
        STA trig_sum,X
        DEX
        BPL .save_x
        JSR fp_copy_fac_to_arg
        JSR fp_mul
        LDX #5
.save_x2
        LDA fac_exp,X
        STA trig_x2,X
        DEX
        BPL .save_x2
        LDA #0
        STA trig_counter
.term
        LDX #5
.restore_term
        LDA trig_term,X
        STA fac_exp,X
        LDA trig_x2,X
        STA arg_exp,X
        DEX
        BPL .restore_term
        JSR fp_mul
        LDX trig_counter
        LDA trig_denominators,X
        STA fp_intlo
        LDA #0
        STA fp_inthi
        LDX #5
.save_product
        LDA fac_exp,X
        STA trig_term,X
        DEX
        BPL .save_product
        JSR fp_from_int
        JSR fp_copy_fac_to_arg
        LDX #5
.restore_product
        LDA trig_term,X
        STA fac_exp,X
        DEX
        BPL .restore_product
        JSR fp_div
        LDX #5
.save_term
        LDA fac_exp,X
        STA trig_term,X
        STA arg_exp,X
        LDA trig_sum,X
        STA fac_exp,X
        DEX
        BPL .save_term
        LDA trig_counter
        AND #1
        BNE .add
        JSR fp_sub
        JMP .save_sum
.add    JSR fp_add
.save_sum
        LDX #5
.sum    LDA fac_exp,X
        STA trig_sum,X
        DEX
        BPL .sum
        INC trig_counter
        LDA trig_counter
        CMP #7
        BCC .term
        LDA trig_negative
        BEQ .done
        JSR fp_neg
.done   RTS

; Compute cosine by shifting its argument and using the sine series.
; Arguments: FAC is radians. Returns: FAC is their cosine.
advanced_cos
        LDX #5
.constant
        LDA trig_half_pi,X
        STA arg_exp,X
        DEX
        BPL .constant
        JSR fp_add
        JMP advanced_sin

; Load the 32-bit approximation of two PI into ARG.
; Arguments: none. Returns: ARG=2*PI.
trig_two_pi_arg
        LDX #5
.copy   LDA trig_two_pi,X
        STA arg_exp,X
        DEX
        BPL .copy
        RTS

; Load PI into ARG while preserving FAC.
; Arguments: none. Returns: ARG=PI.
fp_pi_to_fac_arg
        LDX #5
.copy   LDA fp_pi_constant,X
        STA arg_exp,X
        DEX
        BPL .copy
        RTS

trig_denominators BYT 6,20,42,72,110,156,210
trig_half_pi BYT 129,$c9,$0f,$da,$a2,0
trig_two_pi BYT 131,$c9,$0f,$da,$a2,0

sqrt_input EQU $05b4
sqrt_guess EQU $05ba

; Refine sqrt(x) with eleven Newton steps, entirely in emulated ROM arithmetic.
; Arguments: nonnegative FAC. Returns: square root in FAC; carry on negative.
advanced_sqr
        LDA fac_sign
        BMI .error
        LDA fac_exp
        BNE .positive
        CLC
        RTS
.positive
        LDX #5
.save_input
        LDA fac_exp,X
        STA sqrt_input,X
        STA sqrt_guess,X
        DEX
        BPL .save_input
        LDA #11
        STA trig_counter
.iterate
        LDX #5
.divide_setup
        LDA sqrt_input,X
        STA fac_exp,X
        LDA sqrt_guess,X
        STA arg_exp,X
        DEX
        BPL .divide_setup
        JSR fp_div
        LDX #5
.add_setup
        LDA sqrt_guess,X
        STA arg_exp,X
        DEX
        BPL .add_setup
        JSR fp_add
        LDA #2
        JSR fp_digit_to_arg
        JSR fp_div
        LDX #5
.save_guess
        LDA fac_exp,X
        STA sqrt_guess,X
        DEX
        BPL .save_guess
        DEC trig_counter
        BNE .iterate
        CLC
        RTS
.error SEC
        RTS

tan_input EQU $0590
tan_sine EQU $0596

; Divide the sine of an angle by its cosine using ROM floating operations.
; Arguments: FAC is radians. Returns: tangent in FAC; carry at a pole.
advanced_tan
        LDX #5
.save_input
        LDA fac_exp,X
        STA tan_input,X
        DEX
        BPL .save_input
        JSR advanced_sin
        LDX #5
.save_sine
        LDA fac_exp,X
        STA tan_sine,X
        LDA tan_input,X
        STA fac_exp,X
        DEX
        BPL .save_sine
        JSR advanced_cos
        JSR fp_copy_fac_to_arg
        LDX #5
.restore_sine
        LDA tan_sine,X
        STA fac_exp,X
        DEX
        BPL .restore_sine
        JMP fp_div

log_exponent EQU $055f
log_mantissa EQU $0560
log_numerator EQU $0566
log_y2 EQU $056c
log_term EQU $0572
log_sum EQU $0578
log_counter EQU $057e

; Evaluate ln(x)=k*ln(2)+2*atanh((m-1)/(m+1)), with x=m*2^k.
; Normalizing m into [1,2) keeps the series ratio at most one third.
; Arguments: positive FAC. Returns: natural logarithm in FAC; carry on error.
advanced_log
        LDA fac_exp
        BEQ .invalid
        LDA fac_sign
        BPL .valid
.invalid
        SEC
        RTS
.valid
        LDA fac_exp
        SEC
        SBC #129
        STA log_exponent
        LDA #129
        STA fac_exp
        LDX #5
.mantissa
        LDA fac_exp,X
        STA log_mantissa,X
        DEX
        BPL .mantissa
        JSR log_one_arg
        JSR fp_sub
        LDX #5
.numerator
        LDA fac_exp,X
        STA log_numerator,X
        LDA log_mantissa,X
        STA fac_exp,X
        DEX
        BPL .numerator
        JSR log_one_arg
        JSR fp_add
        JSR fp_copy_fac_to_arg
        LDX #5
.restore_numerator
        LDA log_numerator,X
        STA fac_exp,X
        DEX
        BPL .restore_numerator
        JSR fp_div
        LDX #5
.save_y
        LDA fac_exp,X
        STA log_term,X
        STA log_sum,X
        DEX
        BPL .save_y
        JSR fp_copy_fac_to_arg
        JSR fp_mul
        LDX #5
.save_y2
        LDA fac_exp,X
        STA log_y2,X
        DEX
        BPL .save_y2
        LDA #3
        STA log_counter
.series
        LDX #5
.term_setup
        LDA log_term,X
        STA fac_exp,X
        LDA log_y2,X
        STA arg_exp,X
        DEX
        BPL .term_setup
        JSR fp_mul
        LDX #5
.save_term
        LDA fac_exp,X
        STA log_term,X
        DEX
        BPL .save_term
        LDA log_counter
        JSR fp_digit_to_arg
        JSR fp_div
        JSR fp_copy_fac_to_arg
        LDX #5
.sum_setup
        LDA log_sum,X
        STA fac_exp,X
        DEX
        BPL .sum_setup
        JSR fp_add
        LDX #5
.save_sum
        LDA fac_exp,X
        STA log_sum,X
        DEX
        BPL .save_sum
        INC log_counter
        INC log_counter
        LDA log_counter
        CMP #17
        BCC .series
        JSR fp_copy_fac_to_arg
        JSR fp_add              ; twice the atanh sum
        LDX #5
.scaled
        LDA fac_exp,X
        STA log_sum,X
        DEX
        BPL .scaled
        LDA log_exponent
        STA fp_intlo
        BPL .k_positive
        LDA #$ff
        BNE .k_convert
.k_positive
        LDA #0
.k_convert
        STA fp_inthi
        JSR fp_from_int
        JSR log_ln2_arg
        JSR fp_mul
        LDX #5
.add_sum
        LDA log_sum,X
        STA arg_exp,X
        DEX
        BPL .add_sum
        JSR fp_add
        CLC
        RTS


; Supply the constants used by logarithmic range reduction.
; Arguments: none. Returns: ARG set to one or ln(2), respectively.
log_one_arg
        LDX #5
.copy   LDA log_one,X
        STA arg_exp,X
        DEX
        BPL .copy
        RTS
log_ln2_arg
        LDX #5
.copy   LDA log_ln2,X
        STA arg_exp,X
        DEX
        BPL .copy
        RTS

log_one BYT 129,$80,0,0,0,0
log_ln2 BYT 128,$b1,$72,$17,$f8,0

exp_input EQU $0540
exp_remainder EQU $0546
exp_term EQU $054c
exp_sum EQU $0552
exp_counter EQU $0558
exp_power EQU $0559

; Evaluate exp(x) by subtracting an integer number of ln(2) units and using
; a twelve-term Taylor sum for the small remainder, then scale by 2^k.
; Arguments: FAC is x. Returns: exp(x) in FAC, carry on exponent overflow.
advanced_exp
        LDX #5
.save_input
        LDA fac_exp,X
        STA exp_input,X
        DEX
        BPL .save_input
        JSR log_ln2_arg
        JSR fp_div
        JSR fp_to_int
        LDA fp_inthi
        BEQ .positive_power
        CMP #$ff
        BNE .overflow_jump
        LDA fp_intlo
        CMP #$80
        BCC .overflow_jump
        JMP .power_ok
.positive_power
        LDA fp_intlo
        CMP #126
        BCS .overflow_jump
        JMP .power_ok
.overflow_jump
        JMP .overflow
.power_ok
        LDA fp_intlo
        STA exp_power
        JSR fp_from_int
        JSR log_ln2_arg
        JSR fp_mul
        JSR fp_copy_fac_to_arg
        LDX #5
.restore_input
        LDA exp_input,X
        STA fac_exp,X
        DEX
        BPL .restore_input
        JSR fp_sub
        LDX #5
.save_remainder
        LDA fac_exp,X
        STA exp_remainder,X
        DEX
        BPL .save_remainder
        LDA #1
        STA fp_intlo
        LDA #0
        STA fp_inthi
        JSR fp_from_int
        LDX #5
.initialize
        LDA fac_exp,X
        STA exp_term,X
        STA exp_sum,X
        DEX
        BPL .initialize
        LDA #1
        STA exp_counter
.series
        LDX #5
.term_setup
        LDA exp_term,X
        STA fac_exp,X
        LDA exp_remainder,X
        STA arg_exp,X
        DEX
        BPL .term_setup
        JSR fp_mul
        LDA exp_counter
        JSR fp_digit_to_arg
        JSR fp_div
        LDX #5
.save_term
        LDA fac_exp,X
        STA exp_term,X
        STA arg_exp,X
        LDA exp_sum,X
        STA fac_exp,X
        DEX
        BPL .save_term
        JSR fp_add
        LDX #5
.save_sum
        LDA fac_exp,X
        STA exp_sum,X
        DEX
        BPL .save_sum
        INC exp_counter
        LDA exp_counter
        CMP #13
        BCC .series
        LDA exp_power
        BMI .negative_scale
        CLC
        ADC fac_exp
        BCS .overflow
        STA fac_exp
        CLC
        RTS
.negative_scale
        CLC
        ADC fac_exp
        BCC .underflow
        STA fac_exp
        BNE .success
.underflow
        LDA #0
        STA fac_exp
        STA fac_sign
.success
        CLC
        RTS
.overflow
        SEC
        RTS

atn_value EQU $0520
atn_numerator EQU $0526
atn_square EQU $052c
atn_term EQU $0532
atn_sum EQU $0538
atn_counter EQU $053e
atn_flags EQU $053f          ; bit 0 negative, bit 1 inverted, bit 2 shifted

; Evaluate arctangent with inversion and PI/4 transforms before its odd series.
; After reduction, the series argument has magnitude at most one half.
; Arguments: FAC. Returns: arctangent in radians in FAC.
advanced_atn
        LDA fac_sign
        LSR
        LSR
        LSR
        LSR
        LSR
        LSR
        LSR
        STA atn_flags
        JSR fp_abs
        JSR log_one_arg
        JSR fp_cmp
        CMP #1
        BNE .check_shift
        LDX #5
.save_inverse
        LDA fac_exp,X
        STA atn_value,X
        DEX
        BPL .save_inverse
        JSR fp_copy_fac_to_arg
        LDA #1
        STA fp_intlo
        LDA #0
        STA fp_inthi
        JSR fp_from_int
        JSR fp_div
        LDA atn_flags
        ORA #2
        STA atn_flags
.check_shift
        LDX #5
.save_value
        LDA fac_exp,X
        STA atn_value,X
        DEX
        BPL .save_value
        JSR atn_half_arg
        JSR fp_cmp
        CMP #$ff
        BEQ .series_start
        JSR log_one_arg
        JSR fp_sub              ; numerator = x-1
        LDX #5
.save_numerator
        LDA fac_exp,X
        STA atn_numerator,X
        LDA atn_value,X
        STA fac_exp,X
        DEX
        BPL .save_numerator
        JSR log_one_arg
        JSR fp_add              ; denominator = x+1
        JSR fp_copy_fac_to_arg
        LDX #5
.restore_numerator
        LDA atn_numerator,X
        STA fac_exp,X
        DEX
        BPL .restore_numerator
        JSR fp_div
        LDA atn_flags
        ORA #4
        STA atn_flags
.series_start
        LDX #5
.save_term
        LDA fac_exp,X
        STA atn_term,X
        STA atn_sum,X
        DEX
        BPL .save_term
        JSR fp_copy_fac_to_arg
        JSR fp_mul
        LDX #5
.save_square
        LDA fac_exp,X
        STA atn_square,X
        DEX
        BPL .save_square
        LDA #3
        STA atn_counter
.series
        LDX #5
.term_setup
        LDA atn_term,X
        STA fac_exp,X
        LDA atn_square,X
        STA arg_exp,X
        DEX
        BPL .term_setup
        JSR fp_mul
        LDX #5
.save_power
        LDA fac_exp,X
        STA atn_term,X
        DEX
        BPL .save_power
        LDA atn_counter
        JSR fp_digit_to_arg
        JSR fp_div
        JSR fp_copy_fac_to_arg
        LDX #5
.sum_setup
        LDA atn_sum,X
        STA fac_exp,X
        DEX
        BPL .sum_setup
        LDA atn_counter
        AND #2
        BEQ .add_term
        JSR fp_sub
        JMP .save_sum
.add_term
        JSR fp_add
.save_sum
        LDX #5
.store_sum
        LDA fac_exp,X
        STA atn_sum,X
        DEX
        BPL .store_sum
        INC atn_counter
        INC atn_counter
        LDA atn_counter
        CMP #19
        BCC .series
        LDA atn_flags
        AND #4
        BEQ .inversion
        JSR atn_pi4_arg
        JSR fp_add
.inversion
        LDA atn_flags
        AND #2
        BEQ .sign
        JSR fp_copy_fac_to_arg
        JSR atn_pi2_fac
        JSR fp_sub
.sign   LDA atn_flags
        AND #1
        BEQ .done
        JSR fp_neg
.done   RTS

; Load constants for arctangent reduction.
; Arguments: none. Returns: ARG or FAC set to the named constant.
atn_half_arg
        LDX #5
.copy   LDA atn_half,X
        STA arg_exp,X
        DEX
        BPL .copy
        RTS
atn_pi4_arg
        LDX #5
.copy   LDA atn_pi4,X
        STA arg_exp,X
        DEX
        BPL .copy
        RTS
atn_pi2_fac
        LDX #5
.copy   LDA trig_half_pi,X
        STA fac_exp,X
        DEX
        BPL .copy
        RTS

atn_half BYT 128,$80,0,0,0,0
atn_pi4 BYT 128,$c9,$0f,$da,$a2,0
