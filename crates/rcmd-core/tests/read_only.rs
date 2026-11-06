//! The engine itself refuses to write in read-only mode — the wall behind the UI.
//!
//! An integration test, so it runs in its own process: the policy is process-wide and
//! can only be installed once, which would leak into every other test otherwise.

use std::fs;
use std::time::Duration;

use rcmd_core::ops::{self, Event, Request};
use rcmd_core::policy::{self, Policy};

#[test]
fn nothing_writes_once_read_only_is_installed() {
    let d = std::env::temp_dir().join(format!("rcmd-ro-{}", std::process::id()));
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
    assert!(rcmd_core::rename::rename(&d.join("f.txt"), "g.txt").is_err());

    assert_eq!(fs::read(d.join("f.txt")).unwrap(), b"keep", "untouched");
    assert!(
        fs::read_dir(d.join("dst")).unwrap().next().is_none(),
        "nothing arrived"
    );
    let _ = fs::remove_dir_all(&d);
}
