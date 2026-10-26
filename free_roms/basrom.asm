; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Independently written BASIC subset with a packed numbered-program area.
; Program RAM is $1200-$3fff, a safe common range on 16/32/64 KiB machines.

        ASS $8000

api_line JMP entry             ; $8000: submit the input buffer
api_init JMP initprogram       ; $8003: clear program/variables on reset
api_free JMP freeprint         ; $8006: print measured available bytes

input   EQU $1000               ; zero-terminated line from sysrom
vars    EQU $1100               ; 26 little-endian integer variables
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
        CMP #80                 ; PRINT
        BNE .notprint
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
        BNE syntax
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
        LDA #0
        STA ptrlo
        LDA #$12
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
        BNE syntax
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
        BNE .unknown
        LDY pos
        INY
        LDA input,Y
        CMP #79                 ; O
        BNE .unknown
        INY
        LDA input,Y
        CMP #84                 ; T
        BNE .unknown
        INY
        LDA input,Y
        CMP #79                 ; O
        BNE .unknown
        JSR gotostmt
        RTS
.unknown JMP syntax
syntax  LDA #$01
        STA failed
        LDA #errmsg%256
        STA msglo
        LDA #errmsg/256
        STA msghi
        JSR printz
        JSR newline
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
        CMP #34                 ; quoted string
        BNE .maybehex
        INC pos
.stringlp
        LDY pos
        LDA input,Y
        BEQ .printend
        CMP #34
        BEQ .stringend
        JSR putchar
        INC pos
        JMP .stringlp
.stringend
        INC pos
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
        JSR expr
        LDA failed
        BNE .printdone
        JSR decimal
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

; Format an unsigned 16-bit integer from vallo:valhi in decimal.
decimal LDA #32
        JSR putchar
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
        JSR putchar
.decnext INX
        CPX #5
        BNE .decmore
        RTS

; Parse one direct-mode FOR variable, starting integer, and upper bound.
forstmt LDA pos
        CLC
        ADC #3
        STA pos
        JSR spaces
        SEC
        SBC #65
        ASL
        STA forvar
        INC pos
        JSR spaces
        CMP #61                 ; '='
        BEQ .forequal
        JMP syntax
.forequal
        INC pos
        JSR expr
        LDA failed
        BNE .fordone
        LDX forvar
        LDA vallo
        STA vars,X
        LDA valhi
        STA vars+1,X
        JSR spaces
        CMP #84                 ; TO
        BEQ .forto
        JMP syntax
.forto
        INC pos
        INC pos
        JSR expr
        LDA failed
        BNE .fordone
        LDA vallo
        STA limitlo
        LDA valhi
        STA limithi
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

; Increment the active FOR variable and repeat from its body until done.
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
        INC vars,X
        BNE .compareloop
        INC vars+1,X
.compareloop
        LDA vars+1,X
        CMP limithi
        BCC .loopagain
        BNE .loopdone
        LDA vars,X
        CMP limitlo
        BCC .loopagain
        BEQ .loopagain
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

; Parse additive and multiplicative expressions with BASIC precedence.
expr    JSR term
        LDA vallo
        STA sumlo
        LDA valhi
        STA sumhi
.exprmore
        JSR spaces
        CMP #43                 ; '+'
        BNE .exprdone
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
.exprdone
        LDA sumlo
        STA vallo
        LDA sumhi
        STA valhi
        RTS

term    JSR atom
        LDA vallo
        STA termlo
        LDA valhi
        STA termhi
.termmore
        JSR spaces
        CMP #42                 ; '*'
        BNE .termdone
        INC pos
        JSR atom
        JSR multiply
        JMP .termmore
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

; Parse a decimal literal, one-letter variable, or DEC("hex") function.
atom    JSR spaces
        CMP #48
        BCC .atomletter
        CMP #58
        BCS .atomletter
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
        CLC
        LDA vallo
        ADC digit
        STA vallo
        BCC .digitadded
        INC valhi
.digitadded
        INC pos
        JMP .digitloop
.digitfinish
        RTS
.atomletter
        CMP #68                 ; DEC("hex")
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
        SEC
        SBC #65
        ASL
        TAX
        LDA vars,X
        STA vallo
        LDA vars+1,X
        STA valhi
        INC pos
.atomdone
        RTS

; Clear the packed program and numeric variables when the machine resets.
initprogram
        LDA #$00
        STA endlo
        STA runmode
        LDA #$12
        STA endhi
        JMP clearvars
clearvars
        LDX #51
.initvars
        LDA #$00
        STA vars,X
        DEX
        BPL .initvars
        RTS

; Print the actual number of free bytes from program end to $4000.
freeprint
        SEC
        LDA #$00
        SBC endlo
        STA vallo
        LDA #$40
        SBC endhi
        STA valhi
        JMP printnum

; Find the first record at or after linelo:linehi in ascending line order.
; Return ptr at the record (or end) and found=1 only for an exact match.
findline
        LDA #$00
        STA ptrlo
        STA found
        LDA #$12
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
        LDA (ptrlo),Y
        CMP linehi
        BCC .findadvance
        BNE .finddone
        DEY
        LDA (ptrlo),Y
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
        LDA (ptrlo),Y
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
        LDA (ptrlo),Y
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
        CMP #$40
        BCC .room
        BNE .outofmemory
        LDA newlo
        BNE .outofmemory
.room    LDA found
        BEQ insertline
        JSR deleteline
        JMP insertline
.maybe_delete
        LDA found
        BEQ .stored
        JSR deleteline
.stored  RTS
.outofmemory
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
        LDA (ptrlo),Y
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
        LDA (srclo),Y
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
        LDA (srclo),Y
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
        LDA #$00
        STA ptrlo
        LDA #$12
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
        LDA (ptrlo),Y
        CMP minhi
        BCC .listskip
        BNE .listupper
        DEY
        LDA (ptrlo),Y
        CMP minlo
        BCC .listskip
.listupper
        LDY #1
        LDA (ptrlo),Y
        CMP maxhi
        BCC .listprint
        BNE .listdone
        DEY
        LDA (ptrlo),Y
        CMP maxlo
        BCC .listprint
        BNE .listdone
.listprint
        LDY #0
        LDA (ptrlo),Y
        STA vallo
        INY
        LDA (ptrlo),Y
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
        LDA (ptrlo),Y
        CMP digit
        BEQ .listendline
        LDY count
        LDA (ptrlo),Y
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
        LDA (ptrlo),Y
        STA count
        LDX #0
        LDY #3
.runcopy
        CPX count
        BEQ .runcopied
        LDA (ptrlo),Y
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

errmsg  BYT "?SYNTAX ERROR",0
memmsg  BYT "?OUT OF MEMORY",0
undefmsg BYT "?UNDEF'D STATEMENT",0
breakmsg BYT "?BREAK",0
powerslo BYT $10,$e8,$64,$0a,$01 ; 10000,1000,100,10,1
powershi BYT $27,$03,$00,$00,$00

        ORG $BFFF
        BYT 0
        END
