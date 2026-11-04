; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Expand C16 keyword abbreviations in the ROM before BASIC parses a line.
; A shifted letter is PETSCII $C1..$DA. The preceding ordinary letters and
; that final letter select a keyword. Graphics inside quotes stay untouched.

ab_source EQU $06e9
ab_target EQU $06ea
ab_first EQU $06eb
ab_quote EQU $06ec
ab_marker EQU $06ed
ab_wordptr EQU $70
ab_wordhi EQU $71
ab_buffer EQU $0600

        ORG $E800

; Expand input at $0200 in place, using $0600 as a transient copy. No arguments
; are passed; X/Y and A are clobbered, and the result is zero-terminated.
expand_abbreviations
        LDA #0
        STA ab_source
        STA ab_target
        STA ab_first
        STA ab_quote
.next   LDX ab_source
        LDA input,X
        BEQ .finish
        CMP #34
        BNE .notquote
        LDA ab_quote
        EOR #1
        STA ab_quote
        LDA #34
.notquote
        STA ab_marker
        LDA ab_quote
        BNE .copy
        LDA ab_marker
        CMP #$c1
        BCC .copy
        CMP #$db
        BCS .copy
        JSR ab_tryword
        BCS .advance
.copy   LDY ab_target
        LDA ab_marker
        STA ab_buffer,Y
        INC ab_target
        CMP #65
        BCC .delimiter
        CMP #91
        BCC .advance
.delimiter
        LDA ab_target
        STA ab_first
.advance
        INC ab_source
        LDA ab_target
        CMP #120
        BCC .next
.finish
        LDY ab_target
        LDA #0
        STA ab_buffer,Y
        LDY #0
.copyback
        LDA ab_buffer,Y
        STA input,Y
        BEQ .done
        INY
        BNE .copyback
.done   RTS

; Match the letters since ab_first plus the shifted final letter against the
; ROM's keyword table. Carry reports a match; the expanded word replaces its
; prefix in ab_buffer. X/Y and the table pointer are scratch.
ab_tryword
        LDA #ab_words%256
        STA ab_wordptr
        LDA #ab_words/256
        STA ab_wordhi
.candidate
        LDY #0
        LDA (ab_wordptr),Y
        BEQ .no_match
        LDX ab_first
        CPX ab_target
        BEQ .skip_word
.compare
        LDA ab_buffer,X
        CMP (ab_wordptr),Y
        BNE .skip_word
        INX
        INY
        CPX ab_target
        BNE .compare
        LDA ab_marker
        AND #$7f
        CMP (ab_wordptr),Y
        BNE .skip_word
        LDY #0
.length
        LDA (ab_wordptr),Y
        BEQ .size_known
        INY
        BNE .length
.size_known
        TYA
        CLC
        ADC ab_first
        CMP #120                ; transient buffer ends before $0680
        BCS .no_match
        LDY #0
        LDX ab_first
.replace
        LDA (ab_wordptr),Y
        BEQ .matched
        STA ab_buffer,X
        INX
        INY
        BNE .replace
.matched
        STX ab_target
        STX ab_first
        SEC
        RTS
.skip_word
        LDY #0
.skip_byte
        LDA (ab_wordptr),Y
        BEQ .after_word
        INY
        BNE .skip_byte
.after_word
        INY
        TYA
        CLC
        ADC ab_wordptr
        STA ab_wordptr
        BCC .candidate
        INC ab_wordhi
        JMP .candidate
.no_match
        CLC
        RTS

; Spelled BASIC V3.5 token words in token order. The order matters: for
; example P then Shift-R selects PRINT# before PRINT, as on the original ROM.
ab_words
        BYT "END",0,"FOR",0,"NEXT",0,"DATA",0,"INPUT#",0
        BYT "INPUT",0,"DIM",0,"READ",0,"LET",0,"GOTO",0
        BYT "RUN",0,"IF",0,"RESTORE",0,"GOSUB",0,"RETURN",0
        BYT "REM",0,"STOP",0,"ON",0,"WAIT",0,"LOAD",0
        BYT "SAVE",0,"VERIFY",0,"DEF",0,"POKE",0,"PRINT#",0
        BYT "PRINT",0,"CONT",0,"LIST",0,"CLR",0,"CMD",0
        BYT "SYS",0,"OPEN",0,"CLOSE",0,"GET",0,"NEW",0
        BYT "TAB",0,"TO",0,"FN",0,"SPC",0,"THEN",0
        BYT "NOT",0,"STEP",0,"AND",0,"OR",0,"SGN",0
        BYT "INT",0,"ABS",0,"USR",0,"FRE",0,"POS",0
        BYT "SQR",0,"RND",0,"LOG",0,"EXP",0,"COS",0
        BYT "SIN",0,"TAN",0,"ATN",0,"PEEK",0,"LEN",0
        BYT "STR$",0,"VAL",0,"ASC",0,"CHR$",0,"LEFT$",0
        BYT "RIGHT$",0,"MID$",0,"GO",0,"RGR",0,"RCLR",0
        BYT "RLUM",0,"JOY",0,"RDOT",0,"DEC",0,"HEX$",0
        BYT "ERR$",0,"INSTR",0,"ELSE",0,"RESUME",0,"TRAP",0
        BYT "TRON",0,"TROFF",0,"SOUND",0,"VOL",0,"AUTO",0
        BYT "PUDEF",0,"GRAPHIC",0,"PAINT",0,"CHAR",0,"BOX",0
        BYT "CIRCLE",0,"GSHAPE",0,"SSHAPE",0,"DRAW",0
        BYT "LOCATE",0,"COLOR",0,"SCNCLR",0,"SCALE",0
        BYT "HELP",0,"DO",0,"LOOP",0,"EXIT",0,"DIRECTORY",0
        BYT "DSAVE",0,"DLOAD",0,"HEADER",0,"SCRATCH",0
        BYT "COLLECT",0,"COPY",0,"RENAME",0,"BACKUP",0
        BYT "DELETE",0,"RENUMBER",0,"KEY",0,"MONITOR",0
        BYT "USING",0,"UNTIL",0,"WHILE",0,0
ab_end
