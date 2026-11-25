//! A tree deeper than a thread's default stack can walk. Planning and copying walk a
//! tree one call per level; a stack overflow is an abort, not a panic, so it would
//! end the whole program. Its own test binary: an overflow here ends only it.
//! Linux only: its PATH_MAX (4096) leaves room for 1,900 levels; macOS' (1024) does not.
#![cfg(target_os = "linux")]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use xdevcmd_core::ops::{self, Answer, Event, Kind, Request};
use xdevcmd_core::plan;

const LEVELS: usize = 1_900;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xdc-deep-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// `root/d/d/…/d/leaf`, `LEVELS` deep; one-letter names keep it under PATH_MAX.
fn deep(root: &Path) {
    let mut p = root.to_path_buf();
    for _ in 0..LEVELS {
        p.push("d");
    }
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("leaf"), "x").unwrap();
}

#[test]
fn a_deep_tree_is_planned_and_copied() {
    let d = tmp("tree");
    deep(&d.join("s"));
    fs::create_dir_all(d.join("to")).unwrap();

    let h = plan::start(Kind::Copy, vec![d.join("s")], d.join("to"), None);
    let rows = loop {
        match h.rx.recv_timeout(Duration::from_secs(120)) {
            Ok(plan::Msg::Done(p)) => break p.rows.len(),
            Ok(_) => {}
            Err(e) => panic!("the planner stopped: {e:?}"),
        }
    };
    assert!(rows > LEVELS, "{rows} rows");

    let h = ops::start(Request::copy(vec![d.join("s")], d.join("to")));
    loop {
        match h.events.recv_timeout(Duration::from_secs(120)) {
            Ok(Event::Ask(_)) => h.answer(Answer::Abort),
            Ok(Event::Done(_)) => break,
            Ok(_) => {}
            Err(e) => panic!("the copy stopped: {e:?}"),
        }
    }
    let mut leaf = d.join("to").join("s");
    for _ in 0..LEVELS {
        leaf.push("d");
    }
    assert_eq!(fs::read_to_string(leaf.join("leaf")).unwrap(), "x");
    let _ = fs::remove_dir_all(&d);
}
