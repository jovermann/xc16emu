; Copyright (C) 2026 Johannes Overmann.
; SPDX-License-Identifier: BSL-1.0
;
; Rotating wireframe cube for the C16/Plus/4 PAL TED display.
; Assemble with ass6510 -m to produce a load-address PRG, then load it with
; xc16emu --run-prg examples/rotating_cube.prg.
;
; The program rotates a three-dimensional basis in signed Q4 fixed point,
; perspective-projects eight cube corners, and draws twelve edges into the
; hidden bitmap. The rotation repeats after one turn to prevent fixed-point
; rounding from gradually growing the cube beyond the display.
; Bitmap row offsets and pixel masks are also calculated when the demo starts.

        ASS $6000

ptr     EQU $20                 ; two-byte zero-page bitmap pointer
x0      EQU $22                 ; current line pixel
y0      EQU $23
x1      EQU $24                 ; destination line pixel
y1      EQU $25
dx      EQU $26                 ; unsigned line lengths
dy      EQU $27
sx      EQU $28                 ; signed pixel steps: +1 or -1
sy      EQU $29
err     EQU $2a                 ; Bresenham-style unsigned error
maskbit EQU $2b
drawhi  EQU $2c                 ; high byte of hidden bitmap: $20 or $40
edgeidx EQU $2d                 ; edge number 0..11
vertex  EQU $2e                 ; vertex number 0..7
signs   EQU $2f                 ; positive axis bits for one vertex
signbit EQU $30                 ; axis bit being projected
axes    EQU $31                 ; remaining axes in projection
sumlo   EQU $32                 ; signed 16-bit projected coordinate
sumhi   EQU $33
center  EQU $34                 ; screen center for this coordinate
alo     EQU $35                 ; original first vector component
ahi     EQU $36
clo     EQU $37                 ; original second vector component
chi     EQU $38
shlo    EQU $39                 ; signed arithmetic-shift scratch
shhi    EQU $3a
newlo   EQU $3b                 ; updated first vector component
newhi   EQU $3c
shcount EQU $3d
rowlow  EQU $3e                 ; cumulative TED bitmap row offset
rowhigh EQU $3f
worldz  EQU $42
denom   EQU $43                 ; positive perspective denominator
numlo   EQU $44                 ; unsigned division numerator
numhi   EQU $45
quot    EQU $46                 ; unsigned perspective offset
negative EQU $47               ; sign of projected coordinate
frameidx EQU $48               ; orientation 0..17

; Three-dimensional cube axes in signed pixels * 16. Each component group
; contains the contributions from the cube's X, Y and Z axes.
basis   EQU $50                 ; ax,bx,cx,ay,by,cy,az,bz,cz: 18 bytes
rowlo   EQU $7000               ; generated row offsets, 25 bytes
rowhi   EQU $7100
pixels  EQU $7200               ; generated pixel masks, 8 bytes
vertexx EQU $7300               ; generated cube corner coordinates
vertexy EQU $7400

start   SEI                     ; the demo owns these RAM and TED registers
        JSR maketab
        JSR initaxes
        LDA #$00
        STA frameidx
        LDA #$00
        STA $ff15               ; black background
        STA $ff19               ; black border
        STA $ff16
        STA $ff17
        STA $ff18
        LDA #$08
        STA $ff14               ; screen and color matrix at $0800
        STA $ff07               ; 40 columns, single-color pixels
        STA $ff12               ; bitmap at $2000, RAM character/bitmap bank
        LDA #$3b
        STA $ff06               ; display on, 25 rows, bitmap mode

        ; Bitmap set bits use bright white ($71); clear bits use black.
        LDX #$00
initcol LDA #$07
        STA $0800,X
        STA $0900,X
        STA $0a00,X
        STA $0b00,X
        LDA #$10
        STA $0c00,X
        STA $0d00,X
        STA $0e00,X
        STA $0f00,X
        INX
        BNE initcol

        LDA #$20
        STA drawhi
        JSR clearbuf
        LDA #$40
        STA drawhi
        JSR clearbuf
main    JSR clearbuf            ; erase only the hidden bitmap
        JSR corners             ; project eight corners from rotated axes
        JSR edges               ; draw the next wireframe cube
        JSR waitbot             ; swap in the lower border
        LDA drawhi
        LSR
        LSR
        STA $ff12               ; $20->$08 and $40->$10 bitmap select
        JSR waittop             ; leave this border before drawing again
        LDA drawhi
        EOR #$60                ; alternate $20 and $40
        STA drawhi
        INC frameidx
        LDA frameidx
        CMP #18                 ; approximately one complete 3D turn
        BNE advance
        JSR initaxes            ; remove accumulated rounding drift
        LDA #$00
        STA frameidx
        JMP main
advance JSR rotate              ; advance yaw and pitch for next frame
        JMP main

; Generate row*320 offsets and descending bit masks in RAM. There are no
; embedded graphics lookup tables in this source file.
maketab LDA #$00
        STA rowlow
        STA rowhigh
        LDX #$00
makrow  LDA rowlow
        STA rowlo,X
        CLC
        ADC #$40
        STA rowlow
        LDA rowhigh
        STA rowhi,X
        ADC #$01
        STA rowhigh
        INX
        CPX #25
        BNE makrow
        LDA #$80
        LDX #$00
makmask STA pixels,X
        LSR
        INX
        CPX #8
        BNE makmask
        RTS

; Start with unit-length cube axes X=(32,0,0), Y=(0,32,0), Z=(0,0,32),
; all represented with four fractional bits.
initaxes
        LDA #$00
        STA basis
        LDA #$02
        STA basis+1
        LDA #$00
        STA basis+2
        STA basis+3
        STA basis+4
        STA basis+5
        STA basis+6
        STA basis+7
        LDA #$00
        STA basis+8
        LDA #$02
        STA basis+9
        LDA #$00
        STA basis+10
        STA basis+11
        STA basis+12
        STA basis+13
        STA basis+14
        STA basis+15
        STA basis+16
        LDA #$02
        STA basis+17
        RTS

; Project each corner of a cube. Bits 0,1,2 of signs choose the positive
; X,Y,Z axes; the remaining corners use their negative counterparts.
corners LDX #$00
corner  STX vertex
        TXA
        AND #$03
        CMP #$01
        BEQ xpos
        CMP #$02
        BNE xneg
xpos    LDA #$01
        BNE xready
xneg    LDA #$00
xready  STA signs
        TXA
        AND #$02
        BEQ yneg
        LDA signs
        ORA #$02
        STA signs
yneg    TXA
        AND #$04
        BEQ zneg
        LDA signs
        ORA #$04
        STA signs
zneg    LDX #$0c
        JSR project
        STA worldz
        LDA #128
        SEC
        SBC worldz
        STA denom               ; eye is 128 pixels from cube center
        LDX #$00
        JSR project
        LDA #160
        STA center
        LDA sumlo
        JSR perspective
        LDX vertex
        STA vertexx,X
        LDX #$06
        JSR project
        LDA #100
        STA center
        LDA sumlo
        JSR perspective
        LDX vertex
        STA vertexy,X
        INX
        CPX #8
        BNE corner
        RTS

; Sum the three signed basis components selected by signs. X selects the
; world X, Y or Z group. Return the signed pixel coordinate in A and sumlo.
project LDA #$00
        STA sumlo
        STA sumhi
        LDA #$01
        STA signbit
        LDA #$03
        STA axes
projaxis
        LDA signs
        AND signbit
        BEQ subaxis
        CLC
        LDA sumlo
        ADC basis,X
        STA sumlo
        LDA sumhi
        ADC basis+1,X
        STA sumhi
        JMP nextaxis
subaxis SEC
        LDA sumlo
        SBC basis,X
        STA sumlo
        LDA sumhi
        SBC basis+1,X
        STA sumhi
nextaxis
        INX
        INX
        ASL signbit
        DEC axes
        BNE projaxis
        LDA sumhi
        CMP #$80
        ROR sumhi
        ROR sumlo
        LDA sumhi
        CMP #$80
        ROR sumhi
        ROR sumlo
        LDA sumhi
        CMP #$80
        ROR sumhi
        ROR sumlo
        LDA sumhi
        CMP #$80
        ROR sumhi
        ROR sumlo
        LDA sumlo
        RTS

; Perspective-project signed coordinate A using scale 128/(128-worldz).
; Return its screen coordinate around center. The numerator is at most 8192,
; so unsigned repeated subtraction keeps the division simple on the 6502.
perspective
        LDX #$00
        CMP #$80
        BCC positive
        LDX #$01
        EOR #$ff
        CLC
        ADC #$01
positive
        STX negative
        PHA
        LSR
        STA numhi               ; magnitude * 128, high byte
        PLA
        AND #$01
        BEQ evenmag
        LDA #$80
evenmag STA numlo               ; magnitude * 128, low byte
        LDA #$00
        STA quot
divloop LDA numhi
        BNE divsub
        LDA numlo
        CMP denom
        BCC divdone
divsub  LDA numlo
        SEC
        SBC denom
        STA numlo
        LDA numhi
        SBC #$00
        STA numhi
        INC quot
        JMP divloop
divdone
        LDA negative
        BEQ addcenter
        LDA center
        SEC
        SBC quot
        RTS
addcenter
        LDA center
        CLC
        ADC quot
        RTS

; Rotate X/Z (yaw), then Y/Z (pitch) for all three world coordinates. The
; approximated sine is 1/4 and cosine is 31/32 in signed Q4 arithmetic.
rotate  LDX #$00
        LDY #$04
        JSR rotpair
        LDX #$06
        LDY #$0a
        JSR rotpair
        LDX #$0c
        LDY #$10
        JSR rotpair
        LDX #$02
        LDY #$04
        JSR rotpair
        LDX #$08
        LDY #$0a
        JSR rotpair
        LDX #$0e
        LDY #$10
        JSR rotpair
        RTS

; Rotate the signed 16-bit basis components indexed by X and Y. Original
; components remain in scratch until both updated values are calculated.
rotpair LDA basis,X
        STA alo
        LDA basis+1,X
        STA ahi
        LDA basis,Y
        STA clo
        LDA basis+1,Y
        STA chi
        LDA alo
        STA shlo
        LDA ahi
        STA shhi
        LDA #5
        JSR asr
        LDA alo
        SEC
        SBC shlo
        STA newlo
        LDA ahi
        SBC shhi
        STA newhi
        LDA clo
        STA shlo
        LDA chi
        STA shhi
        LDA #2
        JSR asr
        LDA newlo
        SEC
        SBC shlo
        STA newlo
        LDA newhi
        SBC shhi
        STA newhi
        LDA clo
        STA shlo
        LDA chi
        STA shhi
        LDA #5
        JSR asr
        LDA clo
        SEC
        SBC shlo
        STA clo
        LDA chi
        SBC shhi
        STA chi
        LDA alo
        STA shlo
        LDA ahi
        STA shhi
        LDA #2
        JSR asr
        LDA clo
        CLC
        ADC shlo
        STA basis,Y
        LDA chi
        ADC shhi
        STA basis+1,Y
        LDA newlo
        STA basis,X
        LDA newhi
        STA basis+1,X
        RTS

; Arithmetically shift signed 16-bit shhi:shlo right by A bits. X and Y
; are kept for the caller's basis indexes.
asr     STA shcount
asrloop LDA shhi
        CMP #$80
        ROR shhi
        ROR shlo
        DEC shcount
        BNE asrloop
        RTS

waitbot LDA $ff1c               ; raster bit 8 becomes 1 at line 256
        AND #$01
        BEQ waitbot
        RTS

waittop LDA $ff1c               ; wait until the next frame's top border
        AND #$01
        BNE waittop
        RTS

; Clear exactly 8000 bytes: 31 whole pages and the final 64 bytes.
clearbuf
        LDA #$00
        STA ptr
        LDA drawhi
        STA ptr+1
        LDX #31
        LDY #$00
        LDA #$00
clearpg STA (ptr),Y
        INY
        BNE clearpg
        INC ptr+1
        DEX
        BNE clearpg
cleartl STA (ptr),Y
        INY
        CPY #64
        BNE cleartl
        RTS

; Derive all twelve edge pairs from their edge number: four on each face,
; then four joining matching corners. The corners are computed each frame.
edges   LDX #$00
edgeloop
        STX edgeidx
        TXA
        CMP #8
        BCC gota
        SEC
        SBC #8
        JMP gota
gota
        TAY
        LDA vertexx,Y
        STA x0
        LDA vertexy,Y
        STA y0
        LDA edgeidx
        CMP #8
        BCC faceb
        SEC
        SBC #4
        JMP gotb
faceb   CLC
        ADC #1
        AND #3
        LDX edgeidx
        CPX #4
        BCC gotb
        CLC
        ADC #4
gotb
        TAY
        LDA vertexx,Y
        STA x1
        LDA vertexy,Y
        STA y1
        JSR line
        LDX edgeidx
        INX
        CPX #12
        BNE edgeloop
        RTS

; Draw a line using an unsigned error accumulator. Each iteration advances
; the major axis once and advances the minor axis on accumulator underflow.
line    LDA x1
        SEC
        SBC x0
        BCS xplus
        EOR #$ff
        CLC
        ADC #$01
        STA dx
        LDA #$ff
        STA sx
        JMP ydelta
xplus   STA dx
        LDA #$01
        STA sx
ydelta  LDA y1
        SEC
        SBC y0
        BCS yplus
        EOR #$ff
        CLC
        ADC #$01
        STA dy
        LDA #$ff
        STA sy
        JMP choose
yplus   STA dy
        LDA #$01
        STA sy
choose  LDA dx
        CMP dy
        BCC steep
        LSR
        STA err

shallow JSR plot
        LDA x0
        CMP x1
        BEQ linedone
        LDA err
        SEC
        SBC dy
        BCS shok
        CLC
        ADC dx
        STA err
        LDA y0
        CLC
        ADC sy
        STA y0
        JMP shnext
shok    STA err
shnext  LDA x0
        CLC
        ADC sx
        STA x0
        JMP shallow

steep   LDA dy
        LSR
        STA err
stloop  JSR plot
        LDA y0
        CMP y1
        BEQ linedone
        LDA err
        SEC
        SBC dx
        BCS stok
        CLC
        ADC dy
        STA err
        LDA x0
        CLC
        ADC sx
        STA x0
        JMP stnext
stok    STA err
stnext  LDA y0
        CLC
        ADC sy
        STA y0
        JMP stloop
linedone
        RTS

; TED bitmap order is 40 character cells per eight-line row. Each cell's
; eight scan lines are consecutive bytes, unlike a linear 40-byte scan line.
; The low three Y bits select a byte, and low three X bits select a pixel.
plot    LDA y0
        LSR
        LSR
        LSR
        TAX
        LDA rowlo,X
        STA ptr
        LDA rowhi,X
        CLC
        ADC drawhi
        STA ptr+1
        LDA x0
        AND #$f8
        CLC
        ADC ptr
        STA ptr
        BCC ptrready
        INC ptr+1
ptrready
        LDA x0
        AND #$07
        TAX
        LDA pixels,X
        STA maskbit
        LDA y0
        AND #$07
        TAY
        LDA (ptr),Y
        ORA maskbit
        STA (ptr),Y
        RTS

        END
