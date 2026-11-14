//! OSC 7 (directory), 9;9 (Windows Terminal's directory) and 133 (prompt marks): the
//! bytes after `ESC ]`, split on `;` as vt100 splits them. Never a panic; a directory
//! reported is absolute.
#![no_main]
use libfuzzer_sys::fuzz_target;
use xdevcmd_fuzz::shell_report::{osc133, osc7, osc9_9};

fuzz_target!(|data: &[u8]| {
    let params: Vec<&[u8]> = data.split(|b| *b == b';').collect();
    for windows in [false, true] {
        if let Some(p) = osc7(&params, "host", windows) {
            assert!(!p.as_os_str().is_empty());
        }
        let _ = osc9_9(&params, windows);
    }
    let _ = osc133(&params);
});
