; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Small clean-room TEDMON. Its editor is the system ROM's normal input editor.
; Commands, hexadecimal parsing, memory access and display run on the 7501.

mon_pos EQU $06f0
mon_lo EQU $70
mon_hi EQU $71
mon_endlo EQU $06f3
mon_endhi EQU $06f4
mon_dstlo EQU $72
mon_dsthi EQU $73
mon_value EQU $06f7
mon_digits EQU $06f8
mon_tmp EQU $06f9
mon_pc_lo EQU $06fa
mon_pc_hi EQU $06fb
mon_sr EQU $06fc
mon_ac EQU $06fd
mon_xr EQU $06fe
mon_yr EQU $06ff
mon_sp EQU $06d0
mon_host_sp EQU $07f9
mon_host_retlo EQU $07fa
mon_host_rethi EQU $07fb
mon_starthi EQU $06d1
mon_savedlo EQU $06d2
mon_savedhi EQU $06d3
mon_cursorlo EQU $06d4
mon_cursorhi EQU $06d5

        ORG $EC00

; Enter the monitor from BASIC, set the input owner, and print registers.
; There are no arguments and no result; A/X/Y may change.
monitor_enter
        LDA #1
        STA monitor_active
        LDA #0
        STA mon_pc_lo
        STA mon_pc_hi
        STA mon_sr
        STA mon_ac
        STA mon_xr
        STA mon_yr
        STA mon_cursorlo
        STA mon_cursorhi
        LDX #mon_ramcode_end-mon_ramcode-1
.copy_ram_reader
        LDA mon_ramcode,X
        STA $0720,X
        DEX
        BPL .copy_ram_reader
        TSX
        STX mon_sp
        LDA #mon_banner%256
        STA msglo
        LDA #mon_banner/256
        STA msghi
        JSR printz
        JSR newline
        JSR mon_registers
        JMP mon_prompt

; Execute one zero-terminated monitor line in input. This is called by the
; shared screen editor after Return; it prints a fresh prompt unless X exits.
monitor_process
        LDA #0
        STA mon_pos
        JSR mon_spaces
        BNE .long1
        JMP mon_prompt
.long1
        CMP #88                 ; X: exit to BASIC
        BNE .not_x
        LDA #0
        STA monitor_active
        RTS
.not_x  INC mon_pos
        CMP #82                 ; R: registers
        BNE .not_r
        JSR mon_registers
        JMP mon_prompt
.not_r  CMP #77                 ; M: memory dump
        BNE .not_m
        JMP mon_dump_command
.not_m  CMP #62                 ; >: deposit bytes
        BNE .not_deposit
        JSR mon_address
        BCS .long2
        JMP mon_error
.long2
.deposit
        JSR mon_byte
        BCS .long3
        JMP mon_prompt
.long3
        LDY #0
        LDA mon_value
        STA (mon_lo),Y
        JSR mon_next
        JMP .deposit
.not_deposit
        CMP #70                 ; F: fill inclusive range
        BNE .not_fill
        JSR mon_range
        BCS .long4
        JMP mon_error
.long4
        JSR mon_byte
        BCS .long5
        JMP mon_error
.long5
.fill   LDY #0
        LDA mon_value
        STA (mon_lo),Y
        JSR mon_done
        BCC .long6
        JMP mon_prompt
.long6
        JSR mon_next
        JMP .fill
.not_fill
        CMP #84                 ; T: transfer inclusive range
        BEQ .is_transfer
        JMP .not_transfer
.is_transfer
        JSR mon_range
        BCS .long7
        JMP mon_error
.long7
        JSR mon_address
        BCS .long8
        JMP mon_error
.long8
        LDA mon_lo
        STA mon_dstlo
        LDA mon_hi
        STA mon_dsthi
        LDA mon_tmp
        STA mon_lo
        LDA mon_starthi
        STA mon_hi
        ; A destination inside the source interval must be copied backward.
        LDA mon_dsthi
        CMP mon_hi
        BCC .transfer
        BNE .possible_overlap
        LDA mon_dstlo
        CMP mon_lo
        BCC .transfer
        BNE .possible_overlap
        JMP mon_prompt
.possible_overlap
        LDA mon_dsthi
        CMP mon_endhi
        BCC .backward
        BNE .transfer
        LDA mon_dstlo
        CMP mon_endlo
        BCS .transfer
.backward
        SEC
        LDA mon_endlo
        SBC mon_lo
        STA mon_savedlo
        LDA mon_endhi
        SBC mon_hi
        STA mon_savedhi
        CLC
        LDA mon_savedlo
        ADC mon_dstlo
        STA mon_dstlo
        LDA mon_savedhi
        ADC mon_dsthi
        STA mon_dsthi
        LDA mon_endlo
        STA mon_lo
        LDA mon_endhi
        STA mon_hi
.reverse
        LDY #0
        LDA (mon_lo),Y
        STA (mon_dstlo),Y
        LDA mon_lo
        CMP mon_tmp
        BNE .reverse_next
        LDA mon_hi
        CMP mon_starthi
        BEQ .reverse_done
.reverse_next
        LDA mon_lo
        BNE .no_borrow
        DEC mon_hi
.no_borrow
        DEC mon_lo
        LDA mon_dstlo
        BNE .no_dest_borrow
        DEC mon_dsthi
.no_dest_borrow
        DEC mon_dstlo
        JMP .reverse
.reverse_done
        JMP mon_prompt
.transfer
        LDY #0
        LDA (mon_lo),Y
        STA (mon_dstlo),Y
        JSR mon_done
        BCC .long9
        JMP mon_prompt
.long9
        JSR mon_next
        INC mon_dstlo
        BNE .transfer
        INC mon_dsthi
        JMP .transfer
.not_transfer
        CMP #67                 ; C: compare inclusive range
        BNE .not_compare
        JSR mon_range
        BCS .long10
        JMP mon_error
.long10
        JSR mon_address
        BCS .long11
        JMP mon_error
.long11
        LDA mon_lo
        STA mon_dstlo
        LDA mon_hi
        STA mon_dsthi
        LDA mon_tmp
        STA mon_lo
        LDA mon_starthi
        STA mon_hi
.compare
        LDY #0
        LDA (mon_lo),Y
        CMP (mon_dstlo),Y
        BEQ .same
        JSR mon_print_address
        JSR newline
.same   JSR mon_done
        BCC .long12
        JMP mon_prompt
.long12
        JSR mon_next
        INC mon_dstlo
        BNE .compare
        INC mon_dsthi
        JMP .compare
.not_compare
        CMP #71                 ; G: run machine code
        BNE .not_go
        JSR mon_address
        BCS .go_addr
        LDA mon_pc_lo
        STA mon_lo
        LDA mon_pc_hi
        STA mon_hi
.go_addr
        LDA mon_lo
        STA mon_pc_lo
        LDA mon_hi
        STA mon_pc_hi
        TSX
        STX mon_host_sp
        LDA $0101,X             ; save the screen editor's return address
        STA mon_host_retlo
        LDA $0102,X
        STA mon_host_rethi
        LDA mon_sp
        CLC
        ADC #2                  ; JSR itself consumes two stack bytes
        TAX
        TXS                     ; target begins with the edited SP
        LDA mon_sr
        PHA
        LDA mon_ac
        LDX mon_xr
        LDY mon_yr
        PLP
        JSR mon_call
        STA mon_ac
        STX mon_xr
        STY mon_yr
        PHP
        PLA
        STA mon_sr
        TSX
        STX mon_sp
        LDX mon_host_sp
        TXS
        LDA mon_host_retlo
        STA $0101,X
        LDA mon_host_rethi
        STA $0102,X
        JMP mon_prompt
.not_go
        CMP #72                 ; H: hunt byte pattern
        BNE .not_hunt
        JMP mon_hunt
.not_hunt
        CMP #65                 ; A: assemble
        BEQ .assemble
        CMP #46                 ; .: alternate assembler command
        BNE .not_assemble
.assemble
        JMP mon_assemble
.not_assemble
        CMP #68                 ; D: disassemble
        BNE .not_disassemble
        JMP mon_disassemble
.not_disassemble
        CMP #76                 ; L: load a program
        BNE .not_load
        JMP mon_load
.not_load
        CMP #83                 ; S: save a program
        BNE .not_save
        JMP mon_save
.not_save
        CMP #86                 ; V: verify a program
        BNE .not_verify
        JMP mon_verify
.not_verify
        CMP #59                 ; ;: edit saved registers
        BNE mon_error
        JSR mon_address
        BCS .long13
        JMP mon_error
.long13
        LDA mon_lo
        STA mon_pc_lo
        LDA mon_hi
        STA mon_pc_hi
        JSR mon_byte
        BCC .regs_done
        LDA mon_value
        STA mon_sr
        JSR mon_byte
        BCC .regs_done
        LDA mon_value
        STA mon_ac
        JSR mon_byte
        BCC .regs_done
        LDA mon_value
        STA mon_xr
        JSR mon_byte
        BCC .regs_done
        LDA mon_value
        STA mon_yr
        JSR mon_byte
        BCC .regs_done
        LDA mon_value
        STA mon_sp
.regs_done
        JSR mon_registers
        JMP mon_prompt

; Print a question mark for malformed or unsupported monitor commands.
mon_error
        LDA #63
        JSR putchar
        JSR newline
; Leave the next screen row ready for a monitor command.
mon_prompt
        RTS

; Skip spaces in input and return the next byte in A without consuming it.
mon_spaces
        LDY mon_pos
.space  LDA input,Y
        CMP #32
        BNE .found
        INY
        BNE .space
.found  STY mon_pos
        LDA input,Y             ; return flags for the byte, not STY's index
        RTS

; Parse one to four hexadecimal digits as a 16-bit address. Carry is set on
; success, with the value in mon_lo:mon_hi; otherwise carry is clear.
mon_address
        LDA #0
        STA mon_lo
        STA mon_hi
        STA mon_digits
        JSR mon_spaces
        CMP #36                 ; optional dollar sign
        BNE .digits
        INC mon_pos
.digits JSR mon_hex_digit
        BCC .end
        PHA
        ASL mon_lo
        ROL mon_hi
        ASL mon_lo
        ROL mon_hi
        ASL mon_lo
        ROL mon_hi
        ASL mon_lo
        ROL mon_hi
        PLA
        ORA mon_lo
        STA mon_lo
        INC mon_digits
        INC mon_pos
        LDA mon_digits
        CMP #4
        BCC .digits
.end   LDA mon_digits
        BEQ .absent
        SEC
        RTS
.absent CLC
        RTS

; Parse one hex byte, returning it in mon_value with carry set. Up to two
; digits are accepted. Carry clear means no byte was present.
mon_byte
        LDA mon_lo
        STA mon_savedlo
        LDA mon_hi
        STA mon_savedhi
        JSR mon_address
        BCC .none
        LDA mon_digits
        CMP #3
        BCS .none
        LDA mon_lo
        STA mon_value
        LDA mon_savedlo
        STA mon_lo
        LDA mon_savedhi
        STA mon_hi
        SEC
        RTS
.none   LDA mon_savedlo
        STA mon_lo
        LDA mon_savedhi
        STA mon_hi
        CLC
        RTS

; Convert input at mon_pos from one ASCII hex digit to a nibble. Carry is set
; for a valid digit, clear otherwise; mon_pos itself is left unchanged.
mon_hex_digit
        LDY mon_pos
        LDA input,Y
        CMP #48
        BCC .bad
        CMP #58
        BCC .decimal
        CMP #65
        BCC .bad
        CMP #71
        BCS .bad
        SEC
        SBC #55
        SEC
        RTS
.decimal
        SEC
        SBC #48
        SEC
        RTS
.bad    CLC
        RTS

; Parse start and inclusive end. Carry reports success; the start remains in
; mon_lo:mon_hi and the end in mon_endlo:mon_endhi. The start is also kept in
; mon_tmp:mon_starthi for commands that need a later destination operand.
mon_range
        JSR mon_address
        BCC .bad
        LDA mon_lo
        STA mon_tmp
        LDA mon_hi
        STA mon_starthi
        JSR mon_address
        BCC .bad
        LDA mon_lo
        STA mon_endlo
        LDA mon_hi
        STA mon_endhi
        LDA mon_tmp
        STA mon_lo
        LDA mon_starthi
        STA mon_hi
        SEC
        RTS
.bad    CLC
        RTS

; Read an optional address, retaining the prior dump cursor if omitted.
mon_maybe_address
        JSR mon_address
        BCS .got
        LDA mon_cursorlo
        STA mon_lo
        LDA mon_cursorhi
        STA mon_hi
.got   RTS

; Set carry when mon_lo:mon_hi reached the inclusive end address.
mon_done
        LDA mon_hi
        CMP mon_endhi
        BNE .more
        LDA mon_lo
        CMP mon_endlo
        BNE .more
        SEC
        RTS
.more   CLC
        RTS

; Increment the current 16-bit address, preserving no registers.
mon_next
        INC mon_lo
        BNE .done
        INC mon_hi
.done   RTS

; Dump one line of eight bytes at mon_lo:mon_hi, then retain the next address.
mon_dump
        LDA mon_lo
        STA $74                 ; ASCII column reads the same eight bytes
        LDA mon_hi
        STA $75
        JSR mon_print_address
        LDX #8
.byte   LDA #32
        JSR putchar
        LDY #0
        JSR mon_read
        JSR mon_print_byte
        JSR mon_next
        DEX
        BNE .byte
        LDA #32
        JSR putchar
        LDA #58
        JSR putchar
        LDX #8
.ascii  LDY #0
        LDA ($74),Y
        CMP #32
        BCC .dot
        CMP #123
        BCC .print_ascii
.dot    LDA #46
.print_ascii
        JSR putchar
        INC $74
        BNE .ascii_next
        INC $75
.ascii_next
        DEX
        BNE .ascii
        JSR newline
        LDA mon_lo
        STA mon_cursorlo
        LDA mon_hi
        STA mon_cursorhi
        RTS

; Print the current address as four hex digits followed by a colon.
mon_print_address
        LDA mon_hi
        JSR mon_print_byte
        LDA mon_lo
        JSR mon_print_byte
        LDA #58
        JMP putchar

; Print A as two uppercase hexadecimal digits. A/X are scratch.
mon_print_byte
        PHA
        LSR
        LSR
        LSR
        LSR
        JSR mon_print_nibble
        PLA
        AND #15
        JMP mon_print_nibble

; Print the low nibble of A as an ASCII hex digit.
mon_print_nibble
        AND #15
        CMP #10
        BCC .number
        CLC
        ADC #55
        JMP putchar
.number CLC
        ADC #48
        JMP putchar

; Display saved machine register values in TEDMON's public field order.
mon_registers
        LDA #mon_heading%256
        STA msglo
        LDA #mon_heading/256
        STA msghi
        JSR printz
        JSR newline
        LDA #59                 ; editable register row begins with ';'
        JSR putchar
        LDA #32
        JSR putchar
        LDA mon_pc_hi
        JSR mon_print_byte
        LDA mon_pc_lo
        JSR mon_print_byte
        LDA mon_sr
        JSR mon_register_byte
        LDA mon_ac
        JSR mon_register_byte
        LDA mon_xr
        JSR mon_register_byte
        LDA mon_yr
        JSR mon_register_byte
        LDA mon_sp
        JSR mon_register_byte
        JMP newline

; Print one space and one register byte passed in A.
mon_register_byte
        PHA
        LDA #32
        JSR putchar
        PLA
        JMP mon_print_byte

; Execute a monitor G target as a subroutine so an RTS can return to TEDMON.
; A BRK target remains subject to the machine's normal IRQ/BRK vector.
mon_call
        JMP (mon_lo)

; A BRK during G is redirected here by the shared IRQ handler. Its RTS uses
; G's pending JSR return address, so the monitor command loop resumes.
monitor_break_resume
        RTS

; Read monitor memory at mon_lo:mon_hi with Y offset. $07F8 bit 7 selects
; RAM beneath ROM for M/D; otherwise A receives the currently mapped byte.
mon_read
        LDA $07f8
        BPL .mapped
        LDA mon_hi
        CMP #$80
        BCC .mapped
        JMP $0720
.mapped
        LDA (mon_lo),Y
        RTS

; Copied to low RAM at monitor entry so bank switching never executes from a
; ROM that disappears while RAM beneath it is exposed.
mon_ramcode
        SEI
        STA $ff3f
        LDA (mon_lo),Y
        STA $ff3e
        CLI
        RTS
mon_ramcode_end

mon_banner BYT "MONITOR",0
mon_heading BYT "   PC  SR AC XR YR SP",0
