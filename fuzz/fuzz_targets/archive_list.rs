//! Any bytes, as a .zip and as a .tar.gz (`archive::list`): a listing or an error, never
//! a panic, and every name in a listing is one `archive::clean` keeps.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let dir = std::env::temp_dir().join(format!("xdevcmd-fuzz-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    for ext in ["zip", "tar.gz", "tar"] {
        let p = dir.join(format!("input.{ext}"));
        if std::fs::write(&p, data).is_err() {
            return;
        }
        if let Ok(l) = xdevcmd_core::archive::list(&p) {
            for item in &l.items {
                assert_eq!(
                    xdevcmd_core::archive::clean(&item.path).as_deref(),
                    Some(item.path.as_str()),
                    "{:?}",
                    item.path
                );
            }
        }
    }
});
