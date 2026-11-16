//! `xtask try`: the tester's sandbox (docs/testing-by-hand.md, plan A10 Track T).
//!
//! ```text
//! xtask try --make [--dir D] [--small]   make D/xdevcmd-try (D: here)
//! xtask try --check [--dir D]            PASS / FAIL / NOT DONE, from its manifest
//! xtask try --tamper [--dir D]           one finished copy zeroed, size and date kept
//! xtask try --clean [--dir D]            remove D/xdevcmd-try, and only that
//! ```
//!
//! Everything it makes is in one folder with a marker and a manifest (SHA-256 of every
//! file it wrote). `--check` reads the files the tester's copies made against it, so a
//! lost, short or empty file is a FAIL printed by name, not something to spot by eye.
//! `--clean` removes the folder only when it is a real folder holding the marker this
//! program wrote; it never follows a link.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The folder's name: always this, so `--clean` can never be pointed elsewhere.
pub const NAME: &str = "xdevcmd-try";
const MARKER: &str = ".xdevcmd-try";
const MAGIC: &str = "xdevcmd-try 1";
const MANIFEST: &str = ".xdevcmd-try-manifest";

/// How much it makes.
#[derive(Debug, Clone, Copy)]
pub struct Sizes {
    pub log: u64,
    pub many: usize,
    pub tree_dirs: usize,
    pub tree_files: usize,
    pub tree_file: usize,
    pub replace: usize,
    pub replace_file: usize,
    pub big: u64,
}

impl Sizes {
    pub const FULL: Sizes = Sizes {
        log: 1 << 30,
        many: 100_000,
        tree_dirs: 100,
        tree_files: 500,
        tree_file: 4 << 10,
        replace: 2_000,
        replace_file: 64 << 10,
        big: 1 << 30,
    };
    /// A slow disk or little space; the big file still passes one checkpoint (256 MiB).
    pub const SMALL: Sizes = Sizes {
        log: 64 << 20,
        many: 10_000,
        tree_dirs: 20,
        tree_files: 100,
        tree_file: 4 << 10,
        replace: 200,
        replace_file: 64 << 10,
        big: 300 << 20,
    };

    fn bytes(&self) -> u64 {
        self.log
            + (self.tree_dirs * self.tree_files * self.tree_file) as u64
            + 2 * (self.replace * self.replace_file) as u64
            + self.big
    }
}

pub fn run(args: &[String]) -> i32 {
    let mut dir = std::env::current_dir().unwrap_or_default();
    let mut what = None;
    let mut sizes = Sizes::FULL;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--make" | "--check" | "--tamper" | "--clean" => what = Some(a.clone()),
            "--small" => sizes = Sizes::SMALL,
            "--dir" => match it.next() {
                Some(d) => dir = PathBuf::from(d),
                None => return usage("--dir needs a folder"),
            },
            other => return usage(&format!("unknown: {other}")),
        }
    }
    let root = dir.join(NAME);
    let r = match what.as_deref() {
        Some("--make") => make(&root, sizes).map(|()| 0),
        Some("--check") => check(&root).map(|r| {
            print!("{}", r.text());
            i32::from(r.failed())
        }),
        Some("--tamper") => tamper(&root).map(|p| {
            println!("zeroed, its size and date kept: {}", p.display());
            0
        }),
        Some("--clean") => clean(&root).map(|()| {
            println!("removed {}", root.display());
            0
        }),
        _ => return usage("one of --make, --check, --tamper, --clean"),
    };
    r.unwrap_or_else(|e| {
        eprintln!("try: {e}");
        1
    })
}

fn usage(why: &str) -> i32 {
    eprintln!(
        "try: {why}\nusage: xtask try --make [--small] | --check | --tamper | --clean [--dir D]"
    );
    2
}

/// The bytes of file `n` of a group: different for each file, the same on every run.
fn fill(out: &mut impl Write, seed: u64, len: u64) -> io::Result<()> {
    let mut block = vec![0u8; 1 << 20];
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    for chunk in block.chunks_mut(8) {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let b = x.to_le_bytes();
        chunk.copy_from_slice(&b[..chunk.len()]);
    }
    let mut left = len;
    while left > 0 {
        let n = left.min(block.len() as u64) as usize;
        out.write_all(&block[..n])?;
        left -= n as u64;
    }
    Ok(())
}

fn write_file(path: &Path, seed: u64, len: u64) -> io::Result<()> {
    let mut f = io::BufWriter::new(File::create(path)?);
    fill(&mut f, seed, len)?;
    f.flush()
}

fn hash(path: &Path) -> io::Result<String> {
    let stop = std::sync::atomic::AtomicBool::new(false);
    xdevcmd_core::sha256::file(path, &stop)?.ok_or_else(|| io::Error::other("stopped"))
}

/// One line of the manifest: what a file must hold.
struct Line {
    kind: &'static str,
    rel: String,
    len: u64,
    sha: String,
}

pub fn make(root: &Path, s: Sizes) -> io::Result<()> {
    if fs::symlink_metadata(root).is_ok() {
        return Err(io::Error::other(format!(
            "{} is there already: --clean it first",
            root.display()
        )));
    }
    println!(
        "making {} (about {} MiB; the copies you make add about {} MiB)",
        root.display(),
        s.bytes() >> 20,
        (s.bytes() - s.log - (s.replace * s.replace_file) as u64) >> 20
    );
    fs::create_dir_all(root)?;
    fs::write(root.join(MARKER), format!("{MAGIC}\n"))?;
    let mut lines: Vec<Line> = Vec::new();
    let mut add = |kind, rel: String, len, path: &Path| -> io::Result<()> {
        lines.push(Line {
            kind,
            rel,
            len,
            sha: hash(path)?,
        });
        Ok(())
    };

    // The viewer's file: lines of text, so search and End mean something.
    {
        let mut f = io::BufWriter::new(File::create(root.join("big.log"))?);
        let mut n = 0u64;
        let mut written = 0u64;
        while written < s.log {
            let line = format!("{n:09} 2026-11-17 12:00:00 INFO something happened\n");
            f.write_all(line.as_bytes())?;
            written += line.len() as u64;
            n += 1;
        }
        f.flush()?;
    }
    // A folder of many names to open and leave.
    fs::create_dir(root.join("many"))?;
    for i in 0..s.many {
        File::create(root.join("many").join(format!("f{i:06}")))?;
    }
    // A tree to copy (the plan, a checked continue).
    for d in 0..s.tree_dirs {
        let dir = root.join("tree").join(format!("d{d:03}"));
        fs::create_dir_all(&dir)?;
        for i in 0..s.tree_files {
            let rel = format!("tree/d{d:03}/f{i:04}");
            let p = root.join(&rel);
            write_file(&p, (d * 100_000 + i) as u64, s.tree_file as u64)?;
            add("src", rel, s.tree_file as u64, &p)?;
        }
    }
    // The same names on both sides, the destination's older: a copy that replaces.
    fs::create_dir(root.join("replace-src"))?;
    fs::create_dir(root.join("replace-dst"))?;
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_577_836_800); // 2020-01-01
    for i in 0..s.replace {
        let name = format!("r{i:05}");
        let src = root.join("replace-src").join(&name);
        let dst = root.join("replace-dst").join(&name);
        write_file(&src, 7_000_000 + i as u64, s.replace_file as u64)?;
        write_file(&dst, 8_000_000 + i as u64, s.replace_file as u64)?;
        File::options().write(true).open(&dst)?.set_modified(old)?;
        add(
            "src",
            format!("replace-src/{name}"),
            s.replace_file as u64,
            &src,
        )?;
        add(
            "old",
            format!("replace-dst/{name}"),
            s.replace_file as u64,
            &dst,
        )?;
    }
    // One big file: a copy killed past its first checkpoint (256 MiB) goes on from it.
    fs::create_dir(root.join("big"))?;
    let big = root.join("big").join("big.bin");
    write_file(&big, 9_000_000, s.big)?;
    add("src", "big/big.bin".into(), s.big, &big)?;
    fs::create_dir(root.join("copy-to"))?;
    // Check B on APFS, Btrfs or XFS: there the copy is a clone, whole at once, with no
    // checkpoint to come back to. `XDEVCMD_CONFIG=xdevcmd-try/no-fast-copy.toml`.
    fs::write(
        root.join("no-fast-copy.toml"),
        "# xtask try: for check B where the copy would be a clone\n[general]\nfast_copy = false\n",
    )?;

    let mut m = String::new();
    for l in &lines {
        m.push_str(&format!("{}\t{}\t{}\t{}\n", l.kind, l.len, l.sha, l.rel));
    }
    fs::write(root.join(MANIFEST), m)?;
    println!("made; next: docs/testing-by-hand.md, \"Day 0\"");
    Ok(())
}

fn marked(root: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(root)
        .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", root.display())))?;
    if !meta.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a folder (a link is never followed)",
            root.display()
        )));
    }
    let m = fs::symlink_metadata(root.join(MARKER));
    let ok = m.is_ok_and(|m| m.is_file())
        && fs::read_to_string(root.join(MARKER)).is_ok_and(|t| t.trim() == MAGIC);
    if !ok {
        return Err(io::Error::other(format!(
            "{} has no marker of xtask try: not touched",
            root.display()
        )));
    }
    Ok(())
}

fn manifest(root: &Path) -> io::Result<Vec<Line>> {
    let text = fs::read_to_string(root.join(MANIFEST))?;
    let mut out = Vec::new();
    for l in text.lines() {
        let mut w = l.splitn(4, '\t');
        let (Some(kind), Some(len), Some(sha), Some(rel)) =
            (w.next(), w.next(), w.next(), w.next())
        else {
            continue;
        };
        out.push(Line {
            kind: if kind == "old" { "old" } else { "src" },
            rel: rel.to_string(),
            len: len.parse().unwrap_or(0),
            sha: sha.to_string(),
        });
    }
    Ok(out)
}

/// What `--check` found, by area.
#[derive(Debug, Default)]
pub struct Report {
    /// (area, PASS / FAIL / NOT DONE, what is wrong or left).
    pub areas: Vec<(&'static str, &'static str, Vec<String>)>,
}

impl Report {
    pub fn failed(&self) -> bool {
        self.areas.iter().any(|(_, s, _)| *s == "FAIL")
    }
    pub fn text(&self) -> String {
        let mut s = String::new();
        for (area, state, what) in &self.areas {
            s.push_str(&format!("{state:<8} {area}\n"));
            for w in what.iter().take(10) {
                s.push_str(&format!("         {w}\n"));
            }
            if what.len() > 10 {
                s.push_str(&format!("         … and {} more\n", what.len() - 10));
            }
        }
        s.push_str(if self.failed() {
            "FAIL: a file lost, short or wrong. Report it as P0 (docs/testing-by-hand.md).\n"
        } else {
            "no file lost or wrong\n"
        });
        s
    }
}

/// How a file compares with what it must hold.
fn state(path: &Path, len: u64, sha: &str) -> Result<bool, String> {
    let Ok(m) = fs::symlink_metadata(path) else {
        return Ok(false);
    };
    if !m.is_file() {
        return Err(format!("{}: not a file", path.display()));
    }
    if m.len() != len {
        return Err(format!("{}: {} bytes, not {len}", path.display(), m.len()));
    }
    match hash(path) {
        Ok(h) if h == sha => Ok(true),
        Ok(_) => Err(format!("{}: other bytes than its source", path.display())),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

pub fn check(root: &Path) -> io::Result<Report> {
    marked(root)?;
    let lines = manifest(root)?;
    let mut r = Report::default();
    // Sources: never touched by a copy.
    let mut bad = Vec::new();
    for l in lines.iter().filter(|l| l.kind == "src") {
        match state(&root.join(&l.rel), l.len, &l.sha) {
            Ok(true) => {}
            Ok(false) => bad.push(format!("{}: gone", l.rel)),
            Err(e) => bad.push(e),
        }
    }
    r.areas.push((
        "sources unchanged",
        if bad.is_empty() { "PASS" } else { "FAIL" },
        bad,
    ));
    // A copy into copy-to/: each file there whole, or not there yet.
    for (area, prefix) in [("copy of tree/", "tree/"), ("copy of big/", "big/")] {
        let (mut bad, mut left) = (Vec::new(), 0);
        for l in lines
            .iter()
            .filter(|l| l.kind == "src" && l.rel.starts_with(prefix))
        {
            match state(&root.join("copy-to").join(&l.rel), l.len, &l.sha) {
                Ok(true) => {}
                Ok(false) => left += 1,
                Err(e) => bad.push(e),
            }
        }
        let st = match (bad.is_empty(), left) {
            (false, _) => "FAIL",
            (true, 0) => "PASS",
            _ => "NOT DONE",
        };
        if bad.is_empty() && left > 0 {
            bad.push(format!("{left} not copied yet"));
        }
        r.areas.push((area, st, bad));
    }
    // The replace: each destination is the new file, or still the old one.
    let (mut bad, mut old) = (Vec::new(), 0);
    for l in lines
        .iter()
        .filter(|l| l.kind == "src" && l.rel.starts_with("replace-src/"))
    {
        let name = &l.rel["replace-src/".len()..];
        let dst = root.join("replace-dst").join(name);
        let was = lines
            .iter()
            .find(|o| o.kind == "old" && o.rel == format!("replace-dst/{name}"));
        match state(&dst, l.len, &l.sha) {
            Ok(true) => {}
            Ok(false) => bad.push(format!("replace-dst/{name}: gone")),
            Err(e) => match was.map(|o| state(&dst, o.len, &o.sha)) {
                Some(Ok(true)) => old += 1,
                _ => bad.push(e),
            },
        }
    }
    let st = match (bad.is_empty(), old) {
        (false, _) => "FAIL",
        (true, 0) => "PASS",
        _ => "NOT DONE",
    };
    if bad.is_empty() && old > 0 {
        bad.push(format!("{old} not replaced yet"));
    }
    r.areas.push(("replace-src onto replace-dst", st, bad));
    Ok(r)
}

/// A finished copy under copy-to/tree made wrong as a power cut can: zeros, with its
/// size and its source's date, so only a check that reads it (C) can tell.
pub fn tamper(root: &Path) -> io::Result<PathBuf> {
    marked(root)?;
    let lines = manifest(root)?;
    for l in lines
        .iter()
        .filter(|l| l.kind == "src" && l.rel.starts_with("tree/"))
    {
        let copy = root.join("copy-to").join(&l.rel);
        if state(&copy, l.len, &l.sha) == Ok(true) {
            let when = fs::metadata(root.join(&l.rel))?.modified()?;
            let f = File::options().write(true).truncate(true).open(&copy)?;
            f.set_len(l.len)?;
            f.set_modified(when)?;
            return Ok(copy);
        }
    }
    Err(io::Error::other(
        "no finished copy under copy-to/tree yet: copy tree/ first",
    ))
}

/// Remove the sandbox: a real folder named xdevcmd-try holding this program's marker.
pub fn clean(root: &Path) -> io::Result<()> {
    if root.file_name().and_then(|n| n.to_str()) != Some(NAME) {
        return Err(io::Error::other("not a sandbox of xtask try"));
    }
    marked(root)?;
    // remove_dir_all removes links inside as links: it never follows one out.
    fs::remove_dir_all(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TINY: Sizes = Sizes {
        log: 4 << 10,
        many: 20,
        tree_dirs: 2,
        tree_files: 3,
        tree_file: 1 << 10,
        replace: 3,
        replace_file: 2 << 10,
        big: 64 << 10,
    };

    fn copy_tree(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for e in fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            let t = to.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                copy_tree(&e.path(), &t);
            } else {
                fs::copy(e.path(), &t).unwrap();
            }
        }
    }

    fn state_of<'a>(r: &'a Report, area: &str) -> &'a str {
        r.areas.iter().find(|a| a.0 == area).unwrap().1
    }

    /// The whole loop: made, nothing done, copies done right, one zeroed (FAIL until
    /// repaired), a source changed (FAIL), cleaned.
    #[test]
    fn a_sandbox_is_made_checked_tampered_and_cleaned() {
        let top = std::env::temp_dir().join(format!("xtask-try-{}", std::process::id()));
        let _ = fs::remove_dir_all(&top);
        fs::create_dir_all(&top).unwrap();
        let root = top.join(NAME);
        make(&root, TINY).unwrap();
        assert!(make(&root, TINY).is_err(), "never over an existing one");

        let r = check(&root).unwrap();
        assert!(!r.failed(), "{}", r.text());
        assert_eq!(state_of(&r, "copy of tree/"), "NOT DONE");
        assert_eq!(state_of(&r, "replace-src onto replace-dst"), "NOT DONE");

        copy_tree(&root.join("tree"), &root.join("copy-to/tree"));
        copy_tree(&root.join("big"), &root.join("copy-to/big"));
        for e in fs::read_dir(root.join("replace-src")).unwrap() {
            let e = e.unwrap();
            fs::copy(e.path(), root.join("replace-dst").join(e.file_name())).unwrap();
        }
        let r = check(&root).unwrap();
        assert!(r.areas.iter().all(|a| a.1 == "PASS"), "{}", r.text());

        let z = tamper(&root).unwrap();
        let r = check(&root).unwrap();
        assert_eq!(state_of(&r, "copy of tree/"), "FAIL", "{}", r.text());
        assert!(r.text().contains("other bytes"), "{}", r.text());
        let rel = z.strip_prefix(root.join("copy-to")).unwrap();
        fs::copy(root.join(rel), &z).unwrap();

        fs::write(root.join("replace-src/r00001"), "short").unwrap();
        let r = check(&root).unwrap();
        assert_eq!(state_of(&r, "sources unchanged"), "FAIL", "{}", r.text());

        clean(&root).unwrap();
        assert!(!root.exists());
        let _ = fs::remove_dir_all(&top);
    }

    /// --clean touches nothing it did not make: no marker, another name, or a link.
    #[test]
    fn clean_never_removes_what_it_did_not_make() {
        let top = std::env::temp_dir().join(format!("xtask-try-clean-{}", std::process::id()));
        let _ = fs::remove_dir_all(&top);
        let precious = top.join("precious");
        fs::create_dir_all(&precious).unwrap();
        fs::write(precious.join("keep"), "k").unwrap();
        // The right name, no marker.
        let fake = top.join(NAME);
        fs::create_dir_all(&fake).unwrap();
        fs::write(fake.join("keep"), "k").unwrap();
        assert!(clean(&fake).is_err());
        assert!(fake.join("keep").exists());
        // A marker, another name.
        fs::write(precious.join(MARKER), format!("{MAGIC}\n")).unwrap();
        assert!(clean(&precious).is_err());
        assert!(precious.join("keep").exists());
        #[cfg(unix)]
        {
            // A link named xdevcmd-try to a folder with a marker.
            fs::remove_dir_all(&fake).unwrap();
            std::os::unix::fs::symlink(&precious, &fake).unwrap();
            assert!(clean(&fake).is_err());
            assert!(precious.join("keep").exists());
        }
        let _ = fs::remove_dir_all(&top);
    }
}
