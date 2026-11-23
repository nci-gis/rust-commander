//! A theme file (`config::theme_from_text`) and a colour (`parse_color`): a theme or an
//! error, never a panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let _ = xdevcmd_fuzz::config::theme_from_text(text, "fuzz");
    let _ = xdevcmd_fuzz::config::parse_color(text);
});
