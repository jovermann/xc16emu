; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Calculate and draw a 20-iteration Mandelbrot set in TED bitmap mode.
; A white bitmap is cleared to black for points still bounded after twenty
; iterations. The image is calculated by the emulated CPU, not precalculated.
;
; Signed 16-bit fixed point with seven fractional bits gives 128 units per complex unit. The real and
; imaginary coordinates both advance by an average of 1.5 fixed-point units
; per screen pixel (3/256 complex units), giving a 1:1 geometric aspect ratio.
; Q7 rounding alternates one- and two-unit steps. The view spans about
; -2.75..+1.0 horizontally and -1.16..+1.16 vertically. Each of the 320
; columns has a distinct real coordinate; conjugate symmetry supplies the
; lower 100 scan lines. No horizontal doubling.

        ASS $4000

ptr     EQU $20                 ; zero-page bitmap address
xlo     EQU $22                 ; column 0..319, little endian
xhi     EQU $23
ypos    EQU $24                 ; upper scan line 0..99
crlo    EQU $25                 ; complex C, signed Q7
crhi    EQU $26
cilo    EQU $27
cihi    EQU $28
zrlo    EQU $29                 ; current Z, signed Q7
zrhi    EQU $2a
zilo    EQU $2b
zihi    EQU $2c
nextlo  EQU $2d                 ; saved next real component
nexthi  EQU $2e
steps   EQU $2f                 ; updates still to perform, 20..0
arglo   EQU $30                 ; signed argument to the square lookup
arghi   EQU $31
sqloout EQU $32                 ; unsigned square, raw Q14
sqhiout EQU $33
sqtop   EQU $34
r2lo    EQU $35                 ; real square
r2hi    EQU $36
r2top   EQU $37
i2lo    EQU $38                 ; imaginary square
i2hi   EQU $39
i2top  EQU $3a
sumlo   EQU $3b                 ; squared magnitude
sumhi   EQU $3c
sumtop  EQU $3d
termlo  EQU $3e                 ; signed Q14 real/cross expression
termhi  EQU $3f
termtop EQU $40
caddlo  EQU $41                 ; signed Q7 component to add
caddhi  EQU $42
collo   EQU $43                 ; horizontal bitmap byte offset
colhi   EQU $44
mask    EQU $45                 ; bits to retain while plotting black
rowidx  EQU $46                 ; upper character row
indexlo EQU $47                 ; absolute square-lookup index
indexhi EQU $48
sqvlo   EQU $49                 ; running square while building tables
sqvhi   EQU $4a
sqvtop  EQU $4b
oddlo   EQU $4c                 ; difference to the next square: 2*n+1
oddhi   EQU $4d
rowvlo  EQU $4e                 ; running bitmap row offset
rowvhi  EQU $4f

; Tables are generated into RAM at startup; there are no image data bytes
; or long lookup tables in the assembled program.
sqlo0   EQU $6000               ; squares 0..255, low/high bytes
sqhi0   EQU $6100
sqlo1   EQU $6200               ; squares 256..362, low/high bytes
sqhi1   EQU $6300
rowlo   EQU $6400               ; bitmap offsets for 25 character rows
rowhi   EQU $6500
clearmask EQU $6600             ; one clear-bit mask for each pixel position

start   SEI                     ; demo does not call the ROM IRQ handler
        JSR maketables
        LDX #$00
.matrix  LDA #$07                ; bright white luminance
        STA $0800,X
        STA $0900,X
        STA $0a00,X
        STA $0b00,X
        LDA #$10                ; bitmap foreground hue 1 (white)
        STA $0c00,X
        STA $0d00,X
        STA $0e00,X
        STA $0f00,X
        INX
        BNE .matrix

        ; Fill all 8000 bitmap bytes with white pixels.
        LDA #$00
        STA ptr
        LDA #$20
        STA ptr+1
        LDX #31
        LDY #$00
        LDA #$ff
.fillpg  STA (ptr),Y
        INY
        BNE .fillpg
        INC ptr+1
        DEX
        BNE .fillpg
.filltail
        STA (ptr),Y
        INY
        CPY #64
        BNE .filltail

        LDA #$00
        STA $ff15               ; black picture background
                                ; leave BASIC's blue border at $ff19 alone
        LDA #$08
        STA $ff14               ; matrix at $0800
        STA $ff07               ; 40 columns, high-resolution pixels
        STA $ff12               ; bitmap in RAM at $2000
        LDA #$3b
        STA $ff06               ; picture on, 25 rows, bitmap mode

        LDA #$00
        STA ypos
        LDA #$6b               ; imaginary start -149 in signed Q7
        STA cilo
        LDA #$ff
        STA cihi
.rowloop LDA #$00
        STA xlo
        STA xhi
        LDA #$a0               ; real start -352 in signed Q7
        STA crlo
        LDA #$fe
        STA crhi
.colloop
        LDA #$00
        STA zrlo
        STA zrhi
        STA zilo
        STA zihi
        LDA #20
        STA steps

; Check |z| squared before each update. The final pass checks z20 after
; exactly twenty z <- z*z+c updates.
.iterate LDA zrlo
        STA arglo
        LDA zrhi
        STA arghi
        JSR square
        LDA sqloout
        STA r2lo
        LDA sqhiout
        STA r2hi
        LDA sqtop
        STA r2top
        LDA zilo
        STA arglo
        LDA zihi
        STA arghi
        JSR square
        LDA sqloout
        STA i2lo
        LDA sqhiout
        STA i2hi
        LDA sqtop
        STA i2top
        CLC
        LDA r2lo
        ADC i2lo
        STA sumlo
        LDA r2hi
        ADC i2hi
        STA sumhi
        LDA r2top
        ADC i2top
        STA sumtop
        BEQ .bounded             ; 4.0 in raw Q14 is $010000
        JMP .nextpoint
.bounded LDA steps
        BNE .updatez
        JSR plotblack           ; bounded after 20 updates: black
        JMP .nextpoint

; Next real component: (zr*zr - zi*zi)/128 + cr.
.updatez SEC
        LDA r2lo
        SBC i2lo
        STA termlo
        LDA r2hi
        SBC i2hi
        STA termhi
        LDA r2top
        SBC i2top
        STA termtop
        LDA crlo
        STA caddlo
        LDA crhi
        STA caddhi
        JSR scaleadd
        BCC .realok
        JMP .nextpoint
.realok  LDA termlo
        STA nextlo
        LDA termhi
        STA nexthi

; Next imaginary component: 2*zr*zi = (zr+zi)^2 - zr^2 - zi^2.
        CLC
        LDA zrlo
        ADC zilo
        STA arglo
        LDA zrhi
        ADC zihi
        STA arghi
        JSR square
        SEC
        LDA sqloout
        SBC sumlo
        STA termlo
        LDA sqhiout
        SBC sumhi
        STA termhi
        LDA sqtop
        SBC sumtop
        STA termtop
        LDA cilo
        STA caddlo
        LDA cihi
        STA caddhi
        JSR scaleadd
        BCC .imagok
        JMP .nextpoint
.imagok  LDA termlo
        STA zilo
        LDA termhi
        STA zihi
        LDA nextlo
        STA zrlo
        LDA nexthi
        STA zrhi
        DEC steps
        JMP .iterate

.nextpoint
        INC xlo
        BNE .xready
        INC xhi
.xready  LDA xhi
        CMP #$01
        BNE .morex
        LDA xlo
        CMP #$40                ; $0140 = 320 columns
        BEQ .nextrow
.morex   LDA xlo
        AND #$01                ; new odd columns advance by 2, even by 1
        BEQ .addone
        LDA #$02
        JMP .addreal
.addone  LDA #$01
.addreal CLC
        ADC crlo
        STA crlo
        LDA crhi
        ADC #$00
        STA crhi
        JMP .colloop
.nextrow INC ypos
        LDA ypos
        CMP #100
        BEQ .finished
        AND #$01                ; same 2,1 coordinate steps vertically
        BEQ .imagone
        LDA #$02
        JMP .addimag
.imagone LDA #$01
.addimag CLC
        ADC cilo
        STA cilo
        LDA cihi
        ADC #$00
        STA cihi
        JMP .rowloop
.finished
        JMP .finished            ; leave the completed picture visible

; Generate n^2 for n=0..362 from (n+1)^2 = n^2 + (2*n+1). The third
; square byte is implicit: zero on the first page and one on the second.
; Generate bitmap row offsets and one-bit clear masks in the same RAM area.
maketables
        LDA #$00
        STA sqvlo
        STA sqvhi
        STA sqvtop
        STA oddhi
        LDA #$01
        STA oddlo
        LDX #$00
.genlow  LDA sqvlo
        STA sqlo0,X
        LDA sqvhi
        STA sqhi0,X
        JSR .nextsquare
        INX
        BNE .genlow
        LDX #$00
.genhigh LDA sqvlo
        STA sqlo1,X
        LDA sqvhi
        STA sqhi1,X
        JSR .nextsquare
        INX
        CPX #107                ; 256..362 inclusive
        BNE .genhigh

        LDA #$00
        STA rowvlo
        STA rowvhi
        LDX #$00
.genrow  LDA rowvlo
        STA rowlo,X
        LDA rowvhi
        STA rowhi,X
        CLC
        LDA rowvlo
        ADC #$40
        STA rowvlo
        LDA rowvhi
        ADC #$01                ; $0140 = 320 bytes per row
        STA rowvhi
        INX
        CPX #25
        BNE .genrow

        LDA #$80
        STA sqvlo
        LDX #$00
.genmask LDA sqvlo
        EOR #$ff
        STA clearmask,X
        LDA sqvlo
        LSR
        STA sqvlo
        INX
        CPX #8
        BNE .genmask
        RTS

; Advance the running square and the odd difference without changing X.
.nextsquare
        CLC
        LDA sqvlo
        ADC oddlo
        STA sqvlo
        LDA sqvhi
        ADC oddhi
        STA sqvhi
        LDA sqvtop
        ADC #$00
        STA sqvtop
        CLC
        LDA oddlo
        ADC #$02
        STA oddlo
        LDA oddhi
        ADC #$00
        STA oddhi
        RTS

; Return the square of signed Q7 arg in sqloout/sqhiout/sqtop.
; The bounded radius guarantees |arg| <= 362, so a two-page square table
; holds every possible lookup. Its top byte is 0 for 0..255 and 1 for 256..
square  LDA arghi
        BPL .sqpositive
        LDA #$00
        SEC
        SBC arglo
        STA indexlo
        LDA #$00
        SBC arghi
        STA indexhi
        JMP .sqlookup
.sqpositive
        LDA arglo
        STA indexlo
        LDA arghi
        STA indexhi
.sqlookup
        LDY indexlo
        LDA indexhi
        BEQ .sqpage0
        LDA sqlo1,Y
        STA sqloout
        LDA sqhi1,Y
        STA sqhiout
        LDA #$01
        STA sqtop
        RTS
.sqpage0 LDA sqlo0,Y
        STA sqloout
        LDA sqhi0,Y
        STA sqhiout
        LDA #$00
        STA sqtop
        RTS

; Shift a signed Q14 term right seven bits and add signed Q7 cadd.
; Carry clear means the Q7 result fits -256..255; carry set means escape.
scaleadd
        LDX #7
.ashift  LDA termtop
        CMP #$80                ; put sign into carry for arithmetic ROR
        ROR termtop
        ROR termhi
        ROR termlo
        DEX
        BNE .ashift
        CLC
        LDA termlo
        ADC caddlo
        STA termlo
        LDA termhi
        ADC caddhi
        STA termhi
        BEQ .scaledok
        CMP #$ff
        BEQ .scaledok
        SEC
        RTS
.scaledok
        CLC
        RTS

; Clear one pixel in the upper half and its conjugate lower pixel. TED
; stores eight consecutive scan-line bytes per 8x8 character cell.
plotblack
        LDA xlo
        AND #$f8
        STA collo               ; floor(x/8)*8 = x with low bits cleared
        LDA xhi
        STA colhi
        LDA xlo
        AND #$07
        TAX
        LDA clearmask,X
        STA mask
        LDA ypos
        LSR
        LSR
        LSR
        TAX
        STX rowidx
        JSR rowptr
        LDA ypos
        AND #$07
        TAY
        LDA (ptr),Y
        AND mask
        STA (ptr),Y
        LDA #24
        SEC
        SBC rowidx
        TAX
        JSR rowptr
        LDA ypos
        AND #$07
        EOR #$07                ; 199-y has opposite within-cell row
        TAY
        LDA (ptr),Y
        AND mask
        STA (ptr),Y
        RTS

; Form $2000 + row*320 + floor(x/8)*8 in a zero-page pointer.
rowptr  CLC
        LDA rowlo,X
        ADC collo
        STA ptr
        LDA rowhi,X
        ADC colhi
        CLC
        ADC #$20
        STA ptr+1
        RTS

        END
