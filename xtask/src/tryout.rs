//! `xtask try`: the tester's sandbox (docs/how-to-test.md, plan A10 Track T).
//!
//! ```text
//! xtask try --make [--small] [--dir D]   make D/xdevcmd-try (D: here; it must exist)
//! xtask try --kill-when A|B|C [--dir D]  wait, then kill xdevcmd at the right moment
//! xtask try --check [--dir D]            PASS / FAIL / NOT DONE, from its manifest
//! xtask try --tamper [--dir D]           one finished copy zeroed, size and date kept
//! xtask try --clean [--dir D]            remove D/xdevcmd-try, and only what it made
//! ```
//!
//! Everything it makes is in one folder with a marker and a manifest (SHA-256 of every
//! file it wrote, and how many). `--check` reads the tester's copies against it, so a
//! lost, short or empty file is a FAIL printed by name, not something to spot by eye;
//! a missing or cut manifest is an error, never a PASS. It never runs, looks for or
//! asks an installed xdevcmd, except `--kill-when`, which kills it by name.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// The folder's name: always this, so `--clean` can never be pointed elsewhere.
pub const NAME: &str = "xdevcmd-try";
const MARKER: &str = ".xdevcmd-try";
const MAGIC: &str = "xdevcmd-try 1";
const MANIFEST: &str = ".xdevcmd-try-manifest";
/// What `--make` puts at the top: `--clean` removes these and nothing else.
const MADE: &[&str] = &[
    "big.log",
    "many",
    "tree",
    "replace-src",
    "replace-dst",
    "big",
    "copy-to",
    "no-fast-copy.toml",
    MANIFEST,
];
/// The older date of replace-dst's files: what tells a replaced one apart.
const OLD: u64 = 1_577_836_800; // 2020-01-01
/// xdevcmd keeps a big file's progress every 256 MiB (ops.rs CHECKPOINT): check B kills
/// a little past the first one.
const PAST_CHECKPOINT: u64 = (256 + 32) << 20;

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
    /// A slow disk or little space; the big file still passes the first checkpoint
    /// with room to kill it there.
    pub const SMALL: Sizes = Sizes {
        log: 64 << 20,
        many: 10_000,
        tree_dirs: 20,
        tree_files: 100,
        tree_file: 4 << 10,
        replace: 200,
        replace_file: 64 << 10,
        big: 512 << 20,
    };

    fn bytes(&self) -> u64 {
        self.log
            + (self.tree_dirs * self.tree_files * self.tree_file) as u64
            + 2 * (self.replace * self.replace_file) as u64
            + self.big
    }
}

pub fn run(args: &[String], version: &str) -> i32 {
    let mut dir = std::env::current_dir().unwrap_or_default();
    let mut what = None;
    let mut sizes = Sizes::FULL;
    let mut when = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--make" | "--check" | "--tamper" | "--clean" => what = Some(a.clone()),
            "--kill-when" => match it.next().map(|c| c.to_ascii_uppercase()) {
                Some(c) if ["A", "B", "C"].contains(&c.as_str()) => {
                    what = Some(a.clone());
                    when = Some(c);
                }
                _ => return usage("--kill-when needs A, B or C"),
            },
            "--small" => sizes = Sizes::SMALL,
            "--dir" => match it.next() {
                Some(d) => dir = PathBuf::from(d),
                None => return usage("--dir needs a folder"),
            },
            other => return usage(&format!("unknown: {other}")),
        }
    }
    let r = match what.as_deref() {
        Some("--make") => make(&dir.join(NAME), sizes, version).map(|()| 0),
        Some(w) => find(&dir).and_then(|root| match w {
            "--check" => check(&root).map(|r| {
                print!("{}", r.text());
                i32::from(r.failed())
            }),
            "--tamper" => tamper(&root).map(|p| {
                println!("zeroed, its size and date kept: {}", p.display());
                0
            }),
            "--clean" => clean(&root).map(|()| {
                println!("removed {}", root.display());
                0
            }),
            _ => kill_when(&root, when.as_deref().unwrap_or("A")),
        }),
        None => return usage("one of --make, --kill-when, --check, --tamper, --clean"),
    };
    r.unwrap_or_else(|e| {
        eprintln!("try: {e}");
        1
    })
}

fn usage(why: &str) -> i32 {
    eprintln!("try: {why}\n{HELP}");
    2
}

pub const HELP: &str =
    "usage: xtask try --make [--small] | --kill-when A|B|C | --check | --tamper | --clean [--dir D]
  --make        make D/xdevcmd-try (2.5 GiB; --small: 0.8 GiB), with a manifest
  --kill-when   run before Enter on check A, B or C: waits, kills xdevcmd mid-job
  --check       every copy against the manifest: PASS, FAIL (a P0) or NOT DONE
  --tamper      check C: one finished copy zeroed, its size and date kept
  --clean       remove the sandbox, only what --make made in it";

/// An error that names its path.
fn at(path: &Path) -> impl Fn(io::Error) -> io::Error + '_ {
    move |e| io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

/// The sandbox `--dir` (or here) means: D/xdevcmd-try, or D itself or a folder above it
/// when that is a sandbox (`--check` from xdevcmd's shell, which opens inside it).
fn find(dir: &Path) -> io::Result<PathBuf> {
    let inside = dir.join(NAME);
    if fs::symlink_metadata(&inside).is_ok() {
        return Ok(inside);
    }
    for d in dir.ancestors() {
        if d.file_name().and_then(|n| n.to_str()) == Some(NAME) && marked(d).is_ok() {
            return Ok(d.to_path_buf());
        }
    }
    Err(io::Error::other(format!(
        "no {NAME} in or above {}: --dir names the folder that holds it",
        dir.display()
    )))
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
    let mut f = io::BufWriter::new(File::create(path).map_err(at(path))?);
    fill(&mut f, seed, len).map_err(at(path))?;
    f.flush().map_err(at(path))
}

fn hash(path: &Path) -> io::Result<String> {
    let stop = std::sync::atomic::AtomicBool::new(false);
    xdevcmd_core::sha256::file(path, &stop)
        .map_err(at(path))?
        .ok_or_else(|| io::Error::other("stopped"))
}

/// One line of the manifest: what a file must hold.
struct Line {
    kind: &'static str,
    rel: String,
    len: u64,
    sha: String,
}

pub fn make(root: &Path, s: Sizes, version: &str) -> io::Result<()> {
    let parent = root.parent().unwrap_or(Path::new("."));
    if !parent.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a folder: make it first, or name another with --dir",
            parent.display()
        )));
    }
    if fs::symlink_metadata(root).is_ok() {
        return Err(io::Error::other(format!(
            "{} is there already: --clean it first",
            root.display()
        )));
    }
    println!(
        "xtask {version}: making {} (about {} MiB; the copies you make add about {} MiB)",
        root.display(),
        s.bytes() >> 20,
        (s.bytes() - s.log - (s.replace * s.replace_file) as u64) >> 20
    );
    fs::create_dir(root).map_err(at(root))?;
    fs::write(root.join(MARKER), format!("{MAGIC}\nversion {version}\n")).map_err(at(root))?;
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
        let p = root.join("big.log");
        let mut f = io::BufWriter::new(File::create(&p).map_err(at(&p))?);
        let (mut n, mut written) = (0u64, 0u64);
        while written < s.log {
            let line = format!("{n:09} 2026-11-17 12:00:00 INFO something happened\n");
            f.write_all(line.as_bytes()).map_err(at(&p))?;
            written += line.len() as u64;
            n += 1;
        }
        f.flush().map_err(at(&p))?;
    }
    // A folder of many names to open and leave.
    let many = root.join("many");
    fs::create_dir(&many).map_err(at(&many))?;
    for i in 0..s.many {
        let p = many.join(format!("f{i:06}"));
        File::create(&p).map_err(at(&p))?;
    }
    // A tree to copy (check C).
    for d in 0..s.tree_dirs {
        let dir = root.join("tree").join(format!("d{d:03}"));
        fs::create_dir_all(&dir).map_err(at(&dir))?;
        for i in 0..s.tree_files {
            let rel = format!("tree/d{d:03}/f{i:04}");
            let p = root.join(&rel);
            write_file(&p, (d * 100_000 + i) as u64, s.tree_file as u64)?;
            add("src", rel, s.tree_file as u64, &p)?;
        }
    }
    // The same names on both sides, the destination's older: a copy that replaces.
    for d in ["replace-src", "replace-dst"] {
        fs::create_dir(root.join(d)).map_err(at(&root.join(d)))?;
    }
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(OLD);
    for i in 0..s.replace {
        let name = format!("r{i:05}");
        let src = root.join("replace-src").join(&name);
        let dst = root.join("replace-dst").join(&name);
        write_file(&src, 7_000_000 + i as u64, s.replace_file as u64)?;
        write_file(&dst, 8_000_000 + i as u64, s.replace_file as u64)?;
        File::options()
            .write(true)
            .open(&dst)
            .and_then(|f| f.set_modified(old))
            .map_err(at(&dst))?;
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
    // One big file: a copy killed past its first checkpoint goes on from it.
    let big = root.join("big");
    fs::create_dir(&big).map_err(at(&big))?;
    write_file(&big.join("big.bin"), 9_000_000, s.big)?;
    add("src", "big/big.bin".into(), s.big, &big.join("big.bin"))?;
    fs::create_dir(root.join("copy-to")).map_err(at(root))?;
    // Check B on APFS, Btrfs or XFS: there the copy is a clone, whole at once, with no
    // checkpoint to come back to. `XDEVCMD_CONFIG=xdevcmd-try/no-fast-copy.toml`.
    fs::write(
        root.join("no-fast-copy.toml"),
        "# xtask try: for check B where the copy would be a clone\n[general]\nfast_copy = false\n",
    )
    .map_err(at(root))?;

    let srcs = lines.iter().filter(|l| l.kind == "src").count();
    let olds = lines.len() - srcs;
    let mut m = format!("{MAGIC} manifest src={srcs} old={olds}\n");
    for l in &lines {
        m.push_str(&format!("{}\t{}\t{}\t{}\n", l.kind, l.len, l.sha, l.rel));
    }
    fs::write(root.join(MANIFEST), m).map_err(at(root))?;
    println!("made; next: docs/how-to-test.md, step 4");
    Ok(())
}

fn marked(root: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(root).map_err(at(root))?;
    if !meta.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a folder (a link is never followed)",
            root.display()
        )));
    }
    let m = root.join(MARKER);
    let ok = fs::symlink_metadata(&m).is_ok_and(|m| m.is_file())
        && fs::read_to_string(&m).is_ok_and(|t| t.lines().next() == Some(MAGIC));
    if !ok {
        return Err(io::Error::other(format!(
            "{} has no marker of xtask try: not touched",
            root.display()
        )));
    }
    Ok(())
}

/// The manifest, whole: its first line counts its lines, so a cut or emptied one is an
/// error, never an empty list that passes everything.
fn manifest(root: &Path) -> io::Result<Vec<Line>> {
    let p = root.join(MANIFEST);
    let text = fs::read_to_string(&p).map_err(at(&p))?;
    let bad =
        |why: &str| io::Error::other(format!("{}: {why}; make the sandbox again", p.display()));
    let mut rows = text.lines();
    let head = rows.next().ok_or_else(|| bad("empty"))?;
    let count = |key: &str| -> Option<usize> {
        head.split_whitespace()
            .find_map(|w| w.strip_prefix(key))
            .and_then(|n| n.parse().ok())
    };
    let (Some(srcs), Some(olds)) = (count("src="), count("old=")) else {
        return Err(bad("no counts on its first line"));
    };
    let mut out = Vec::new();
    for l in rows {
        let l = l.trim_end_matches('\r');
        let mut w = l.splitn(4, '\t');
        let (Some(kind), Some(len), Some(sha), Some(rel)) =
            (w.next(), w.next(), w.next(), w.next())
        else {
            return Err(bad("a line cut short"));
        };
        let (Ok(len), true) = (len.parse(), sha.len() == 64) else {
            return Err(bad("a line that does not read"));
        };
        out.push(Line {
            kind: match kind {
                "src" => "src",
                "old" => "old",
                _ => return Err(bad("a line of no known kind")),
            },
            rel: rel.to_string(),
            len,
            sha: sha.to_string(),
        });
    }
    let n = out.iter().filter(|l| l.kind == "src").count();
    if n != srcs || out.len() - n != olds || srcs == 0 {
        return Err(bad("lines missing"));
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
        let all = self.areas.iter().all(|(_, s, _)| *s == "PASS");
        s.push_str(if self.failed() {
            "FAIL: a file lost, short or wrong. Report it as P0 (docs/how-to-test.md).\n"
        } else if all {
            "PASS: every copy whole, no file lost or wrong\n"
        } else {
            "no file lost or wrong so far; what is NOT DONE is not checked yet\n"
        });
        s
    }
}

/// How a file compares with what it must hold: `Ok(true)` it does, `Ok(false)` it is
/// not there, `Err` it is there and wrong.
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
        Err(e) => Err(e.to_string()),
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
    for (area, prefix) in [
        ("copy of tree/ (check C)", "tree/"),
        ("copy of big/ (check B)", "big/"),
    ] {
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
    r.areas
        .push(("replace-src onto replace-dst (check A)", st, bad));
    Ok(r)
}

/// A finished copy under copy-to/tree made wrong as a power cut can: zeros, with its
/// size and its source's date, so only a check that reads it (C) can tell. Never a
/// link, nor a file that is also its source (a hard link, a copy-to that is a link).
pub fn tamper(root: &Path) -> io::Result<PathBuf> {
    marked(root)?;
    let lines = manifest(root)?;
    let copy_to = root.join("copy-to");
    if !fs::symlink_metadata(&copy_to).is_ok_and(|m| m.is_dir()) {
        return Err(io::Error::other(format!(
            "{} is not a folder of its own: not touched",
            copy_to.display()
        )));
    }
    for l in lines
        .iter()
        .filter(|l| l.kind == "src" && l.rel.starts_with("tree/"))
    {
        let copy = copy_to.join(&l.rel);
        let src = root.join(&l.rel);
        let Ok(m) = fs::symlink_metadata(&copy) else {
            continue;
        };
        if !m.is_file() || !own_file(&m, &src) || state(&copy, l.len, &l.sha) != Ok(true) {
            continue;
        }
        let when = fs::metadata(&src)
            .and_then(|m| m.modified())
            .map_err(at(&src))?;
        let f = File::options()
            .write(true)
            .truncate(true)
            .open(&copy)
            .map_err(at(&copy))?;
        f.set_len(l.len).map_err(at(&copy))?;
        f.set_modified(when).map_err(at(&copy))?;
        return Ok(copy);
    }
    Err(io::Error::other(
        "no finished copy of its own under copy-to/tree yet: do check C's copy first",
    ))
}

/// Whether a file is only itself: not the source by another name (a hard link).
fn own_file(m: &fs::Metadata, src: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let s = fs::metadata(src);
        m.nlink() == 1 && !s.is_ok_and(|s| s.ino() == m.ino() && s.dev() == m.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = (m, src);
        true
    }
}

/// Remove the sandbox: a real folder named xdevcmd-try holding this program's marker
/// and nothing at its top that `--make` did not make. The marker goes last, so a clean
/// cut short (a file held open on Windows) can be run again.
pub fn clean(root: &Path) -> io::Result<()> {
    if root.file_name().and_then(|n| n.to_str()) != Some(NAME) {
        return Err(io::Error::other("not a sandbox of xtask try"));
    }
    marked(root)?;
    let mut extra = Vec::new();
    for e in fs::read_dir(root).map_err(at(root))? {
        let n = e.map_err(at(root))?.file_name();
        let n = n.to_string_lossy();
        if n != MARKER && !MADE.contains(&n.as_ref()) {
            extra.push(n.into_owned());
        }
    }
    if !extra.is_empty() {
        return Err(io::Error::other(format!(
            "{} holds what xtask try did not make: {}. Move it out, then --clean.",
            root.display(),
            extra.join(", ")
        )));
    }
    for n in MADE {
        let p = root.join(n);
        match fs::symlink_metadata(&p) {
            // remove_dir_all removes links inside as links: it never follows one out.
            Ok(m) if m.is_dir() => fs::remove_dir_all(&p).map_err(at(&p))?,
            Ok(_) => fs::remove_file(&p).map_err(at(&p))?,
            Err(_) => {}
        }
    }
    fs::remove_file(root.join(MARKER)).map_err(at(root))?;
    fs::remove_dir(root).map_err(at(root))
}

/// How far the copy of check `c` has got, from the sandbox: `None` before it starts,
/// `Some(true)` at the point to kill, `Some(false)` past it (finished: too late).
fn progress(root: &Path, c: &str, lines: &[Line]) -> Option<bool> {
    let count = |prefix: &str| {
        lines
            .iter()
            .filter(|l| l.kind == "src" && l.rel.starts_with(prefix))
            .count()
    };
    match c {
        // A: replace-dst's files that no longer have the old date.
        "A" => {
            let n = count("replace-src/");
            let old = SystemTime::UNIX_EPOCH + Duration::from_secs(OLD);
            let done = fs::read_dir(root.join("replace-dst"))
                .ok()?
                .flatten()
                .filter(|e| {
                    e.metadata()
                        .and_then(|m| m.modified())
                        .is_ok_and(|t| t != old)
                })
                .count();
            reached(done, n, (n / 10).max(1))
        }
        // B: the big file's temporary copy past the first checkpoint.
        "B" => {
            let dir = root.join("copy-to").join("big");
            if fs::symlink_metadata(dir.join("big.bin")).is_ok() {
                return Some(false);
            }
            let part = fs::read_dir(&dir)
                .ok()?
                .flatten()
                .find(|e| e.file_name().to_string_lossy().contains(".xdevcmd-part-"))?;
            let len = part.metadata().ok()?.len();
            (len >= PAST_CHECKPOINT).then_some(true)
        }
        // C: a fifth of the tree's files copied, under their own names.
        _ => {
            let n = count("tree/");
            let done: usize = fs::read_dir(root.join("copy-to").join("tree"))
                .ok()?
                .flatten()
                .map(|d| {
                    fs::read_dir(d.path()).map_or(0, |f| {
                        f.flatten()
                            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                            .count()
                    })
                })
                .sum();
            reached(done, n, (n / 5).max(1))
        }
    }
}

/// `None` before `at`, `Some(true)` from `at` until all are done, `Some(false)` then.
fn reached(done: usize, all: usize, at: usize) -> Option<bool> {
    if done >= all {
        Some(false)
    } else {
        (done >= at).then_some(true)
    }
}

/// `--kill-when A|B|C`: run before Enter on that check's copy. Waits for the copy to
/// reach its point, then kills xdevcmd as a crash would (`kill -9`, End task).
fn kill_when(root: &Path, c: &str) -> io::Result<i32> {
    marked(root)?;
    let lines = manifest(root)?;
    println!("check {c}: waiting for the copy; press Enter on it in xdevcmd now");
    let start = Instant::now();
    loop {
        match progress(root, c, &lines) {
            Some(true) => break,
            Some(false) => {
                return Err(io::Error::other(
                    "the copy finished before its point: --clean, --make, and try again",
                ))
            }
            None if start.elapsed() > Duration::from_secs(600) => {
                return Err(io::Error::other("no copy started in 10 minutes"))
            }
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    let killed = if cfg!(windows) {
        std::process::Command::new("taskkill")
            .args(["/F", "/IM", "xdevcmd.exe"])
            .output()
    } else {
        std::process::Command::new("pkill")
            .args(["-9", "-x", "xdevcmd"])
            .output()
    }
    .map_err(|e| io::Error::other(format!("cannot kill xdevcmd: {e}")))?;
    if !killed.status.success() {
        return Err(io::Error::other("no xdevcmd was running to kill"));
    }
    println!(
        "killed xdevcmd mid-job. Start it again{} and do as the check says.",
        if cfg!(windows) {
            ""
        } else {
            " (type `reset` first if the terminal looks wrong)"
        }
    );
    Ok(0)
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

    fn top(tag: &str) -> PathBuf {
        let t = std::env::temp_dir().join(format!("xtask-try-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        fs::create_dir_all(&t).unwrap();
        t
    }

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
        r.areas.iter().find(|a| a.0.starts_with(area)).unwrap().1
    }

    /// The whole loop: made, nothing done, copies done right, one zeroed (FAIL until
    /// repaired), a source changed (FAIL), found from inside, cleaned.
    #[test]
    fn a_sandbox_is_made_checked_tampered_and_cleaned() {
        let t = top("loop");
        let root = t.join(NAME);
        make(&root, TINY, "test").unwrap();
        assert!(
            make(&root, TINY, "test").is_err(),
            "never over an existing one"
        );

        let r = check(&root).unwrap();
        assert!(!r.failed(), "{}", r.text());
        assert_eq!(state_of(&r, "copy of tree/"), "NOT DONE");
        assert!(
            r.text().contains("NOT DONE is not checked yet"),
            "{}",
            r.text()
        );

        copy_tree(&root.join("tree"), &root.join("copy-to/tree"));
        copy_tree(&root.join("big"), &root.join("copy-to/big"));
        for e in fs::read_dir(root.join("replace-src")).unwrap() {
            let e = e.unwrap();
            fs::copy(e.path(), root.join("replace-dst").join(e.file_name())).unwrap();
        }
        let r = check(&root).unwrap();
        assert!(r.text().contains("PASS: every copy whole"), "{}", r.text());

        let z = tamper(&root).unwrap();
        let r = check(&root).unwrap();
        assert_eq!(state_of(&r, "copy of tree/"), "FAIL", "{}", r.text());
        let rel = z.strip_prefix(root.join("copy-to")).unwrap();
        fs::copy(root.join(rel), &z).unwrap();

        fs::write(root.join("replace-src/r00001"), "short").unwrap();
        let r = check(&root).unwrap();
        assert_eq!(state_of(&r, "sources unchanged"), "FAIL", "{}", r.text());

        // From inside it, as xdevcmd's shell is: found without --dir.
        assert_eq!(find(&root.join("copy-to")).unwrap(), root);
        assert!(make(&t.join("no-such").join(NAME), TINY, "test").is_err());
        clean(&root).unwrap();
        assert!(!root.exists());
        let _ = fs::remove_dir_all(&t);
    }

    /// A manifest cut, emptied or changed is an error: never a PASS for everything
    /// (red-team: an emptied one passed with replace-dst deleted).
    #[test]
    fn a_cut_manifest_fails_closed() {
        let t = top("manifest");
        let root = t.join(NAME);
        make(&root, TINY, "test").unwrap();
        let m = root.join(MANIFEST);
        let whole = fs::read_to_string(&m).unwrap();
        for bad in [
            String::new(),
            whole.lines().take(3).collect::<Vec<_>>().join("\n"),
            whole.replacen("src=", "srcs=", 1),
            whole.clone() + "src\t1\tnope\tx\n",
        ] {
            fs::write(&m, &bad).unwrap();
            assert!(check(&root).is_err(), "passed with: {bad:?}");
        }
        fs::write(&m, whole).unwrap();
        clean(&root).unwrap();
        let _ = fs::remove_dir_all(&t);
    }

    /// --clean touches nothing it did not make: no marker, another name, a link, or a
    /// file the tester put in the sandbox.
    #[test]
    fn clean_never_removes_what_it_did_not_make() {
        let t = top("clean");
        let precious = t.join("precious");
        fs::create_dir_all(&precious).unwrap();
        fs::write(precious.join("keep"), "k").unwrap();
        let fake = t.join(NAME);
        fs::create_dir_all(&fake).unwrap();
        fs::write(fake.join("keep"), "k").unwrap();
        assert!(clean(&fake).is_err(), "no marker");
        assert!(fake.join("keep").exists());
        fs::write(precious.join(MARKER), format!("{MAGIC}\n")).unwrap();
        assert!(clean(&precious).is_err(), "another name");
        assert!(precious.join("keep").exists());
        fs::remove_dir_all(&fake).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&precious, &fake).unwrap();
            assert!(clean(&fake).is_err(), "a link");
            assert!(precious.join("keep").exists());
            fs::remove_file(&fake).unwrap();
        }
        let root = t.join(NAME);
        make(&root, TINY, "test").unwrap();
        fs::write(root.join("my-notes.txt"), "mine").unwrap();
        let e = clean(&root).unwrap_err().to_string();
        assert!(e.contains("my-notes.txt"), "{e}");
        assert!(root.join("my-notes.txt").exists() && root.join(MARKER).exists());
        fs::remove_file(root.join("my-notes.txt")).unwrap();
        clean(&root).unwrap();
        let _ = fs::remove_dir_all(&t);
    }

    /// --tamper never zeroes a source: not through a copy-to that is a link, nor a copy
    /// that is a hard link to its source.
    #[cfg(unix)]
    #[test]
    fn tamper_never_touches_a_source() {
        let t = top("tamper");
        let root = t.join(NAME);
        make(&root, TINY, "test").unwrap();
        let src = root.join("tree/d000/f0000");
        let before = fs::read(&src).unwrap();
        fs::create_dir_all(root.join("copy-to/tree/d000")).unwrap();
        fs::hard_link(&src, root.join("copy-to/tree/d000/f0000")).unwrap();
        assert!(tamper(&root).is_err());
        fs::remove_dir_all(root.join("copy-to")).unwrap();
        std::os::unix::fs::symlink(&root, root.join("copy-to")).unwrap();
        assert!(tamper(&root).is_err());
        assert_eq!(fs::read(&src).unwrap(), before);
        fs::remove_file(root.join("copy-to")).unwrap();
        fs::create_dir(root.join("copy-to")).unwrap();
        clean(&root).unwrap();
        let _ = fs::remove_dir_all(&t);
    }

    /// The kill points: A at a tenth replaced, B past the first checkpoint, C at a fifth
    /// copied; a finished copy is "too late".
    #[test]
    fn the_kill_points_are_read_from_the_sandbox() {
        let t = top("points");
        let root = t.join(NAME);
        make(&root, TINY, "test").unwrap();
        let lines = manifest(&root).unwrap();
        for c in ["A", "B", "C"] {
            assert_eq!(progress(&root, c, &lines), None, "{c} before it starts");
        }
        fs::copy(
            root.join("replace-src/r00000"),
            root.join("replace-dst/r00000"),
        )
        .unwrap();
        assert_eq!(progress(&root, "A", &lines), Some(true));
        fs::create_dir_all(root.join("copy-to/big")).unwrap();
        let part = root.join("copy-to/big/.big.bin.xdevcmd-part-1-0");
        File::create(&part)
            .unwrap()
            .set_len(PAST_CHECKPOINT)
            .unwrap();
        assert_eq!(progress(&root, "B", &lines), Some(true));
        fs::rename(&part, root.join("copy-to/big/big.bin")).unwrap();
        assert_eq!(
            progress(&root, "B", &lines),
            Some(false),
            "finished: too late"
        );
        copy_tree(&root.join("tree/d000"), &root.join("copy-to/tree/d000"));
        assert_eq!(progress(&root, "C", &lines), Some(true));
        fs::remove_dir_all(root.join("copy-to")).unwrap();
        fs::create_dir(root.join("copy-to")).unwrap();
        clean(&root).unwrap();
        let _ = fs::remove_dir_all(&t);
    }
}
