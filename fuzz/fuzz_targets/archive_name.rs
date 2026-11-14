//! A name inside an archive (`archive::clean`): what it keeps never climbs out of the
//! folder it is extracted into, and names no drive or stream.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|name: &str| {
    if let Some(p) = xdevcmd_core::archive::clean(name) {
        assert!(!p.is_empty());
        assert!(
            !p.starts_with('/') && !p.contains('\\') && !p.contains(':'),
            "{p:?}"
        );
        for part in p.split('/') {
            assert!(!part.is_empty() && part != "." && part != "..", "{p:?}");
        }
    }
});
