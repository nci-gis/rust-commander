use super::*;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

static N: AtomicU32 = AtomicU32::new(0);

fn tmp(tag: &str) -> PathBuf {
    let n = N.fetch_add(1, Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("xdevcmd-ops-{tag}-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

/// Drive a job to completion, answering every question the same way.
fn run_with(req: Request, answer: Answer) -> (Summary, u32) {
    let h = start(req);
    let mut asked = 0;
    loop {
        match h.events.recv_timeout(Duration::from_secs(20)) {
            Ok(Event::Ask(_)) => {
                asked += 1;
                h.answer(answer);
            }
            Ok(Event::Done(s)) => return (s, asked),
            Ok(_) => {}
            Err(e) => panic!("worker never finished: {e}"),
        }
    }
}

#[test]
fn a_count_can_be_stopped_and_otherwise_arrives() {
    let root = std::env::temp_dir().join(format!("xdevcmd-ops-count-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("a/b")).unwrap();
    fs::write(root.join("a/b/f"), "1234").unwrap();
    fs::write(root.join("g"), "56").unwrap();
    assert_eq!(
        scan_until(std::slice::from_ref(&root), &AtomicBool::new(true)),
        None
    );
    let c = count(vec![root.clone()]);
    let got =
        c.rx.recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    assert_eq!(got, (2, 6));
    let _ = fs::remove_dir_all(&root);
}

fn run(req: Request) -> Summary {
    run_with(req, Answer::Yes).0
}

/// A copy of one file `tag` from a fresh `src` into `to`, with `backoff`, its events
/// collected; questions answered `answer`, and the job cancelled at the first
/// `Retrying` when `cancel_at_retry`.
fn retry_job(
    name: &str,
    backoff: Vec<Duration>,
    answer: Answer,
    cancel_at_retry: bool,
) -> (Summary, Vec<Event>, PathBuf) {
    let d = tmp(name);
    fs::create_dir_all(d.join("src")).unwrap();
    fs::create_dir_all(d.join("to")).unwrap();
    fs::write(d.join("src").join(name), "payload").unwrap();
    let mut req = Request::copy(vec![d.join("src").join(name)], d.join("to"));
    req.backoff = backoff;
    let h = start(req);
    let mut seen = Vec::new();
    loop {
        let e = h.events.recv_timeout(Duration::from_secs(20)).unwrap();
        match &e {
            Event::Ask(_) => h.answer(answer),
            Event::Retrying { .. } if cancel_at_retry => h.cancel(),
            Event::Done(s) => return (s.clone(), seen, d.join("to").join(name)),
            _ => {}
        }
        seen.push(e);
    }
}

fn timed_out() -> crate::faults::Fault {
    #[cfg(unix)]
    let code = rustix::io::Errno::TIMEDOUT.raw_os_error();
    #[cfg(not(unix))]
    let code = 121; // ERROR_SEM_TIMEOUT
    crate::faults::Fault::Os(code)
}

/// The first `n` writes of `name` time out.
fn fail_first(name: &str, n: u32) -> Vec<crate::faults::Guard> {
    (1..=n)
        .map(|k| crate::faults::fail(crate::faults::Point::Write, name, k, timed_out()))
        .collect()
}

fn retries(seen: &[Event]) -> Vec<u32> {
    seen.iter()
        .filter_map(|e| match e {
            Event::Retrying { attempt, .. } => Some(*attempt),
            _ => None,
        })
        .collect()
}

/// Round 031: an error that passes (a timeout) is tried again, without a question,
/// and the copy completes.
#[test]
fn a_network_error_that_passes_is_tried_again_without_asking() {
    let name = format!("xnet-ok-{}", std::process::id());
    let _f = fail_first(&name, 2);
    let (s, seen, dst) = retry_job(&name, vec![Duration::ZERO; 5], Answer::Abort, false);
    assert_eq!(retries(&seen), [1, 2]);
    assert!(!seen.iter().any(|e| matches!(e, Event::Ask(_))), "{seen:?}");
    assert_eq!((s.done, s.failed), (1, 0), "{s:?}");
    assert_eq!(fs::read_to_string(dst).unwrap(), "payload");
}

/// After the last try the user is asked, told how many tries were made.
#[test]
fn a_network_error_that_stays_is_asked_about_after_the_tries() {
    let name = format!("xnet-stays-{}", std::process::id());
    let _f = fail_first(&name, 6);
    let (s, seen, dst) = retry_job(&name, vec![Duration::ZERO; 5], Answer::Skip, false);
    assert_eq!(retries(&seen), [1, 2, 3, 4, 5]);
    let asked: Vec<_> = seen
        .iter()
        .filter_map(|e| match e {
            Event::Ask(Question::Failed { error, .. }) => Some(error.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(asked.len(), 1, "{seen:?}");
    assert!(asked[0].contains("tried 6 times"), "{asked:?}");
    assert_eq!(s.skipped, 1);
    assert!(!dst.exists());
}

/// An I/O error on a local disk is the disk failing: asked at once, not retried.
#[cfg(unix)]
#[test]
fn an_io_error_on_a_local_disk_is_not_tried_again() {
    let name = format!("xnet-eio-{}", std::process::id());
    let _f = crate::faults::fail(
        crate::faults::Point::Write,
        &name,
        1,
        crate::faults::Fault::Os(rustix::io::Errno::IO.raw_os_error()),
    );
    let (_, seen, _) = retry_job(&name, vec![Duration::ZERO; 5], Answer::Skip, false);
    assert!(retries(&seen).is_empty(), "{seen:?}");
    assert!(transient(
        &io::Error::from_raw_os_error(rustix::io::Errno::IO.raw_os_error()),
        true
    ));
}

/// Esc while waiting for the next try ends the job at once, not after the wait.
#[test]
fn a_wait_before_trying_again_ends_at_once_on_cancel() {
    let name = format!("xnet-esc-{}", std::process::id());
    let _f = fail_first(&name, 1);
    let t = std::time::Instant::now();
    let (s, _, dst) = retry_job(&name, vec![Duration::from_secs(30); 5], Answer::Abort, true);
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert!(s.cancelled, "{s:?}");
    assert!(!dst.exists());
}

/// Round 031: with a trash on the volume, a replaced file goes there, after the
/// journal is on disk and before the rename; undo removes the new one and brings
/// the old one back.
/// #5: a file never replaces a folder, nor a folder a file — not with "replace all",
/// not with Yes to the question, not in a move. Skipped, and the summary says why.
/// #5, said every time (red-team): a folder that appears while the file is
/// written is kept, and the skip is counted and said, so the Job box stays open; an
/// archive's entry onto a folder is counted and said too.
#[test]
fn a_late_folder_and_an_archive_entry_are_skips_said_aloud() {
    journals();
    for safe in [true, false] {
        let d = tmp(&format!("late-folder-{safe}"));
        fs::create_dir_all(d.join("src")).unwrap();
        fs::create_dir_all(d.join("to")).unwrap();
        fs::write(d.join("src/a"), "new").unwrap();
        fs::write(d.join("to/a"), "old").unwrap();
        APPEAR_DIR.lock().unwrap().push(d.join("to/a"));
        let mut req = Request::copy(vec![d.join("src/a")], d.join("to"));
        req.trash_root = Some(d.join("trash"));
        req.safe = safe;
        req.existing = Existing::Replace;
        let (s, _) = run_with(req, Answer::Yes);
        let read = fs::read_to_string(d.join("to/a/inner/keep.txt")).ok();
        assert_eq!(read.as_deref(), Some("kept"), "safe={safe}: {s:?}");
        assert_eq!((s.skipped, s.kind_skips), (1, 1), "safe={safe}: {s:?}");
        assert!(s.describe(Kind::Copy).contains("never replaced"), "{s:?}");
        let _ = fs::remove_dir_all(&d);
    }
    let d = tmp("extract-over-folder");
    let zp = d.join("x.zip");
    {
        let f = File::create(&zp).unwrap();
        let mut z = zip::ZipWriter::new(f);
        z.start_file("a", zip::write::SimpleFileOptions::default())
            .unwrap();
        use std::io::Write;
        z.write_all(b"zipped").unwrap();
        z.finish().unwrap();
    }
    fs::create_dir_all(d.join("out/a")).unwrap();
    fs::write(d.join("out/a/keep.txt"), "kept").unwrap();
    let mut req = Request::extract(zp, vec!["a".into()], String::new(), d.join("out"));
    req.trash_root = Some(d.join("trash"));
    req.existing = Existing::Replace;
    let (s, _) = run_with(req, Answer::Yes);
    let read = fs::read_to_string(d.join("out/a/keep.txt")).ok();
    assert_eq!(read.as_deref(), Some("kept"), "{s:?}");
    assert!(
        s.describe(Kind::Extract).contains("never replaced"),
        "{}",
        s.describe(Kind::Extract)
    );
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn a_file_never_replaces_a_folder_nor_the_reverse() {
    journals();
    let d = tmp("file-over-dir");
    for (existing, answer, mv) in [
        (Existing::Replace, Answer::Yes, false),
        (Existing::Ask, Answer::Yes, false),
        (Existing::Ask, Answer::YesToAll, false),
        (Existing::Replace, Answer::Yes, true),
    ] {
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("src/b")).unwrap();
        fs::create_dir_all(d.join("to/a")).unwrap();
        fs::write(d.join("src/a"), "a file").unwrap();
        fs::write(d.join("src/b/in.txt"), "in").unwrap();
        fs::write(d.join("to/a/keep.txt"), "kept").unwrap();
        fs::write(d.join("to/b"), "a file there").unwrap();
        let sources = vec![d.join("src/a"), d.join("src/b")];
        let mut req = if mv {
            Request::mv(sources, d.join("to"))
        } else {
            Request::copy(sources, d.join("to"))
        };
        req.trash_root = Some(d.join("trash"));
        req.safe = true;
        req.existing = existing;
        let (s, _) = run_with(req, answer);
        let case = format!("{existing:?} {answer:?} move={mv}: {s:?}");
        assert_eq!(
            fs::read_to_string(d.join("to/a/keep.txt")).unwrap(),
            "kept",
            "{case}"
        );
        assert_eq!(
            fs::read_to_string(d.join("to/b")).unwrap(),
            "a file there",
            "{case}"
        );
        assert!(
            d.join("src/a").is_file() && d.join("src/b/in.txt").is_file(),
            "{case}"
        );
        assert!(s.skipped >= 2, "{case}");
        assert!(
            s.errors
                .iter()
                .filter(|e| e.contains("is a folder") || e.contains("is a file"))
                .count()
                >= 2,
            "{case}"
        );
    }
    let _ = fs::remove_dir_all(&d);
}

#[test]
fn a_replaced_file_goes_to_the_trash_first_and_undo_brings_it_back() {
    journals();
    let d = tmp("replace-trash");
    let tag = format!("xrt-{}", std::process::id());
    fs::create_dir_all(d.join("src")).unwrap();
    fs::create_dir_all(d.join("to")).unwrap();
    fs::write(d.join("src").join(&tag), "new contents").unwrap();
    fs::write(d.join("to").join(&tag), "old").unwrap();
    crate::faults::trace(&tag);
    let mut req = Request::copy(vec![d.join("src").join(&tag)], d.join("to"));
    req.trash_root = Some(d.join("trash"));
    req.safe = true;
    req.existing = Existing::Replace;
    let (s, _) = run_with(req, Answer::Yes);
    let seen = crate::faults::traced(&tag);
    let dst = d.join("to").join(&tag);
    assert_eq!(fs::read_to_string(&dst).unwrap(), "new contents");
    assert_eq!(
        (s.done, s.replaced_to_trash, s.replaced_for_good),
        (1, 1, 0),
        "{s:?}"
    );
    assert!(s
        .describe(Kind::Copy)
        .contains("1 replaced (the old one in the trash)"));
    let old = s.made[0].old.clone().expect("the old file is remembered");
    let crate::trashed::Place::Freedesktop { file, .. } = &old.place else {
        panic!("{old:?}")
    };
    assert_eq!(fs::read_to_string(file).unwrap(), "old");
    let at = |point: &str| {
        seen.iter()
            .rposition(|(p, path)| p.name() == point && path.to_string_lossy().contains(&tag))
            .unwrap_or_else(|| panic!("no {point} in {seen:?}"))
    };
    assert!(at("sync-data") < at("trash"), "{seen:?}");
    assert!(at("journal-sync") < at("trash"), "{seen:?}");
    assert!(at("trash") < at("rename"), "{seen:?}");

    let h = crate::undo::unmake(s.made.clone());
    let u = loop {
        if let Event::Done(u) = h
            .events
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
        {
            break u;
        }
    };
    assert_eq!((u.done, u.failed), (1, 0), "{u:?}");
    assert_eq!(
        fs::read_to_string(&dst).unwrap(),
        "old",
        "the old file is back"
    );
    let _ = fs::remove_dir_all(&d);
}

/// The rename failing after the old file went to the trash: the old file comes back
/// under its name, and the temporary file goes.
#[test]
fn a_failed_rename_after_the_trash_puts_the_old_file_back() {
    let d = tmp("replace-trash-fail");
    let tag = format!("xrtf-{}", std::process::id());
    fs::create_dir_all(d.join("src")).unwrap();
    fs::create_dir_all(d.join("to")).unwrap();
    fs::write(d.join("src").join(&tag), "new contents").unwrap();
    fs::write(d.join("to").join(&tag), "old").unwrap();
    let _f = crate::faults::fail(
        crate::faults::Point::Rename,
        &tag,
        1,
        crate::faults::Fault::Os(5),
    );
    let mut req = Request::copy(vec![d.join("src").join(&tag)], d.join("to"));
    req.trash_root = Some(d.join("trash"));
    req.existing = Existing::Replace;
    let (s, _) = run_with(req, Answer::Skip);
    assert_eq!(s.replaced_to_trash, 0, "{s:?}");
    assert_eq!(fs::read_to_string(d.join("to").join(&tag)).unwrap(), "old");
    let left: Vec<_> = fs::read_dir(d.join("to")).unwrap().flatten().collect();
    assert_eq!(left.len(), 1, "no temporary file left: {left:?}");
    let _ = fs::remove_dir_all(&d);
}

/// QA 2026-11-17 (P0): a move by rename over a file sends the old file to the trash;
/// the rename failing after that puts it back, and the source stays where it was.
#[test]
fn a_move_over_a_file_trashes_the_old_one_and_a_failed_rename_puts_it_back() {
    let d = tmp("move-over-trash");
    let tag = format!("xmot-{}", std::process::id());
    let setup = || {
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("src")).unwrap();
        fs::create_dir_all(d.join("to")).unwrap();
        fs::write(d.join("src").join(&tag), "new contents").unwrap();
        fs::write(d.join("to").join(&tag), "old").unwrap();
    };
    let trashed = || -> Vec<String> {
        fs::read_dir(d.join("trash/files"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| fs::read_to_string(e.path()).ok())
            .collect()
    };
    let req = || {
        let mut r = Request::mv(vec![d.join("src").join(&tag)], d.join("to"));
        r.trash_root = Some(d.join("trash"));
        r.existing = Existing::Replace;
        r
    };

    setup();
    let (s, _) = run_with(req(), Answer::Skip);
    assert_eq!((s.done, s.replaced_to_trash), (1, 1), "{s:?}");
    assert_eq!(
        fs::read_to_string(d.join("to").join(&tag)).unwrap(),
        "new contents"
    );
    assert!(!d.join("src").join(&tag).exists());
    assert_eq!(trashed(), vec!["old".to_string()]);

    setup();
    let _f = crate::faults::fail(
        crate::faults::Point::Rename,
        &tag,
        1,
        crate::faults::Fault::Os(5),
    );
    let (s, _) = run_with(req(), Answer::Skip);
    assert_eq!(s.replaced_to_trash, 0, "{s:?}");
    assert_eq!(fs::read_to_string(d.join("to").join(&tag)).unwrap(), "old");
    assert_eq!(
        fs::read_to_string(d.join("src").join(&tag)).unwrap(),
        "new contents"
    );
    assert!(trashed().is_empty(), "{:?}", trashed());
    let _ = fs::remove_dir_all(&d);
}

/// Round 031: a copy says what it made, a directory before what is in it, and not
/// what it merged into, replaced or skipped.
#[test]
fn a_copy_reports_what_it_made_in_order() {
    let d = tmp("made");
    fs::create_dir_all(d.join("src/sub")).unwrap();
    fs::write(d.join("src/sub/new.txt"), "n").unwrap();
    fs::write(d.join("src/sub/old.txt"), "new contents").unwrap();
    fs::write(d.join("src/sub/kept.txt"), "k").unwrap();
    fs::create_dir_all(d.join("to/src/sub")).unwrap();
    fs::write(d.join("to/src/sub/old.txt"), "old").unwrap();
    fs::write(d.join("to/src/sub/kept.txt"), "mine").unwrap();
    let mut req = Request::copy(vec![d.join("src")], d.join("to"));
    req.expect
        .insert(d.join("to/src/sub/kept.txt"), Expect::Skip);
    let sum = run(req);
    let made: Vec<_> = sum.made.iter().map(|m| m.path.clone()).collect();
    assert_eq!(made, [d.join("to/src/sub/new.txt")], "{sum:?}");
    assert_eq!(sum.replaced_for_good, 1);
    assert_eq!(
        sum.made[0].ident,
        Ident::at(&d.join("to/src/sub/new.txt")).unwrap()
    );

    let e = tmp("made-dirs");
    fs::create_dir_all(e.join("a/b")).unwrap();
    fs::write(e.join("a/b/f"), "f").unwrap();
    fs::create_dir_all(e.join("to")).unwrap();
    let sum = run(Request::copy(vec![e.join("a")], e.join("to")));
    let made: Vec<_> = sum.made.iter().map(|m| m.path.clone()).collect();
    assert_eq!(made, [e.join("to/a"), e.join("to/a/b"), e.join("to/a/b/f")]);
    assert!(sum.made[0].ident.is_dir());
}

/// A move remembers nothing of the kind: its undo moves back. It says where each
/// source went, for the lists that follow them.
#[test]
fn a_move_reports_nothing_made() {
    let d = tmp("made-move");
    fs::create_dir_all(d.join("to")).unwrap();
    fs::write(d.join("f"), "x").unwrap();
    let sum = run(Request::mv(
        vec![d.join("f"), d.join("missing")],
        d.join("to"),
    ));
    assert!(sum.made.is_empty() && !sum.made_over);
    assert_eq!(sum.relocated, [(d.join("f"), d.join("to/f"))]);
}

/// A folder moved onto a folder of the same name, on one drive: the two merge, as the
/// plan shows and as a move across drives does. Before (QA 2026-11-17; the pre-rc
/// review), the rename refused a folder that was not empty: "0 moved, 1 failed".
#[test]
fn a_folder_moved_onto_a_folder_merges_into_it() {
    let d = tmp("move-merge");
    fs::create_dir_all(d.join("src/A/deep")).unwrap();
    fs::create_dir_all(d.join("to/A/deep")).unwrap();
    fs::write(d.join("src/A/x"), "x").unwrap();
    fs::write(d.join("src/A/deep/z"), "z").unwrap();
    fs::write(d.join("to/A/y"), "y").unwrap();
    fs::write(d.join("to/A/deep/w"), "w").unwrap();
    let (sum, asked) = run_with(
        Request::mv(vec![d.join("src/A")], d.join("to")),
        Answer::Yes,
    );
    assert_eq!((sum.failed, asked), (0, 0), "{sum:?}");
    for (f, text) in [("x", "x"), ("y", "y"), ("deep/z", "z"), ("deep/w", "w")] {
        assert_eq!(
            fs::read_to_string(d.join("to/A").join(f)).unwrap(),
            text,
            "{f}"
        );
    }
    assert!(!d.join("src/A").exists(), "the emptied source folder goes");
    assert_eq!(sum.done, 2, "{sum:?}");
}

/// What a merge skips stays, and so does the folder that holds it.
#[test]
fn a_merge_keeps_the_source_folder_of_what_it_skipped() {
    let d = tmp("move-merge-skip");
    fs::create_dir_all(d.join("src/A")).unwrap();
    fs::create_dir_all(d.join("to/A")).unwrap();
    fs::write(d.join("src/A/x"), "new").unwrap();
    fs::write(d.join("src/A/only"), "o").unwrap();
    fs::write(d.join("to/A/x"), "old").unwrap();
    let (sum, _) = run_with(
        Request::mv(vec![d.join("src/A")], d.join("to")),
        Answer::Skip,
    );
    assert_eq!(fs::read_to_string(d.join("to/A/x")).unwrap(), "old");
    assert_eq!(fs::read_to_string(d.join("src/A/x")).unwrap(), "new");
    assert_eq!(fs::read_to_string(d.join("to/A/only")).unwrap(), "o");
    assert_eq!((sum.done, sum.skipped, sum.failed), (1, 1, 0), "{sum:?}");
}

#[test]
fn a_planned_job_does_not_walk_its_sources_again() {
    let d = tmp("planned");
    fs::create_dir_all(d.join("to")).unwrap();
    fs::write(d.join("f"), "x").unwrap();
    let mut req = Request::copy(vec![d.join("f")], d.join("to"));
    // Numbers no walk of this one file would give: they can only be the plan's.
    req.planned = Some((7, 70));
    let h = start(req);
    let scanned = loop {
        let e = h
            .events
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        if let Event::Scanned { files, bytes } = e {
            break (files, bytes);
        }
    };
    assert_eq!(scanned, (7, 70));
}

#[test]
fn copies_a_tree_and_leaves_the_source_alone() {
    let root = tmp("copy-tree");
    let src = root.join("src");
    let dst = root.join("dst");
    fs::create_dir_all(src.join("nested")).unwrap();
    fs::create_dir_all(&dst).unwrap();
    fs::write(src.join("a.txt"), b"hello").unwrap();
    fs::write(src.join("nested/b.txt"), b"world").unwrap();

    let s = run(Request::copy(vec![src.clone()], dst.clone()));
    assert_eq!(s.failed, 0, "{:?}", s.errors);

    assert_eq!(fs::read(dst.join("src/a.txt")).unwrap(), b"hello");
    assert_eq!(fs::read(dst.join("src/nested/b.txt")).unwrap(), b"world");
    assert!(src.join("a.txt").exists(), "copy must not move");

    fs::remove_dir_all(&root).unwrap();
}

/// Item 22: a file of several kernel steps arrives whole by the platform's fast copy
/// (a clone, or the kernel's copy), which the summary names; off, xdevcmd reads and
/// writes, and says nothing about an engine.
#[test]
fn the_fast_copy_brings_the_file_whole_and_is_named_and_off_is_the_own_loop() {
    let root = tmp("fast-copy");
    let dst_on = root.join("on");
    let dst_off = root.join("off");
    fs::create_dir_all(&dst_on).unwrap();
    fs::create_dir_all(&dst_off).unwrap();
    // Not a multiple of a step, and not the same byte everywhere.
    let body: Vec<u8> = (0..(9 << 20) + 12_345u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
        .collect();
    let src = root.join("big.bin");
    fs::write(&src, &body).unwrap();

    let mut req = Request::copy(vec![src.clone()], dst_on.clone());
    req.fast = true;
    let s = run(req);
    assert_eq!((s.done, s.failed), (1, 0), "{:?}", s.errors);
    assert!(fs::read(dst_on.join("big.bin")).unwrap() == body);
    let fast = s.cloned + s.by_kernel;
    if cfg!(any(target_os = "linux", target_vendor = "apple")) {
        assert_eq!(fast, 1, "{s:?}");
        let said = s.describe(Kind::Copy);
        assert!(
            said == "1 copied (cloned)" || said == "1 copied (by the kernel)",
            "{said}"
        );
    } else {
        assert_eq!(fast, 0, "{s:?}");
    }

    let mut req = Request::copy(vec![src.clone()], dst_off.clone());
    req.fast = false;
    let s = run(req);
    assert_eq!((s.done, s.cloned, s.by_kernel), (1, 0, 0), "{s:?}");
    assert!(fs::read(dst_off.join("big.bin")).unwrap() == body);
    assert_eq!(s.describe(Kind::Copy), "1 copied");
    let _ = fs::remove_dir_all(&root);

    // Engines mixed in one job are counted.
    let mixed = Summary {
        done: 3,
        cloned: 2,
        by_kernel: 1,
        ..Summary::default()
    };
    assert_eq!(
        mixed.describe(Kind::Copy),
        "3 copied (2 cloned, 1 by the kernel)"
    );
}

/// A file the kernel will not copy (a /proc file says it is empty, and
/// `copy_file_range` copies nothing) is read and written by xdevcmd instead.
#[cfg(target_os = "linux")]
#[test]
fn what_the_kernel_will_not_copy_is_read_and_written_instead() {
    let root = tmp("fast-refused");
    fs::create_dir_all(&root).unwrap();
    let mut req = Request::copy(vec![PathBuf::from("/proc/self/mounts")], root.clone());
    req.fast = true;
    let s = run(req);
    assert_eq!((s.done, s.failed), (1, 0), "{:?}", s.errors);
    assert_eq!((s.cloned, s.by_kernel), (0, 0), "{s:?}");
    assert!(!fs::read(root.join("mounts")).unwrap().is_empty());
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn a_copy_keeps_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt;
    let root = tmp("copy-mode");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let script = root.join("run.sh");
    fs::write(&script, b"#!/bin/sh\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    run(Request::copy(vec![script], dst.clone()));

    let mode = fs::metadata(dst.join("run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(
        mode & 0o111,
        0o111,
        "a copy that drops +x has broken the file"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn an_existing_destination_is_never_overwritten_without_asking() {
    let root = tmp("conflict");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("f.txt"), b"new").unwrap();
    fs::write(dst.join("f.txt"), b"old").unwrap();

    let (s, asked) = run_with(
        Request::copy(vec![root.join("f.txt")], dst.clone()),
        Answer::Skip,
    );
    assert_eq!(asked, 1, "it must ask before touching an existing file");
    assert_eq!(s.skipped, 1);
    assert_eq!(
        fs::read(dst.join("f.txt")).unwrap(),
        b"old",
        "skip must keep the original"
    );

    let (s, asked) = run_with(
        Request::copy(vec![root.join("f.txt")], dst.clone()),
        Answer::Yes,
    );
    assert_eq!(asked, 1);
    assert_eq!(s.done, 1);
    assert_eq!(fs::read(dst.join("f.txt")).unwrap(), b"new");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_file_that_appears_while_copying_or_moving_is_asked_about_not_overwritten() {
    let root = tmp("appears");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("c.txt"), b"ours").unwrap();
    fs::write(root.join("m.txt"), b"ours").unwrap();

    // dst/c.txt is free when checked, and taken just before the copy writes it.
    APPEAR.lock().unwrap().push(dst.join("c.txt"));
    let (s, asked) = run_with(
        Request::copy(vec![root.join("c.txt")], dst.clone()),
        Answer::Skip,
    );
    assert_eq!(asked, 1, "the copy asks about the file that appeared");
    assert_eq!(s.skipped, 1);
    assert_eq!(fs::read(dst.join("c.txt")).unwrap(), b"theirs");

    // The same for a move: the source stays, the newcomer is kept.
    APPEAR.lock().unwrap().push(dst.join("m.txt"));
    let (s, asked) = run_with(
        Request::mv(vec![root.join("m.txt")], dst.clone()),
        Answer::Skip,
    );
    assert_eq!(asked, 1, "the move asks about the file that appeared");
    assert_eq!(s.skipped, 1);
    assert_eq!(fs::read(dst.join("m.txt")).unwrap(), b"theirs");
    assert_eq!(fs::read(root.join("m.txt")).unwrap(), b"ours");

    // Answered Yes, it replaces it, as for any file that was there.
    APPEAR.lock().unwrap().push(dst.join("m2.txt"));
    fs::write(root.join("m2.txt"), b"ours").unwrap();
    let (s, _) = run_with(
        Request::mv(vec![root.join("m2.txt")], dst.clone()),
        Answer::Yes,
    );
    assert_eq!(s.done, 1);
    assert_eq!(fs::read(dst.join("m2.txt")).unwrap(), b"ours");

    fs::remove_dir_all(&root).unwrap();
}

/// A directory on another file system than the temporary directory, for moves
/// across devices. None, and said, where there is no such place.
fn other_device(tag: &str) -> Option<PathBuf> {
    crate::faults::places::other_device(tag)
}

/// Run `req`, answering each question with `answer(question)`; the summary and
/// the questions asked.
fn run_answering(req: Request, answer: impl Fn(&Question) -> Answer) -> (Summary, Vec<String>) {
    let h = start(req);
    let mut asked = Vec::new();
    loop {
        match h.events.recv_timeout(Duration::from_secs(20)) {
            Ok(Event::Ask(q)) => {
                asked.push(format!("{q:?}"));
                h.answer(answer(&q));
            }
            Ok(Event::Done(s)) => return (s, asked),
            Ok(_) => {}
            Err(e) => panic!("worker never finished: {e}"),
        }
    }
}

#[test]
fn a_move_across_devices_keeps_what_was_skipped() {
    let Some(other) = other_device("skip") else {
        return;
    };
    let root = tmp("xdev-skip");
    let src = root.join("d");
    fs::create_dir_all(src.join("sub")).unwrap();
    fs::write(src.join("a"), b"A").unwrap();
    fs::write(src.join("b"), b"only copy of b").unwrap();
    fs::write(src.join("sub/c"), b"C").unwrap();
    fs::create_dir_all(other.join("d")).unwrap();
    fs::write(other.join("d/b"), b"theirs").unwrap();
    // Merge into the existing d, copy-then-delete across devices, but skip b.
    let (s, asked) = run_answering(Request::mv(vec![src.clone()], other.clone()), |q| match q {
        Question::Conflict { path, .. } if path.ends_with("b") => Answer::Skip,
        _ => Answer::Yes,
    });
    assert_eq!(s.skipped, 1, "{asked:#?} {s:?}");
    assert_eq!(
        fs::read(src.join("b")).unwrap(),
        b"only copy of b",
        "b stays: it was not copied"
    );
    assert_eq!(fs::read(other.join("d/b")).unwrap(), b"theirs");
    assert!(!src.join("a").exists() && fs::read(other.join("d/a")).unwrap() == b"A");
    assert!(!src.join("sub").exists(), "a directory moved in full goes");
    assert_eq!(fs::read(other.join("d/sub/c")).unwrap(), b"C");
    assert!(src.is_dir(), "d stays, since b is still in it");
    fs::remove_dir_all(&other).unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_file_moved_across_devices_onto_another_is_asked_about_once() {
    let Some(other) = other_device("once") else {
        return;
    };
    let root = tmp("xdev-once");
    fs::write(root.join("f"), b"ours").unwrap();
    fs::write(other.join("f"), b"theirs").unwrap();
    let (s, asked) = run_answering(Request::mv(vec![root.join("f")], other.clone()), |_| {
        Answer::Yes
    });
    let conflicts = asked.iter().filter(|q| q.starts_with("Conflict")).count();
    assert_eq!(conflicts, 1, "{asked:?}");
    assert_eq!(s.done, 1);
    assert_eq!(fs::read(other.join("f")).unwrap(), b"ours");
    assert!(!root.join("f").exists());
    // Skipped at the one question: nothing moves, nothing is lost.
    fs::write(root.join("g"), b"ours").unwrap();
    fs::write(other.join("g"), b"theirs").unwrap();
    let (s, _) = run_answering(
        Request::mv(vec![root.join("g")], other.clone()),
        |q| match q {
            Question::Conflict { .. } => Answer::Skip,
            _ => Answer::Yes,
        },
    );
    assert_eq!(s.skipped, 1);
    assert_eq!(fs::read(root.join("g")).unwrap(), b"ours");
    assert_eq!(fs::read(other.join("g")).unwrap(), b"theirs");
    fs::remove_dir_all(&other).unwrap();
    fs::remove_dir_all(&root).unwrap();
}

/// A real "disk full" from the kernel (CI's small file system): the replacement fails,
/// the old file is kept, and nothing is left under a temporary name.
#[test]
fn a_replacement_that_fills_the_disk_keeps_the_old_file() {
    let Some(small) = crate::faults::places::small_fs("full-replace") else {
        return;
    };
    let root = tmp("full-replace");
    // Sparse: bigger than the whole small file system, free to make here.
    let big = File::create(root.join("f")).unwrap();
    big.set_len(64 << 20).unwrap();
    drop(big);
    fs::write(small.join("f"), b"old").unwrap();
    let (s, asked) = run_answering(
        Request::copy(vec![root.join("f")], small.clone()),
        |q| match q {
            Question::Failed { .. } => Answer::Skip,
            _ => Answer::Yes,
        },
    );
    assert!(
        asked.iter().any(|q| q.starts_with("Failed")),
        "the full disk is asked about: {asked:?}"
    );
    assert_eq!((s.done, s.failed), (0, 1), "{s:?}");
    assert_eq!(fs::read(small.join("f")).unwrap(), b"old");
    assert_eq!(
        fs::read_dir(&small).unwrap().count(),
        1,
        "no temporary file left"
    );
    fs::remove_dir_all(&small).unwrap();
    fs::remove_dir_all(&root).unwrap();
}

/// Journals for these tests, in a directory of their own (set once per test run).
fn journals() {
    crate::journal::init(
        std::env::temp_dir().join(format!("xdevcmd-test-journals-{}", std::process::id())),
    );
}

/// A big file is synced at each checkpoint while it is copied (Round 033); one no
/// bigger than a checkpoint is not (new files stay fast).
#[test]
fn a_big_file_is_synced_at_its_checkpoints_and_a_small_one_is_not() {
    journals();
    let root = tmp("ckpt-sync");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("ck-big.bin"), vec![1u8; 300 << 10]).unwrap();
    fs::write(root.join("ck-small.bin"), vec![1u8; 100 << 10]).unwrap();
    let _f = crate::faults::fail(
        crate::faults::Point::SyncData,
        ".ck-",
        1,
        crate::faults::Fault::Os(5),
    );
    let mut req = Request::copy(vec![root.join("ck-small.bin")], dst.clone());
    req.safe = true;
    req.checkpoint = 128 << 10;
    // The copy's own loop: a clone (APFS) brings the file whole, with no checkpoint.
    req.fast = false;
    let s = run(req.clone());
    assert_eq!((s.done, s.failed), (1, 0), "no checkpoint, no sync: {s:?}");
    req.sources = vec![root.join("ck-big.bin")];
    req.backoff = Vec::new();
    let (s, _) = run_answering(req, |_| Answer::Skip);
    assert_eq!(s.done, 0, "the checkpoint's sync failed: {s:?}");
    assert!(!dst.join("ck-big.bin").exists());
    fs::remove_dir_all(&root).unwrap();
}

/// A folder is told to the journal as finished only when nothing in it failed: a
/// file's error, or a failed sync in the batch its last files were in.
#[test]
fn only_a_folder_without_a_failure_is_told_finished() {
    journals();
    let root = tmp("told");
    let (src, dst) = (root.join("src"), root.join("dst"));
    for d in ["good", "bad", "batch"] {
        fs::create_dir_all(src.join(d)).unwrap();
        fs::create_dir_all(dst.join("src").join(d)).unwrap();
    }
    fs::write(src.join("good/g.txt"), "g").unwrap();
    fs::write(src.join("bad/told-bad.txt"), "b").unwrap();
    // A replacement: batched, and its batch's sync fails.
    fs::write(src.join("batch/told-batch.txt"), "new").unwrap();
    fs::write(dst.join("src/batch/told-batch.txt"), "old").unwrap();
    let _w = crate::faults::fail(
        crate::faults::Point::Write,
        ".told-bad.txt",
        1,
        crate::faults::Fault::Os(5),
    );
    let _s = crate::faults::fail(
        crate::faults::Point::SyncData,
        ".told-batch.txt",
        1,
        crate::faults::Fault::Os(5),
    );
    // `good` first, on its own: copied inside `src`, it may finish while the failing
    // batch is pending (the order of a listing is the file system's), and waits for
    // it, and is rightly not told. Then all of `src`, `good` again among the rest.
    let mut req = Request::copy(vec![src.join("good"), src.clone()], dst.clone());
    req.safe = true;
    req.existing = Existing::Replace;
    req.backoff = Vec::new();
    let (s, _) = run_answering(req, |_| Answer::Skip);
    assert!(s.failed >= 2, "{s:?}");
    let told: Vec<PathBuf> = TOLD
        .lock()
        .unwrap()
        .iter()
        .filter(|p| p.starts_with(&root))
        .cloned()
        .collect();
    assert!(told.contains(&src.join("good")), "{told:?}");
    assert!(!told.contains(&src.join("bad")), "a file failed: {told:?}");
    assert!(
        !told.contains(&src.join("batch")),
        "its batch failed: {told:?}"
    );
    assert!(!told.contains(&src), "something under it failed: {told:?}");
    fs::remove_dir_all(&root).unwrap();
}

/// A job from a confirmed plan leaves an identical file it has no row for (past the
/// plan's cap) without asking; one that differs is still asked about.
#[test]
fn a_planned_job_leaves_identical_files_without_a_row() {
    let root = tmp("planned-same");
    let (src, dst) = (root.join("src"), root.join("dst"));
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    let when = std::time::SystemTime::now();
    for dir in [&src, &dst] {
        let p = dir.join("same.txt");
        fs::write(&p, "same").unwrap();
        let f = File::options().write(true).open(&p).unwrap();
        f.set_times(FileTimes::new().set_modified(when)).unwrap();
    }
    fs::write(src.join("differs.txt"), "new").unwrap();
    fs::write(dst.join("differs.txt"), "old!").unwrap();
    let mut req = Request::copy(
        vec![src.join("same.txt"), src.join("differs.txt")],
        dst.clone(),
    );
    req.planned = Some((2, 7));
    let (s, asked) = run_answering(req, |_| Answer::Skip);
    assert_eq!(asked.len(), 1, "only the one that differs: {asked:?}");
    assert_eq!(s.skipped, 2);
    fs::remove_dir_all(&root).unwrap();
}

/// Resume does not walk a folder the run before finished while its copy is there
/// (a file added to it since is not copied, as the job's own snapshot); one whose
/// copy is gone is copied again.
#[test]
fn resume_does_not_walk_a_folder_finished_before() {
    journals();
    let root = tmp("done-dirs");
    let (src, dst) = (root.join("src"), root.join("dst"));
    fs::create_dir_all(src.join("finished")).unwrap();
    fs::create_dir_all(src.join("open")).unwrap();
    fs::write(src.join("finished").join("added-since.txt"), "new").unwrap();
    fs::write(src.join("open").join("o.txt"), "o").unwrap();
    fs::create_dir_all(dst.join("src").join("finished")).unwrap();
    let mut req = Request::copy(vec![src.clone()], dst.clone());
    req.safe = true;
    req.existing = Existing::Resume;
    req.done_dirs = [src.join("finished")].into_iter().collect();
    let s = run(req.clone());
    assert_eq!(s.failed, 0, "{s:?}");
    assert!(
        !dst.join("src/finished/added-since.txt").exists(),
        "not walked"
    );
    assert_eq!(fs::read_to_string(dst.join("src/open/o.txt")).unwrap(), "o");
    fs::remove_dir_all(dst.join("src").join("finished")).unwrap();
    let s = run(req);
    assert_eq!(s.failed, 0, "{s:?}");
    assert!(
        dst.join("src/finished/added-since.txt").exists(),
        "its copy was gone: copied again"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// B2: after a power cut a new file can keep its size and date with no data in it
/// (zeros, as XFS leaves it). A plain resume trusts it, in a finished folder and
/// out of one; a checked resume reads both sides and copies again only what differs.
#[test]
fn a_checked_resume_copies_again_a_file_with_the_right_size_and_wrong_bytes() {
    journals();
    let root = tmp("verify");
    let (src, dst) = (root.join("src"), root.join("dst"));
    fs::create_dir_all(src.join("finished")).unwrap();
    fs::write(src.join("finished/torn.txt"), "hello world").unwrap();
    fs::write(src.join("finished/good.txt"), "fine").unwrap();
    fs::write(src.join("loose.txt"), "abc").unwrap();
    let out = dst.join("src");
    fs::create_dir_all(out.join("finished")).unwrap();
    // The copies as a power cut left them: same size and date as the source.
    let lay = |rel: &str, body: &[u8]| {
        let to = out.join(rel);
        fs::write(&to, body).unwrap();
        let mtime = fs::metadata(src.join(rel)).unwrap().modified().unwrap();
        File::options()
            .write(true)
            .open(&to)
            .unwrap()
            .set_modified(mtime)
            .unwrap();
    };
    lay("finished/torn.txt", &[0; 11]);
    lay("finished/good.txt", b"fine");
    lay("loose.txt", &[0; 3]);
    let mut req = Request::copy(vec![src.clone()], dst.clone());
    req.safe = true;
    req.existing = Existing::Resume;
    req.done_dirs = [src.join("finished")].into_iter().collect();

    // Trusted (Enter): nothing read, the zeros stay.
    let s = run(req.clone());
    assert_eq!((s.failed, s.repaired), (0, 0), "{s:?}");
    assert_eq!(fs::read(out.join("finished/torn.txt")).unwrap(), [0; 11]);
    assert_eq!(fs::read(out.join("loose.txt")).unwrap(), [0; 3]);

    // Checked: the two with other bytes are copied again; the good one is left.
    req.verify = true;
    let s = run(req);
    assert_eq!((s.failed, s.repaired), (0, 2), "{s:?}");
    assert_eq!(
        fs::read_to_string(out.join("finished/torn.txt")).unwrap(),
        "hello world"
    );
    assert_eq!(fs::read_to_string(out.join("loose.txt")).unwrap(), "abc");
    assert_eq!(
        fs::read_to_string(out.join("finished/good.txt")).unwrap(),
        "fine"
    );
    assert!(
        s.describe(Kind::Copy)
            .starts_with("2 found different and copied again; "),
        "{}",
        s.describe(Kind::Copy)
    );
    fs::remove_dir_all(&root).unwrap();
}

/// A checked resume of a move: a copy with other bytes is replaced by its source,
/// which then goes, as the move asked; a trusted one leaves both.
#[test]
fn a_checked_resume_of_a_move_replaces_a_torn_copy_with_its_source() {
    journals();
    let root = tmp("verify-move");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let src = root.join("m.txt");
    fs::write(&src, "moved").unwrap();
    let to = dst.join("m.txt");
    fs::write(&to, [0; 5]).unwrap();
    let mtime = fs::metadata(&src).unwrap().modified().unwrap();
    File::options()
        .write(true)
        .open(&to)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    let mut req = Request::mv(vec![src.clone()], dst.clone());
    req.safe = true;
    req.existing = Existing::Resume;
    let s = run(req.clone());
    assert_eq!((s.failed, s.repaired), (0, 0), "{s:?}");
    assert!(src.exists(), "trusted: both kept");
    req.verify = true;
    let s = run(req);
    assert_eq!((s.failed, s.repaired), (0, 1), "{s:?}");
    assert_eq!(fs::read_to_string(&to).unwrap(), "moved");
    assert!(!src.exists(), "the source went, as the move asked");
    fs::remove_dir_all(&root).unwrap();
}

/// Checked is only for a resume: on any other job the flag changes nothing (a
/// file already there is asked about as always).
/// C compares regular files only. A link is checked by its target alone: opening a
/// link to a FIFO blocked the job before its first read, where Stop is looked at
/// (red-team F2). A cancelled check repairs nothing, so it counts none (F4).
#[cfg(unix)]
#[test]
fn a_checked_resume_reads_regular_files_only() {
    journals();
    let root = tmp("verify-fifo");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let fifo = root.join("pipe");
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let src = root.join("l");
    std::os::unix::fs::symlink(&fifo, &src).unwrap();
    std::os::unix::fs::symlink(&fifo, dst.join("l")).unwrap();
    let mut req = Request::copy(vec![src], dst);
    req.safe = true;
    req.existing = Existing::Resume;
    req.verify = true;
    let h = start(req);
    let mut done = None;
    let t = std::time::Instant::now();
    while done.is_none() && t.elapsed() < Duration::from_secs(5) {
        if let Ok(Event::Done(s)) = h.events.recv_timeout(Duration::from_millis(100)) {
            done = Some(s);
        }
    }
    if done.is_none() {
        h.cancel();
        // A writer frees the blocked open, so the worker can end.
        let _ = fs::OpenOptions::new().write(true).open(&fifo);
    }
    let s = done.expect("the check of a link to a FIFO hung");
    assert_eq!((s.skipped, s.repaired), (1, 0), "{s:?}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn verify_outside_a_resume_asks_as_before() {
    let root = tmp("verify-ask");
    let (src, dst) = (root.join("a.txt"), root.join("dst"));
    fs::create_dir_all(&dst).unwrap();
    fs::write(&src, "new").unwrap();
    fs::write(dst.join("a.txt"), "old").unwrap();
    let mut req = Request::copy(vec![src], dst.clone());
    req.verify = true;
    let (s, asked) = run_with(req, Answer::Skip);
    assert_eq!((asked, s.repaired), (1, 0), "{s:?}");
    assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "old");
    fs::remove_dir_all(&root).unwrap();
}

/// Repo review RR01: C (a checked continue) read every finished copy again, but took
/// over a big file's kept part on trust: a part damaged after its checkpoint (`XXXX`
/// where the source has other bytes) was finished as a good copy. Under C the kept
/// bytes are compared with the source's: a part that differs goes, and the copy
/// starts again; one that matches is still taken over. Enter (not checked) is
/// unchanged.
#[test]
fn a_checked_resume_reads_a_kept_part_again_with_its_source() {
    journals();
    let root = tmp("ckpt-checked");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let src = root.join("big.bin");
    let body: Vec<u8> = (0..(200u32 << 10)).map(|i| (i % 251) as u8).collect();
    fs::write(&src, &body).unwrap();
    let part = dst.join(".big.bin.xdevcmd-part-2147483646-0");
    let kept = 64 << 10;
    let req = |part_bytes: &[u8]| {
        fs::write(&part, part_bytes).unwrap();
        let mut r = Request::copy(vec![src.clone()], dst.clone());
        r.safe = true;
        r.existing = Existing::Resume;
        r.verify = true;
        r.checkpoint = 32 << 10;
        r.partial = vec![crate::journal::Partial {
            part: part.clone(),
            src: src.clone(),
            dst: dst.join("big.bin"),
            ident: Ident::at(&src).unwrap(),
            bytes: kept as u64,
        }];
        r
    };
    // Damaged: the start written over after its checkpoint.
    let mut damaged = body[..kept].to_vec();
    damaged[..4].copy_from_slice(b"XXXX");
    let s = run(req(&damaged));
    assert_eq!((s.done, s.failed), (1, 0), "{s:?}");
    assert!(
        fs::read(dst.join("big.bin")).unwrap() == body,
        "copied again"
    );
    assert!(!part.exists());
    // As it should be: still taken over, and the copy is whole.
    fs::remove_file(dst.join("big.bin")).unwrap();
    let s = run(req(&body[..kept]));
    assert_eq!((s.done, s.failed), (1, 0), "{s:?}");
    assert!(fs::read(dst.join("big.bin")).unwrap() == body);
    assert!(!part.exists());
    fs::remove_dir_all(&root).unwrap();
}

/// C's look at a kept part reaches its last byte when the part ends on the read
/// buffer's edge and one past it (red-team on RR01): damage there is copied again.
#[test]
fn a_checked_resume_sees_damage_at_the_last_byte_of_a_kept_part() {
    journals();
    let root = tmp("ckpt-edge");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let src = root.join("big.bin");
    let body: Vec<u8> = (0..(3u32 << 20)).map(|i| (i % 251) as u8).collect();
    fs::write(&src, &body).unwrap();
    let part = dst.join(".big.bin.xdevcmd-part-2147483646-0");
    for kept in [1usize << 20, (1 << 20) + 1, 2 << 20] {
        let mut damaged = body[..kept].to_vec();
        damaged[kept - 1] ^= 0xFF;
        fs::write(&part, &damaged).unwrap();
        let mut r = Request::copy(vec![src.clone()], dst.clone());
        r.safe = true;
        r.existing = Existing::Resume;
        r.verify = true;
        r.checkpoint = 1 << 20;
        r.partial = vec![crate::journal::Partial {
            part: part.clone(),
            src: src.clone(),
            dst: dst.join("big.bin"),
            ident: Ident::at(&src).unwrap(),
            bytes: kept as u64,
        }];
        let s = run(r);
        assert_eq!((s.done, s.failed), (1, 0), "{kept}: {s:?}");
        assert!(fs::read(dst.join("big.bin")).unwrap() == body, "{kept}");
        assert!(!part.exists(), "{kept}");
        fs::remove_file(dst.join("big.bin")).unwrap();
    }
    fs::remove_dir_all(&root).unwrap();
}

/// A resumed job writes the plan's decisions to its own journal before the old one
/// goes, so a second kill keeps them too (repo review RR05: only a job with a plan
/// wrote them, and a resume has none, so the choices were lost the second time).
#[test]
fn a_resumed_job_carries_the_plans_decisions_into_its_own_journal() {
    journals();
    let root = tmp("resume-decided");
    let tag = format!("xrd-{}", std::process::id());
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let src = root.join(&tag);
    fs::write(&src, "new").unwrap();
    fs::write(dst.join(&tag), "old, left by the plan").unwrap();
    let old_journal = root.join("old.journal");
    fs::write(&old_journal, "kind copy\n").unwrap();
    crate::faults::trace(&tag);
    let mut req = Request::copy(vec![src.clone()], dst.clone());
    req.safe = true;
    req.existing = Existing::Resume;
    req.expect = [(dst.join(&tag), Expect::Skip)].into_iter().collect();
    req.resumed = Some(old_journal.clone());
    let s = run(req);
    let seen = crate::faults::traced(&tag);
    assert_eq!(
        fs::read_to_string(dst.join(&tag)).unwrap(),
        "old, left by the plan"
    );
    assert_eq!(s.failed, 0, "{s:?}");
    assert!(
        seen.iter()
            .any(|(p, at)| *p == crate::faults::Point::JournalLine && at == &dst.join(&tag)),
        "the skip was written to the new journal: {seen:?}"
    );
    assert!(!old_journal.exists());
    fs::remove_dir_all(&root).unwrap();
}

/// Resume takes over a big file kept after a checkpoint: what it holds is not
/// copied again (marked here with X, so a copy from the start would show), and the
/// rest follows. A source changed since starts again; a kept file no file of the
/// job reaches goes at the end, and so does the old journal.
#[test]
fn resume_goes_on_from_a_kept_checkpoint_while_the_source_is_unchanged() {
    journals();
    let root = tmp("ckpt-resume");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let src = root.join("big.bin");
    let body: Vec<u8> = (0..(200u32 << 10)).map(|i| (i % 251) as u8).collect();
    fs::write(&src, &body).unwrap();
    let part = dst.join(".big.bin.xdevcmd-part-2147483646-0");
    let kept = 64 << 10;
    // More than the checkpoint said: what is past it is cut off.
    fs::write(&part, vec![b'X'; kept + 100]).unwrap();
    let stray = dst.join(".other.bin.xdevcmd-part-2147483646-1");
    fs::write(&stray, "x").unwrap();
    let old_journal = root.join("old.journal");
    fs::write(&old_journal, "kind copy\n").unwrap();
    let partial = |part: &Path, dst_name: &str| crate::journal::Partial {
        part: part.to_path_buf(),
        src: src.clone(),
        dst: dst.join(dst_name),
        ident: Ident::at(&src).unwrap(),
        bytes: kept as u64,
    };
    let mut req = Request::copy(vec![src.clone()], dst.clone());
    req.safe = true;
    req.existing = Existing::Resume;
    req.checkpoint = 32 << 10;
    req.partial = vec![partial(&part, "big.bin"), partial(&stray, "other.bin")];
    req.resumed = Some(old_journal.clone());
    let s = run(req.clone());
    assert_eq!((s.done, s.failed), (1, 0), "{s:?}");
    let got = fs::read(dst.join("big.bin")).unwrap();
    assert_eq!(got.len(), body.len());
    assert!(
        got[..kept].iter().all(|b| *b == b'X'),
        "taken over, not copied again"
    );
    assert_eq!(got[kept..], body[kept..], "the rest follows");
    assert!(!part.exists() && !stray.exists(), "nothing kept is left");
    assert!(!old_journal.exists(), "the old journal went");
    let names: Vec<_> = fs::read_dir(&dst)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");

    // The source changed since: copied from the start.
    fs::remove_file(dst.join("big.bin")).unwrap();
    fs::write(&part, vec![b'X'; kept]).unwrap();
    req.partial = vec![partial(&part, "big.bin")];
    req.resumed = None;
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut changed = body.clone();
    changed[0] = 0xEE;
    fs::write(&src, &changed).unwrap();
    let s = run(req);
    assert_eq!((s.done, s.failed), (1, 0), "{s:?}");
    assert_eq!(fs::read(dst.join("big.bin")).unwrap(), changed);
    assert!(!part.exists());
    fs::remove_dir_all(&root).unwrap();
}

/// The journal's line for a replacement cannot be written (disk full, say): the
/// replacement is not made until the user says go on without it, and Stop keeps the
/// old file. Before Round 029 the line's error was dropped and the file replaced.
#[test]
fn a_journal_that_fails_before_a_replace_is_asked_about_and_stop_keeps_the_old_file() {
    journals();
    let root = tmp("journal-line");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("jl-a1.txt"), b"new").unwrap();
    fs::write(dst.join("jl-a1.txt"), b"old").unwrap();
    // The line naming the temporary file's directory, written as the copy starts.
    let _f = crate::faults::fail(
        crate::faults::Point::JournalLine,
        ".jl-a1.txt.xdevcmd-part",
        1,
        crate::faults::Fault::Os(28),
    );
    let mut req = Request::copy(vec![root.join("jl-a1.txt")], dst.clone());
    req.safe = true;
    let (s, asked) = run_answering(req, |q| match q {
        Question::Conflict { .. } => Answer::Yes,
        _ => Answer::Abort,
    });
    assert!(
        asked.iter().any(|q| q.starts_with("NoJournal")),
        "asked before replacing: {asked:?}"
    );
    assert_eq!(fs::read(dst.join("jl-a1.txt")).unwrap(), b"old", "{s:?}");
    assert_eq!(
        fs::read_dir(&dst).unwrap().count(),
        1,
        "no temporary file left"
    );
    // Go on without it: replaced, and the summary says the job ran without one.
    drop(_f);
    let _f = crate::faults::fail(
        crate::faults::Point::JournalLine,
        ".jl-a1.txt.xdevcmd-part",
        1,
        crate::faults::Fault::Os(28),
    );
    let mut req = Request::copy(vec![root.join("jl-a1.txt")], dst.clone());
    req.safe = true;
    let (s, asked) = run_answering(req, |_| Answer::Yes);
    let asked_once = asked.iter().filter(|q| q.starts_with("NoJournal")).count();
    assert_eq!(asked_once, 1, "{asked:?}");
    assert_eq!(fs::read(dst.join("jl-a1.txt")).unwrap(), b"new");
    assert!(
        s.describe(Kind::Copy).contains("safe operations off"),
        "{s:?}"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// The journal cannot be begun: asked before anything is done. Stop does nothing;
/// go on copies and says it ran without one. Before Round 029 nothing was asked and
/// only the summary said so, once the job was over.
#[test]
fn a_journal_that_cannot_be_begun_is_asked_about_before_anything_is_done() {
    journals();
    let root = tmp("journal-begin");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("a"), b"a").unwrap();
    let frag = root.to_string_lossy().into_owned();
    let _f = crate::faults::fail(
        crate::faults::Point::JournalBegin,
        &frag,
        1,
        crate::faults::Fault::Os(28),
    );
    let mut req = Request::copy(vec![root.join("a")], dst.clone());
    req.safe = true;
    let (s, asked) = run_answering(req, |_| Answer::Abort);
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].starts_with("NoJournal"), "{asked:?}");
    assert!(!dst.join("a").exists(), "stopped before anything: {s:?}");
    assert!(s.cancelled);
    drop(_f);
    let _f = crate::faults::fail(
        crate::faults::Point::JournalBegin,
        &frag,
        1,
        crate::faults::Fault::Os(28),
    );
    let mut req = Request::copy(vec![root.join("a")], dst.clone());
    req.safe = true;
    let (s, _) = run_answering(req, |_| Answer::Yes);
    assert_eq!(fs::read(dst.join("a")).unwrap(), b"a");
    assert!(
        s.describe(Kind::Copy)
            .contains("safe operations off for this job"),
        "{s:?}"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// The points a trace saw, in order, as their names.
fn points(fragment: &str) -> Vec<&'static str> {
    crate::faults::traced(fragment)
        .into_iter()
        .map(|(p, _)| p.name())
        .collect()
}

fn first(seen: &[&str], name: &str) -> usize {
    seen.iter()
        .position(|p| *p == name)
        .unwrap_or_else(|| panic!("no {name} in {seen:?}"))
}

/// A replacement's data is on disk before its rename, with safe operations on (the
/// journal's line synced in between) or off (the rule holds either way: the
/// owner's decision 1). Before Round 029 nothing was synced.
#[test]
fn a_replacement_is_synced_before_it_is_renamed_into_place() {
    journals();
    for safe in [false, true] {
        let root = tmp(if safe {
            "sync-order-safe"
        } else {
            "sync-order"
        });
        let dst = root.join("dst");
        fs::create_dir_all(&dst).unwrap();
        fs::write(root.join("f"), b"new").unwrap();
        fs::write(dst.join("f"), b"old").unwrap();
        let frag = root.to_string_lossy().into_owned();
        crate::faults::trace(&frag);
        let mut req = Request::copy(vec![root.join("f")], dst.clone());
        req.safe = safe;
        let (s, _) = run_with(req, Answer::Yes);
        let seen = points(&frag);
        assert_eq!(s.done, 1, "{s:?}");
        assert!(
            first(&seen, "sync-data") < first(&seen, "rename"),
            "safe {safe}: {seen:?}"
        );
        if safe {
            let (synced, renamed) = (first(&seen, "sync-data"), first(&seen, "rename"));
            assert!(
                seen[synced..renamed].contains(&"journal-sync"),
                "the journal's line is synced between: {seen:?}"
            );
        }
        assert_eq!(fs::read(dst.join("f")).unwrap(), b"new");
        fs::remove_dir_all(&root).unwrap();
    }
}

/// Round 033, group commit: `n` small files over older ones in `d/to/src`.
fn many_to_replace(d: &Path, n: usize) {
    for dir in [d.join("src"), d.join("to").join("src")] {
        fs::create_dir_all(&dir).unwrap();
    }
    for i in 0..n {
        fs::write(d.join("src").join(format!("f-{i:04}")), format!("new {i}")).unwrap();
        fs::write(d.join("to").join("src").join(format!("f-{i:04}")), "old").unwrap();
    }
}

/// Every file under `dir` either the old one or the new one, whole, and no
/// temporary file left: (new, old).
fn whole_files(dir: &Path) -> (usize, usize) {
    let (mut new, mut old) = (0, 0);
    for e in fs::read_dir(dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        assert!(
            !name.contains("xdevcmd-part"),
            "a temporary file left: {name}"
        );
        let i: usize = name.trim_start_matches("f-").parse().unwrap();
        match fs::read_to_string(e.path()).unwrap().as_str() {
            "old" => old += 1,
            s if s == format!("new {i}") => new += 1,
            s => panic!("{name} is neither: {s:?}"),
        }
    }
    (new, old)
}

/// Group commit: many small replacements go in batches, each file synced before its
/// rename (the trace's order holds per file), all replaced, counted and kept for
/// undo, nothing temporary left.
#[test]
fn small_replacements_are_synced_in_batches_and_each_before_its_rename() {
    let d = tmp("group");
    many_to_replace(&d, 300);
    let frag = d.to_string_lossy().into_owned();
    crate::faults::trace(&frag);
    let mut req = Request::copy(vec![d.join("src")], d.join("to"));
    req.existing = Existing::Replace;
    req.trash_root = Some(d.join("trash"));
    let (s, asked) = run_with(req, Answer::Yes);
    let seen = crate::faults::traced(&frag);
    assert_eq!(asked, 0);
    assert_eq!(s.replaced_to_trash, 300, "{s:?}");
    assert_eq!(whole_files(&d.join("to").join("src")), (300, 0));
    // Per file: its sync before its rename.
    for i in [0, 150, 299] {
        let name = format!("f-{i:04}");
        let at = |what: crate::faults::Point| {
            seen.iter()
                .position(|(p, path)| *p == what && path.to_string_lossy().contains(&name))
                .unwrap_or_else(|| panic!("{what:?} of {name}: {seen:?}"))
        };
        assert!(
            at(crate::faults::Point::SyncData) < at(crate::faults::Point::Rename),
            "{name}"
        );
    }
    let _ = fs::remove_dir_all(&d);
}

/// One file's sync failing in a batch: that file alone is asked about (Skip keeps
/// the old one), the rest of the batch is replaced.
#[test]
fn a_failed_sync_in_a_batch_is_asked_about_that_file_alone() {
    let d = tmp("group-fail");
    many_to_replace(&d, 100);
    let _f = crate::faults::fail(
        crate::faults::Point::SyncData,
        // The temporary file's name: `.f-0042.xdevcmd-part-…`.
        ".f-0042.",
        1,
        crate::faults::Fault::Os(5),
    );
    let mut req = Request::copy(vec![d.join("src")], d.join("to"));
    req.existing = Existing::Replace;
    req.trash_root = Some(d.join("trash"));
    let (s, asked) = run_answering(req, |_| Answer::Skip);
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert!(asked[0].contains("f-0042"), "{asked:?}");
    assert_eq!((s.failed, s.replaced_to_trash), (1, 99), "{s:?}");
    assert_eq!(whole_files(&d.join("to").join("src")), (99, 1));
    assert_eq!(
        fs::read_to_string(d.join("to").join("src").join("f-0042")).unwrap(),
        "old"
    );
    let _ = fs::remove_dir_all(&d);
}

/// Stopped with a batch waiting: its temporary files go and their old files stay;
/// every file is whole, old or new.
#[test]
fn a_job_stopped_with_a_batch_waiting_leaves_every_file_whole() {
    let d = tmp("group-stop");
    many_to_replace(&d, 400);
    let mut req = Request::copy(vec![d.join("src")], d.join("to"));
    req.existing = Existing::Replace;
    req.trash_root = Some(d.join("trash"));
    let h = start(req);
    let mut stopped = false;
    let s = loop {
        match h.events.recv_timeout(Duration::from_secs(20)) {
            Ok(Event::Progress { .. }) if !stopped => {
                h.cancel();
                stopped = true;
            }
            Ok(Event::Ask(_)) => h.answer(Answer::Abort),
            Ok(Event::Done(s)) => break s,
            Ok(_) => {}
            Err(e) => panic!("worker never finished: {e}"),
        }
    };
    let (new, old) = whole_files(&d.join("to").join("src"));
    assert_eq!(new + old, 400);
    assert_eq!(new as u64, s.replaced_to_trash, "{s:?}");
    let _ = fs::remove_dir_all(&d);
}

/// A directory copied read-only: its batch is finished before its permissions take
/// writing away, so its files are still replaced.
#[cfg(unix)]
#[test]
fn a_read_only_directory_gets_its_batch_before_its_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp("group-ro");
    many_to_replace(&d, 20);
    fs::set_permissions(d.join("src"), fs::Permissions::from_mode(0o555)).unwrap();
    let mut req = Request::copy(vec![d.join("src")], d.join("to"));
    req.existing = Existing::Replace;
    req.trash_root = Some(d.join("trash"));
    let (s, _) = run_with(req, Answer::Yes);
    let to = d.join("to").join("src");
    let (new, old) = whole_files(&to);
    for p in [d.join("src"), to] {
        let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o755));
    }
    assert_eq!((new, old), (20, 0), "{s:?}");
    let _ = fs::remove_dir_all(&d);
}

/// The probe: after [`GROUP_PROBE_AFTER`] files, the default first, then 2, 8, 16
/// and 32 (the default not twice), one batch each; the fastest a file is kept.
#[test]
fn the_probe_tries_the_default_first_then_the_others_and_keeps_the_fastest() {
    let mut g = Group::new();
    let d = g.threads;
    g.batched = GROUP_PROBE_AFTER - 1;
    g.measured(64, 64.0);
    assert!(g.probe.is_none(), "not before");
    g.batched = GROUP_PROBE_AFTER;
    g.measured(64, 64.0);
    let order = g.probe.as_ref().unwrap().0.clone();
    assert_eq!(order[0], d);
    let expected = GROUP_PROBE.len() + usize::from(!GROUP_PROBE.contains(&d));
    assert_eq!(order.len(), expected);
    assert_eq!(g.threads_now(), d);
    assert_eq!(g.cap(), (4 * d).max(16));
    // Each tried count's time a file: 32 is fastest.
    for n in order {
        assert_eq!(g.threads_now(), n);
        let per = if n == 32 { 0.1 } else { 1.0 };
        g.measured(g.cap(), per * g.cap() as f64);
    }
    assert!(g.probe.is_none() && g.probed);
    assert_eq!(g.threads, 32);
    assert_eq!(g.cap(), 128);
    // Once only.
    g.measured(64, 64.0);
    assert!(g.probe.is_none());
}

/// A sync that fails: the temporary file goes, the old file stays, and Retry writes
/// the file again from its source instead of syncing the same one twice.
#[test]
fn a_failed_sync_keeps_the_old_file_and_retry_copies_again() {
    let root = tmp("sync-fail");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("f"), b"new").unwrap();
    fs::write(dst.join("f"), b"old").unwrap();
    let frag = root.to_string_lossy().into_owned();
    let _f = crate::faults::fail(
        crate::faults::Point::SyncData,
        &frag,
        1,
        crate::faults::Fault::Os(5),
    );
    let (s, asked) = run_answering(
        Request::copy(vec![root.join("f")], dst.clone()),
        |q| match q {
            Question::Failed { .. } => Answer::Skip,
            _ => Answer::Yes,
        },
    );
    assert!(asked.iter().any(|q| q.starts_with("Failed")), "{asked:?}");
    assert_eq!((s.done, s.failed), (0, 1));
    assert_eq!(fs::read(dst.join("f")).unwrap(), b"old");
    assert_eq!(
        fs::read_dir(&dst).unwrap().count(),
        1,
        "no temporary file left"
    );
    drop(_f);
    let _f = crate::faults::fail(
        crate::faults::Point::SyncData,
        &frag,
        1,
        crate::faults::Fault::Os(5),
    );
    crate::faults::trace(&frag);
    let (s, _) = run_answering(
        Request::copy(vec![root.join("f")], dst.clone()),
        |q| match q {
            Question::Failed { .. } => Answer::Retry,
            _ => Answer::Yes,
        },
    );
    let seen = points(&frag);
    assert_eq!(s.done, 1, "{s:?}");
    let order: Vec<&str> = seen
        .iter()
        .copied()
        .filter(|p| matches!(*p, "write" | "sync-data" | "rename"))
        .collect();
    assert_eq!(
        order,
        ["write", "sync-data", "write", "sync-data", "rename"],
        "written again, never synced twice"
    );
    assert_eq!(fs::read(dst.join("f")).unwrap(), b"new");
    assert_eq!(fs::read_dir(&dst).unwrap().count(), 1);
    fs::remove_dir_all(&root).unwrap();
}

/// Replacing with a link: making the link fails, and the old file is still there.
/// Before Round 029 the old file was removed first and lost.
#[cfg(unix)]
#[test]
fn a_link_that_cannot_be_made_does_not_lose_the_file_it_was_to_replace() {
    let root = tmp("link-replace");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    std::os::unix::fs::symlink("target", root.join("l")).unwrap();
    fs::write(dst.join("l"), b"the only copy").unwrap();
    let _f = crate::faults::fail(
        crate::faults::Point::Write,
        &dst.join("l").to_string_lossy(),
        1,
        crate::faults::Fault::Os(28),
    );
    let (s, _) = run_with(
        Request::copy(vec![root.join("l")], dst.clone()),
        Answer::Yes,
    );
    assert_eq!(s.failed, 1, "{s:?}");
    assert_eq!(fs::read(dst.join("l")).unwrap(), b"the only copy");
    drop(_f);
    let (s, _) = run_with(
        Request::copy(vec![root.join("l")], dst.clone()),
        Answer::Yes,
    );
    assert_eq!(s.done, 1, "{s:?}");
    assert_eq!(fs::read_link(dst.join("l")).unwrap(), Path::new("target"));
    assert_eq!(
        fs::read_dir(&dst).unwrap().count(),
        1,
        "no temporary link left"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// A move across devices, per file: the copy's data synced, its directory synced,
/// the journal's `moved` line synced, and only then the source removed. Before Round
/// 029 the source was removed right after an unsynced copy.
#[test]
fn a_move_across_devices_removes_a_source_only_once_its_copy_is_on_disk() {
    journals();
    let Some(other) = other_device("move-order") else {
        return;
    };
    let root = tmp("move-order");
    let tag = format!("xmvd-{}", std::process::id());
    let dir = root.join(&tag);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("f"), b"payload").unwrap();
    crate::faults::trace(&tag);
    let mut req = Request::mv(vec![dir.clone()], other.clone());
    req.safe = true;
    let (s, _) = run_with(req, Answer::Yes);
    let seen = crate::faults::traced(&tag);
    assert_eq!(s.failed, 0, "{s:?}");
    assert_eq!(fs::read(other.join(&tag).join("f")).unwrap(), b"payload");
    assert!(!dir.exists(), "moved in full");
    let at = |point: &str, file: bool| {
        seen.iter()
            .position(|(p, path)| p.name() == point && (path.ends_with("f") || !file))
            .unwrap_or_else(|| panic!("no {point} in {seen:?}"))
    };
    let gone = at("remove-source", true);
    for before in ["sync-data", "sync-dir", "journal-sync"] {
        assert!(
            at(before, false) < gone,
            "{before} before the source goes: {seen:?}"
        );
    }
    let synced_dir = seen
        .iter()
        .position(|(p, path)| p.name() == "sync-dir" && path.ends_with(&tag))
        .unwrap_or_else(|| panic!("the copy's directory is synced: {seen:?}"));
    assert!(synced_dir < gone);
    fs::remove_dir_all(&other).unwrap();
    fs::remove_dir_all(&root).unwrap();
}

/// The copy's sync fails: the source stays, and the copy, which may not be what was
/// read, is removed.
#[test]
fn a_move_across_devices_whose_copy_cannot_be_synced_keeps_the_source() {
    let Some(other) = other_device("move-sync-fail") else {
        return;
    };
    let root = tmp("move-sync-fail");
    fs::write(root.join("only.txt"), b"the only copy").unwrap();
    let _f = crate::faults::fail(
        crate::faults::Point::SyncData,
        &other.to_string_lossy(),
        1,
        crate::faults::Fault::Os(5),
    );
    let (s, _) = run_with(
        Request::mv(vec![root.join("only.txt")], other.clone()),
        Answer::Yes,
    );
    assert_eq!((s.done, s.failed), (0, 1), "{s:?}");
    assert_eq!(fs::read(root.join("only.txt")).unwrap(), b"the only copy");
    assert_eq!(
        fs::read_dir(&other).unwrap().count(),
        0,
        "no copy, no temporary file"
    );
    fs::remove_dir_all(&other).unwrap();
    fs::remove_dir_all(&root).unwrap();
}

/// Whether this process can write to a file whatever its mode (root on Unix): a test
/// about a read-only file then proves nothing, and says it is skipped.
fn writes_anything(tag: &str) -> bool {
    #[cfg(unix)]
    if rustix::process::geteuid().is_root() {
        eprintln!("{tag}: skipped, root writes a read-only file; run it as another user");
        return true;
    }
    let _ = tag;
    false
}

/// A read-only file moved across devices: its copy is read-only as soon as it is
/// written (the mode is copied), so syncing it through a handle opened afresh for
/// writing failed (EACCES), the move failed, and on Windows the copy was left. The
/// handle that wrote it is the one synced.
#[test]
fn a_read_only_file_moves_across_devices() {
    if writes_anything("read-only-move") {
        return;
    }
    let Some(other) = other_device("read-only-move") else {
        return;
    };
    let root = tmp("read-only-move");
    let f = root.join("ro.txt");
    fs::write(&f, b"read only").unwrap();
    let mut perm = fs::metadata(&f).unwrap().permissions();
    perm.set_readonly(true);
    fs::set_permissions(&f, perm).unwrap();
    let (s, _) = run_with(Request::mv(vec![f.clone()], other.clone()), Answer::Yes);
    assert_eq!((s.done, s.failed), (1, 0), "{s:?}");
    assert!(!f.exists(), "the source is gone");
    assert_eq!(fs::read(other.join("ro.txt")).unwrap(), b"read only");
    assert!(fs::metadata(other.join("ro.txt"))
        .unwrap()
        .permissions()
        .readonly());
    let mut perm = fs::metadata(other.join("ro.txt")).unwrap().permissions();
    // Cleaning up: writable again so the directory can go (Windows).
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    let _ = fs::set_permissions(other.join("ro.txt"), perm);
    let _ = fs::remove_dir_all(&other);
    let _ = fs::remove_dir_all(&root);
}

/// A write error that surfaces only at the sync (NFS, SMB: at close or at the sync
/// of the handle that wrote) keeps that file's source and removes its copy; the rest
/// of the batch moves.
#[test]
fn a_copy_whose_sync_fails_keeps_its_source_and_the_rest_moves() {
    let Some(other) = other_device("sync-one-fails") else {
        return;
    };
    let root = tmp("sync-one-fails");
    for n in ["a", "b", "c"] {
        fs::write(root.join(n), n).unwrap();
    }
    let _f = crate::faults::fail(
        crate::faults::Point::SyncData,
        &other.join("b").to_string_lossy(),
        1,
        crate::faults::Fault::Os(5),
    );
    let srcs = vec![root.join("a"), root.join("b"), root.join("c")];
    let mut req = Request::mv(srcs, other.clone());
    req.existing = Existing::Ask;
    let (s, _) = run_with(req, Answer::YesToAll);
    assert_eq!((s.done, s.failed), (2, 1), "{s:?}");
    assert_eq!(fs::read(root.join("b")).unwrap(), b"b", "b's source stays");
    assert!(!other.join("b").exists(), "b's copy goes");
    assert!(!root.join("a").exists() && !root.join("c").exists());
    let _ = fs::remove_dir_all(&other);
    let _ = fs::remove_dir_all(&root);
}

/// Stop at the journal's question means nothing more that cannot be undone: a source
/// whose copy was waiting in the batch is kept. Before, the batch was still flushed
/// once the journal was gone (no journal: nothing to wait for), and its sources went.
#[test]
fn stop_at_the_journal_question_keeps_the_sources_of_a_move() {
    journals();
    let Some(other) = other_device("stop-keeps") else {
        return;
    };
    let root = tmp("stop-keeps");
    let tag = format!("stopkeep-{}", std::process::id());
    fs::create_dir_all(root.join(&tag)).unwrap();
    fs::create_dir_all(other.join(&tag)).unwrap();
    for n in ["a", "b"] {
        fs::write(root.join(&tag).join(n), b"ours").unwrap();
        fs::write(other.join(&tag).join(n), b"theirs").unwrap();
    }
    // The job's start (1), the move onto the existing directory (2), the first
    // replacement (3), the second (4) fails.
    let _f = crate::faults::fail(
        crate::faults::Point::JournalSync,
        &tag,
        4,
        crate::faults::Fault::Os(28),
    );
    let mut req = Request::mv(vec![root.join(&tag)], other.clone());
    req.safe = true;
    let (s, asked) = run_answering(req, |q| match q {
        Question::NoJournal { .. } => Answer::Abort,
        _ => Answer::YesToAll,
    });
    assert!(
        asked.iter().any(|q| q.starts_with("NoJournal")),
        "{asked:?}"
    );
    for n in ["a", "b"] {
        assert!(
            root.join(&tag).join(n).exists(),
            "{n}'s source is kept: {s:?}"
        );
    }
    let _ = fs::remove_dir_all(&other);
    let _ = fs::remove_dir_all(&root);
}

/// Every file of a move across devices, across batch boundaries (64, 65, 128, 129…):
/// its copy's data, then its directory, then the journal, are synced after it was
/// written and before ITS source goes. An off-by-one at a boundary shows here.
#[test]
fn every_file_of_a_batched_move_is_on_disk_before_its_own_source_goes() {
    journals();
    let Some(other) = other_device("batch-order") else {
        return;
    };
    let root = tmp("batch-order");
    let tag = format!("batchord-{}", std::process::id());
    let dir = root.join(&tag);
    fs::create_dir_all(&dir).unwrap();
    let n = 2 * MOVE_BATCH_FILES + 2;
    for i in 0..n {
        fs::write(dir.join(format!("f{i:03}")), b"x").unwrap();
    }
    crate::faults::trace(&tag);
    let mut req = Request::mv(vec![dir.clone()], other.clone());
    req.safe = true;
    let (s, _) = run_with(req, Answer::Yes);
    let seen = crate::faults::traced(&tag);
    assert_eq!(s.failed, 0, "{s:?}");
    let to_dir = other.join(&tag);
    for i in 0..n {
        let name = format!("f{i:03}");
        let at = |point: &str, path: &Path| {
            seen.iter()
                .position(|(p, q)| p.name() == point && q == path)
                .unwrap_or_else(|| panic!("{name}: no {point} for {}", path.display()))
        };
        let gone = at("remove-source", &dir.join(&name));
        let data = at("sync-data", &to_dir.join(&name));
        assert!(data < gone, "{name}: its data before its source goes");
        let between = |point: &str| {
            seen[data..gone]
                .iter()
                .any(|(p, q)| p.name() == point && (point != "sync-dir" || q == &to_dir))
        };
        assert!(
            between("sync-dir"),
            "{name}: its directory synced after its data, before its source goes"
        );
        assert!(
            between("journal-sync"),
            "{name}: the journal synced after its data, before its source goes"
        );
    }
    let _ = fs::remove_dir_all(&other);
    let _ = fs::remove_dir_all(&root);
}

/// The cost of the sync rules on a move across devices, measured: `n` files of
/// `size` bytes moved with every sync made and again with the seam leaving them out.
/// `XDEVCMD_MEASURE=<n>x<size>[,…]`, from where `XDEVCMD_MEASURE_FROM` says (default:
/// the temporary directory) to the other device, or back with `XDEVCMD_MEASURE_BACK`.
/// Run by hand: `cargo test -p xdevcmd-core --release -- --ignored --nocapture measure`.
#[test]
#[ignore = "a measurement; run by hand (Round 029, step 3)"]
fn measure_a_move_across_devices_with_and_without_syncs() {
    journals();
    let Some(other) = other_device("measure") else {
        return;
    };
    let spec = std::env::var("XDEVCMD_MEASURE").unwrap_or_else(|_| "10000x1024,3x104857600".into());
    let back = std::env::var_os("XDEVCMD_MEASURE_BACK").is_some();
    for case in spec.split(',') {
        let Some((n, size)) = case.split_once('x') else {
            continue;
        };
        let (n, size): (usize, u64) = (n.parse().unwrap(), size.parse().unwrap());
        for synced in [true, false] {
            let here = tmp("measure");
            let (from, to) = if back {
                (other.clone(), here.clone())
            } else {
                (here.clone(), other.clone())
            };
            let src = from.join(format!("m-{n}-{size}-{synced}"));
            fs::create_dir_all(&src).unwrap();
            let chunk = vec![0x5au8; size as usize];
            for i in 0..n {
                fs::write(src.join(format!("f{i}")), &chunk).unwrap();
            }
            let frag = src.file_name().unwrap().to_string_lossy().into_owned();
            let _skips: Vec<_> = if synced {
                Vec::new()
            } else {
                [
                    crate::faults::Point::SyncData,
                    crate::faults::Point::SyncDir,
                    crate::faults::Point::JournalSync,
                ]
                .into_iter()
                .map(|p| crate::faults::set(p, &frag, crate::faults::Action::Skip))
                .collect()
            };
            // What was written to make the files is not the move's to flush.
            let _ = std::process::Command::new("sync").status();
            let mut req = Request::mv(vec![src.clone()], to.clone());
            req.safe = true;
            let t = std::time::Instant::now();
            let (s, _) = run_with(req, Answer::Yes);
            let took = t.elapsed();
            assert_eq!(s.failed, 0, "{s:?}");
            eprintln!(
                "MEASURE {n} x {size} B {} {}: {:.3} s",
                if back { "back" } else { "out" },
                if synced { "synced" } else { "no sync" },
                took.as_secs_f64()
            );
            let _ = fs::remove_dir_all(to.join(src.file_name().unwrap()));
            let _ = fs::remove_dir_all(&here);
        }
    }
    let _ = fs::remove_dir_all(&other);
}

#[test]
fn a_replacement_cancelled_part_way_keeps_the_old_file() {
    let root = tmp("replace-cancel");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("f"), vec![7u8; 1 << 20]).unwrap();
    fs::write(dst.join("f"), b"old").unwrap();
    let h = start(Request::copy(vec![root.join("f")], dst.clone()));
    loop {
        match h.events.recv_timeout(Duration::from_secs(20)).unwrap() {
            // Agree to overwrite, and cancel at once: the copy stops before it is
            // complete.
            Event::Ask(_) => {
                h.cancel();
                h.answer(Answer::Yes);
            }
            Event::Done(_) => break,
            _ => {}
        }
    }
    assert_eq!(
        fs::read(dst.join("f")).unwrap(),
        b"old",
        "the old file is intact"
    );
    let left: Vec<_> = fs::read_dir(&dst)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(left.len(), 1, "no temporary file left behind: {left:?}");
    // Not cancelled, the replacement lands whole.
    let (s, _) = run_with(
        Request::copy(vec![root.join("f")], dst.clone()),
        Answer::Yes,
    );
    assert_eq!(s.done, 1);
    assert_eq!(fs::read(dst.join("f")).unwrap().len(), 1 << 20);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn overwrite_all_does_not_also_agree_to_copy_then_delete() {
    let Some(other) = other_device("split") else {
        return;
    };
    let root = tmp("split-all");
    fs::write(root.join("a"), b"A").unwrap();
    fs::write(root.join("b"), b"B").unwrap();
    fs::write(other.join("a"), b"theirs").unwrap();
    // "Overwrite all" at a's question; b's copy-then-delete is still asked about.
    let (_, asked) = run_answering(
        Request::mv(vec![root.join("a"), root.join("b")], other.clone()),
        |q| match q {
            Question::Conflict { .. } => Answer::YesToAll,
            _ => Answer::Yes,
        },
    );
    let cross = asked
        .iter()
        .filter(|q| q.starts_with("CrossDevice"))
        .count();
    assert_eq!(cross, 2, "{asked:?}");
    fs::remove_dir_all(&other).unwrap();
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_sync_replaces_only_what_its_plan_named() {
    let root = tmp("sync-planned");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("a"), b"new a").unwrap();
    fs::write(root.join("b"), b"new b").unwrap();
    fs::write(dst.join("a"), b"old a").unwrap();
    // a is a planned overwrite; b was free, and a file takes its name meanwhile.
    APPEAR.lock().unwrap().push(dst.join("b"));
    let (_, asked) = run_with(
        Request::sync(vec![
            (
                root.join("a"),
                dst.clone(),
                Expect::Replace(Ident::at(&dst.join("a")).unwrap()),
            ),
            (root.join("b"), dst.clone(), Expect::Absent),
        ]),
        Answer::Skip,
    );
    assert_eq!(
        asked, 1,
        "only b, which the plan did not name, is asked about"
    );
    assert_eq!(fs::read(dst.join("a")).unwrap(), b"new a");
    assert_eq!(fs::read(dst.join("b")).unwrap(), b"theirs");
    fs::remove_dir_all(&root).unwrap();
}

/// The fault test (B05): the user says Yes to replacing a file, and another program
/// replaces that file while the copy is written. Asked again, saying so; Skip keeps
/// the newcomer. Before Round 029 it was overwritten unasked.
#[test]
fn a_file_replaced_after_the_user_agreed_is_asked_about_again() {
    let root = tmp("swap-after-yes");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("f"), b"ours").unwrap();
    fs::write(dst.join("f"), b"the file the user saw").unwrap();
    // At the replacement's sync: after the answer, before the rename.
    let _g = crate::faults::set(
        crate::faults::Point::SyncData,
        &root.to_string_lossy(),
        crate::faults::Action::Swap {
            target: dst.join("f"),
            body: b"another program's file".to_vec(),
        },
    );
    let (s, asked) = run_answering(Request::copy(vec![root.join("f")], dst.clone()), |q| {
        if format!("{q:?}").contains("changed: true") {
            Answer::Skip
        } else {
            Answer::Yes
        }
    });
    assert_eq!(asked.len(), 2, "asked again: {asked:?}");
    assert!(asked[1].contains("changed: true"), "{asked:?}");
    assert_eq!(
        fs::read(dst.join("f")).unwrap(),
        b"another program's file",
        "{s:?}"
    );
    assert_eq!(
        fs::read_dir(&dst).unwrap().count(),
        1,
        "no temporary file left"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// The re-check is the last thing before the rename: the journal's question can be
/// open for minutes, and a file replaced while it is was overwritten on a check made
/// before it. Asked again instead.
#[test]
fn a_file_replaced_while_the_journal_question_is_open_is_asked_about_again() {
    journals();
    let root = tmp("swap-during-nojournal");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("sq-a1.txt"), b"ours").unwrap();
    fs::write(dst.join("sq-a1.txt"), b"seen").unwrap();
    let _f = crate::faults::fail(
        crate::faults::Point::JournalLine,
        ".sq-a1.txt.xdevcmd-part",
        1,
        crate::faults::Fault::Os(28),
    );
    let target = dst.join("sq-a1.txt");
    let mut req = Request::copy(vec![root.join("sq-a1.txt")], dst.clone());
    req.safe = true;
    let (_, asked) = run_answering(req, |q| {
        let q = format!("{q:?}");
        if q.starts_with("NoJournal") {
            // Another program replaces it while the question is open.
            fs::remove_file(&target).unwrap();
            fs::write(&target, b"replaced while asked, longer").unwrap();
            Answer::Yes
        } else if q.contains("changed: true") {
            Answer::Skip
        } else {
            Answer::Yes
        }
    });
    assert!(
        asked.iter().any(|q| q.contains("changed: true")),
        "{asked:?}"
    );
    assert_eq!(fs::read(&target).unwrap(), b"replaced while asked, longer");
    fs::remove_dir_all(&root).unwrap();
}

/// The same on a move within one file system, whose replace is a rename.
#[test]
fn a_move_onto_a_file_replaced_after_the_user_agreed_asks_again() {
    let root = tmp("swap-move");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("m"), b"ours").unwrap();
    fs::write(dst.join("m"), b"seen").unwrap();
    let target = dst.join("m");
    let mut first = true;
    let h = start(Request::mv(vec![root.join("m")], dst.clone()));
    let mut asked = Vec::new();
    loop {
        match h.events.recv_timeout(Duration::from_secs(20)).unwrap() {
            Event::Ask(q) => {
                asked.push(format!("{q:?}"));
                if first {
                    // Another program replaces it as the user answers.
                    fs::remove_file(&target).unwrap();
                    fs::write(&target, b"replaced meanwhile, longer").unwrap();
                    first = false;
                    h.answer(Answer::Yes);
                } else {
                    h.answer(Answer::Skip);
                }
            }
            Event::Done(_) => break,
            _ => {}
        }
    }
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert_eq!(fs::read(&target).unwrap(), b"replaced meanwhile, longer");
    assert_eq!(
        fs::read(root.join("m")).unwrap(),
        b"ours",
        "the source stays"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// A file made, after Alt+Y's compare, where the plan saw nothing: asked about, not
/// overwritten. Before Round 029 the job took every destination that existed when
/// it started for one the plan named.
#[test]
fn a_file_made_where_the_plan_saw_none_is_asked_about() {
    let root = tmp("sync-absent");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("only-left"), b"left").unwrap();
    let req = Request::sync(vec![(root.join("only-left"), dst.clone(), Expect::Absent)]);
    // Between the compare and Enter.
    fs::write(dst.join("only-left"), b"made since").unwrap();
    let (_, asked) = run_with(req, Answer::Skip);
    assert_eq!(asked, 1);
    assert_eq!(fs::read(dst.join("only-left")).unwrap(), b"made since");
    // And a planned replacement whose file changed since the compare: asked too.
    let seen = Ident::at(&dst.join("only-left")).unwrap();
    fs::remove_file(dst.join("only-left")).unwrap();
    fs::write(dst.join("only-left"), b"changed since the plan").unwrap();
    let req = Request::sync(vec![(
        root.join("only-left"),
        dst.clone(),
        Expect::Replace(seen),
    )]);
    let (_, asked) = run_with(req, Answer::Skip);
    assert_eq!(asked, 1);
    assert_eq!(
        fs::read(dst.join("only-left")).unwrap(),
        b"changed since the plan"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// Set `p`'s modification time to `secs` after the epoch.
fn age(p: &Path, secs: u64) {
    let t = std::time::UNIX_EPOCH + Duration::from_secs(secs);
    File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_times(FileTimes::new().set_modified(t))
        .unwrap();
}

#[test]
fn update_replaces_only_what_is_older_and_skips_the_same_and_newer() {
    let root = tmp("update");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    // older: the copy there is older; same: equal times (within 2 s); newer: the
    // copy there is newer; fresh: not there at all.
    for (name, ours, theirs) in [
        ("older", 2_000_000, 1_000_000),
        ("same", 1_000_000, 1_000_001),
        ("newer", 1_000_000, 2_000_000),
    ] {
        fs::write(root.join(name), b"ours").unwrap();
        age(&root.join(name), ours);
        fs::write(dst.join(name), b"theirs").unwrap();
        age(&dst.join(name), theirs);
    }
    fs::write(root.join("fresh"), b"ours").unwrap();
    let names = ["older", "same", "newer", "fresh"];
    let mut req = Request::copy(names.iter().map(|n| root.join(n)).collect(), dst.clone());
    req.existing = Existing::Update;
    let (s, asked) = run_with(req, Answer::Abort);
    assert_eq!(asked, 0, "update never asks");
    assert_eq!((s.done, s.skipped), (2, 2));
    assert_eq!(fs::read(dst.join("older")).unwrap(), b"ours");
    assert_eq!(fs::read(dst.join("same")).unwrap(), b"theirs");
    assert_eq!(fs::read(dst.join("newer")).unwrap(), b"theirs");
    assert_eq!(fs::read(dst.join("fresh")).unwrap(), b"ours");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn resuming_skips_what_is_identical_and_asks_about_the_rest() {
    let root = tmp("resume");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    // done: finished by the first run (same size, same date). kept: the user chose
    // to keep their older, different file in the first run; Update would replace
    // it, its source being newer. fresh: not there yet.
    fs::write(root.join("done"), b"ours").unwrap();
    age(&root.join("done"), 1_000_000);
    fs::write(dst.join("done"), b"ours").unwrap();
    age(&dst.join("done"), 1_000_001);
    fs::write(root.join("kept"), b"new version").unwrap();
    age(&root.join("kept"), 2_000_000);
    fs::write(dst.join("kept"), b"theirs").unwrap();
    age(&dst.join("kept"), 1_000_000);
    fs::write(root.join("fresh"), b"ours").unwrap();
    let names = ["done", "kept", "fresh"];
    let mut req = Request::copy(names.iter().map(|n| root.join(n)).collect(), dst.clone());
    req.existing = Existing::Resume;
    let (s, asked) = run_with(req, Answer::Skip);
    assert_eq!(
        asked, 1,
        "only kept is asked about; done is skipped silently"
    );
    assert_eq!(fs::read(dst.join("kept")).unwrap(), b"theirs", "kept stays");
    assert_eq!(fs::read(dst.join("fresh")).unwrap(), b"ours");
    assert_eq!((s.done, s.skipped, s.left), (1, 2, 0));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn resuming_a_move_leaves_a_source_whose_copy_is_already_there_and_says_so() {
    // A move across drives killed after its copy and before removing the source.
    let root = tmp("resume-move");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    fs::write(root.join("a"), b"ours").unwrap();
    age(&root.join("a"), 1_000_000);
    fs::write(dst.join("a"), b"ours").unwrap();
    age(&dst.join("a"), 1_000_000);
    let mut req = Request::mv(vec![root.join("a")], dst.clone());
    req.existing = Existing::Resume;
    let (s, asked) = run_with(req, Answer::Abort);
    assert_eq!(asked, 0);
    assert!(
        root.join("a").exists(),
        "the source is never removed unasked"
    );
    assert_eq!(s.left, 1);
    assert!(
        s.describe(Kind::Move)
            .contains("1 left where it was: already there"),
        "{}",
        s.describe(Kind::Move)
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn answering_update_at_a_question_applies_it_to_the_rest() {
    let root = tmp("update-answer");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    for (name, ours, theirs) in [("a", 1_000_000, 2_000_000), ("b", 2_000_000, 1_000_000)] {
        fs::write(root.join(name), b"ours").unwrap();
        age(&root.join(name), ours);
        fs::write(dst.join(name), b"theirs").unwrap();
        age(&dst.join(name), theirs);
    }
    let req = Request::copy(vec![root.join("a"), root.join("b")], dst.clone());
    let (s, asked) = run_with(req, Answer::Update);
    assert_eq!(asked, 1, "asked once, then the rule decides");
    assert_eq!(
        fs::read(dst.join("a")).unwrap(),
        b"theirs",
        "newer there: kept"
    );
    assert_eq!(
        fs::read(dst.join("b")).unwrap(),
        b"ours",
        "older there: replaced"
    );
    assert_eq!((s.done, s.skipped), (1, 1));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_summary_says_when_its_job_ran_without_a_journal() {
    let mut s = Summary {
        done: 1,
        ..Summary::default()
    };
    assert_eq!(s.describe(Kind::Copy), "1 copied");
    s.journal_off = Some("permission denied".into());
    assert_eq!(
        s.describe(Kind::Copy),
        "1 copied; safe operations off for this job: permission denied"
    );
}

#[test]
fn with_safe_operations_a_cancelled_new_copy_leaves_nothing_under_any_name() {
    let root = tmp("safe-cancel");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    // Big enough that the cancel lands before the copy ends even on a loaded machine:
    // with 8 MiB the rest was sometimes copied before the test thread ran, and a
    // copy that completes is rightly kept. Sparse, so it costs nothing to make.
    let big = File::create(root.join("big")).unwrap();
    big.set_len(1 << 30).unwrap();
    drop(big);
    let mut req = Request::copy(vec![root.join("big")], dst.clone());
    req.safe = true;
    // The copy's own loop: a clone (APFS) is whole at once, nothing left to cancel.
    req.fast = false;
    let h = start(req);
    loop {
        match h.events.recv_timeout(Duration::from_secs(20)).unwrap() {
            // Stop it as soon as it starts writing.
            Event::Progress { .. } => h.cancel(),
            Event::Done(_) => break,
            _ => {}
        }
    }
    let left: Vec<_> = fs::read_dir(&dst)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert!(
        left.is_empty(),
        "nothing under the real name or a temporary one: {left:?}"
    );
    fs::remove_dir_all(&root).unwrap();
}

/// B1: the temporary name used to add ~40 bytes to the real one, and a non-UTF-8
/// name grew threefold through `to_string_lossy`: a long name then failed with
/// "File name too long".
#[test]
fn the_temporary_name_fits_the_name_limit_and_keeps_the_runs_mark() {
    let mark = format!(".xdevcmd-part-{}-", crate::journal::run_tag());
    #[cfg_attr(not(unix), allow(unused_mut))] // a non-UTF-8 name is added on Unix
    let mut names: Vec<std::ffi::OsString> = vec![
        "a".into(),
        "x".repeat(255).into(),
        "é".repeat(127).into(),
        "日本".repeat(42).into(),
    ];
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        names.push(std::ffi::OsString::from_vec(vec![0xff; 90]));
    }
    for name in names {
        let part = part_name(&Path::new("/d").join(&name));
        let got = part.file_name().unwrap().to_string_lossy();
        assert!(got.len() <= NAME_MAX, "{} bytes: {got}", got.len());
        assert!(got.starts_with('.') && got.contains(&mark), "{got}");
        assert_eq!(part.parent(), Some(Path::new("/d")));
    }
    let short = part_name(Path::new("/d/a"));
    let short = short.file_name().unwrap().to_string_lossy();
    assert!(
        short.starts_with(&format!(".a{mark}")),
        "kept whole: {short}"
    );
}

/// B1, end to end: with safe operations on, a long name and a non-UTF-8 name are
/// copied and then replaced.
#[test]
fn with_safe_operations_long_and_non_utf8_names_are_copied_and_replaced() {
    let root = tmp("safe-long");
    let (src, dst) = (root.join("src"), root.join("dst"));
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dst).unwrap();
    #[cfg_attr(not(target_os = "linux"), allow(unused_mut))] // non-UTF-8 on Linux only
    let mut names: Vec<std::ffi::OsString> = vec!["n".repeat(250).into()];
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStringExt;
        names.push(std::ffi::OsString::from_vec(vec![0xff; 90]));
    }
    for name in &names {
        fs::write(src.join(name), "one").unwrap();
    }
    let files: Vec<PathBuf> = names.iter().map(|n| src.join(n)).collect();
    for body in ["one", "two"] {
        for f in &files {
            fs::write(f, body).unwrap();
        }
        let mut req = Request::copy(files.clone(), dst.clone());
        req.safe = true;
        let s = run(req);
        assert_eq!(s.failed, 0, "{body}: {:?}", s.errors);
        for name in &names {
            assert_eq!(fs::read_to_string(dst.join(name)).unwrap(), body);
        }
    }
    let left: Vec<_> = fs::read_dir(&dst)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .filter(|n| n.to_string_lossy().contains("xdevcmd-part"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn two_jobs_at_once_each_write_the_way_their_request_says() {
    let root = tmp("safe-per-job");
    let (on, off) = (root.join("on"), root.join("off"));
    fs::create_dir_all(&on).unwrap();
    fs::create_dir_all(&off).unwrap();
    let src: Vec<PathBuf> = (0..20)
        .map(|i| {
            let p = root.join(format!("f{i}"));
            fs::write(&p, vec![b'x'; 64 << 10]).unwrap();
            p
        })
        .collect();
    let mut a = Request::copy(src.clone(), on.clone());
    a.safe = true;
    let mut b = Request::copy(src, off.clone());
    b.safe = false;
    let (ha, hb) = (start(a), start(b));
    for h in [ha, hb] {
        while !matches!(
            h.events.recv_timeout(Duration::from_secs(20)).unwrap(),
            Event::Done(_)
        ) {}
    }
    let written = WRITING.lock().unwrap().clone();
    let names = |dir: &Path| -> Vec<String> {
        written
            .iter()
            .filter(|p| p.parent() == Some(dir))
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };
    let (safe_names, plain_names) = (names(&on), names(&off));
    assert_eq!((safe_names.len(), plain_names.len()), (20, 20));
    assert!(
        safe_names.iter().all(|n| n.contains(".xdevcmd-part-")),
        "the safe job writes every file under a temporary name: {safe_names:?}"
    );
    assert!(
        plain_names.iter().all(|n| !n.contains(".xdevcmd-part-")),
        "the other writes in place: {plain_names:?}"
    );
    assert_eq!(fs::read_dir(&on).unwrap().count(), 20);
    assert_eq!(fs::read_dir(&off).unwrap().count(), 20);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_move_on_one_filesystem_removes_the_source() {
    let root = tmp("move");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let f = root.join("f.txt");
    fs::write(&f, b"payload").unwrap();

    let s = run(Request::mv(vec![f.clone()], dst.clone()));
    assert_eq!(s.failed, 0, "{:?}", s.errors);
    assert!(!f.exists(), "the source must be gone after a move");
    assert_eq!(fs::read(dst.join("f.txt")).unwrap(), b"payload");

    fs::remove_dir_all(&root).unwrap();
}

/// A trailing slash goes through a link: deleting `link/` emptied the folder it
/// points to, and then failed on the link (red-team of #2). Refused whole.
#[cfg(unix)]
#[test]
fn a_delete_never_goes_through_a_link_by_its_trailing_slash() {
    let root = tmp("delete-link-slash");
    let target = root.join("precious");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("f"), b"keep").unwrap();
    std::os::unix::fs::symlink(&target, root.join("link")).unwrap();
    let through = PathBuf::from(format!("{}/", root.join("link").display()));
    let s = run(Request::delete(vec![through], false));
    assert!(target.join("f").exists(), "emptied through the link: {s:?}");
    assert_eq!(s.done, 0, "{s:?}");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn delete_removes_a_whole_tree() {
    let root = tmp("delete");
    let victim = root.join("victim");
    fs::create_dir_all(victim.join("deep")).unwrap();
    fs::write(victim.join("deep/x"), b"x").unwrap();

    let s = run(Request::delete(vec![victim.clone()], false));
    assert_eq!(s.done, 1);
    assert!(!victim.exists());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn trash_moves_the_file_and_records_how_to_restore_it() {
    let root = tmp("trash");
    let trash_root = root.join("Trash");
    let f = root.join("note.txt");
    fs::write(&f, b"keep me").unwrap();

    let mut req = Request::delete(vec![f.clone()], true);
    req.trash_root = Some(trash_root.clone());
    let s = run(req);
    assert_eq!(s.done, 1, "{:?}", s.errors);

    assert!(!f.exists(), "the original is gone from where it was");
    assert_eq!(
        fs::read(trash_root.join("files/note.txt")).unwrap(),
        b"keep me"
    );

    let info = fs::read_to_string(trash_root.join("info/note.txt.trashinfo")).unwrap();
    assert!(info.starts_with("[Trash Info]\n"), "{info}");
    // The XDG trash is the Linux one; the absolute-path form is Unix's. Elsewhere
    // this code only runs here, in its test.
    #[cfg(unix)]
    assert!(
        info.contains("Path=/"),
        "the original path must be recorded\n{info}"
    );
    assert!(info.contains("note.txt"), "{info}");
    assert!(info.contains("DeletionDate="), "{info}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn what_went_to_the_trash_comes_back_and_only_while_it_can() {
    let root = tmp("trash-back");
    let trash_root = root.join("Trash");
    let f = root.join("note.txt");
    fs::write(&f, b"keep me").unwrap();
    let mut req = Request::delete(vec![f.clone()], true);
    req.trash_root = Some(trash_root.clone());
    let s = run(req);
    let [t] = s.trashed.as_slice() else {
        panic!("one item trashed: {s:?}");
    };
    assert_eq!(t.original, f);
    // Its old name taken meanwhile: refused, nothing replaced.
    fs::write(&f, b"newer").unwrap();
    assert!(crate::trashed::check(t)
        .unwrap_err()
        .contains("exists again"));
    assert!(crate::trashed::restore(t).is_err());
    assert_eq!(fs::read(&f).unwrap(), b"newer");
    // Free again: back where it was, its record gone from the trash.
    fs::remove_file(&f).unwrap();
    crate::trashed::restore(t).unwrap();
    assert_eq!(fs::read(&f).unwrap(), b"keep me");
    assert!(!trash_root.join("info/note.txt.trashinfo").exists());
    assert!(!trash_root.join("files/note.txt").exists());
    // Gone from the trash: undo says so.
    assert!(crate::trashed::check(t).is_err());
    fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn a_trashed_link_records_its_own_path_not_its_target() {
    let root = tmp("trash-link");
    let trash_root = root.join("Trash");
    fs::create_dir_all(root.join("elsewhere")).unwrap();
    fs::write(root.join("elsewhere/target.txt"), b"t").unwrap();
    let link = root.join("link.txt");
    std::os::unix::fs::symlink(root.join("elsewhere/target.txt"), &link).unwrap();
    let mut req = Request::delete(vec![link.clone()], true);
    req.trash_root = Some(trash_root.clone());
    run(req);
    let info = fs::read_to_string(trash_root.join("info/link.txt.trashinfo")).unwrap();
    assert!(info.contains("link.txt"), "{info}");
    assert!(!info.contains("target.txt"), "{info}");
    assert!(
        root.join("elsewhere/target.txt").exists(),
        "the target is untouched"
    );
    fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn a_file_on_another_volume_goes_to_that_volumes_trash_and_back() {
    let Some(other) = other_device("vol-trash") else {
        return;
    };
    let f = other.join("on-the-other-volume.txt");
    fs::write(&f, b"v").unwrap();
    let t = send_to_trash(&f, None).unwrap();
    let crate::trashed::Place::Freedesktop { file, info } = &t.place else {
        panic!("{t:?}");
    };
    let uid = rustix::process::getuid().as_raw();
    // The top of that volume: /dev/shm, or what XDEVCMD_TEST_OTHER_DEV names.
    let top = other.parent().unwrap().to_path_buf();
    assert!(
        file.starts_with(top.join(format!(".Trash-{uid}/files"))),
        "{file:?}"
    );
    let record = fs::read_to_string(info).unwrap();
    // Relative to the volume's top, as the spec prefers for these.
    assert!(record.contains("Path=xdevcmd-xdev-vol-trash-"), "{record}");
    crate::trashed::restore(&t).unwrap();
    assert_eq!(fs::read(&f).unwrap(), b"v");
    fs::remove_dir_all(&other).unwrap();
    // Leave no trash directory behind if this test made it (only empty ones go).
    let trash = top.join(format!(".Trash-{uid}"));
    let _ = fs::remove_dir(trash.join("files"));
    let _ = fs::remove_dir(trash.join("info"));
    let _ = fs::remove_dir(&trash);
}

#[test]
fn trashing_two_files_of_the_same_name_does_not_lose_one() {
    let root = tmp("trash-dup");
    let trash_root = root.join("Trash");
    for dir in ["a", "b"] {
        fs::create_dir_all(root.join(dir)).unwrap();
        fs::write(root.join(dir).join("same.txt"), dir.as_bytes()).unwrap();
    }
    for dir in ["a", "b"] {
        let mut req = Request::delete(vec![root.join(dir).join("same.txt")], true);
        req.trash_root = Some(trash_root.clone());
        assert_eq!(run(req).done, 1);
    }
    assert_eq!(fs::read(trash_root.join("files/same.txt")).unwrap(), b"a");
    assert_eq!(fs::read(trash_root.join("files/same.txt.1")).unwrap(), b"b");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn copy_as_puts_a_second_copy_beside_the_first_and_still_asks_before_overwriting() {
    let root = std::env::temp_dir().join(format!("xdevcmd-ops-copyas-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("tree/sub")).unwrap();
    fs::write(root.join("a.txt"), "A").unwrap();
    fs::write(root.join("tree/sub/x"), "X").unwrap();
    let mut req = Request::copy(vec![root.join("a.txt")], root.clone());
    req.rename_to = Some("b.txt".into());
    assert_eq!(run(req.clone()).done, 1);
    assert_eq!(fs::read_to_string(root.join("b.txt")).unwrap(), "A");
    assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "A");
    // Again: b.txt exists now, so the worker asks; Skip leaves it.
    fs::write(root.join("b.txt"), "keep").unwrap();
    let (s, asked) = run_with(req, Answer::Skip);
    assert_eq!((s.skipped, asked), (1, 1));
    assert_eq!(fs::read_to_string(root.join("b.txt")).unwrap(), "keep");
    // A directory, recursively.
    let mut req = Request::copy(vec![root.join("tree")], root.clone());
    req.rename_to = Some("tree2".into());
    run(req);
    assert_eq!(fs::read_to_string(root.join("tree2/sub/x")).unwrap(), "X");
    let _ = fs::remove_dir_all(&root);
}

/// Reproduced in review: through `link -> d/inner` a copy of `d` into `link`
/// recursed about 500 levels and aborted on a stack overflow.
#[cfg(unix)]
#[test]
fn copying_or_moving_a_directory_into_itself_through_a_link_is_refused() {
    let root = tmp("self-link");
    let d = root.join("d");
    fs::create_dir_all(d.join("inner")).unwrap();
    fs::write(d.join("f"), b"f").unwrap();
    std::os::unix::fs::symlink(d.join("inner"), root.join("link")).unwrap();
    for req in [
        Request::copy(vec![d.clone()], root.join("link")),
        Request::mv(vec![d.clone()], root.join("link")),
        Request::copy(vec![d.clone()], d.join("inner").join("..").join("inner")),
    ] {
        let s = run(req);
        assert_eq!(s.failed, 1, "refused");
        assert_eq!(s.done, 0);
        assert!(!d.join("inner").join("d").exists(), "nothing made inside");
        assert!(d.join("f").is_file(), "the source is untouched");
    }
    // Onto itself: the destination is a link to d's parent.
    std::os::unix::fs::symlink(&root, root.join("up")).unwrap();
    let s = run(Request::copy(vec![d.clone()], root.join("up")));
    assert_eq!((s.failed, s.done, s.skipped), (0, 0, 1));
    assert_eq!(fs::read(d.join("f")).unwrap(), b"f");
    fs::remove_dir_all(&root).unwrap();
}

/// cosf C01, reproduced: through `link -> d/inner` to a folder not made yet
/// (`link/new`), the check could not resolve the folder and said no; the copy
/// recursed and aborted on a stack overflow.
#[cfg(unix)]
#[test]
fn copying_into_itself_through_a_link_to_a_missing_folder_is_refused() {
    let root = tmp("self-link-missing");
    let d = root.join("d");
    fs::create_dir_all(d.join("inner")).unwrap();
    fs::write(d.join("f"), b"f").unwrap();
    std::os::unix::fs::symlink(d.join("inner"), root.join("link")).unwrap();
    for req in [
        Request::copy(vec![d.clone()], root.join("link").join("new")),
        Request::mv(vec![d.clone()], root.join("link").join("new")),
        Request::copy(
            vec![d.clone()],
            root.join("link").join("new").join("deeper"),
        ),
    ] {
        let s = run(req);
        assert_eq!((s.failed, s.done), (1, 0), "refused: {s:?}");
        assert!(
            !d.join("inner").join("new").join("d").exists(),
            "nothing made inside"
        );
        assert!(d.join("f").is_file(), "the source is untouched");
    }
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn copying_a_directory_into_itself_is_refused() {
    let root = tmp("self-copy");
    let d = root.join("d");
    fs::create_dir_all(d.join("inner")).unwrap();
    fs::write(d.join("f"), b"f").unwrap();

    let s = run(Request::copy(vec![d.clone()], d.join("inner")));
    assert_eq!(s.failed, 1, "it must refuse rather than recurse forever");
    assert_eq!(s.done, 0);

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn cancelling_mid_file_removes_the_half_written_copy() {
    let root = tmp("cancel");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let big = root.join("big.bin");
    // Sparse and large, so the cancel lands mid-copy even on a loaded machine.
    File::create(&big).unwrap().set_len(1 << 30).unwrap();

    let mut req = Request::copy(vec![big], dst.clone());
    // The copy's own loop: a clone (APFS) is whole at once, nothing left to cancel.
    req.fast = false;
    let h = start(req);
    let mut cancelled_at_progress = false;
    let summary = loop {
        match h.events.recv_timeout(Duration::from_secs(20)) {
            Ok(Event::Progress { .. }) if !cancelled_at_progress => {
                cancelled_at_progress = true;
                h.cancel();
            }
            Ok(Event::Done(s)) => break s,
            Ok(_) => {}
            Err(e) => panic!("worker never finished: {e}"),
        }
    };

    assert!(
        cancelled_at_progress,
        "the file was too small to cancel mid-copy"
    );
    assert!(summary.cancelled, "the summary must say it was cancelled");
    assert_eq!(summary.done, 0, "a cancelled file is not a copied file");
    assert!(
        !dst.join("big.bin").exists(),
        "a partial copy must not be left looking complete"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn copying_a_fifo_fails_fast_instead_of_hanging() {
    let root = tmp("fifo");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    let fifo = root.join("pipe");
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());

    // run() panics if the worker has not finished in 20 s; before the fix it never did.
    let s = run(Request::copy(vec![fifo], dst.clone()));
    assert_eq!(s.failed, 1);
    assert!(s.errors[0].contains("FIFO"), "{:?}", s.errors);
    assert!(!dst.join("pipe").exists());
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn yes_to_all_stops_asking() {
    let root = tmp("yes-all");
    let dst = root.join("dst");
    fs::create_dir_all(&dst).unwrap();
    for i in 0..4 {
        fs::write(root.join(format!("f{i}")), b"new").unwrap();
        fs::write(dst.join(format!("f{i}")), b"old").unwrap();
    }
    let sources: Vec<PathBuf> = (0..4).map(|i| root.join(format!("f{i}"))).collect();

    let (s, asked) = run_with(Request::copy(sources, dst.clone()), Answer::YesToAll);
    assert_eq!(asked, 1, "after 'all' it must not ask again");
    assert_eq!(s.done, 4);
    assert_eq!(fs::read(dst.join("f3")).unwrap(), b"new");

    fs::remove_dir_all(&root).unwrap();
}
