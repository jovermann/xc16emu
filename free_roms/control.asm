; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; BASIC control-flow records live in the spare RAM below the screen matrix.
; A FOR record occupies 20 bytes, a GOSUB record five. The final byte of each
; record is its size, so RETURN can discard FOR records created in a subroutine
; and NEXT can find a named outer loop without a second stack.

ctrl_return_pos EQU $0757

; Save the FOR state that forstmt just parsed in a new stack record.
; Arguments: loop_* hold variable, limit, step, and continuation location.
; Returns: ctrl_sp advances by 20, or failed reports stack overflow.
for_push
        LDA ctrl_sp
        CMP #129                ; $076c + 148 is the last safe byte
        BCC .room
        JMP control_overflow
.room
        TAX
        LDA #1
        STA ctrl_base,X
        INX
        LDA loop_first
        STA ctrl_base,X
        INX
        LDA loop_second
        STA ctrl_base,X
        INX
        LDA loop_type
        STA ctrl_base,X
        INX
        LDY #0
.limit  LDA loop_limit,Y
        STA ctrl_base,X
        INX
        INY
        CPY #6
        BNE .limit
        LDY #0
.step   LDA loop_step,Y
        STA ctrl_base,X
        INX
        INY
        CPY #6
        BNE .step
        LDA looplo
        STA ctrl_base,X
        INX
        LDA loophi
        STA ctrl_base,X
        INX
        LDA looppos
        STA ctrl_base,X
        INX
        LDA #20
        STA ctrl_base,X
        INX
        STX ctrl_sp
        RTS

; Find the top FOR record, optionally matching NEXT's named variable.
; Arguments: found is zero for bare NEXT, nonzero for a parsed variable.
; Returns: ctrl_ix is the matching frame, or failed reports an error.
control_find_for
        LDA ctrl_sp
        STA ctrl_ix
.search LDX ctrl_ix
        BEQ .missing
        DEX
        SEC
        LDA ctrl_ix
        SBC ctrl_base,X
        STA ctrl_ix
        TAX
        LDA ctrl_base,X
        CMP #1
        BNE .missing           ; NEXT cannot cross a pending GOSUB
        LDA found
        BEQ .match
        LDA ctrl_base+1,X
        CMP loop_first
        BNE .search
        LDA ctrl_base+2,X
        CMP loop_second
        BNE .search
        LDA ctrl_base+3,X
        CMP loop_type
        BNE .search
.match  LDA ctrl_ix
        CLC
        ADC #20
        STA ctrl_sp             ; discard any unmatched inner FOR frames
        RTS
.missing
        JMP syntax

; Perform one NEXT item, including optional variable and comma list.
; Arguments: pos points at NEXT. Returns: pos or restart repeats the body.
control_nextstmt
        LDA pos
        CLC
        ADC #4
        STA pos
.item   JSR spaces
        LDA #0
        STA found
        LDA input,Y
        CMP #65
        BCC .find
        CMP #91
        BCS .find
        JSR number_name
        BCS .named
        JMP syntax
.named
        LDA #1
        STA found
        LDA number_first
        STA loop_first
        LDA number_second
        STA loop_second
        LDA number_type
        STA loop_type
        LDA number_after
        STA pos
.find   JSR control_find_for
        LDA failed
        BEQ .active
        RTS
.active
        LDX ctrl_ix
        LDA ctrl_base+1,X
        STA number_first
        LDA ctrl_base+2,X
        STA number_second
        LDA ctrl_base+3,X
        STA number_type
        JSR number_load_float
        LDX ctrl_ix
        LDY #0
.step   LDA ctrl_base+10,X
        STA arg_exp,Y
        INX
        INY
        CPY #6
        BNE .step
        JSR $d900              ; FAC += saved STEP
        JSR number_store_float
        LDX ctrl_ix
        LDY #0
.limit  LDA ctrl_base+4,X
        STA arg_exp,Y
        INX
        INY
        CPY #6
        BNE .limit
        JSR $d918              ; compare FAC to the FOR limit
        LDX ctrl_ix
        LDY ctrl_base+15,X    ; STEP sign decides the end comparison
        BMI .descending
        CMP #1
        BNE .repeat
        JMP .pop
.descending
        CMP #$ff
        BNE .repeat
.pop    LDA ctrl_ix
        STA ctrl_sp
        JSR spaces
        CMP #44                 ; NEXT J,I advances both loop levels
        BNE .done
        INC pos
        JMP .item
.repeat
        LDX ctrl_ix
        LDA ctrl_base+16,X
        STA looplo
        LDA ctrl_base+17,X
        STA loophi
        LDA ctrl_base+18,X
        STA looppos
        LDA runmode
        BEQ .same_line
        LDA runlo
        CMP looplo
        BNE .other_line
        LDA runhi
        CMP loophi
        BEQ .same_line
.other_line
        LDA #1
        STA restart
        RTS
.same_line
        LDA looppos
        STA pos
.done   RTS
.bad    JMP syntax

; Push the caller location and jump to a numbered BASIC subroutine.
; Arguments: pos points at GOSUB. Returns: restart selects the target line.
gosubstmt
        LDA #kw_gosub%256
        STA kwlo
        LDA #kw_gosub/256
        STA kwhi
        JSR keyword
        BCC .bad
        JSR spaces
        JSR expr
        LDA failed
        BNE .done
        LDA pos
        STA ctrl_return_pos
        LDA vallo
        STA linelo
        LDA valhi
        STA linehi
        JSR findline
        LDA found
        BEQ .missing
        LDA ctrl_sp
        CMP #144
        BCC .room
        JMP control_overflow
.room
        TAX
        LDA #2
        STA ctrl_base,X
        INX
        LDA runlo
        LDY runmode
        BNE .stored
        LDA #0                  ; zero high byte marks direct GOSUB
.stored STA ctrl_base,X
        INX
        LDA runhi
        LDY runmode
        BNE .stored_hi
        LDA #0
.stored_hi
        STA ctrl_base,X
        INX
        LDA ctrl_return_pos
        STA ctrl_base,X
        INX
        LDA #5
        STA ctrl_base,X
        INX
        STX ctrl_sp
        LDA runmode
        BNE .jump
        JSR runprogram_keepvars
        RTS
.jump   LDA ptrlo
        STA runlo
        LDA ptrhi
        STA runhi
        LDA #2
        STA restart
.done   RTS
.missing
        JMP control_missing_line
.bad    JMP syntax

; Pop the nearest GOSUB, discarding FOR records created by that call.
; Arguments: pos points at RETURN. Returns: restart resumes its caller.
returnstmt
        LDA #kw_return%256
        STA kwlo
        LDA #kw_return/256
        STA kwhi
        JSR keyword
        BCC .bad
        LDA ctrl_sp
        STA ctrl_ix
.search LDX ctrl_ix
        BEQ .bad
        DEX
        SEC
        LDA ctrl_ix
        SBC ctrl_base,X
        STA ctrl_ix
        TAX
        LDA ctrl_base,X
        CMP #2
        BNE .search
        LDA ctrl_ix
        STA ctrl_sp
        LDA ctrl_base+2,X
        BEQ .direct
        STA runhi
        LDA ctrl_base+1,X
        STA runlo
        LDA ctrl_base+3,X
        STA resume
        LDA #2
        STA restart
        RTS
.direct
        LDA #3
        STA restart
        RTS
.bad    JMP syntax

; Evaluate a floating condition and choose its THEN or ELSE clause.
; Arguments: pos points at IF. Returns: pos/restart select the chosen clause.
ifstmt
        LDA #kw_if%256
        STA kwlo
        LDA #kw_if/256
        STA kwhi
        JSR keyword
        BCS .matched
        JMP syntax
.matched
        JSR fexpr
        LDA failed
        BEQ .expr_ok
        RTS
.expr_ok
        JSR spaces
        CMP #61
        BEQ .equal
        CMP #60
        BEQ .less
        CMP #62
        BEQ .greater
        LDA fac_exp             ; zero exponent means false
        BNE .nonzero
        JMP .false
.nonzero
        LDA #1
        STA if_op
        JMP .then
.equal LDA #1
        BNE .operator
.less  LDA #2
        BNE .operator
.greater
        LDA #3
.operator
        STA if_op
        INC pos
        JSR spaces
        CMP #61
        BNE .not_equal_suffix
        LDA if_op
        CMP #2
        BNE .greater_equal
        LDA #4
        BNE .suffix
.greater_equal
        CMP #3
        BNE .not_equal_suffix
        LDA #5
.suffix STA if_op
        INC pos
        JMP .right
.not_equal_suffix
        CMP #62
        BNE .right
        LDA if_op
        CMP #2
        BNE .right
        LDA #6
        STA if_op
        INC pos
.right  LDX #0
.save   LDA fac_exp,X
        PHA
        INX
        CPX #6
        BNE .save
        JSR fexpr
        JSR $d924              ; right FAC becomes ARG
        LDX #5
.restore
        PLA
        STA fac_exp,X
        DEX
        BPL .restore
        LDA failed
        BEQ .cmp_ok
        RTS
.cmp_ok
        JSR $d918              ; -1, 0, 1 for left versus right
        LDX if_op
        CPX #1
        BEQ .eqtest
        CPX #6
        BEQ .netest
        CPX #2
        BEQ .lttest
        CPX #3
        BEQ .gttest
        CPX #4
        BEQ .letest
        CMP #$ff               ; >=
        BEQ .false
        BNE .true
.eqtest CMP #0
        BEQ .true
        BNE .false
.netest CMP #0
        BNE .true
        BEQ .false
.lttest CMP #$ff
        BEQ .true
        BNE .false
.gttest CMP #1
        BEQ .true
        BNE .false
.letest CMP #1
        BEQ .false
.true   LDA #1
        STA if_op
        JMP .then
.false  LDA #0
        STA if_op
.then   LDA #kw_then%256
        STA kwlo
        LDA #kw_then/256
        STA kwhi
        JSR spaces
        JSR keyword
        BCC .bad
        LDA if_op
        BEQ .skip_then
        JMP if_clause
.skip_then
        LDA #0
        STA trail               ; quoted text does not contain keywords
.scan   LDY pos
        LDA input,Y
        BEQ .no_else
        CMP #34
        BNE .unquoted
        LDA trail
        EOR #1
        STA trail
        JMP .advance
.unquoted
        LDA trail
        BNE .advance
        LDA input,Y
        CMP #69                 ; first ELSE on the source line
        BNE .advance
        INY
        LDA input,Y
        CMP #76
        BNE .advance
        INY
        LDA input,Y
        CMP #83
        BNE .advance
        INY
        LDA input,Y
        CMP #69
        BEQ .found_else
.advance
        INC pos
        JMP .scan
.found_else
        LDA pos
        CLC
        ADC #4
        STA pos
        JMP if_clause
.no_else
.done   RTS
.bad    JMP syntax

; Run a chosen IF clause; a bare number is an implicit GOTO line.
; Arguments: pos follows THEN or ELSE. Returns: pos or restart advances.
if_clause
        JSR spaces
        CMP #48
        BCC .statement
        CMP #58
        BCS .statement
        JSR expr
        LDA failed
        BNE .done
        LDA vallo
        STA linelo
        LDA valhi
        STA linehi
        JSR findline
        LDA found
        BEQ .missing
        LDA runmode
        BEQ .direct
        LDA ptrlo
        STA runlo
        LDA ptrhi
        STA runhi
        LDA #2
        STA restart
        RTS
.direct
        JSR runprogram_keepvars
.done   RTS
.statement
        RTS
.missing
        JMP control_missing_line

; Report a branch target that is not present in the numbered program.
; Arguments: none. Returns: failed set and an undefined-line diagnostic.
control_missing_line
        LDA #1
        STA failed
        JSR error_to_text
        LDA #undefmsg%256
        STA msglo
        LDA #undefmsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

; Report a full control stack while leaving the screen in text mode.
; Arguments: none. Returns: failed set and a visible diagnostic.
control_overflow
        LDA #1
        STA failed
        JSR error_to_text
        LDA #ctrl_overflow_msg%256
        STA msglo
        LDA #ctrl_overflow_msg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

kw_gosub BYT "GOSUB",0
kw_return BYT "RETURN",0
kw_if BYT "IF",0
kw_then BYT "THEN",0
ctrl_overflow_msg BYT "?OUT OF MEMORY",0
