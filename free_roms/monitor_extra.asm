; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Additional TEDMON commands. Included after monitor.asm in the system ROM.

mon_pattern EQU $06c0
mon_patlen EQU $06c8
mon_scanlo EQU $74
mon_scanhi EQU $75
mon_tablelo EQU $76
mon_tablehi EQU $77
mon_name0 EQU $06d6
mon_name1 EQU $06d7
mon_name2 EQU $06d8
mon_mode EQU $06d9
mon_opcode EQU $06da
mon_arglo EQU $06db
mon_arghi EQU $06dc
mon_count EQU $06dd
mon_file_endlo EQU $06de
mon_file_endhi EQU $06df
mon_rangeflag EQU $06e7

; Parse an optional inclusive end after the first M/D address. The first
; address remains in mon_lo:mon_hi; a present end is stored separately.
mon_optional_end
        LDA mon_lo
        STA mon_tmp
        LDA mon_hi
        STA mon_starthi
        JSR mon_address
        LDA #0
        ROL
        STA mon_rangeflag
        LDA mon_lo
        STA mon_file_endlo
        LDA mon_hi
        STA mon_file_endhi
        LDA mon_tmp
        STA mon_lo
        LDA mon_starthi
        STA mon_hi
        RTS

; Return carry set if the current address is past the optional inclusive end.
mon_after_end
        LDA mon_rangeflag
        BEQ .more
        LDA mon_hi
        CMP mon_file_endhi
        BCC .more
        BNE .past
        LDA mon_lo
        CMP mon_file_endlo
        BCC .more
        BEQ .more
.past   SEC
        RTS
.more   CLC
        RTS

; M [start [end]]: show eight bytes per row, continuing on a later bare M.
; Explicit end addresses stop the dump once that address has been shown.
mon_dump_command
        JSR mon_maybe_address
        JSR mon_optional_end
        LDA #8
        STA mon_count
.row   JSR mon_dump
        JSR mon_after_end
        BCS .done
        DEC mon_count
        BNE .row
.done   JMP mon_prompt

; H start end byte... or H start end 'text: find each matching address.
; No arguments are passed besides input/mon_pos. A/X/Y and monitor scratch
; bytes are clobbered; the monitor prompt is printed on return.
mon_hunt
        JSR mon_range
        BCS .range_ok
        JMP mon_error
.range_ok
        LDA #0
        STA mon_patlen
        JSR mon_spaces
        CMP #39
        BNE .hex_pattern
        INC mon_pos
.text_byte
        LDY mon_pos
        LDA input,Y
        BEQ .search
        LDX mon_patlen
        CPX #8
        BCS .search
        STA mon_pattern,X
        INC mon_patlen
        INC mon_pos
        JMP .text_byte
.hex_pattern
        JSR mon_byte
        BCC .search
        LDX mon_patlen
        CPX #8
        BCS .search
        LDA mon_value
        STA mon_pattern,X
        INC mon_patlen
        JMP .hex_pattern
.search
        LDA mon_patlen
        BNE .search_next
        JMP mon_error
.search_next
        LDA mon_lo
        STA mon_scanlo
        LDA mon_hi
        STA mon_scanhi
        LDX #0
.compare
        LDY #0
        LDA (mon_scanlo),Y
        CMP mon_pattern,X
        BNE .advance
        INX
        CPX mon_patlen
        BEQ .found
        INC mon_scanlo
        BNE .compare
        INC mon_scanhi
        JMP .compare
.found  JSR mon_print_address
        JSR newline
.advance
        JSR mon_done
        BCS .finish
        JSR mon_next
        JMP .search_next
.finish
        JMP mon_prompt

; Look up the opcode in mon_opcode. Carry is set for a documented instruction,
; and mon_mode receives its addressing mode. The table pointer is scratch.
mon_decode
        LDA #0
        STA mon_mode
        LDA #mon_opcodes%256
        STA mon_tablelo
        LDA #mon_opcodes/256
        STA mon_tablehi
.entry
        LDY #0
        LDA (mon_tablelo),Y
        CMP #$ff
        BEQ .missing
        CMP mon_opcode
        BEQ .found
        JSR mon_table_next
        JMP .entry
.found  INY
        LDA (mon_tablelo),Y
        STA mon_mode
        SEC
        RTS
.missing
        CLC
        RTS

; Move the five-byte opcode-table pointer to the following entry.
mon_table_next
        CLC
        LDA mon_tablelo
        ADC #5
        STA mon_tablelo
        BCC .done
        INC mon_tablehi
.done   RTS

; A/. address mnemonic [operand]: assemble one documented 7501 instruction.
; The result is written into RAM at the supplied address and the next address
; is displayed. Malformed operands return '?' through mon_error.
mon_assemble
        JSR mon_address
        BCS .address_ok
        JMP mon_error
.address_ok
        LDA mon_lo
        STA mon_dstlo
        LDA mon_hi
        STA mon_dsthi
        JSR mon_spaces
        LDY mon_pos
        LDA input,Y
        STA mon_name0
        INY
        LDA input,Y
        STA mon_name1
        INY
        LDA input,Y
        STA mon_name2
        INY
        STY mon_pos
        LDA #0
        STA mon_mode
        STA mon_arglo
        STA mon_arghi
        JSR mon_spaces
        BNE .operand_present
        JMP .find
.operand_present
        CMP #35                 ; # immediate
        BNE .not_immediate
        INC mon_pos
        LDA #1
        STA mon_mode
        JMP .operand
.not_immediate
        CMP #40                 ; parenthesized indirect
        BNE .ordinary
        INC mon_pos
        LDA #11
        STA mon_mode
        JMP .operand
.ordinary
        LDA #2
        STA mon_mode
.operand
        JSR mon_address
        BCS .have_operand
        JMP mon_error
.have_operand
        LDA mon_lo
        STA mon_arglo
        LDA mon_hi
        STA mon_arghi
        LDA mon_mode
        CMP #2
        BNE .check_indirect
        LDA mon_digits
        CMP #3
        BCC .suffix
        LDA #3
        STA mon_mode
        JMP .suffix
.check_indirect
        CMP #11
        BNE .suffix
        JSR mon_spaces
        CMP #44                 ; ($nn,X)
        BNE .close_indirect
        INC mon_pos
        JSR mon_spaces
        CMP #88
        BNE .bad_operand
        INC mon_pos
        JSR mon_spaces
        CMP #41
        BNE .bad_operand
        INC mon_pos
        LDA #8
        STA mon_mode
        JMP .find
.close_indirect
        CMP #41
        BNE .bad_operand
        INC mon_pos
        JSR mon_spaces
        CMP #44                 ; ($nn),Y
        BNE .find
        INC mon_pos
        JSR mon_spaces
        CMP #89
        BNE .bad_operand
        INC mon_pos
        LDA #9
        STA mon_mode
        JMP .find
.suffix
        JSR mon_spaces
        CMP #44
        BNE .find
        INC mon_pos
        JSR mon_spaces
        CMP #88
        BEQ .suffix_x
        CMP #89
        BNE .bad_operand
        INC mon_pos
        LDA mon_mode
        CLC
        ADC #3
        STA mon_mode
        JMP .find
.suffix_x
        INC mon_pos
        LDA mon_mode
        CLC
        ADC #2
        STA mon_mode
        JMP .find
.bad_operand
        JMP mon_error
.find
        LDA #mon_opcodes%256
        STA mon_tablelo
        LDA #mon_opcodes/256
        STA mon_tablehi
.find_entry
        LDY #0
        LDA (mon_tablelo),Y
        CMP #$ff
        BNE .check_entry
        ; Branch mnemonics encode relative targets rather than a byte operand.
        LDA mon_mode
        CMP #3
        BEQ .try_relative
        CMP #4
        BEQ .try_absx
        CMP #5
        BEQ .try_absy
        CMP #2
        BNE .bad_operand
        LDA #3                 ; zero-page spelling can use absolute opcodes
        STA mon_mode
        JMP .find
.try_relative
        LDA #10
        STA mon_mode
        JMP .find
.try_absx
        LDA #6
        STA mon_mode
        JMP .find
.try_absy
        LDA #7
        STA mon_mode
        JMP .find
.check_entry
        INY
        LDA (mon_tablelo),Y
        CMP mon_mode
        BNE .next_entry
        INY
        LDA (mon_tablelo),Y
        CMP mon_name0
        BNE .next_entry
        INY
        LDA (mon_tablelo),Y
        CMP mon_name1
        BNE .next_entry
        INY
        LDA (mon_tablelo),Y
        CMP mon_name2
        BNE .next_entry
        LDY #0
        LDA (mon_tablelo),Y
        STA mon_opcode
        JMP .emit
.next_entry
        JSR mon_table_next
        JMP .find_entry
.emit   LDY #0
        LDA mon_opcode
        STA (mon_dstlo),Y
        LDA mon_mode
        BEQ .shown
        CMP #10
        BNE .normal_operand
        SEC
        LDA mon_arglo
        SBC mon_dstlo
        STA mon_arglo
        LDA mon_arghi
        SBC mon_dsthi
        STA mon_arghi
        SEC
        LDA mon_arglo
        SBC #2                  ; relative to the byte after the operand
        STA mon_arglo
        LDA mon_arghi
        SBC #0
        STA mon_arghi
        LDA mon_arglo
        BMI .negative_branch
        LDA mon_arghi
        BEQ .normal_operand
        JMP mon_error
.negative_branch
        LDA mon_arghi
        CMP #$ff
        BEQ .normal_operand
        JMP mon_error
.normal_operand
        INY
        LDA mon_arglo
        STA (mon_dstlo),Y
        LDA mon_mode
        CMP #3
        BEQ .word_operand
        CMP #6
        BEQ .word_operand
        CMP #7
        BEQ .word_operand
        CMP #11
        BNE .shown
.word_operand
        INY
        LDA mon_arghi
        STA (mon_dstlo),Y
.shown  TYA
        CLC
        ADC mon_dstlo
        ADC #1
        STA mon_lo
        LDA mon_dsthi
        ADC #0
        STA mon_hi
        JSR mon_print_address
        JSR newline
        JMP mon_prompt

; D [start [end]]: disassemble eight instructions, then retain the next PC.
; Unknown bytes are shown as hex data so the display always advances.
mon_disassemble
        JSR mon_maybe_address
        JSR mon_optional_end
        LDA #8
        STA mon_count
.line   JSR mon_print_address
        LDA #32
        JSR putchar
        LDY #0
        JSR mon_read
        STA mon_opcode
        JSR mon_print_byte
        JSR mon_decode
        BCS .known
        LDA #mon_data%256
        STA msglo
        LDA #mon_data/256
        STA msghi
        JSR printz
        LDA mon_opcode
        JSR mon_print_byte
        JMP .line_done
.known  LDA #32
        LDA mon_mode
        BEQ .implied_bytes
        LDA #32
        JSR putchar
        LDY #1
        JSR mon_read
        JSR mon_print_byte
        LDA mon_mode
        CMP #3
        BEQ .word_bytes
        CMP #6
        BEQ .word_bytes
        CMP #7
        BEQ .word_bytes
        CMP #11
        BEQ .word_bytes
        LDX #4
        JMP .pad_bytes
.word_bytes
        LDA #32
        JSR putchar
        LDY #2
        JSR mon_read
        JSR mon_print_byte
        LDX #1
        JMP .pad_bytes
.implied_bytes
        LDX #7
.pad_bytes
        LDA #32
        JSR putchar
        DEX
        BNE .pad_bytes
        LDY #2
        LDA (mon_tablelo),Y
        STA mon_name0
        INY
        LDA (mon_tablelo),Y
        STA mon_name1
        INY
        LDA (mon_tablelo),Y
        STA mon_name2
        LDA mon_name0
        JSR putchar
        LDA mon_name1
        JSR putchar
        LDA mon_name2
        JSR putchar
        LDA mon_mode
        BNE .has_operand
        JMP .line_done
.has_operand
        LDA #32
        JSR putchar
        LDY #1
        JSR mon_read
        STA mon_arglo
        LDA mon_mode
        CMP #10
        BNE .not_relative
        LDA mon_arglo
        STA mon_value
        CLC
        LDA mon_lo
        ADC mon_value
        STA mon_arglo
        LDA mon_hi
        ADC #0
        STA mon_arghi
        LDA mon_value
        BPL .relative_add_two
        DEC mon_arghi
.relative_add_two
        CLC
        LDA mon_arglo
        ADC #2
        STA mon_arglo
        BCS .relative_carry
        JMP .absolute_text
.relative_carry
        INC mon_arghi
        JMP .absolute_text
.not_relative
        CMP #1
        BNE .not_immediate
        LDA #35
        JSR putchar
.not_immediate
        LDA mon_mode
        CMP #8
        BEQ .open_paren
        CMP #9
        BEQ .open_paren
        CMP #11
        BNE .dollar
.open_paren
        LDA #40
        JSR putchar
.dollar
        LDA #36
        JSR putchar
        LDA mon_mode
        CMP #3
        BEQ .word
        CMP #6
        BEQ .word
        CMP #7
        BEQ .word
        CMP #11
        BNE .byte
.word   LDY #2
        JSR mon_read
        JSR mon_print_byte
.byte   LDA mon_arglo
        JSR mon_print_byte
        LDA mon_mode
        CMP #8
        BNE .not_indx
        LDA #44
        JSR putchar
        LDA #88
        JSR putchar
        LDA #41
        JSR putchar
        JMP .line_done
.not_indx
        CMP #9
        BNE .not_indy
        LDA #41
        JSR putchar
        LDA #44
        JSR putchar
        LDA #89
        JSR putchar
        JMP .line_done
.not_indy
        CMP #11
        BNE .not_indirect
        LDA #41
        JSR putchar
        JMP .line_done
.not_indirect
        CMP #4
        BEQ .index_x
        CMP #6
        BEQ .index_x
        CMP #5
        BEQ .index_y
        CMP #7
        BNE .line_done
.index_y
        LDA #89
        BNE .index_suffix
.index_x
        LDA #88
.index_suffix
        PHA
        LDA #44
        JSR putchar
        PLA
        JSR putchar
        JMP .line_done
.absolute_text
        LDA #36
        JSR putchar
        LDA mon_arghi
        JSR mon_print_byte
        LDA mon_arglo
        JSR mon_print_byte
.line_done
        JSR newline
        LDA mon_mode
        CMP #3
        BEQ .three
        CMP #6
        BEQ .three
        CMP #7
        BEQ .three
        CMP #11
        BEQ .three
        CMP #0
        BEQ .one
.two    JSR mon_next
.one   JSR mon_next
        JMP .count
.three JSR mon_next
        JSR mon_next
        JSR mon_next
.count
        JSR mon_after_end
        BCS .done
        DEC mon_count
        BEQ .done
        JMP .line
.done   LDA mon_lo
        STA mon_cursorlo
        LDA mon_hi
        STA mon_cursorhi
        JMP mon_prompt

mon_data BYT " BYT $",0

; Send A as one IEC filename or data byte. The virtual IEC device only
; transports bytes; parsing PRG load addresses remains entirely in this ROM.
mon_iec_send
        PHA
        LDA #$83
        STA $fef0
        PLA
        STA $fef0
        RTS

; Select an IEC secondary address in A, preserving no registers.
mon_iec_secondary
        PHA
        LDA #$82
        STA $fef0
        PLA
        STA $fef0
        RTS

; Request one byte from the virtual IEC device, returning it in A.
mon_iec_read
        LDA #$84
        STA $fef0
        LDA $fef0
        RTS

; Parse "filename",device; begin a new IEC filename phase. Device must be
; hexadecimal 8. Carry reports a complete, valid syntax, with mon_pos at the
; comma or end following the device argument.
mon_filename
        JSR mon_spaces
        CMP #34
        BNE .bad
        INC mon_pos
        LDA #$f0
        JSR mon_iec_secondary
.name   LDY mon_pos
        LDA input,Y
        BEQ .bad
        CMP #34
        BEQ .name_end
        JSR mon_iec_send
        INC mon_pos
        JMP .name
.name_end
        INC mon_pos
        JSR mon_spaces
        CMP #44
        BNE .bad
        INC mon_pos
        JSR mon_address
        BCC .bad
        LDA mon_lo
        CMP #8
        BNE .bad
        LDA mon_hi
        BNE .bad
        SEC
        RTS
.bad    CLC
        RTS

; Consume the comma before a monitor address, then parse that address.
; Carry reports success and mon_lo:mon_hi contains the value.
mon_comma_address
        JSR mon_spaces
        CMP #44
        BNE .bad
        INC mon_pos
        JMP mon_address
.bad    CLC
        RTS

; L "file",8: read a two-byte PRG load address followed by its data.
mon_load
        JSR mon_filename
        BCS .named
        JMP mon_error
.named  LDA #0
        JSR mon_iec_secondary
        LDA $fef1
        BEQ .opened
        JMP mon_error
.opened
        JSR mon_iec_read
        STA mon_lo
        JSR mon_iec_read
        STA mon_hi
        LDA $fef1
        CMP #3
        BEQ .done
.byte   JSR mon_iec_read
        LDY #0
        STA (mon_lo),Y
        LDA $fef1
        CMP #3
        BEQ .done
        JSR mon_next
        JMP .byte
.done   LDA #$e0
        JSR mon_iec_secondary
        JMP mon_prompt

; V "file",8: compare a load-address PRG with current memory. Print the
; address of every differing byte, as TEDMON does for C.
mon_verify
        JSR mon_filename
        BCS .named
        JMP mon_error
.named  LDA #0
        JSR mon_iec_secondary
        LDA $fef1
        BEQ .opened
        JMP mon_error
.opened
        JSR mon_iec_read
        STA mon_lo
        JSR mon_iec_read
        STA mon_hi
        LDA $fef1
        CMP #3
        BEQ .done
.byte   JSR mon_iec_read
        LDY #0
        CMP (mon_lo),Y
        BEQ .same
        JSR mon_print_address
        JSR newline
.same   LDA $fef1
        CMP #3
        BEQ .done
        JSR mon_next
        JMP .byte
.done   LDA #$e0
        JSR mon_iec_secondary
        JMP mon_prompt

; S "file",8,start,end-plus-one: write a standard load-address PRG.
; The final address is exclusive and the close operation commits the file.
mon_save
        JSR mon_filename
        BCS .named
        JMP mon_error
.named  JSR mon_comma_address
        BCS .start_ok
        JMP mon_error
.start_ok
        LDA mon_lo
        STA mon_tmp
        LDA mon_hi
        STA mon_starthi
        JSR mon_comma_address
        BCS .end_ok
        JMP mon_error
.end_ok
        LDA mon_lo
        STA mon_file_endlo
        LDA mon_hi
        STA mon_file_endhi
        LDA mon_tmp
        STA mon_lo
        LDA mon_starthi
        STA mon_hi
        LDA #1
        JSR mon_iec_secondary
        LDA mon_lo
        JSR mon_iec_send
        LDA mon_hi
        JSR mon_iec_send
.byte   LDA mon_lo
        CMP mon_file_endlo
        BNE .send
        LDA mon_hi
        CMP mon_file_endhi
        BEQ .close
.send  LDY #0
        LDA (mon_lo),Y
        JSR mon_iec_send
        JSR mon_next
        JMP .byte
.close LDA #$e0
        JSR mon_iec_secondary
        LDA $fef1
        BEQ .done
        JMP mon_error
.done   JMP mon_prompt
