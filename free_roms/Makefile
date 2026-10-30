# Build the two independent 16 KiB ROM images with the sibling ass6510 project.
ASS6510 ?= ../../ass6510/target/release/ass6510

.PHONY: all test
all: basrom.dat sysrom.dat

basrom.dat: basrom.asm variables.asm strings.asm numbers.asm float_parser.asm extensions.asm shapes.asm
	$(ASS6510) -o $@ $<

sysrom.dat: sysrom.asm font.asm math.asm math_advanced.asm
	$(ASS6510) -o $@ $<

test: all
	cargo test --manifest-path ../Cargo.toml --test free_rom
