; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; String values owned entirely by free BASIC. Sixteen descriptors reside in
; low RAM; byte payloads grow downward from the physical RAM limit. The
; existing RAM reader exposes payloads below the ROM overlay to BASIC code.

string_slots EQU $0340         ; 16 entries: first, second, lo, hi, length, used
str_slot EQU $80
str_first EQU $81
str_second EQU $82
str_ptrlo EQU $83
str_ptrhi EQU $84
str_len EQU $85
str_toplo EQU $86
str_tophi EQU $87
str_newlo EQU $88
str_newhi EQU $89
str_index EQU $8a
str_temp EQU $8b
str_target_first EQU $8c
str_target_second EQU $8d
str_destlo EQU $8e
str_desthi EQU $8f
str_savedlo EQU $03e0
str_savedhi EQU $03e1
str_savedlen EQU $03e2
str_start EQU $03e3
str_count EQU $03e4
str_char EQU $03e5
str_hex EQU $03e6
str_mid_limit EQU $03eb
str_mid_count EQU $03ec
str_leftlo EQU $03ed
str_lefthi EQU $03ee
str_leftlen EQU $03ef
str_total EQU $03f0
str_rightlo EQU $03f1
str_righthi EQU $03f2
str_rightlen EQU $03f3
str_concatlo EQU $b0
str_concathi EQU $b1
str_concat_index EQU $b2
str_negative EQU $b3
str_numberbuf EQU $0470        ; temporary decimal text, copied before reuse

; Clear all string descriptors and restore the downward-growing heap.
; Arguments: memlimhi is the exclusive RAM limit. Returns: nothing.
string_clear
        LDA #0
        STA str_toplo
        LDX #95
.clear  STA string_slots,X
        DEX
        BPL .clear
        LDA memlimhi
        STA str_tophi
        RTS

; Detect and execute NAME$=expression at input[pos]. The first two name
; characters are significant. Return carry set if an assignment was handled;
; otherwise leave pos unchanged and return carry clear.
string_assignment
        LDY pos
        LDA input,Y
        CMP #65
        BCC .miss
        CMP #91
        BCS .miss
        STA str_first
        LDA #0
        STA str_second
        INY
.name   LDA input,Y
        CMP #65
        BCC .suffix
        CMP #91
        BCS .suffix
        LDA str_second
        BNE .more_name
        LDA input,Y
        STA str_second
.more_name
        INY
        JMP .name
.suffix
        CMP #36                 ; '$'
        BNE .miss
        INY
.spaces LDA input,Y
        CMP #32
        BNE .equal
        INY
        JMP .spaces
.equal  CMP #61
        BNE .miss
        INY
        STY pos
        LDA str_first
        STA str_target_first
        LDA str_second
        STA str_target_second
        JSR string_parse
        BCS .value
        JSR string_type_error
        SEC
        RTS
.value  LDA str_target_first
        STA str_first
        LDA str_target_second
        STA str_second
        JSR string_find_or_create
        LDA failed
        BNE .handled
        JSR string_store
.handled
        SEC
        RTS
.miss   CLC
        RTS

; MID$(NAME$,start[,count])=text replaces bytes of an existing string.
; Arguments: pos at MID$. Returns: carry set when handled, failed on error.
string_mid_assignment
        LDY pos
        LDA input,Y
        CMP #77                 ; MID$(
        BNE .not_mid
        INY
        LDA input,Y
        CMP #73
        BNE .not_mid
        INY
        LDA input,Y
        CMP #68
        BNE .not_mid
        INY
        LDA input,Y
        CMP #36
        BNE .not_mid
        INY
        LDA input,Y
        CMP #40
        BNE .not_mid
        JMP .open_mid
.not_mid
        CLC
        RTS
.open_mid
        INY
        STY pos
        JSR string_parse
        BCS .target
        JMP .syntax
.target
        LDA str_ptrlo
        STA str_destlo
        LDA str_ptrhi
        STA str_desthi
        LDA str_len
        STA str_mid_limit
        JSR string_numeric_argument
        LDA failed
        BEQ .argument_ok
        JMP .handled
.argument_ok
        LDA valhi
        BNE .illegal
        LDA vallo
        BEQ .illegal
        SEC
        SBC #1
        CMP str_mid_limit
        BCS .illegal
        STA str_start
        LDA str_mid_limit
        SEC
        SBC str_start
        STA str_mid_count
        JSR spaces
        CMP #44
        BNE .close
        INC pos
        JSR expr
        LDA failed
        BNE .handled
        LDA valhi
        BNE .close
        LDA vallo
        CMP str_mid_count
        BCS .close
        STA str_mid_count
.close  JSR string_close
        LDA failed
        BNE .handled
        JSR spaces
        CMP #61
        BNE .syntax
        INC pos
        JSR string_parse
        BCC .syntax
        LDA str_len
        CMP str_mid_count
        BCS .copy
        STA str_mid_count
.copy   CLC
        LDA str_destlo
        ADC str_start
        STA str_destlo
        LDA str_desthi
        ADC #0
        STA str_desthi
        LDA str_ptrlo
        STA srclo
        LDA str_ptrhi
        STA srchi
        LDY #0
.byte   CPY str_mid_count
        BEQ .handled
        JSR rampeek_src
        STA (str_destlo),Y
        INY
        JMP .byte
.illegal
        JSR badargument
        JMP .handled
.syntax
        JSR syntax
.handled
        SEC
        RTS
.miss   CLC
        RTS

; Parse one or more string primaries joined by '+'. All concatenation bytes
; are written by 6502 code to emulated RAM, with a 255-byte result limit.
; Arguments: pos at a string expression. Returns: pointer/length and carry.
string_parse
        JSR string_primary
        BCS .first
        RTS
.first   JSR spaces
        CMP #43
        BNE .done
        LDA str_ptrlo
        PHA
        LDA str_ptrhi
        PHA
        LDA str_len
        PHA
        INC pos
        JSR string_primary
        PLA
        STA str_leftlen
        PLA
        STA str_lefthi
        PLA
        STA str_leftlo
        BCS .right
        JMP syntax
.right  JSR string_concat
        LDA failed
        BNE .done
        JMP .first
.done   SEC
        RTS

; Parse a quoted literal, variable, or ROM string function.
; Arguments: pos at one value. Returns: pointer/length and carry on success.
string_primary
        JSR spaces
        CMP #34
        BEQ .literal
        JSR string_function
        BCS .found_function
        JSR spaces
        CMP #65
        BCC .miss
        CMP #91
        BCS .miss
        STA str_first
        LDA #0
        STA str_second
        LDY pos
        INY
.name   LDA input,Y
        CMP #65
        BCC .suffix
        CMP #91
        BCS .suffix
        LDA str_second
        BNE .more_name
        LDA input,Y
        STA str_second
.more_name
        INY
        JMP .name
.suffix
        CMP #36
        BNE .miss
        INY
        LDA input,Y
        CMP #40                 ; HEX$(...) is numeric PRINT syntax
        BEQ .miss
        STY pos
        JSR string_find
        BCC .empty
        LDX str_slot
        LDA string_slots+2,X
        STA str_ptrlo
        LDA string_slots+3,X
        STA str_ptrhi
        LDA string_slots+4,X
        STA str_len
        SEC
        RTS
.empty  LDA #0
        STA str_len
        STA str_ptrlo
        STA str_ptrhi
        SEC
        RTS
.found_function
        SEC
        RTS
.literal
        INC pos
        LDA pos
        STA str_ptrlo
        LDA #2
        STA str_ptrhi
        LDY pos
.scan   LDA input,Y
        BEQ .miss
        CMP #34
        BEQ .end_literal
        INY
        JMP .scan
.end_literal
        TYA
        SEC
        SBC pos
        STA str_len
        INY
        STY pos
        SEC
        RTS
.miss   CLC
        RTS

; Join saved left bytes to the current right string in a new heap allocation.
; Arguments: str_left* and str_ptr/str_len. Returns: combined source or failed.
string_concat
        LDA str_ptrlo
        STA str_rightlo
        LDA str_ptrhi
        STA str_righthi
        LDA str_len
        STA str_rightlen
        CLC
        ADC str_leftlen
        BCC .length_ok
        JMP string_too_long
.length_ok
        STA str_total
        SEC
        LDA str_toplo
        SBC str_total
        STA str_concatlo
        LDA str_tophi
        SBC #0
        STA str_concathi
        CMP endhi
        BCC .no_room
        BNE .room
        LDA str_concatlo
        CMP endlo
        BCC .no_room
        BEQ .no_room
.room   LDA str_leftlo
        STA srclo
        LDA str_lefthi
        STA srchi
        LDY #0
.left   CPY str_leftlen
        BEQ .right_source
        JSR rampeek_src
        STA (str_concatlo),Y
        INY
        JMP .left
.right_source
        LDA str_rightlo
        STA srclo
        LDA str_righthi
        STA srchi
        LDA str_leftlen
        STA str_concat_index
        LDY #0
.right  CPY str_rightlen
        BEQ .done
        JSR rampeek_src
        STY str_temp
        LDY str_concat_index
        STA (str_concatlo),Y
        INC str_concat_index
        LDY str_temp
        INY
        JMP .right
.done   LDA str_concatlo
        STA str_toplo
        STA str_ptrlo
        LDA str_concathi
        STA str_tophi
        STA str_ptrhi
        LDA str_total
        STA str_len
        RTS
.no_room
        JMP string_out_of_memory

; Recognize ROM string functions. Each helper consumes its own closing ')'
; and leaves a source pointer/length for PRINT, assignment, CHAR, or IEC.
; Arguments: pos starts at a name. Returns: carry set on a match.
string_function
        LDA #kwleft%256
        STA kwlo
        LDA #kwleft/256
        STA kwhi
        JSR keyword
        BCC .right
        JSR string_left
        SEC
        RTS
.right  LDA #kwright%256
        STA kwlo
        LDA #kwright/256
        STA kwhi
        JSR keyword
        BCC .mid
        JSR string_right
        SEC
        RTS
.mid    LDA #kwmid%256
        STA kwlo
        LDA #kwmid/256
        STA kwhi
        JSR keyword
        BCC .chr
        JSR string_mid
        SEC
        RTS
.chr    LDA #kwchr%256
        STA kwlo
        LDA #kwchr/256
        STA kwhi
        JSR keyword
        BCC .hex
        JSR string_chr
        SEC
        RTS
.hex    LDA #kwhex%256
        STA kwlo
        LDA #kwhex/256
        STA kwhi
        JSR keyword
        BCC .str
        JSR string_hex
        SEC
        RTS
.str    LDA #kwstr%256
        STA kwlo
        LDA #kwstr/256
        STA kwhi
        JSR keyword
        BCC .miss
        JSR string_str
        SEC
        RTS
.miss   CLC
        RTS

; Consume '(' and parse one string argument for a string function.
; Arguments: pos after the function name. Returns: parsed string or failed.
string_first_argument
        JSR spaces
        CMP #40
        BEQ .open
        JMP syntax
.open   INC pos
        JSR string_parse
        BCS .parsed
        JMP syntax
.parsed RTS

; Consume comma and evaluate a numeric argument with the ROM expression
; parser. Arguments: pos at a comma. Returns: vallo/valhi or failed.
string_numeric_argument
        JSR spaces
        CMP #44
        BEQ .comma
        JMP syntax
.comma  INC pos
        JMP expr

; Require a function-closing parenthesis and advance pos by one.
; Arguments: pos after the final argument. Returns: failed on bad syntax.
string_close
        JSR spaces
        CMP #41
        BEQ .close
        JMP syntax
.close  INC pos
        RTS

; LEFT$(text,count) retains at most count bytes from the current source.
; Arguments: pos after LEFT$. Returns: source pointer and shortened length.
string_left
        JSR string_first_argument
        LDA failed
        BNE .done
        JSR string_numeric_argument
        LDA failed
        BNE .done
        LDA valhi
        BNE .close
        LDA vallo
        CMP str_len
        BCS .close
        STA str_len
.close  JSR string_close
.done   RTS

; RIGHT$(text,count) advances the source to its rightmost count bytes.
; Arguments: pos after RIGHT$. Returns: adjusted pointer and length.
string_right
        JSR string_first_argument
        LDA failed
        BNE .done
        JSR string_numeric_argument
        LDA failed
        BNE .done
        LDA valhi
        BNE .close
        LDA vallo
        CMP str_len
        BCS .close
        STA str_count
        LDA str_len
        SEC
        SBC str_count
        CLC
        ADC str_ptrlo
        STA str_ptrlo
        LDA str_ptrhi
        ADC #0
        STA str_ptrhi
        LDA str_count
        STA str_len
.close  JSR string_close
.done   RTS

; MID$(text,start[,count]) returns a one-based substring.
; Arguments: pos after MID$. Returns: adjusted pointer and length.
string_mid
        JSR string_first_argument
        LDA failed
        BNE .done
        LDA str_ptrlo
        STA str_savedlo
        LDA str_ptrhi
        STA str_savedhi
        LDA str_len
        STA str_savedlen
        JSR string_numeric_argument
        LDA failed
        BNE .done
        LDA valhi
        BNE .empty
        LDA vallo
        BEQ .empty
        SEC
        SBC #1
        STA str_start
        CMP str_savedlen
        BCS .empty
        LDA str_savedlen
        SEC
        SBC str_start
        STA str_len
        CLC
        LDA str_savedlo
        ADC str_start
        STA str_ptrlo
        LDA str_savedhi
        ADC #0
        STA str_ptrhi
        JMP .optional
.empty  LDA #0
        STA str_len
.optional
        JSR spaces
        CMP #44
        BNE .close
        INC pos
        JSR expr
        LDA failed
        BNE .done
        LDA valhi
        BNE .close
        LDA vallo
        CMP str_len
        BCS .close
        STA str_len
.close  JSR string_close
.done   RTS

; CHR$(byte) yields one byte, including zero, in stable low RAM.
; Arguments: pos after CHR$. Returns: str_char as a one-byte source.
string_chr
        JSR spaces
        CMP #40
        BEQ .open
        JMP syntax
.open   INC pos
        JSR expr
        LDA failed
        BNE .done
        LDA valhi
        BEQ .byte
        JMP badargument
.byte   LDA vallo
        STA str_char
        LDA #str_char%256
        STA str_ptrlo
        LDA #str_char/256
        STA str_ptrhi
        LDA #1
        STA str_len
        JSR string_close
        JSR string_heap_copy
.done   RTS

; HEX$(number) formats a 16-bit number as four uppercase hexadecimal bytes.
; Arguments: pos after HEX$. Returns: a four-byte source in low RAM.
string_hex
        JSR spaces
        CMP #40
        BEQ .open
        JMP syntax
.open   INC pos
        JSR expr
        LDA failed
        BNE .done
        LDA valhi
        LSR
        LSR
        LSR
        LSR
        JSR string_hex_nibble
        STA str_hex
        LDA valhi
        AND #15
        JSR string_hex_nibble
        STA str_hex+1
        LDA vallo
        LSR
        LSR
        LSR
        LSR
        JSR string_hex_nibble
        STA str_hex+2
        LDA vallo
        AND #15
        JSR string_hex_nibble
        STA str_hex+3
        LDA #str_hex%256
        STA str_ptrlo
        LDA #str_hex/256
        STA str_ptrhi
        LDA #4
        STA str_len
        JSR string_close
        JSR string_heap_copy
.done   RTS

; Convert a nibble in A to an uppercase hexadecimal ASCII byte.
; Arguments: A is 0..15. Returns: ASCII byte in A.
string_hex_nibble
        CMP #10
        BCC .number
        CLC
        ADC #55
        RTS
.number CLC
        ADC #48
        RTS

; STR$(number) formats the floating FAC using the same digits as PRINT.
; Arguments: pos after STR$. Returns: a decimal string in temporary RAM.
string_str
        JSR spaces
        CMP #40
        BEQ .open
        JMP syntax
.open   INC pos
        JSR fexpr
        LDA failed
        BNE .done
        JSR string_close
        LDA #str_numberbuf%256
        STA str_ptrlo
        LDA #str_numberbuf/256
        STA str_ptrhi
        LDA #0
        STA str_len
        LDA #1
        STA fprint_mode
        JSR float_print
        LDA #0
        STA fprint_mode
        DEC str_len              ; STR$ omits PRINT's trailing space
        JSR string_heap_copy
.done   RTS

; Copy a generated string into stable heap RAM before another function can
; reuse its temporary bytes. This also supports adjacent STR$, HEX$, CHR$.
; Arguments: str_ptrlo/hi and str_len. Returns: a persistent source pointer.
string_heap_copy
        SEC
        LDA str_toplo
        SBC str_len
        STA str_newlo
        LDA str_tophi
        SBC #0
        STA str_newhi
        CMP endhi
        BCC .no_room
        BNE .room
        LDA str_newlo
        CMP endlo
        BCC .no_room
        BEQ .no_room
.room   LDA str_ptrlo
        STA srclo
        LDA str_ptrhi
        STA srchi
        LDY #0
.byte   CPY str_len
        BEQ .copied
        JSR rampeek_src
        STA (str_newlo),Y
        INY
        JMP .byte
.copied
        LDA str_newlo
        STA str_toplo
        STA str_ptrlo
        LDA str_newhi
        STA str_tophi
        STA str_ptrhi
        RTS
.no_room
        JMP string_out_of_memory

kwleft  BYT "LEFT$",0
kwright BYT "RIGHT$",0
kwmid   BYT "MID$",0
kwchr   BYT "CHR$",0
kwhex   BYT "HEX$",0
kwstr   BYT "STR$",0

; Locate the descriptor for str_first/str_second. Carry says whether found.
; Arguments: two name bytes. Returns: str_slot as a six-byte table offset.
string_find
        LDX #0
.loop   LDA string_slots,X
        CMP str_first
        BNE .next
        LDA string_slots+1,X
        CMP str_second
        BEQ .found
.next   TXA
        CLC
        ADC #6
        TAX
        CPX #96
        BCC .loop
        CLC
        RTS
.found  STX str_slot
        SEC
        RTS

; Find a descriptor or claim the first empty slot. An exhausted descriptor
; table is reported with BASIC's out-of-memory diagnostic.
; Arguments: str_first/str_second. Returns: str_slot or failed set.
string_find_or_create
        JSR string_find
        BCS .done
        LDX #0
.scan   LDA string_slots,X
        BEQ .create
        TXA
        CLC
        ADC #6
        TAX
        CPX #96
        BCC .scan
        JMP string_out_of_memory
.create
        STX str_slot
        LDA str_first
        STA string_slots,X
        LDA str_second
        STA string_slots+1,X
.done   RTS

; Copy a parsed string to newly allocated emulated RAM. The old payload may
; be left behind until a later reset; the program-area bounds guard checks
; the new heap top before storing another numbered line.
; Arguments: str_ptrlo/hi and str_len. Returns: descriptor updated or failed.
string_store
        SEC
        LDA str_toplo
        SBC str_len
        STA str_newlo
        LDA str_tophi
        SBC #0
        STA str_newhi
        CMP endhi
        BCC .no_room
        BNE .room
        LDA str_newlo
        CMP endlo
        BCC .no_room
        BEQ .no_room
.room   LDA str_ptrlo
        STA srclo
        LDA str_ptrhi
        STA srchi
        LDY #0
.copy   CPY str_len
        BEQ .copied
        JSR rampeek_src
        STA (str_newlo),Y
        INY
        JMP .copy
.copied
        LDA str_newlo
        STA str_toplo
        LDA str_newhi
        STA str_tophi
        LDX str_slot
        LDA str_newlo
        STA string_slots+2,X
        LDA str_newhi
        STA string_slots+3,X
        LDA str_len
        STA string_slots+4,X
        RTS
.no_room
        JMP string_out_of_memory

; Print the current string through the system ROM's character writer.
; Arguments: str_ptrlo/hi and str_len. Returns: nothing; pos is preserved.
string_print
        LDA str_ptrlo
        STA srclo
        LDA str_ptrhi
        STA srchi
        LDY #0
.byte   CPY str_len
        BEQ .done
        JSR rampeek_src
        STY str_temp
        JSR putchar
        LDY str_temp
        INY
        JMP .byte
.done   RTS

; Report a failed string allocation using the ROM's standard memory error.
; Arguments: none. Returns: failed set and a visible diagnostic.
string_out_of_memory
        LDA #1
        STA failed
        JSR error_to_text
        LDA #memmsg%256
        STA msglo
        LDA #memmsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

; Report a concatenation longer than the BASIC 255-byte string limit.
; Arguments: none. Returns: failed set and a visible diagnostic.
string_too_long
        LDA #1
        STA failed
        JSR error_to_text
        LDA #longmsg%256
        STA msglo
        LDA #longmsg/256
        STA msghi
        JSR printz
        JSR newline
        RTS

; Report a numeric expression assigned to a string variable.
; Arguments: none. Returns: failed set and a visible diagnostic.
string_type_error
        LDA #1
        STA failed
        JSR error_to_text
        LDA #typeerr%256
        STA msglo
        LDA #typeerr/256
        STA msghi
        JSR printz
        JSR newline
        RTS
