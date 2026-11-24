//! The app's parsers of untrusted input, compiled here from its own source files so the
//! fuzz targets reach them: `xdevcmd` is a binary crate, and these modules depend only
//! on each other, std and the crates in Cargo.toml. Nothing is copied: a change to the
//! app is fuzzed as it is.
#![allow(dead_code, unused_imports, unused_macros, clippy::all)]

/// OSC 7, 9;9 and 133: what a shell (or any program in it) prints.
#[path = "../../crates/xdevcmd/src/pane/shell_report.rs"]
pub mod shell_report;

/// The config file and theme files.
#[path = "../../crates/xdevcmd/src/config.rs"]
pub mod config;
#[path = "../../crates/xdevcmd/src/decor.rs"]
pub mod decor;
#[path = "../../crates/xdevcmd/src/keys.rs"]
pub mod keys;
#[path = "../../crates/xdevcmd/src/theme.rs"]
pub mod theme;
/// The walker's hold range, which config.rs checks `animation_hold_secs` against
/// (beta.3, W0).
#[path = "../../crates/xdevcmd/src/walker.rs"]
pub mod walker;
