; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Rotation for BASIC CIRCLE. The BASIC ROM owns syntax and midpoint ellipse
; sampling; this system-ROM extension rotates its four signed quadrant points.
; Q6 trigonometric coefficients keep each product within signed 16-bit range.

        ORG $E2E0
circle_rotation_prepare JMP circle_prepare ; $E2E0: rotation -> sin/cos
circle_rotation_four JMP circle_plot_four   ; $E2E3: plot four points
circle_stepped_outline JMP circle_polygon   ; $E2E6: closed stepped arc

rot_oct_x EQU $0308
rot_oct_y EQU $0309
rot_cxlo EQU $0300
rot_cxhi EQU $0301
rot_cy EQU $0302
rot_gxlo EQU $72
rot_gxhi EQU $73
rot_gy EQU $74
rot_gyhi EQU $7a
rot_angle EQU $06b5
rot_saved_fac EQU $06b7
rot_sin EQU $06bd
rot_cos EQU $06be
rot_coef EQU $06bf
rot_mlo EQU $06c0
rot_mhi EQU $06c1
rot_prodlo EQU $06c2
rot_prodhi EQU $06c3
rot_sign EQU $06c4
rot_a EQU $06c5
rot_b EQU $06c6
rot_c EQU $06c7
rot_d EQU $06c8
rot_offx EQU $06c9
rot_offy EQU $06ca
poly_remlo EQU $06cb
poly_remhi EQU $06cc
poly_anglelo EQU $06cd
poly_anglehi EQU $06ce
poly_firstlo EQU $06cf
poly_firsthi EQU $06d0
poly_firsty EQU $06d1
poly_step EQU $06b6
poly_radius EQU $0303
poly_yradius EQU $0322
poly_targetlo EQU $030a
poly_targethi EQU $030b
poly_targety EQU $030c

; Turn the requested degree rotation into Q6 sin/cos once per CIRCLE call.
; Arguments: rot_angle is 0..255 degrees. Returns: rot_sin/rot_cos signed.
circle_prepare
        LDA rot_angle
        STA poly_anglelo
        LDA #0
        STA poly_anglehi
        JMP circle_prepare_angle

; Prepare Q6 sine/cosine for a signed 16-bit degree angle.
; Arguments: poly_anglelo/hi hold degrees. Returns: rot_sin/rot_cos.
circle_prepare_angle
        LDA #180
        STA fp_intlo
        LDA #0
        STA fp_inthi
        JSR fp_from_int
        JSR fp_copy_fac_to_arg
        JSR fp_pi_to_fac
        JSR fp_div              ; radians per degree = pi/180
        JSR fp_copy_fac_to_arg
        LDA poly_anglelo
        STA fp_intlo
        LDA poly_anglehi
        STA fp_inthi
        JSR fp_from_int
        JSR fp_mul              ; FAC = angle in radians
        LDX #5
.save   LDA fac_exp,X
        STA rot_saved_fac,X
        DEX
        BPL .save
        JSR advanced_sin
        JSR circle_q6_from_fac
        STA rot_sin
        LDX #5
.restore
        LDA rot_saved_fac,X
        STA fac_exp,X
        DEX
        BPL .restore
        JSR advanced_cos
        JSR circle_q6_from_fac
        STA rot_cos
        RTS

; Draw a closed polyline at the requested angular step. A 120-degree step
; yields the three rotating sides used by the TOLL BASIC example.
; Arguments: rot_angle, poly_step, radius, yradius, center and source in RAM.
; Returns: the outline is drawn and the current graphics point is its start.
circle_polygon
        SEC
        LDA rot_angle
        SBC #90                 ; CIRCLE zero degrees starts at the top
        STA poly_anglelo
        LDA #0
        SBC #0
        STA poly_anglehi
        JSR circle_vertex
        LDA poly_targetlo
        STA poly_firstlo
        STA rot_gxlo
        LDA poly_targethi
        STA poly_firsthi
        STA rot_gxhi
        LDA poly_targety
        STA poly_firsty
        STA rot_gy
        LDA #0
        STA rot_gyhi
        LDA #$68               ; 360 degrees remaining
        STA poly_remlo
        LDA #1
        STA poly_remhi
.next   LDA poly_remhi
        BNE .advance
        LDA poly_remlo
        CMP poly_step
        BCC .close
        BEQ .close
.advance
        SEC
        LDA poly_remlo
        SBC poly_step
        STA poly_remlo
        LDA poly_remhi
        SBC #0
        STA poly_remhi
        CLC
        LDA poly_anglelo
        ADC poly_step
        STA poly_anglelo
        LDA poly_anglehi
        ADC #0
        STA poly_anglehi
        JSR circle_vertex
        JSR $800f               ; draw one side, leaving current at target
        JMP .next
.close  LDA poly_firstlo
        STA poly_targetlo
        LDA poly_firsthi
        STA poly_targethi
        LDA poly_firsty
        STA poly_targety
        JMP $800f

; Evaluate one ellipse vertex from its signed degree angle.
; Arguments: poly_anglelo/hi and center/radii. Returns: line target in RAM.
circle_vertex
        JSR circle_prepare_angle
        LDA poly_radius
        LDY rot_cos
        JSR circle_mul_q6
        STA rot_offx
        LDA poly_yradius
        LDY rot_sin
        JSR circle_mul_q6
        STA rot_offy
        CLC
        LDA rot_cxlo
        ADC rot_offx
        STA poly_targetlo
        LDY rot_offx
        BPL .x_positive
        LDA rot_cxhi
        ADC #$ff
        JMP .x_done
.x_positive
        LDA rot_cxhi
        ADC #0
.x_done STA poly_targethi
        CLC
        LDA rot_cy
        ADC rot_offy
        STA poly_targety
        RTS

; Scale the FAC by 64 and truncate to one signed coefficient byte.
; Arguments: FAC is a trig result in [-1,1]. Returns: A is its Q6 value.
circle_q6_from_fac
        JSR fp_copy_fac_to_arg
        LDA #64
        STA fp_intlo
        LDA #0
        STA fp_inthi
        JSR fp_from_int
        JSR fp_mul
        JSR fp_to_int
        LDA fp_intlo
        RTS

; Plot all four sign combinations after applying the requested rotation.
; Arguments: rot_oct_x/y are positive ellipse offsets. Returns: four pixels.
circle_plot_four
        LDA rot_oct_x
        LDY rot_cos
        JSR circle_mul_q6
        STA rot_a
        LDA rot_oct_y
        LDY rot_sin
        JSR circle_mul_q6
        STA rot_b
        LDA rot_oct_x
        LDY rot_sin
        JSR circle_mul_q6
        STA rot_c
        LDA rot_oct_y
        LDY rot_cos
        JSR circle_mul_q6
        STA rot_d

        ; (+x,+y): (a-b,c+d)
        SEC
        LDA rot_a
        SBC rot_b
        STA rot_offx
        CLC
        LDA rot_c
        ADC rot_d
        TAY
        LDA rot_offx
        JSR circle_plot_offset

        ; (-x,+y): (-a-b,-c+d)
        SEC
        LDA #0
        SBC rot_a
        SEC
        SBC rot_b
        STA rot_offx
        SEC
        LDA rot_d
        SBC rot_c
        TAY
        LDA rot_offx
        JSR circle_plot_offset

        ; (+x,-y): (a+b,c-d)
        CLC
        LDA rot_a
        ADC rot_b
        STA rot_offx
        SEC
        LDA rot_c
        SBC rot_d
        TAY
        LDA rot_offx
        JSR circle_plot_offset

        ; (-x,-y): (-a+b,-c-d)
        SEC
        LDA rot_b
        SBC rot_a
        STA rot_offx
        SEC
        LDA #0
        SBC rot_c
        SEC
        SBC rot_d
        TAY
        LDA rot_offx
        JMP circle_plot_offset

; Multiply an unsigned offset by a signed Q6 coefficient and truncate /64.
; Arguments: A is offset 0..199, Y is signed coefficient. Returns: signed A.
circle_mul_q6
        STA rot_mlo
        LDA #0
        STA rot_mhi
        STA rot_prodlo
        STA rot_prodhi
        STA rot_sign
        TYA
        BPL .positive
        INC rot_sign
        EOR #$ff
        CLC
        ADC #1
.positive
        STA rot_coef
        LDX #8
.bit    LSR rot_coef
        BCC .shift
        CLC
        LDA rot_prodlo
        ADC rot_mlo
        STA rot_prodlo
        LDA rot_prodhi
        ADC rot_mhi
        STA rot_prodhi
.shift  ASL rot_mlo
        ROL rot_mhi
        DEX
        BNE .bit
        LDX #6
.divide
        LSR rot_prodhi
        ROR rot_prodlo
        DEX
        BNE .divide
        LDA rot_prodlo
        LDY rot_sign
        BEQ .done
        EOR #$ff
        CLC
        ADC #1
.done   RTS

; Add signed offsets to the center and use the BASIC ROM's clipped plot API.
; Arguments: A is X offset, Y is Y offset. Returns: one plotted pixel.
circle_plot_offset
        STA rot_offx
        STY rot_offy
        CLC
        LDA rot_cxlo
        ADC rot_offx
        STA rot_gxlo
        LDY rot_offx
        BPL .x_positive
        LDA rot_cxhi
        ADC #$ff
        JMP .x_done
.x_positive
        LDA rot_cxhi
        ADC #0
.x_done STA rot_gxhi
        CLC
        LDA rot_cy
        ADC rot_offy
        STA rot_gy
        LDY rot_offy
        BPL .y_positive
        LDA #0
        ADC #$ff
        JMP .y_done
.y_positive
        LDA #0
        ADC #0
.y_done STA rot_gyhi
        JMP $800c               ; BASIC ROM owns TED pixel memory layout
