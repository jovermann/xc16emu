// Copyright (C) 2026 Johannes Overmann.
// SPDX-License-Identifier: BSL-1.0
//! Run one shared behavior suite against both BASIC ROM implementations.

mod original {
    const ROM_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/old/roms");
    const SUPPORTS_TOM_FROZEN_IMAGE: bool = true;
    const SUPPORTS_GALAXY_PRG: bool = true;
    include!("support/rom_cases.rs");
}

mod free {
    const ROM_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/free_roms");
    const SUPPORTS_TOM_FROZEN_IMAGE: bool = false;
    const SUPPORTS_GALAXY_PRG: bool = false;
    include!("support/rom_cases.rs");
}
