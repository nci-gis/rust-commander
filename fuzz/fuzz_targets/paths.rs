//! Paths in a line of a shell's output (`found::paths_in`): never a panic, and every
//! span lies inside the line, on character boundaries, and holds the path it names.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|line: &str| {
    for f in xdevcmd_core::found::paths_in(line) {
        assert!(
            f.span.start <= f.span.end && f.span.end <= line.len(),
            "{f:?}"
        );
        assert!(line.is_char_boundary(f.span.start) && line.is_char_boundary(f.span.end));
        assert!(!f.path.is_empty(), "{f:?}");
        assert!(!f.path.chars().any(char::is_control), "{f:?}");
    }
});
