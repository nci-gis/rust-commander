//! The engine itself refuses to write in read-only mode — the wall behind the UI.
//!
//! An integration test, so it runs in its own process: the policy is process-wide and
//! can only be installed once, which would leak into every other test otherwise.

use std::fs;
use std::time::Duration;

use xdevcmd_core::ops::{self, Event, Request};
use xdevcmd_core::policy::{self, Policy};

#[test]
fn nothing_writes_once_read_only_is_installed() {
    let d = std::env::temp_dir().join(format!("xdevcmd-ro-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(d.join("dst")).unwrap();
    fs::write(d.join("f.txt"), b"keep").unwrap();

    policy::install(Policy {
        read_only: true,
        root: None,
    });
    // A second install cannot loosen it.
    policy::install(Policy::default());
    assert!(policy::read_only());

    let run = |req| {
        let h = ops::start(req);
        loop {
            match h.events.recv_timeout(Duration::from_secs(10)).unwrap() {
                Event::Done(s) => return s,
                _ => continue,
            }
        }
    };
    for req in [
        Request::copy(vec![d.join("f.txt")], d.join("dst")),
        Request::mv(vec![d.join("f.txt")], d.join("dst")),
        Request::delete(vec![d.join("f.txt")], false),
        Request::delete(vec![d.join("f.txt")], true),
    ] {
        let s = run(req);
        assert_eq!(s.done, 0);
        assert!(s.errors[0].contains("read-only"), "{:?}", s.errors);
    }
    assert!(xdevcmd_core::rename::rename(&d.join("f.txt"), "g.txt").is_err());
    // Save as… of a shell's history, new name and replacement alike.
    let saved = d.join("dst").join("history.txt");
    let e = xdevcmd_core::document::save_bytes(&saved, b"h", false, true).unwrap_err();
    assert!(e.to_string().contains("read-only"), "{e}");
    assert!(xdevcmd_core::document::save_bytes(&d.join("f.txt"), b"h", true, true).is_err());
    // A terminal tab's log: neither a new file nor appending to one that exists.
    let log = d.join("dst").join("tab.log");
    let e = xdevcmd_core::termlog::open(&log).unwrap_err();
    assert!(e.to_string().contains("read-only"), "{e}");
    assert!(!log.exists());
    assert!(xdevcmd_core::termlog::open(&d.join("f.txt")).is_err());
    assert_eq!(fs::read(d.join("f.txt")).unwrap(), b"keep");

    // A killed run's journal and its part file: tidying them is a write too.
    let jdir = d.join("journal");
    fs::create_dir_all(&jdir).unwrap();
    let part = d.join(".f.txt.xdevcmd-part-2147483646-0");
    fs::write(&part, b"half").unwrap();
    let journal = jdir.join("2147483646-0.journal");
    fs::write(&journal, format!("kind copy\npart {}\n", hex(&part))).unwrap();
    xdevcmd_core::journal::init(jdir.clone());
    assert!(xdevcmd_core::journal::recover().is_empty());
    assert!(
        journal.exists() && part.exists(),
        "left for a run that may write"
    );

    assert_eq!(fs::read(d.join("f.txt")).unwrap(), b"keep", "untouched");
    assert!(
        fs::read_dir(d.join("dst")).unwrap().next().is_none(),
        "nothing arrived"
    );
    let _ = fs::remove_dir_all(&d);
}

/// A path as a journal writes it: its bytes in hex (Unix), its UTF-16 units (Windows).
fn hex(p: &std::path::Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        p.as_os_str()
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        p.as_os_str()
            .encode_wide()
            .map(|u| format!("{u:04x}"))
            .collect()
    }
}
