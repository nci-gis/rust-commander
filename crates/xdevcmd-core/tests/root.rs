//! The engine holds `--root` for paths it makes, restores or opens, which may not exist
//! yet: one check for all of them (plan v3 H-03, cosf #3 and E2).
//!
//! An integration test, so it runs in its own process: the policy is process-wide and
//! can only be installed once.

use std::fs;
use std::path::PathBuf;

use xdevcmd_core::policy::{self, Policy};
use xdevcmd_core::trashed::{self, Place, Trashed};

fn scratch() -> PathBuf {
    let d = std::env::temp_dir().join(format!("xdevcmd-root-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("in/trash")).unwrap();
    fs::create_dir_all(d.join("out")).unwrap();
    xdevcmd_core::paths::canonical(&d).unwrap()
}

#[test]
fn the_root_holds_for_paths_not_made_yet() {
    let d = scratch();
    policy::install(Policy {
        read_only: false,
        root: Some(d.join("in")),
    });

    // E2: a file put back from the trash goes to a name that is free by definition. It
    // was refused under --root whatever its place, as the free name did not resolve.
    fs::write(d.join("in/trash/a.txt"), "a").unwrap();
    let back = Trashed {
        original: d.join("in/a.txt"),
        when: std::time::SystemTime::now(),
        place: Place::At(d.join("in/trash/a.txt")),
    };
    trashed::restore(&back).expect("restored inside the root");
    assert_eq!(fs::read_to_string(d.join("in/a.txt")).unwrap(), "a");
    // Outside, it still is refused.
    fs::write(d.join("in/trash/b.txt"), "b").unwrap();
    let out = Trashed {
        original: d.join("out/b.txt"),
        when: std::time::SystemTime::now(),
        place: Place::At(d.join("in/trash/b.txt")),
    };
    assert!(trashed::restore(&out).is_err());
    assert!(!d.join("out/b.txt").exists());

    // cosf #3 (c): a terminal log onto a link left in the root that leads out of it.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(d.join("out/log.txt"), d.join("in/log.txt")).unwrap();
        let e = xdevcmd_core::termlog::open(&d.join("in/log.txt")).unwrap_err();
        assert!(e.to_string().contains("outside the root"), "{e}");
        assert!(!d.join("out/log.txt").exists());
    }

    // Undo of a file that is gone says so, not that it is outside.
    let e = xdevcmd_core::undo::remove(&d.join("in/gone.txt")).unwrap_err();
    assert!(!e.to_string().contains("outside the root"), "{e}");

    let _ = fs::remove_dir_all(&d);
}
