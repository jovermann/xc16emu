# Build the two independent 16 KiB ROM images with the sibling ass6510 project.
ASS6510 ?= ../../ass6510/target/release/ass6510

.PHONY: all test
all: basrom.dat sysrom.dat

basrom.dat: basrom.asm
	$(ASS6510) -o $@ $<

sysrom.dat: sysrom.asm
	$(ASS6510) -o $@ $<

test: all
	cargo test --manifest-path ../Cargo.toml --test free_rom
