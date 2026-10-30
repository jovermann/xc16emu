; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Independently written BASIC subset with a packed numbered-program area.
; Program RAM begins at $0FE8 and ends before $4000, $8000, or $FD00,
; according to the installed 16, 32, or 64 KiB RAM. A low-RAM reader lets
; the ROM inspect program records beneath the BASIC and system ROM overlays.

        ASS $8000

api_line JMP entry             ; $8000: submit the input buffer
api_init JMP initprogram       ; $8003: clear program/variables on reset
api_free JMP freeprint         ; $8006: print measured available bytes
api_float JMP fexpr            ; $8009: ROM-only floating expression parser

input   EQU $0200               ; zero-terminated line from sysrom
vars    EQU $0280               ; 26 one-letter unsigned integer variables
assign_first EQU $033e          ; numeric name saved across expression calls
assign_second EQU $033f
assign_type EQU $03dc
num_overflow EQU $aa           ; decimal literal exceeded signed integer range
pos     EQU $30                 ; current input byte index
vallo   EQU $31                 ; current expression result
valhi   EQU $32
sumlo   EQU $33                 ; additive left operand
sumhi   EQU $34
termlo  EQU $35                 ; multiplicative left operand
termhi  EQU $36
digit   EQU $37
op      EQU $38
forvar  EQU $39                 ; byte offset of active FOR variable
limitlo EQU $3a
limithi EQU $3b
looppos EQU $3c                 ; parser position at the loop's colon
tmplo   EQU $3d
tmphi   EQU $3e
mullo   EQU $3f
mulhi   EQU $40
count   EQU $41
trail   EQU $42                 ; PRINT ends in comma/semicolon
started EQU $43                 ; decimal conversion began output
failed  EQU $44                 ; abort the line after a syntax error
linelo  EQU $45                 ; number parsed at the start of a source line
linehi  EQU $46
endlo   EQU $47                 ; first unused byte in packed program RAM
endhi   EQU $48
ptrlo   EQU $49                 ; current program record / insertion point
ptrhi   EQU $4a
srclo   EQU $4b                 ; source pointer while moving records
srchi   EQU $4c
dstlo   EQU $4d                 ; destination pointer while moving records
dsthi   EQU $4e
reclen  EQU $4f                 ; text bytes excluding three-byte header
recsize EQU $50                 ; complete record size
found   EQU $51                 ; existing line matched
runmode EQU $52                 ; executing stored lines
runlo   EQU $53                 ; current record while RUN is active
runhi   EQU $54
looplo  EQU $55                 ; record containing the active FOR body
loophi  EQU $56
restart EQU $57                 ; NEXT branches to a different record
newlo   EQU $59                 ; program end after an insertion/replacement
newhi   EQU $5a
resume  EQU $5b                 ; parser offset when NEXT repeats a later line
minlo   EQU $5c                 ; LIST lower line bound
minhi   EQU $5d
maxlo   EQU $5e                 ; LIST upper line bound
maxhi   EQU $5f
storedline EQU $60            ; tell sysrom that a numbered line needs no READY
memlimhi EQU $61               ; exclusive program limit: $40/$80/$FD
gfxmode EQU $62                 ; shared with the system ROM raster IRQ
gfxphase EQU $63
sound1lo EQU $64
sound1hi EQU $65
sound2lo EQU $66
sound2hi EQU $67
gfxlimit EQU $68                ; program limit while bitmap is allocated
scaleflag EQU $7b               ; map BASIC's 0..1023 graphics coordinates
gfxmatrix EQU $7e               ; FF14 value for graphics matrix pair
rampeek_ptr EQU $0700          ; copied reader for (ptrlo),Y beneath ROM
rampeek_src EQU $070b          ; copied reader for (srclo),Y beneath ROM

putchar EQU $c800
printz  EQU $c803
newline EQU $c806
column  EQU $27                 ; text column owned by sysrom
msglo   EQU $22                 ; zero-terminated message pointer
msghi   EQU $23

entry   LDA #$00
        STA pos
        STA failed
        STA runmode
        STA storedline
        LDA #$ff
        STA forvar              ; no active direct-mode loop
        JSR spaces
        CMP #48
        BCC execute
        CMP #58
        BCS execute
        JSR atom                ; decimal line number
        LDA vallo
        STA linelo
        LDA valhi
        STA linehi
        JSR spaces
        LDA #1
        STA storedline
        JMP storeline
finished
        RTS
execute JSR checkbreak
        LDA failed
        BEQ .execute_ready
        RTS
.execute_ready
        JSR spaces
        LDY pos
        LDA input,Y
        BEQ finished
        CMP #58                 ; ':' joins statements
        BNE .dispatch
        INC pos
        JMP execute
.dispatch
        JSR string_mid_assignment
        BCC .ordinary_assignment
        LDA failed
        BNE finished
        JMP execute
.ordinary_assignment
        JSR string_assignment
        BCC .numeric_or_keyword
        LDA failed
        BNE finished
        JMP execute
.numeric_or_keyword
        JSR assignstmt
        BCC .keyword
        LDA failed
        BNE finished
        JMP execute
.keyword
        LDY pos
        LDA input,Y
        CMP #80                 ; PRINT
        BNE .notprint
        LDY pos
        INY
        LDA input,Y
        CMP #82                 ; PAINT shares the initial P
        BEQ .isprint
        JMP .tryextra
.isprint
        JSR printstmt
        LDA failed
        BNE finished
        JMP execute
.notprint
        CMP #70                 ; FOR
        BNE .notfor
        JSR forstmt
        LDA failed
        BNE finished
        JMP execute
.notfor  CMP #78                 ; NEXT
        BNE .notnext
        JSR nextstmt
        LDA failed
        BNE finished
        LDA restart
        BNE finished
        JMP execute
.notnext CMP #82                 ; REM or RUN
        BNE .notrem
        LDY pos
        INY
        LDA input,Y
        CMP #69                 ; E
        BNE .tryrun
        INY
        LDA input,Y
        CMP #77                 ; M
        BEQ .rem_done
        JMP syntax
.rem_done
        RTS
.tryrun  CMP #85                 ; U
        BEQ .run_u
        JMP syntax
.run_u
        INY
        LDA input,Y
        CMP #78                 ; N
        BEQ .runvalid
        JMP syntax
.runvalid
        LDA pos
        CLC
        ADC #3
        STA pos
        JSR spaces
        CMP #48
        BCC .runfromstart
        CMP #58
        BCS .runfromstart
        JSR atom
        LDA vallo
        STA linelo
        LDA valhi
        STA linehi
        JSR findline
        LDA found
        BNE .runnow
        JMP syntax
.runfromstart
        LDA #$e8
        STA ptrlo
        LDA #$0f
        STA ptrhi
.runnow
        JSR runprogram
        RTS
.notrem  CMP #76                 ; LIST
        BNE .notlist
        LDY pos
        INY
        LDA input,Y
        CMP #73                 ; I
        BNE .tryextra           ; LOCATE also begins with L
        INY
        LDA input,Y
        CMP #83                 ; S
        BNE syntax
        INY
        LDA input,Y
        CMP #84                 ; T
        BNE syntax
        JSR listprogram
        RTS
.notlist CMP #71                 ; GOTO
        BNE .tryextra
        LDY pos
        INY
        LDA input,Y
        CMP #79                 ; O
        BNE .tryextra
        INY
        LDA input,Y
        CMP #84                 ; T
        BNE .tryextra
        INY
        LDA input,Y
        CMP #79                 ; O
        BNE .tryextra
        JSR gotostmt
        RTS
.tryextra
        JSR extra_dispatch
        BCC .unknown
        LDA failed
        BEQ .extracontinue
        RTS
.extracontinue
        JMP execute
.unknown JMP syntax
syntax  LDA #$01
        STA failed
        JSR error_to_text
        LDA #errmsg%256
        STA msglo
        LDA #errmsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

; Report an integer quantity outside the signed 16-bit BASIC % range.
; Arguments: none. Returns: failed set and a visible diagnostic.
quantity_error
        LDA #1
        STA failed
        JSR error_to_text
        LDA #quantmsg%256
        STA msglo
        LDA #quantmsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

; Report a division whose right-hand argument is zero.
; Arguments: none. Returns: failed set and a visible diagnostic.
division_zero_error
        LDA #1
        STA failed
        JSR error_to_text
        LDA #zeromsg%256
        STA msglo
        LDA #zeromsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

; BASIC errors leave the allocated bitmap intact but restore ordinary text
; for the diagnostic and following READY prompt. The frame IRQ keeps running
; for SOUND durations; gfxmode zero prevents it from restoring graphics.
error_to_text
        LDA #0
        STA gfxmode
        STA gfxphase
        STA $ff0b
        LDA #2
        STA $ff09               ; discard a pending split-line interrupt
        LDA #$1b
        STA $ff06
        LDA #8
        STA $ff07
        STA $ff14
        RTS

; Poll the C16 RUN/STOP matrix position before each BASIC statement.
; The host sets row 6, bit 4 directly even while the text queue is busy.
checkbreak
        LDA #$bf                ; select keyboard row 6
        STA $fd30
        LDA #$ff
        STA $ff08
        LDA $ff08
        AND #$10                ; active-low RUN/STOP bit
        BNE .nobreak
        LDA #1
        STA failed
        JSR error_to_text
        LDA #breakmsg%256
        STA msglo
        LDA #breakmsg/256
        STA msghi
        JSR printz
        JSR newline
.nobreak RTS

; Jump to an existing numbered line. A direct GOTO preserves variables;
; a GOTO inside RUN changes the next record without nesting another runner.
gotostmt
        LDA pos
        CLC
        ADC #4
        STA pos
        JSR spaces
        CMP #48                 ; a target expression must start with a digit
        BCC .goto_syntax
        CMP #58
        BCS .goto_syntax
        JSR expr
        LDA failed
        BNE .goto_done
        JSR spaces
        CMP #0
        BEQ .goto_target
        CMP #58                 ; allow a following statement separator
        BNE .goto_syntax
.goto_target
        LDA vallo
        STA linelo
        LDA valhi
        STA linehi
        JSR findline
        LDA found
        BEQ .goto_missing
        LDA runmode
        BEQ .goto_direct
        LDA ptrlo
        STA runlo
        LDA ptrhi
        STA runhi
        LDA #2                  ; tell RUN that its record was replaced
        STA restart
.goto_done
        RTS
.goto_direct
        JSR runprogram_keepvars
        RTS
.goto_missing
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
.goto_syntax
        JMP syntax

; Consume ordinary spaces and leave the next nonspace byte in A.
spaces  LDY pos
        LDA input,Y
        CMP #32
        BNE .spacedone
        INC pos
        JMP spaces
.spacedone
        RTS

; Parse a numeric assignment entirely on the emulated CPU.
; On entry pos names the candidate; carry is set only when '=' was found.
assignstmt
        JSR number_name
        BCC .not_assignment
        LDY number_after
.skip_space
        LDA input,Y
        CMP #32
        BNE .check_equal
        INY
        JMP .skip_space
.check_equal
        CMP #61
        BNE .not_assignment
        INY
        STY pos
        LDA number_first
        STA assign_first
        LDA number_second
        STA assign_second
        LDA number_type
        STA assign_type
        JSR fexpr
        LDA failed
        BNE .done
        LDA assign_type
        BEQ .store
        LDA fac_exp
        CMP #144               ; magnitudes >= 32768 need a range check
        BCC .store
        BNE .overflow
        LDA fac_sign
        BEQ .overflow
        LDA fac_m0
        CMP #$80
        BNE .overflow
        LDA fac_m1
        ORA fac_m2
        ORA fac_m3
        BEQ .store
.overflow
        JSR quantity_error
        JMP .done
.store
        LDA assign_first
        STA number_first
        LDA assign_second
        STA number_second
        LDA assign_type
        STA number_type
        JSR number_store_float
.done   SEC
        RTS
.not_assignment
        CLC
        RTS

; Parse PRINT items, semicolon continuation, and ten-column comma zones.
printstmt
        LDA pos
        CLC
        ADC #5
        STA pos
        LDA #$00
        STA trail
.printitem
        JSR spaces
        CMP #$00
        BEQ .printend
        CMP #58
        BEQ .printend
        JSR string_parse
        BCC .maybehex
        JSR string_print
        JMP .itemdone
.maybehex
        CMP #72                 ; HEX$(integer)
        BNE .numericitem
        LDA pos
        CLC
        ADC #5
        STA pos
        JSR expr
        LDA failed
        BNE .printdone
        JSR spaces
        CMP #41
        BEQ .hexclose
        JMP syntax
.hexclose
        INC pos
        LDA valhi
        JSR hexbyte
        LDA vallo
        JSR hexbyte
        JMP .itemdone
.numericitem
        JSR fexpr
        LDA failed
        BNE .printdone
        JSR float_print
.itemdone
        LDA #$00
        STA trail
        JSR spaces
        CMP #59                 ; ';' suppresses newline
        BNE .checkcomma
        INC pos
        LDA #$01
        STA trail
        JMP .printitem
.checkcomma
        CMP #44                 ; ',' moves to next print zone
        BNE .printend
        INC pos
        JSR comma
        LDA #$01
        STA trail
        JMP .printitem
.printend
        LDA trail
        BNE .printdone
        JSR newline
.printdone
        RTS

; Advance to the next ten-column print zone, wrapping near the right edge.
comma   LDA column
        CMP #30
        BCC .commacount
        JSR newline
        RTS
.commacount
        SEC
.commarem
        SBC #10
        BCS .commarem
        CLC
        ADC #10                 ; remainder in 0..9
        STA count
        LDA #10
        SEC
        SBC count
        TAX
.commalp LDA #32
        JSR putchar
        DEX
        BNE .commalp
        RTS

; Print four hex digits, high byte first. A is an unsigned byte.
hexbyte PHA
        LSR
        LSR
        LSR
        LSR
        JSR .hexdigit
        PLA
        AND #$0f
.hexdigit
        CMP #10
        BCC .hexnumber
        CLC
        ADC #55                 ; 10 becomes 'A'
        JMP putchar
.hexnumber
        CLC
        ADC #48
        JMP putchar

; Format a signed 16-bit integer from vallo:valhi in decimal.
decimal LDA valhi
        BPL .positive
        LDA #45
        JSR putchar
        SEC
        LDA #0
        SBC vallo
        STA vallo
        LDA #0
        SBC valhi
        STA valhi
        JMP .magnitude
.positive
        LDA #32
        JSR putchar
.magnitude
        JSR printnum
        LDA #32
        JMP putchar
printnum
        LDA #$00
        STA started
        LDX #$00
.decmore LDA #$00
        STA digit
.decsub  LDA valhi
        CMP powershi,X
        BCC .decemit
        BNE .decsubtract
        LDA vallo
        CMP powerslo,X
        BCC .decemit
.decsubtract
        LDA vallo
        SEC
        SBC powerslo,X
        STA vallo
        LDA valhi
        SBC powershi,X
        STA valhi
        INC digit
        JMP .decsub
.decemit LDA digit
        BNE .decchar
        LDA started
        BNE .decchar
        CPX #4
        BNE .decnext
.decchar LDA #1
        STA started
        LDA digit
        CLC
        ADC #48
        JSR float_putchar
.decnext INX
        CPX #5
        BNE .decmore
        RTS

; Parse one FOR variable, floating start, bound, and optional STEP.
; The active loop's name and FAC values live in RAM after the bank readers.
loop_first EQU $0754
loop_second EQU $0755
loop_type EQU $0756
loop_limit EQU $0758
loop_step EQU $075e
forstmt LDA pos
        CLC
        ADC #3
        STA pos
        JSR spaces
        JSR number_name
        BCS .name
        JMP syntax
.name   LDA number_first
        STA loop_first
        SEC
        SBC #65
        ASL
        STA forvar
        LDA number_second
        STA loop_second
        LDA number_type
        STA loop_type
        LDA number_after
        STA pos
        JSR spaces
        CMP #61                 ; '='
        BEQ .forequal
        JMP syntax
.forequal
        INC pos
        JSR fexpr
        LDA failed
        BNE .fordone
        JSR number_store_float
        JSR spaces
        CMP #84                 ; TO
        BEQ .forto
        JMP syntax
.forto
        INC pos
        INC pos
        JSR fexpr
        LDA failed
        BNE .fordone
        LDX #5
.limit  LDA fac_exp,X
        STA loop_limit,X
        DEX
        BPL .limit
        JSR spaces
        CMP #83                 ; STEP
        BNE .unitstep
        LDY pos
        INY
        LDA input,Y
        CMP #84
        BNE .unitstep
        INY
        LDA input,Y
        CMP #69
        BNE .unitstep
        INY
        LDA input,Y
        CMP #80
        BNE .unitstep
        LDA pos
        CLC
        ADC #4
        STA pos
        JSR fexpr
        JMP .save_step
.unitstep
        LDA #1
        STA fp_intlo
        LDA #0
        STA fp_inthi
        JSR $d906
.save_step
        LDX #5
.step   LDA fac_exp,X
        STA loop_step,X
        DEX
        BPL .step
        LDA pos
        STA looppos
        LDA runmode
        BEQ .fordone
        LDA runlo
        STA looplo
        LDA runhi
        STA loophi
.fordone
        RTS

; Advance the active floating FOR variable and compare it to the bound.
nextstmt
        LDA pos
        CLC
        ADC #4
        STA pos
        LDX forvar
        CPX #$ff
        BNE .nextactive
        JMP syntax
.nextactive
        LDA loop_first
        STA number_first
        LDA loop_second
        STA number_second
        LDA loop_type
        STA number_type
        JSR number_load_float
        LDX #5
.step_arg
        LDA loop_step,X
        STA arg_exp,X
        DEX
        BPL .step_arg
        JSR $d900
        JSR number_store_float
        LDX #5
.limit_arg
        LDA loop_limit,X
        STA arg_exp,X
        DEX
        BPL .limit_arg
        JSR $d918
        LDY loop_step+5        ; signed STEP selects the comparison direction
        BMI .descending
        CMP #1
        BNE .loopagain
        JMP .loopdone
.descending
        CMP #$ff
        BNE .loopagain
.loopdone
        LDA #$ff
        STA forvar
        RTS
.loopagain
        LDA runmode
        BEQ .loopwithin
        LDA runlo
        CMP looplo
        BNE .otherline
        LDA runhi
        CMP loophi
        BEQ .loopwithin
.otherline
        LDA #1
        STA restart
        RTS
.loopwithin
        LDA looppos
        STA pos
        RTS

; Parse signed addition/subtraction after multiplication. Preserve the outer
; expression's temporaries so functions and parentheses can evaluate nested
; expressions without corrupting the caller.
expr    LDA sumlo
        PHA
        LDA sumhi
        PHA
        LDA termlo
        PHA
        LDA termhi
        PHA
        JSR term
        LDA vallo
        STA sumlo
        LDA valhi
        STA sumhi
.exprmore
        JSR spaces
        CMP #43                 ; '+'
        BEQ .add
        CMP #45                 ; '-'
        BEQ .subtract
        JMP .exprdone
.add
        INC pos
        JSR term
        CLC
        LDA sumlo
        ADC vallo
        STA sumlo
        LDA sumhi
        ADC valhi
        STA sumhi
        JMP .exprmore
.subtract
        INC pos
        JSR term
        SEC
        LDA sumlo
        SBC vallo
        STA sumlo
        LDA sumhi
        SBC valhi
        STA sumhi
        JMP .exprmore
.exprdone
        LDA sumlo
        STA vallo
        LDA sumhi
        STA valhi
        PLA
        STA termhi
        PLA
        STA termlo
        PLA
        STA sumhi
        PLA
        STA sumlo
        RTS

term    JSR atom
        LDA vallo
        STA termlo
        LDA valhi
        STA termhi
.termmore
        JSR spaces
        CMP #42                 ; '*'
        BEQ .multiply
        CMP #47                 ; '/'
        BEQ .divide
        JMP .termdone
.multiply
        INC pos
        JSR atom
        JSR multiply
        JMP .termmore
.divide
        INC pos
        JSR atom
        JSR divide
        LDA failed
        BEQ .termmore
        RTS
.termdone
        LDA termlo
        STA vallo
        LDA termhi
        STA valhi
        RTS

; Multiply two 16-bit unsigned values, retaining the low 16 bits.
multiply
        LDA termlo
        STA mullo
        LDA termhi
        STA mulhi
        LDA #$00
        STA tmplo
        STA tmphi
        LDA #16
        STA count
.mulstep LDA vallo
        AND #1
        BEQ .mulshift
        CLC
        LDA tmplo
        ADC mullo
        STA tmplo
        LDA tmphi
        ADC mulhi
        STA tmphi
.mulshift
        LSR valhi
        ROR vallo
        ASL mullo
        ROL mulhi
        DEC count
        BNE .mulstep
        LDA tmplo
        STA termlo
        LDA tmphi
        STA termhi
        RTS

; Divide a signed 16-bit term by the next atom, truncating toward zero.
; A zero divisor emits a BASIC error; the quotient stays in termlo:termhi.
divide
        LDA vallo
        ORA valhi
        BNE .nonzero
        JMP division_zero_error
.nonzero
        LDA termhi
        EOR valhi
        AND #$80
        STA op
        LDA termhi
        BPL .divisor
        SEC
        LDA #0
        SBC termlo
        STA termlo
        LDA #0
        SBC termhi
        STA termhi
.divisor
        LDA valhi
        BPL .init
        SEC
        LDA #0
        SBC vallo
        STA vallo
        LDA #0
        SBC valhi
        STA valhi
.init   LDA #0
        STA tmplo
        STA tmphi
        LDA #16
        STA count
.bit    ASL termlo
        ROL termhi
        ROL tmplo
        ROL tmphi
        LDA tmphi
        CMP valhi
        BCC .next
        BNE .subtract
        LDA tmplo
        CMP vallo
        BCC .next
.subtract
        SEC
        LDA tmplo
        SBC vallo
        STA tmplo
        LDA tmphi
        SBC valhi
        STA tmphi
        INC termlo
.next   DEC count
        BNE .bit
        LDA op
        BEQ .done
        SEC
        LDA #0
        SBC termlo
        STA termlo
        LDA #0
        SBC termhi
        STA termhi
.done   RTS

; Parse a decimal literal, one-letter variable, or DEC("hex") function.
atom    JSR spaces
        CMP #45                 ; unary minus
        BNE .check_plus
        JMP .negative
.check_plus
        CMP #43                 ; unary plus
        BNE .check_digit
        JMP .positive
.check_digit
        CMP #48
        BCS .upper_digit
        JMP .atomletter
.upper_digit
        CMP #58
        BCC .digit_start
        JMP .atomletter
.digit_start
        LDA #$00
        STA vallo
        STA valhi
.digitloop
        LDY pos
        LDA input,Y
        CMP #48
        BCC .digitfinish
        CMP #58
        BCS .digitfinish
        SEC
        SBC #48
        STA digit
        LDA vallo
        STA tmplo
        LDA valhi
        STA tmphi
        ASL vallo             ; old value * 2
        ROL valhi
        ASL tmplo             ; old value * 8
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
        BPL .positive_literal
        LDA #1
        STA num_overflow
.positive_literal
        CLC
        LDA vallo
        ADC digit
        STA vallo
        BCC .digitadded
        INC valhi
.digitadded
        LDA valhi
        BPL .digit_in_range
        LDA #1
        STA num_overflow
.digit_in_range
        INC pos
        JMP .digitloop
.digitfinish
        RTS
.negative
        INC pos
        JSR atom
        LDA valhi
        CMP #$80
        BNE .negate
        LDA vallo
        BNE .negate
        LDA #0
        STA num_overflow
.negate
        SEC
        LDA #0
        SBC vallo
        STA vallo
        LDA #0
        SBC valhi
        STA valhi
        RTS
.positive
        INC pos
        JMP atom
.atomletter
        CMP #40                 ; parenthesized numeric expression
        BEQ .parenthesized
        JSR numeric_function
        BCC .legacy_atom
        RTS
.parenthesized
        INC pos
        JSR expr
        JSR spaces
        CMP #41
        BEQ .close_paren
        JMP syntax
.close_paren
        INC pos
        RTS
.legacy_atom
        JSR spaces
        CMP #68                 ; DEC("hex")
        BNE .variable
        LDY pos
        INY
        LDA input,Y
        CMP #69
        BNE .variable
        INY
        LDA input,Y
        CMP #67
        BNE .variable
        INY
        LDA input,Y
        CMP #40
        BNE .variable
        LDA pos
        CLC
        ADC #4
        STA pos
        JSR spaces
        CMP #34
        BEQ .decopen
        JMP syntax
.decopen
        INC pos
        LDA #$00
        STA vallo
        STA valhi
.decloop LDY pos
        LDA input,Y
        CMP #34
        BEQ .decquote
        CMP #58
        BCC .decdigit
        SEC
        SBC #55                 ; A-F to 10-15
        JMP .decvalue
.decdigit
        SEC
        SBC #48
.decvalue
        STA digit
        ASL vallo
        ROL valhi
        ASL vallo
        ROL valhi
        ASL vallo
        ROL valhi
        ASL vallo
        ROL valhi
        CLC
        LDA vallo
        ADC digit
        STA vallo
        BCC .decnextbyte
        INC valhi
.decnextbyte
        INC pos
        JMP .decloop
.decquote
        INC pos
        JSR spaces
        CMP #41
        BEQ .decclose
        JMP syntax
.decclose
        INC pos
        RTS
.variable
        JSR number_name
        BCS .named
        JMP syntax
.named  LDA number_after
        STA pos
        JSR number_load
.atomdone
        RTS

; Clear the packed program and numeric variables when the machine resets.
initprogram
        ; Copy position-independent readers below the program area. They must
        ; execute in RAM while $FF3F hides both ROMs, then re-enable ROM before
        ; returning to this interpreter. A direct ROM-resident read cannot do
        ; that because the next instruction would disappear with the ROM.
        LDX #rampeek_code_end-rampeek_code_ptr-1
.copyreader
        LDA rampeek_code_ptr,X
        STA rampeek_ptr,X
        DEX
        BPL .copyreader

        ; Probe RAM mirroring with known bytes; reset discards the old program.
        ; $BFF0 aliases $3FF0 on 16/32 KiB machines; $7FF0 also aliases it
        ; on 16 KiB machines. Writes reach RAM beneath the BASIC ROM.
        LDA #$11
        STA $3ff0
        LDA #$22
        STA $7ff0
        LDA #$33
        STA $bff0
        LDA $3ff0
        CMP #$33
        BNE .ram64
        LDA $7ff0
        CMP #$33
        BEQ .ram16
        LDA #$80
        BNE .setlimit
.ram16  LDA #$40
        BNE .setlimit
.ram64  LDA #$fd
.setlimit
        STA memlimhi
        STA gfxlimit
        LDA #$e8
        STA endlo
        LDA #$00
        STA runmode
        STA scaleflag
        STA shape_valid
        LDA #$0f
        STA endhi
        JMP clearvars
clearvars
        LDX #51
.initvars
        LDA #$00
        STA vars,X
        DEX
        BPL .initvars
        JSR number_clear
        JMP string_clear

; Print the actual number of free bytes up to the detected physical RAM limit.
freeprint
        LDA str_tophi
        CMP gfxlimit
        BCC .use_string_top
        LDA #0
        STA tmplo
        LDA gfxlimit
        BNE .subtract
.use_string_top
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
        JMP printnum

; Find the first record at or after linelo:linehi in ascending line order.
; Return ptr at the record (or end) and found=1 only for an exact match.
findline
        LDA #$e8
        STA ptrlo
        LDA #$00
        STA found
        LDA #$0f
        STA ptrhi
.findmore
        LDA ptrhi
        CMP endhi
        BNE .findrecord
        LDA ptrlo
        CMP endlo
        BEQ .finddone
.findrecord
        LDY #1
        JSR rampeek_ptr
        CMP linehi
        BCC .findadvance
        BNE .finddone
        DEY
        JSR rampeek_ptr
        CMP linelo
        BCC .findadvance
        BNE .finddone
        LDA #1
        STA found
.finddone
        RTS
.findadvance
        JSR advanceptr
        JMP .findmore

; Advance ptr by the size byte at record offset two plus its header.
advanceptr
        LDY #2
        JSR rampeek_ptr
        CLC
        ADC #3
        CLC
        ADC ptrlo
        STA ptrlo
        BCC .advanceend
        INC ptrhi
.advanceend
        RTS

; Store, replace, or delete the line number already parsed from input.
; Number alone deletes a previous definition; the program remains sorted.
storeline
        LDX pos
        LDA #0
        STA reclen
.measure LDA input,X
        BEQ .measured
        INC reclen
        INX
        JMP .measure
.measured
        LDA reclen
        CLC
        ADC #3
        STA recsize
        JSR findline
        LDA reclen
        BEQ .maybe_delete

        ; Reserve space before removing the old definition, so a failed edit
        ; leaves the original line intact.
        CLC
        LDA endlo
        ADC recsize
        STA newlo
        LDA endhi
        ADC #0
        STA newhi
        LDA found
        BEQ .capacity
        LDY #2
        JSR rampeek_ptr
        CLC
        ADC #3
        STA count
        SEC
        LDA newlo
        SBC count
        STA newlo
        LDA newhi
        SBC #0
        STA newhi
.capacity
        LDA newhi
        CMP gfxlimit
        BCC .check_string_heap
        JMP .outofmemory
.check_string_heap
        LDA newhi
        CMP str_tophi
        BCC .room
        BNE .outofmemory
        LDA newlo
        CMP str_toplo
        BCS .outofmemory
.room   LDA found
        BEQ insertline
        JSR deleteline
        JMP insertline
.maybe_delete
        LDA found
        BEQ .stored
        JSR deleteline
.stored  RTS
.outofmemory
        JSR error_to_text
        LDA #memmsg%256
        STA msglo
        LDA #memmsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

; Close the gap occupied by ptr's record, moving following bytes forward.
deleteline
        LDY #2
        JSR rampeek_ptr
        CLC
        ADC #3
        CLC
        ADC ptrlo
        STA srclo
        LDA ptrhi
        ADC #0
        STA srchi
        LDA ptrlo
        STA dstlo
        LDA ptrhi
        STA dsthi
.deletecopy
        LDA srchi
        CMP endhi
        BNE .deletebyte
        LDA srclo
        CMP endlo
        BEQ .deleted
.deletebyte
        LDY #0
        JSR rampeek_src
        STA (dstlo),Y
        INC srclo
        BNE .deletedst
        INC srchi
.deletedst
        INC dstlo
        BNE .deletecopy
        INC dsthi
        JMP .deletecopy
.deleted LDA dstlo
        STA endlo
        LDA dsthi
        STA endhi
        RTS

; Open a gap at ptr by moving later bytes backward from the old end.
insertline
        LDA endlo
        STA srclo
        CLC
        ADC recsize
        STA dstlo
        LDA endhi
        STA srchi
        ADC #0
        STA dsthi
.insertmove
        LDA srchi
        CMP ptrhi
        BNE .insertbyte
        LDA srclo
        CMP ptrlo
        BEQ .insertheader
.insertbyte
        LDA srclo
        BNE .insertsrcdec
        DEC srchi
.insertsrcdec
        DEC srclo
        LDA dstlo
        BNE .insertdstdec
        DEC dsthi
.insertdstdec
        DEC dstlo
        LDY #0
        JSR rampeek_src
        STA (dstlo),Y
        JMP .insertmove
.insertheader
        LDA newlo
        STA endlo
        LDA newhi
        STA endhi
        LDY #0
        LDA linelo
        STA (ptrlo),Y
        INY
        LDA linehi
        STA (ptrlo),Y
        INY
        LDA reclen
        STA (ptrlo),Y
        INY
        LDX pos
.inserttext
        LDA input,X
        BEQ .inserted
        STA (ptrlo),Y
        INX
        INY
        JMP .inserttext
.inserted
        RTS

; List stored source in line-number order using the existing text renderer.
listprogram
        LDA pos
        CLC
        ADC #4
        STA pos
        LDA #$00
        STA minlo
        STA minhi
        LDA #$ff
        STA maxlo
        STA maxhi
        JSR spaces
        CMP #45                 ; LIST -last
        BEQ .listhyphen
        CMP #48
        BCC .liststart
        CMP #58
        BCS .liststart
        JSR atom
        LDA vallo
        STA minlo
        STA maxlo
        LDA valhi
        STA minhi
        STA maxhi
        JSR spaces
        CMP #45                 ; LIST first-last or first-
        BNE .liststart
        LDA #$ff
        STA maxlo
        STA maxhi
.listhyphen
        INC pos
        JSR spaces
        CMP #48
        BCC .liststart
        CMP #58
        BCS .liststart
        JSR atom
        LDA vallo
        STA maxlo
        LDA valhi
        STA maxhi
.liststart
        LDA #$e8
        STA ptrlo
        LDA #$0f
        STA ptrhi
.listmore
        LDA ptrhi
        CMP endhi
        BNE .listrecord
        LDA ptrlo
        CMP endlo
        BEQ .listdone
.listrecord
        LDY #1
        JSR rampeek_ptr
        CMP minhi
        BCC .listskip
        BNE .listupper
        DEY
        JSR rampeek_ptr
        CMP minlo
        BCC .listskip
.listupper
        LDY #1
        JSR rampeek_ptr
        CMP maxhi
        BCC .listprint
        BNE .listdone
        DEY
        JSR rampeek_ptr
        CMP maxlo
        BCC .listprint
        BNE .listdone
.listprint
        LDY #0
        JSR rampeek_ptr
        STA vallo
        INY
        JSR rampeek_ptr
        STA valhi
        JSR printnum
        LDA #32
        JSR putchar
        LDA #3
        STA count
.listtext
        LDA count
        SEC
        SBC #3
        STA digit
        LDY #2
        JSR rampeek_ptr
        CMP digit
        BEQ .listendline
        LDY count
        JSR rampeek_ptr
        JSR putchar
        INC count
        LDA count
        JMP .listtext
.listendline
        JSR newline
.listskip
        JSR advanceptr
        JMP .listmore
.listdone
        RTS

; Execute each stored line through the same parser used by direct mode.
runprogram
        JSR clearvars
runprogram_keepvars
        LDA #1
        STA runmode
        LDA ptrlo
        STA runlo
        LDA ptrhi
        STA runhi
        LDA #0
        STA resume
        STA restart
        LDA #$ff
        STA forvar
.runnext
        LDA runhi
        CMP endhi
        BNE .runload
        LDA runlo
        CMP endlo
        BEQ .rundone
.runload
        LDA runlo
        STA ptrlo
        LDA runhi
        STA ptrhi
        LDY #2
        JSR rampeek_ptr
        STA count
        LDX #0
        LDY #3
.runcopy
        CPX count
        BEQ .runcopied
        JSR rampeek_ptr
        STA input,X
        INX
        INY
        JMP .runcopy
.runcopied
        LDA #0
        STA input,X
        LDA resume
        STA pos
        LDA #0
        STA resume
        JSR execute
        LDA failed
        BNE .rundone
        LDA restart
        BEQ .runforward
        CMP #2
        BEQ .runjump
        LDA #0
        STA restart
        LDA looplo
        STA runlo
        LDA loophi
        STA runhi
        LDA looppos
        STA resume
        JMP .runnext
.runjump
        LDA #0
        STA restart
        JMP .runnext
.runforward
        LDA runlo
        STA ptrlo
        LDA runhi
        STA ptrhi
        JSR advanceptr
        LDA ptrlo
        STA runlo
        LDA ptrhi
        STA runhi
        JMP .runnext
.rundone
        LDA #0
        STA runmode
        RTS

; Relocatable machine code copied to $0700 at reset. Every instruction must
; remain valid at that RAM address; no ROM call or absolute self-reference is
; allowed while the RAM bank is visible. Both routines preserve X and Y, and
; return the addressed RAM byte in A with its zero/negative flags intact.
rampeek_code_ptr
        SEI                     ; IRQ vector is in ROM: keep this switch atomic
        STA $ff3f               ; show RAM beneath both ROM overlays
        LDA (ptrlo),Y           ; read program record through ptrlo:ptrhi
        STA $ff3e               ; show ROM before RTS returns to BASIC
        CLI
        RTS
rampeek_code_src
        SEI
        STA $ff3f
        LDA (srclo),Y           ; read source byte during line compaction
        STA $ff3e
        CLI
        RTS
rampeek_code_end

        INCLUDE "variables.asm"
        INCLUDE "strings.asm"
        INCLUDE "numbers.asm"
        INCLUDE "float_parser.asm"
        INCLUDE "extensions.asm"
        INCLUDE "shapes.asm"

errmsg  BYT "?SYNTAX ERROR",0
typeerr BYT "?TYPE MISMATCH ERROR",0
quantmsg BYT "?ILLEGAL QUANTITY ERROR",0
zeromsg BYT "?DIVISION BY ZERO ERROR",0
longmsg BYT "?STRING TOO LONG ERROR",0
memmsg  BYT "?OUT OF MEMORY",0
undefmsg BYT "?UNDEF'D STATEMENT",0
breakmsg BYT "?BREAK",0
powerslo BYT $10,$e8,$64,$0a,$01 ; 10000,1000,100,10,1
powershi BYT $27,$03,$00,$00,$00

        ORG $BFFF
        BYT 0
        END
