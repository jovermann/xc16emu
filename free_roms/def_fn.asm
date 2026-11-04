; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; A compact one-argument DEF FN implementation. The definition remembers a
; location in numbered program RAM; calls evaluate that source on the 6502.
; The transient $0600 buffer is shared with graphics flood fill, which cannot
; run concurrently with a numeric expression.

        ORG $E600
fn_call_api JMP fn_call       ; $E600: try FN name(argument), carry on match
fn_define_api JMP fn_define   ; $E603: parse body after DEF

fn_valid EQU $03dd
fn_name EQU $03de
fn_formal EQU $03df
fn_reclo EQU $03fa
fn_rechi EQU $03fb
fn_offset EQU $03fc
fn_length EQU $03fd
fn_active EQU $03fe
fn_inputsave EQU $0600
fn_argfac EQU $0680
fn_oldfac EQU $0686
fn_resultfac EQU $068c
fn_callerpos EQU $0692
fn_ptrlo EQU $0693
fn_ptrhi EQU $0694
fn_source_index EQU $0695
fn_input EQU $0200
fn_pos EQU $30
fn_failed EQU $44
fn_runmode EQU $52
fn_runlo EQU $53
fn_runhi EQU $54
fn_ram_ptrlo EQU $49
fn_ram_ptrhi EQU $4a
fn_number_first EQU $a2
fn_number_second EQU $a3
fn_number_type EQU $a4

; Record a one-letter function name, formal name, and source expression.
; Arguments: fn_pos follows the DEF keyword in a numbered program line.
; Returns: fn_valid is set and pos skips the definition, or failed is set.
fn_define
        JSR fn_spaces
        CMP #70                 ; F
        BNE fn_def_error
        INC fn_pos
        JSR fn_spaces
        CMP #78                 ; N
        BNE fn_def_error
        INC fn_pos
        JSR fn_spaces
        CMP #65
        BCC fn_def_error
        CMP #91
        BCS fn_def_error
        STA fn_name
        INC fn_pos
        JSR fn_spaces
        CMP #40                 ; (
        BNE fn_def_error
        INC fn_pos
        JSR fn_spaces
        CMP #65
        BCC fn_def_error
        CMP #91
        BCS fn_def_error
        STA fn_formal
        INC fn_pos
        JMP fn_def_after_error
fn_def_error
        JMP fn_error
fn_def_after_error
        JSR fn_spaces
        CMP #41                 ; )
        BNE fn_def_error
        INC fn_pos
        JSR fn_spaces
        CMP #61                 ; =
        BNE fn_def_error
        INC fn_pos
        LDA fn_runmode
        BEQ fn_def_error        ; the expression must have a RAM record
        LDA fn_runlo
        STA fn_reclo
        LDA fn_runhi
        STA fn_rechi
        LDA fn_pos
        CLC
        ADC #3                  ; skip line number and source length
        STA fn_offset
        LDY fn_pos
        LDX #0
.measure
        LDA fn_input,Y
        BEQ .measured
        CMP #58                 ; a later colon starts another statement
        BEQ .measured
        INY
        INX
        CPX #127
        BCS fn_error
        JMP .measure
.measured
        CPX #0
        BEQ fn_error
        STX fn_length
        STY fn_pos
        LDA #0
        STA fn_active
        LDA #1
        STA fn_valid
        RTS

; Report malformed, missing, or recursive function syntax through BASIC.
; Arguments: none. Returns: failed set and carry set to claim FN/DEF.
fn_error
        JSR $8018
        SEC
        RTS

; Evaluate FN name(expression) in the stored function body.
; Arguments: fn_pos points at a possible FN; current FAC/parser are live.
; Returns: carry clear if this is another built-in, else FAC and carry set.
fn_call
        LDY fn_pos
        LDA fn_input,Y
        CMP #70                 ; F
        BNE .miss
        INY
        LDA fn_input,Y
        CMP #78                 ; N
        BNE .miss
        INY
        STY fn_pos
        JMP .matched
.miss   CLC
        RTS
.matched
        JSR fn_spaces
        CMP fn_name
        BNE fn_error
        LDA fn_valid
        BEQ fn_error
        LDA fn_active
        BNE fn_error            ; recursive call has no second input buffer
        INC fn_pos
        JSR fn_spaces
        CMP #40
        BNE fn_error
        INC fn_pos
        JSR $8009               ; evaluate actual argument in caller source
        LDA fn_failed
        BEQ .arg_ready
        SEC
        RTS
.arg_ready
        JSR fn_spaces
        CMP #41
        BNE fn_error
        INC fn_pos
        LDX #5
.save_arg
        LDA fac_exp,X
        STA fn_argfac,X
        DEX
        BPL .save_arg
        LDA fn_pos
        STA fn_callerpos
        LDX #0
.save_source
        LDA fn_input,X
        STA fn_inputsave,X
        INX
        CPX #128
        BNE .save_source
        LDA fn_ram_ptrlo
        STA fn_ptrlo
        LDA fn_ram_ptrhi
        STA fn_ptrhi

        ; Temporarily bind the formal name, then restore its former value.
        LDA fn_formal
        STA fn_number_first
        LDA #0
        STA fn_number_second
        STA fn_number_type
        JSR $8015               ; old numeric value -> FAC
        LDX #5
.save_old
        LDA fac_exp,X
        STA fn_oldfac,X
        LDA fn_argfac,X
        STA fac_exp,X
        DEX
        BPL .save_old
        JSR $8012               ; formal := actual argument
        LDA fn_failed
        BNE .restore
        LDA #1
        STA fn_active
        LDA fn_reclo
        STA fn_ram_ptrlo
        LDA fn_rechi
        STA fn_ram_ptrhi
        LDY fn_offset
        LDX #0
.copy_body
        CPX fn_length
        BEQ .body_ready
        JSR $0700               ; read program RAM beneath ROM overlays
        STA fn_input,X
        INY
        INX
        JMP .copy_body
.body_ready
        LDA #0
        STA fn_input,X
        STA fn_pos
        JSR $8009               ; evaluate the saved expression in the ROM
        LDX #5
.save_result
        LDA fac_exp,X
        STA fn_resultfac,X
        LDA fn_oldfac,X
        STA fac_exp,X
        DEX
        BPL .save_result
        LDA fn_formal
        STA fn_number_first
        LDA #0
        STA fn_number_second
        STA fn_number_type
        JSR $8012               ; restore formal's previous value
        LDX #5
.result
        LDA fn_resultfac,X
        STA fac_exp,X
        DEX
        BPL .result
        LDA #0
        STA fn_active
.restore
        LDX #0
.restore_source
        LDA fn_inputsave,X
        STA fn_input,X
        INX
        CPX #128
        BNE .restore_source
        LDA fn_ptrlo
        STA fn_ram_ptrlo
        LDA fn_ptrhi
        STA fn_ram_ptrhi
        LDA fn_callerpos
        STA fn_pos
        SEC
        RTS

; Skip source spaces and return the following character in A.
; Arguments: fn_pos is a source offset. Returns: updated pos and byte A.
fn_spaces
        LDY fn_pos
.again  LDA fn_input,Y
        CMP #32
        BNE .done
        INY
        JMP .again
.done   STY fn_pos
        RTS
def_fn_end

