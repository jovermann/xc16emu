; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; ROM-owned numeric names. One-letter default numeric variables retain the
; original compact table used by FOR/NEXT. Other names use twelve five-byte
; descriptors: first letter, second letter, type, low value, high value.

number_slots EQU $03a0
float_slots EQU $0718           ; twelve five-byte FAC values, after RAM readers
number_first EQU $a2
number_second EQU $a3
number_type EQU $a4
number_slot EQU $a5
number_after EQU $a6

; Clear named numeric descriptors when RUN, NEW, or reset clears variables.
; Arguments: none. Returns: nothing.
number_clear
        LDX #59
        LDA #0
.byte   STA number_slots,X
        DEX
        BPL .byte
        LDX #59
.float  STA float_slots,X
        DEX
        BPL .float
        RTS

; Parse an alphabetic numeric name at input[pos], keeping the first two
; letters and an optional integer '%' suffix. Return carry set with
; number_after at the first unconsumed byte, or carry clear on a nonname.
number_name
        LDY pos
        LDA input,Y
        CMP #65
        BCC .miss
        CMP #91
        BCS .miss
        STA number_first
        LDA #0
        STA number_second
        STA number_type
        INY
.letter LDA input,Y
        CMP #65
        BCC .suffix
        CMP #91
        BCS .suffix
        LDA number_second
        BNE .next
        LDA input,Y
        STA number_second
.next   INY
        JMP .letter
.suffix
        CMP #36                 ; string name is handled elsewhere
        BEQ .miss
        CMP #37                 ; signed integer suffix
        BNE .done
        LDA #1
        STA number_type
        INY
.done   STY number_after
        SEC
        RTS
.miss   CLC
        RTS

; Return carry set and number_slot for an existing typed name.
; Arguments: number_first/second/type. Returns: carry and slot offset.
number_find
        LDX #0
.loop   LDA number_slots,X
        CMP number_first
        BNE .next
        LDA number_slots+1,X
        CMP number_second
        BNE .next
        LDA number_slots+2,X
        AND #$7f               ; high bit carries the floating sign
        CMP number_type
        BEQ .found
.next   TXA
        CLC
        ADC #5
        TAX
        CPX #60
        BCC .loop
        CLC
        RTS
.found  STX number_slot
        SEC
        RTS

; Store vallo/valhi into the named ROM variable. One-letter default values
; stay in the FOR-compatible table; other values claim a descriptor.
; Arguments: parsed name and value. Returns: failed on exhausted table.
number_store
        LDA number_second
        ORA number_type
        BNE .named
        LDA number_first
        SEC
        SBC #65
        ASL
        TAX
        LDA vallo
        STA vars,X
        LDA valhi
        STA vars+1,X
        RTS
.named  JSR number_find
        BCS .write
        LDX #0
.empty  LDA number_slots,X
        BEQ .claim
        TXA
        CLC
        ADC #5
        TAX
        CPX #60
        BCC .empty
        JMP string_out_of_memory
.claim  STX number_slot
        LDA number_first
        STA number_slots,X
        LDA number_second
        STA number_slots+1,X
        LDA number_type
        STA number_slots+2,X
.write  LDX number_slot
        LDA vallo
        STA number_slots+3,X
        LDA valhi
        STA number_slots+4,X
        RTS

; Load the named scalar into vallo/valhi; missing names default to zero.
; Arguments: parsed name. Returns: a 16-bit value, no error.
number_load
        LDA number_second
        ORA number_type
        BNE .named
        LDA number_first
        SEC
        SBC #65
        ASL
        TAX
        LDA vars,X
        STA vallo
        LDA vars+1,X
        STA valhi
        RTS
.named  JSR number_find
        BCC .zero
        LDX number_slot
        LDA number_slots+3,X
        STA vallo
        LDA number_slots+4,X
        STA valhi
        RTS
.zero   LDA #0
        STA vallo
        STA valhi
        RTS

; Allocate a descriptor for the parsed name if it does not exist already.
; Arguments: number_first/second/type. Returns: number_slot or failed.
number_claim
        JSR number_find
        BCS .done
        LDX #0
.search LDA number_slots,X
        BEQ .empty
        TXA
        CLC
        ADC #5
        TAX
        CPX #60
        BCC .search
        JMP string_out_of_memory
.empty  STX number_slot
        LDA number_first
        STA number_slots,X
        LDA number_second
        STA number_slots+1,X
        LDA number_type
        STA number_slots+2,X
.done   RTS

; Persist a BASIC floating value in the ROM variable table. Integer-suffix
; variables use the truncated projection; default variables preserve FAC.
; Arguments: parsed name and FAC. Returns: descriptor plus integer projection.
number_store_float
        JSR $d909              ; integer projection for older graphics paths
        LDA fp_intlo
        STA vallo
        LDA fp_inthi
        STA valhi
        JSR number_store
        LDA number_type
        BNE .done
        JSR number_claim
        LDA failed
        BNE .done
        LDX number_slot
        LDA fac_sign
        STA number_slots+2,X
        LDA fac_exp
        STA float_slots,X
        LDA fac_m0
        STA float_slots+1,X
        LDA fac_m1
        STA float_slots+2,X
        LDA fac_m2
        STA float_slots+3,X
        LDA fac_m3
        STA float_slots+4,X
.done   RTS

; Read a default numeric variable as a full FAC value. An unset variable is
; zero; a signed-integer variable is converted from its 16-bit descriptor.
; Arguments: parsed name. Returns: FAC.
number_load_float
        JSR number_find
        BCC .integer
        LDA number_type
        BNE .integer
        LDX number_slot
        LDA float_slots,X
        STA fac_exp
        LDA float_slots+1,X
        STA fac_m0
        LDA float_slots+2,X
        STA fac_m1
        LDA float_slots+3,X
        STA fac_m2
        LDA float_slots+4,X
        STA fac_m3
        LDA number_slots+2,X
        AND #$80
        STA fac_sign
        RTS
.integer
        JSR number_load
        LDA vallo
        STA fp_intlo
        LDA valhi
        STA fp_inthi
        JMP $d906
