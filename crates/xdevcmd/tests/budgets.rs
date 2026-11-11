//! Budgets 0.4.0 promises (the scope proposal, §3), kept by tests where a test can.
//!
//! - The binary is at most 7 MB (the owner, 2026-11-11). Checked on the release build
//!   when one has been made (`cargo build --release`); skipped, and said so, when not.
//! - A 100,000-entry directory is listed and sorted in under a second. Ignored by
//!   default (making the files takes a while): `cargo test --test budgets -- --ignored`.

use std::path::PathBuf;
use std::time::Instant;

const MAX_BINARY: u64 = 7 * 1024 * 1024;

#[test]
fn the_release_binary_is_at_most_7_mb() {
    let name = if cfg!(windows) {
        "xdevcmd.exe"
    } else {
        "xdevcmd"
    };
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    let bin = target.join("release").join(name);
    let Ok(meta) = std::fs::metadata(&bin) else {
        eprintln!("no release build at {}: size not checked", bin.display());
        return;
    };
    let mb = meta.len() as f64 / 1024.0 / 1024.0;
    eprintln!("release binary: {mb:.2} MB");
    assert!(
        meta.len() <= MAX_BINARY,
        "{} is {mb:.2} MB, over the 7 MB budget",
        bin.display()
    );
}

#[test]
#[ignore = "makes 100,000 files; run by hand and before a release"]
fn a_100000_entry_directory_is_listed_and_sorted_in_under_a_second() {
    let dir = std::env::temp_dir().join(format!("xdevcmd-budget-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..100_000 {
        std::fs::write(dir.join(format!("file-{i:06}.txt")), b"").unwrap();
    }
    let start = Instant::now();
    let mut v = xdevcmd_core::read_dir(&dir, false).unwrap();
    xdevcmd_core::sort_entries(&mut v, xdevcmd_core::Sort::default());
    let took = start.elapsed();
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("100,000 entries listed and sorted in {took:?}");
    assert_eq!(v.len(), 100_001, "every file, and `..`");
    assert!(took.as_secs_f64() < 1.0, "over the 1 s budget: {took:?}");
}
