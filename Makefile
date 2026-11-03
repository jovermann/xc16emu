# Copyright (C) 2026 Johannes Overmann.
# SPDX-License-Identifier: BSL-1.0

.DEFAULT_GOAL := build
.PHONY: build debug clean

build:
	cargo build --release

debug:
	cargo build

clean:
	cargo clean
