//! A config file (`config::parse`): any text gives settings or an error, never a panic.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let _ = xdevcmd_fuzz::config::parse(text);
});
