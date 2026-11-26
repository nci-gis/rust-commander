//! A tree deeper than a thread's default stack can walk. Planning and copying walk a
//! tree one call per level; a stack overflow is an abort, not a panic, so it would
//! end the whole program. Its own test binary: an overflow here ends only it.
//! The 1,900-level tree is Linux only: its PATH_MAX (4096) leaves room for it; macOS'
//! (1024) does not. The limit's test runs everywhere.

use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use xdevcmd_core::ops::{self, Answer, Event, Kind, Request};
use xdevcmd_core::plan;

#[cfg(target_os = "linux")]
const LEVELS: usize = 1_900;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xdc-deep-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// `root/d/d/…/d/leaf`, `LEVELS` deep; one-letter names keep it under PATH_MAX.
#[cfg(target_os = "linux")]
fn deep(root: &Path) {
    let mut p = root.to_path_buf();
    for _ in 0..LEVELS {
        p.push("d");
    }
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("leaf"), "x").unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn a_deep_tree_is_planned_and_copied() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
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

/// Not run by `cargo test`: how much stack a level takes, to set
/// `depth::MAX_DEPTH` and `FALLBACK_DEPTH`. Walk threads get `XDC_STACK` bytes and no
/// limit; a tree `XDC_DEPTH` deep is planned (`XDC_WHAT=plan`) or copied (`copy`).
/// It passes if the walk got through, and aborts if its stack ran out: a script finds
/// the deepest that passes (see `xtask/src/depth.sh` in the round's notes).
#[test]
#[ignore]
fn measure_stack_per_level() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    let stack: usize = env("XDC_STACK").parse().expect("XDC_STACK");
    let levels: usize = env("XDC_DEPTH").parse().expect("XDC_DEPTH");
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            xdevcmd_core::depth::STACK_FOR_MEASURING.store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let _reset = Reset;
    xdevcmd_core::depth::STACK_FOR_MEASURING.store(stack, std::sync::atomic::Ordering::Relaxed);
    let d = tmp(&format!("measure-{levels}"));
    let mut p = d.join("s");
    for _ in 0..levels {
        p.push("d");
    }
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("leaf"), "x").unwrap();
    fs::create_dir_all(d.join("to")).unwrap();
    if env("XDC_WHAT") == "plan" {
        let h = plan::start(Kind::Copy, vec![d.join("s")], d.join("to"), None);
        loop {
            match h.rx.recv_timeout(Duration::from_secs(120)) {
                Ok(plan::Msg::Done(_)) => break,
                Ok(_) => {}
                Err(e) => panic!("{e:?}"),
            }
        }
    } else {
        let h = ops::start(Request::copy(vec![d.join("s")], d.join("to")));
        loop {
            match h.events.recv_timeout(Duration::from_secs(120)) {
                Ok(Event::Ask(_)) => h.answer(Answer::Abort),
                Ok(Event::Done(_)) => break,
                Ok(_) => {}
                Err(e) => panic!("{e:?}"),
            }
        }
    }
    let _ = fs::remove_dir_all(&d);
}

/// The tests here set walk threads' limit or stack for the whole process: one at a time.
static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// At the limit a walk stops going down and says why: the plan shows the folder as one
/// that cannot be done, the copy reports it, the rest is copied, and nothing ends the
/// program.
#[test]
fn a_tree_deeper_than_the_limit_is_said_and_the_rest_is_copied() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            xdevcmd_core::depth::LIMIT_FOR_TESTING.store(0, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let _reset = Reset;
    xdevcmd_core::depth::LIMIT_FOR_TESTING.store(50, std::sync::atomic::Ordering::Relaxed);
    let d = tmp("too-deep");
    let mut p = d.join("s").join("deep");
    for _ in 0..70 {
        p.push("d");
    }
    fs::create_dir_all(&p).unwrap();
    fs::write(p.join("leaf"), "x").unwrap();
    fs::write(d.join("s").join("shallow.txt"), "s").unwrap();
    fs::create_dir_all(d.join("to")).unwrap();

    let h = plan::start(Kind::Copy, vec![d.join("s")], d.join("to"), None);
    let plan = loop {
        match h.rx.recv_timeout(Duration::from_secs(60)) {
            Ok(plan::Msg::Done(p)) => break p,
            Ok(_) => {}
            Err(e) => panic!("the planner stopped: {e:?}"),
        }
    };
    assert!(
        plan.rows
            .iter()
            .any(|r| r.blocked == Some(plan::Blocked::TooDeep(50))),
        "a row says the folder is too deep"
    );

    let h = ops::start(Request::copy(vec![d.join("s")], d.join("to")));
    let sum = loop {
        match h.events.recv_timeout(Duration::from_secs(60)) {
            Ok(Event::Ask(_)) => h.answer(Answer::Skip),
            Ok(Event::Done(s)) => break s,
            Ok(_) => {}
            Err(e) => panic!("the copy stopped: {e:?}"),
        }
    };
    assert_eq!(sum.failed, 1, "{sum:?}");
    assert!(
        sum.errors
            .iter()
            .any(|e| e.contains("too deep: more than 50 levels")),
        "{sum:?}"
    );
    assert_eq!(fs::read_to_string(d.join("to/s/shallow.txt")).unwrap(), "s");
    let _ = fs::remove_dir_all(&d);
}

/// A compare also stops at the limit: one row that cannot be decided, nothing below it
/// planned, and no stack overflow. A plain thread has the fallback limit.
#[test]
fn a_compare_deeper_than_the_limit_is_one_unknown_row() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|p| p.into_inner());
    let levels = xdevcmd_core::depth::FALLBACK_DEPTH + 10;
    let d = tmp("compare-deep");
    for side in ["l", "r"] {
        let mut p = d.join(side);
        for _ in 0..levels {
            p.push("d");
        }
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join(format!("only-{side}")), "x").unwrap();
    }
    let rows = xdevcmd_core::compare::compare(&d.join("l"), &d.join("r"), true, true).unwrap();
    let deep: Vec<_> = rows
        .iter()
        .filter(|r| r.error.as_deref().is_some_and(|e| e.contains("too deep")))
        .collect();
    assert_eq!(deep.len(), 1, "{rows:?}");
    assert!(deep[0].state.unknown());
    assert!(
        !rows.iter().any(|r| r.rel.ends_with("only-l")),
        "nothing below is planned"
    );
    let _ = fs::remove_dir_all(&d);
}
