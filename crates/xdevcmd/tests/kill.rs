//! The data-safety cases a machine can run (Round 036): killed at a chosen step, as a
//! power cut or `kill -9` would end it, then started again, on Linux, macOS and Windows
//! alike. Each kill waits for a state, not a time: a fault seam's `halt` marker or the
//! journal's checkpoint line, so a slow runner only takes longer. The hand cases that
//! need a real device or network stay in tests/manual (M-DS-02, 06, 07).
//!
//! | Case | Test |
//! |---|---|
//! | M-DS-01 kill a copy that replaces | `a_planned_copy_killed_…`; with the trash, `a_replace_killed_…` (Linux: the test build's trash) |
//! | M-DS-03 a big copy past a checkpoint | `a_big_copy_killed_after_a_checkpoint_…` |
//! | M-DS-04 a checked continue | `a_checked_resume_repairs_…` |
//! | M-DS-05 a move across devices | `a_move_killed_before_removing_its_source_…` (needs a second device) |

mod harness;

use std::path::Path;
use std::time::{Duration, Instant};

use harness::*;

/// No config of the user's: the defaults.
const CFG: &str = "";

/// M-DS-01. Round 030: a planned copy killed part way, then continued, does exactly what was
/// left of the plan: the file the user chose to replace is replaced, the one they chose
/// to leave is left, and neither is asked about again; a source finished is not redone.
#[test]
fn a_planned_copy_killed_then_continued_keeps_every_decision() {
    let s = Scratch::new("kill-plan");
    let aged = |p: &Path, body: &str, secs: u64| {
        std::fs::write(p, body).unwrap();
        std::fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - Duration::from_secs(secs))
            .unwrap();
    };
    s.file("a/a-fresh.txt", "a");
    s.file("a/b-fresh.txt", "b");
    aged(&s.a().join("c-newer.txt"), "mine", 3600);
    s.file("b/c-newer.txt", "theirs");
    s.file("a/d-older.txt", "new");
    aged(&s.b().join("d-older.txt"), "old", 3600);
    let marker = s.dir.join("halted");
    let spec = format!(
        "halt|rename|{}|{}",
        s.b().join("b-fresh.txt").display(),
        marker.display()
    );
    let mut x = Run::start_in(
        &s,
        &[s.a(), s.b()],
        CFG,
        &[("XDEVCMD_FAULTS", spec.as_str())],
    );
    x.wait_for("d-older ");
    x.send(b"*");
    x.wait_for("4 selected");
    x.send(F5);
    // Groups worst first: newer, then older. Older to replace; newer stays skip.
    x.wait_for("would replace a newer file  [skip]");
    x.send(DOWN);
    x.send(b" ");
    x.wait_for("would replace an older file  [replace]");
    x.send(ENTER);
    wait_file(&marker, "the job held at b's rename", |c| c.is_some());
    x.kill_hard();
    drop(x);
    assert_eq!(
        std::fs::read_to_string(s.b().join("a-fresh.txt")).unwrap(),
        "a"
    );
    assert!(
        !s.b().join("b-fresh.txt").exists(),
        "held before its rename"
    );
    assert_eq!(
        std::fs::read_to_string(s.b().join("d-older.txt")).unwrap(),
        "old"
    );
    // a-fresh is written over by nobody: a resume that redid it would change its date.
    let a_before = std::fs::metadata(s.b().join("a-fresh.txt"))
        .unwrap()
        .modified()
        .unwrap();

    let mut x = Run::start_in(&s, &[s.a(), s.b()], CFG, &[]);
    x.wait_for("Interrupted");
    x.send(ENTER); // continue
    x.wait_for_status("copied");
    assert!(!x.screen().contains("Already exists"), "{}", x.screen());
    assert_eq!(
        std::fs::read_to_string(s.b().join("b-fresh.txt")).unwrap(),
        "b"
    );
    assert_eq!(
        std::fs::read_to_string(s.b().join("d-older.txt")).unwrap(),
        "new"
    );
    assert_eq!(
        std::fs::read_to_string(s.b().join("c-newer.txt")).unwrap(),
        "theirs"
    );
    assert_eq!(
        std::fs::metadata(s.b().join("a-fresh.txt"))
            .unwrap()
            .modified()
            .unwrap(),
        a_before,
        "a finished source is not done again"
    );
    x.until("marks to clear", |scr| !scr.contains('\u{2022}'));
    x.quit();
}

/// M-DS-01 with the trash. Round 031: killed after the old file went to the trash and before the new one was
/// renamed in, the next start finishes the rename and says so; the old file is in the
/// trash (the scratch one: XDG_DATA_HOME). Linux only: the test build puts a replaced
/// file in the trash only there, where the trash is the test's own.
#[cfg(target_os = "linux")]
#[test]
fn a_replace_killed_after_the_trash_is_finished_at_the_next_start() {
    let s = Scratch::new("kill-swap");
    s.file("a/d-older.txt", "new");
    s.file("b/d-older.txt", "old");
    std::fs::File::options()
        .write(true)
        .open(s.b().join("d-older.txt"))
        .unwrap()
        .set_modified(std::time::SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
    let marker = s.dir.join("halted");
    let spec = format!(
        "halt|rename|{}|{}",
        s.b().join("d-older.txt").display(),
        marker.display()
    );
    let mut x = Run::start_in(
        &s,
        &[s.a(), s.b()],
        CFG,
        &[("XDEVCMD_FAULTS", spec.as_str())],
    );
    x.wait_for("d-older ");
    x.send(b"*");
    x.wait_for("1 selected");
    x.send(F5);
    x.wait_for("would replace an older file  [skip]");
    x.send(b" ");
    x.wait_for("would replace an older file  [replace]");
    x.send(ENTER);
    wait_file(&marker, "the job held at the rename", |c| c.is_some());
    x.kill_hard();
    drop(x);
    assert!(
        !s.b().join("d-older.txt").exists(),
        "held between the trash and the rename"
    );
    let trashed = s.dir.join("data/Trash/files/d-older.txt");
    assert_eq!(std::fs::read_to_string(&trashed).unwrap(), "old");

    let mut x = Run::start_in(&s, &[s.a(), s.b()], CFG, &[]);
    x.wait_for("1 replacement finished");
    assert_eq!(
        std::fs::read_to_string(s.b().join("d-older.txt")).unwrap(),
        "new"
    );
    assert_eq!(std::fs::read_to_string(&trashed).unwrap(), "old");
    x.send(b"\x1b");
    x.wait_gone("1 replacement finished");
    x.quit();
}

/// M-DS-05. A move across devices killed (SIGKILL) after a file's copy is on disk and the
/// journal says so, before its source is removed: the next start says both are there,
/// and both are, whole (Round 029, step 3). The fault seam holds the job at that point.
#[test]
fn a_move_killed_before_removing_its_source_says_both_are_there() {
    let s = Scratch::new("kill-move");
    let Some(other) = other_device("kill-move") else {
        return;
    };
    s.file("a/a.txt", "moved data");
    let marker = s.dir.join("halted");
    let spec = format!(
        "halt|remove-source|{}|{}",
        s.a().join("a.txt").display(),
        marker.display()
    );
    let mut x = Run::start_in(
        &s,
        &[s.a(), other.clone()],
        CFG,
        &[("XDEVCMD_FAULTS", spec.as_str())],
    );
    x.send(DOWN);
    x.send(F6);
    x.wait_for("[Enter] go");
    x.send(ENTER);
    x.wait_for("copy then delete");
    x.send(b"y");
    wait_file(&marker, "the job held before removing the source", |c| {
        c.is_some()
    });
    // As a power cut or `kill -9` would: no handler runs.
    x.kill_hard();
    drop(x);
    assert_eq!(
        std::fs::read_to_string(s.a().join("a.txt")).unwrap(),
        "moved data"
    );
    assert_eq!(
        std::fs::read_to_string(other.join("a.txt")).unwrap(),
        "moved data"
    );
    let mut x = Run::start_in(&s, &[s.a(), other.clone()], CFG, &[]);
    x.wait_for("Interrupted");
    x.wait_for("copied completely");
    x.wait_for("Both are kept");
    assert!(
        s.a().join("a.txt").exists(),
        "the next start removes nothing"
    );
    assert_eq!(
        std::fs::read_to_string(other.join("a.txt")).unwrap(),
        "moved data"
    );
    drop(x);
    let _ = std::fs::remove_dir_all(&other);
}

/// M-DS-03. A big copy killed (SIGKILL; TerminateProcess on Windows) after a checkpoint (Round 033): the next start keeps
/// what was on disk and Continue goes on from there; the debug log says how much was
/// not copied again. Read and write (fast copy off), slowed per chunk, so the kill
/// lands after the first checkpoint (256 MiB) and before the end.
#[test]
fn a_big_copy_killed_after_a_checkpoint_goes_on_from_there() {
    use std::io::{Read, Seek, SeekFrom, Write};
    let s = Scratch::new("kill-ckpt-kill");
    let src = s.a().join("big.bin");
    let size: u64 = 384 << 20;
    {
        // Sparse, with marks at both ends and in the middle.
        let mut f = std::fs::File::create(&src).unwrap();
        f.set_len(size).unwrap();
        for at in [0, 300 << 20, size - 4] {
            f.seek(SeekFrom::Start(at)).unwrap();
            f.write_all(b"MARK").unwrap();
        }
    }
    // A folder copied first (folders come before files): its folders are told to the
    // journal as finished.
    std::fs::create_dir_all(s.a().join("a-tree/sub")).unwrap();
    s.file("a/a-tree/sub/f.txt", "f");
    // Read and write: on APFS, Btrfs or XFS a copy on one volume is a clone, with no
    // checkpoint.
    let fast_off = "[general]\nfast_copy = false\n";
    let mut x = Run::start_in(
        &s,
        &[s.a(), s.b()],
        fast_off,
        &[("XDEVCMD_FAULTS", "delay|write|.big.bin.xdevcmd-part|2")],
    );
    x.send(DOWN);
    x.send(INSERT);
    x.send(INSERT);
    x.send(F5);
    x.wait_for("[Enter] go");
    x.send(ENTER);
    let journal = s.journal();
    let start = Instant::now();
    let has_ckpt = || {
        std::fs::read_dir(&journal).ok().is_some_and(|d| {
            d.flatten()
                .any(|e| std::fs::read_to_string(e.path()).is_ok_and(|t| t.contains("\nckpt ")))
        })
    };
    while !has_ckpt() {
        assert!(
            start.elapsed() < PATIENCE * 4,
            "no checkpoint:\n{}",
            x.screen()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let lines: String = std::fs::read_dir(&journal)
        .unwrap()
        .flatten()
        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
        .collect();
    assert!(
        lines.contains(&format!(
            "\ndone {}\n",
            hex(&s.a().join("a-tree").join("sub"))
        )),
        "a finished folder is in the journal:\n{lines}"
    );
    x.kill_hard();
    drop(x);
    assert!(
        !s.b().join("big.bin").exists(),
        "killed before the end: no big.bin under its name"
    );

    let mut x = Run::start_in(&s, &[s.a(), s.b()], fast_off, &[("XDEVCMD_DEBUG", "jobs")]);
    x.wait_for("Interrupted");
    x.wait_for("kept part way");
    x.send(ENTER); // continue
    x.wait_for("1 copied");
    assert_eq!(
        std::fs::read_to_string(s.b().join("a-tree/sub/f.txt")).unwrap(),
        "f"
    );
    let mut got = std::fs::File::open(s.b().join("big.bin")).unwrap();
    assert_eq!(got.metadata().unwrap().len(), size);
    for at in [0, 300 << 20, size - 4] {
        let mut b = [0u8; 4];
        got.seek(SeekFrom::Start(at)).unwrap();
        got.read_exact(&mut b).unwrap();
        assert_eq!(&b, b"MARK", "at {at}");
    }
    assert!(
        std::fs::read_dir(s.b()).unwrap().count() == 2,
        "no temporary file left"
    );
    x.quit();
    let log = std::fs::read_dir(&s.dir)
        .unwrap()
        .flatten()
        .find(|e| e.file_name().to_string_lossy().ends_with(".jsonl"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .expect("a debug log");
    let resumed: u64 = log
        .lines()
        .filter(|l| l.contains("\"job_end\""))
        .filter_map(|l| {
            let at = l.find("\"resumed_bytes\":")? + "\"resumed_bytes\":".len();
            l[at..]
                .split(|c: char| !c.is_ascii_digit())
                .next()?
                .parse()
                .ok()
        })
        .max()
        .unwrap_or(0);
    assert!(
        resumed >= 256 << 20,
        "taken over from the checkpoint: {resumed}"
    );
}

/// M-DS-04. After a power cut a file the journal calls done can hold the wrong bytes under its
/// real name, with the size and date of its source. C checks every file the run wrote,
/// the done ones too; it skipped them, so the wrong bytes stayed (#3, 2026-11-16).
#[test]
fn a_checked_resume_repairs_a_file_the_journal_called_done() {
    let s = Scratch::new("kill-resume-check");
    let (src, dst) = (s.a(), s.b());
    std::fs::write(src.join("a.txt"), "AAAA").unwrap();
    std::fs::write(src.join("b.txt"), "b").unwrap();
    std::fs::write(dst.join("a.txt"), "ZZZZ").unwrap();
    let date = std::fs::metadata(src.join("a.txt"))
        .unwrap()
        .modified()
        .unwrap();
    std::fs::File::options()
        .write(true)
        .open(dst.join("a.txt"))
        .unwrap()
        .set_modified(date)
        .unwrap();
    let journal = s.journal();
    std::fs::create_dir_all(&journal).unwrap();
    std::fs::write(
        journal.join("2147483646-0.journal"),
        format!(
            "{}kind copy\nsrc {}\nsrc {}\ndest {}\ndone {}\n",
            host_line(),
            hex(&src.join("a.txt")),
            hex(&src.join("b.txt")),
            hex(&dst),
            hex(&src.join("a.txt")),
        ),
    )
    .unwrap();
    let mut x = Run::start_in(&s, &[s.a(), s.b()], CFG, &[]);
    x.wait_for("Interrupted");
    x.send(b"c");
    wait_file(&dst.join("b.txt"), "b copied", |c| {
        c.as_deref() == Some("b")
    });
    wait_file(&dst.join("a.txt"), "a repaired", |c| {
        c.as_deref() == Some("AAAA")
    });
    // The job box ignores q: wait for its summary on the status line.
    x.wait_for_status("copied");
    x.quit();
}
