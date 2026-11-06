//! Render the whole app off-screen and assert on the result.
//!
//! `--screenshot` draws one frame into a `TestBackend`, so the layout can be checked in
//! CI without a terminal, a pty or a human squinting at a screen.

use std::process::Command;

/// rcmd, cut off from the config, trash and session of whoever runs the tests. A user who has
/// rebound F5 must not see every copy test fail.
fn isolated() -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rcmd"));
    let sandbox = std::env::temp_dir().join(format!("rcmd-isolation-{}", std::process::id()));
    c.env("RCMD_CONFIG", sandbox.join("no-such-config.toml"));
    c.env("XDG_DATA_HOME", sandbox.join("data"));
    // And from their session: a test must neither reopen their panes nor overwrite them.
    c.env("RCMD_STATE", sandbox.join("session.toml"));
    c
}

fn shot(size: &str, left: &std::path::Path, right: &std::path::Path) -> String {
    let out = isolated()
        .arg(format!("--screenshot={size}"))
        .arg(left)
        .arg(right)
        .output()
        .expect("rcmd should run");
    assert!(out.status.success(), "rcmd exited with {:?}", out.status);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Two directories with something in them. Built rather than borrowed from the system:
/// the suite has to pass on Windows and macOS, where `/etc` and `/usr` do not exist.
fn tree() -> (std::path::PathBuf, std::path::PathBuf) {
    let n = N.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("rcmd-tree-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (a, b) = (root.join("alpha"), root.join("bravo"));
    std::fs::create_dir_all(a.join("nested")).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(a.join("readme.md"), vec![b'r'; 4096]).unwrap();
    std::fs::write(a.join("notes.txt"), b"short").unwrap();
    std::fs::write(b.join("other.log"), vec![b'o'; 2048]).unwrap();
    (a, b)
}

/// How `pane/term.rs` names the shell it started.
fn shell_label() -> String {
    std::env::var("SHELL")
        .ok()
        .or_else(|| {
            if cfg!(windows) {
                std::env::var("COMSPEC").ok()
            } else {
                None
            }
        })
        .and_then(|s| {
            std::path::Path::new(&s)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "shell".to_string())
}

#[test]
fn draws_both_panes_the_shell_and_the_key_bar() {
    let (a, b) = tree();
    let s = shot("118x34", &a, &b);

    assert!(s.contains("alpha"), "left pane should show its path\n{s}");
    assert!(s.contains("bravo"), "right pane should show its path\n{s}");
    assert!(s.contains("<DIR>"), "directories should be labelled\n{s}");
    assert!(
        s.contains("10 Quit"),
        "the function key bar should be drawn\n{s}"
    );
    assert!(
        s.contains("12 Term"),
        "the terminal key should be offered\n{s}"
    );
    assert!(
        s.contains("items"),
        "the status line should count entries\n{s}"
    );
}

#[test]
fn a_wide_pane_shows_every_column() {
    let (a, b) = tree();
    let s = shot("118x34", &a, &b);
    for col in ["Name", "Ext", "Size", "Modified"] {
        assert!(
            s.contains(col),
            "column {col} should be present at 118 columns\n{s}"
        );
    }
    let _ = std::fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_narrow_pane_drops_columns_instead_of_overflowing() {
    let (a, b) = tree();
    let width = 56;
    let s = shot(&format!("{width}x20"), &a, &b);

    for (i, line) in s.lines().enumerate() {
        assert!(
            line.chars().count() <= width,
            "line {i} is {} columns wide, past the {width} available:\n{line}",
            line.chars().count()
        );
    }
    assert!(s.contains("Name"), "Name must survive at any width\n{s}");
    assert!(
        !s.contains("Modified"),
        "Modified should be dropped when narrow\n{s}"
    );
}

#[test]
fn the_shell_pane_starts_and_reports_its_program() {
    let (a, b) = tree();
    let s = shot("118x34", &a, &b);
    let name = shell_label();
    assert!(
        s.contains(&name) || s.contains("no shell"),
        "the shell pane should name the program it started ({name})\n{s}"
    );
    let _ = std::fs::remove_dir_all(a.parent().unwrap());
}

// ── dialogs ────────────────────────────────────────────────────────────────────────
//
// `--keys` types into the app before the frame is drawn, so every confirmation can be
// asserted on exactly as the user sees it. Checking this against a pty instead does not
// work: ratatui only emits the cells that changed, so the escape stream has no spaces
// between words and "cannot be undone" never matches.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

static N: AtomicU32 = AtomicU32::new(0);

/// Two directories with a file in the left one, under short paths so dialog text is not
/// elided before the assertion can see it.
fn fixture() -> (PathBuf, PathBuf) {
    let n = N.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("rcmd-ui-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (a, b) = (root.join("a"), root.join("b"));
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::write(a.join("victim.txt"), b"content").unwrap();
    (a, b)
}

fn shot_keys(size: &str, keys: &str, left: &std::path::Path, right: &std::path::Path) -> String {
    let out = isolated()
        .arg(format!("--screenshot={size}"))
        .arg(format!("--keys={keys}"))
        .arg(left)
        .arg(right)
        .output()
        .expect("rcmd should run");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn f5_asks_before_copying_and_names_the_destination() {
    let (a, b) = fixture();
    let s = shot_keys("110x24", "Down,Insert,F5", &a, &b);
    assert!(s.contains("Copy"), "the dialog should be titled Copy\n{s}");
    assert!(
        s.contains("victim.txt"),
        "it should name what is being copied\n{s}"
    );
    assert!(
        s.contains(&b.display().to_string()),
        "it should show where the files are going\n{s}"
    );
    assert!(s.contains("[Enter] go"), "{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn f8_goes_to_the_trash_and_shift_f8_says_it_cannot_be_undone() {
    let (a, b) = fixture();

    let trash = shot_keys("110x24", "Down,Insert,F8", &a, &b);
    assert!(trash.contains("Moved to the trash"), "{trash}");
    assert!(
        !trash.contains("cannot be undone"),
        "a recoverable delete must not borrow the scary wording\n{trash}"
    );

    let perm = shot_keys("110x24", "Down,Insert,S-F8", &a, &b);
    assert!(perm.contains("Delete permanently"), "{perm}");
    assert!(
        perm.contains("cannot be undone"),
        "an unrecoverable delete must say so\n{perm}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_confirmation_counts_one_file_in_the_singular() {
    let (a, b) = fixture();
    let s = shot_keys("110x24", "Down,Insert,F8", &a, &b);
    assert!(
        s.contains("1 file "),
        "expected \"1 file\", not \"1 files\"\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn f7_prompts_for_a_name() {
    let (a, b) = fixture();
    let s = shot_keys("110x24", "F7,d,o,c,s", &a, &b);
    assert!(s.contains("Create directory"), "{s}");
    assert!(s.contains("docs"), "typed characters should appear\n{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn copying_with_no_second_pane_refuses_instead_of_guessing() {
    let (a, b) = fixture();
    // Tab to the right pane, close it, then try to copy out of the only pane left.
    let s = shot_keys("110x24", "Tab,A-w,Down,Insert,F5", &a, &b);
    assert!(
        s.contains("no second pane"),
        "it must say why, not silently copy somewhere\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── viewer ─────────────────────────────────────────────────────────────────────────

fn with_file(name: &str, body: &[u8]) -> (PathBuf, PathBuf) {
    let (a, b) = fixture();
    fs::write(a.join(name), body).unwrap();
    (a, b)
}

#[test]
fn f3_shows_the_file_with_line_numbers() {
    let (a, b) = with_file("doc.txt", b"first line\nsecond line\nthird line\n");
    // victim.txt sorts before doc.txt? No: doc < victim, so one Down lands on doc.txt.
    let s = shot_keys("100x16", "Down,F3", &a, &b);
    assert!(s.contains("view"), "{s}");
    assert!(s.contains("first line"), "{s}");
    assert!(s.contains("third line"), "{s}");
    assert!(s.contains("3 lines"), "the header should count lines\n{s}");
    assert!(
        s.contains("Esc close"),
        "the key hints should be visible\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn tabs_are_expanded_rather_than_sent_to_the_terminal() {
    let (a, b) = with_file("doc.txt", b"ab\tc\n");
    let s = shot_keys("100x10", "Down,F3", &a, &b);
    assert!(
        s.contains("ab  c"),
        "a tab should land on the 4-column grid, not pass through\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn h_switches_to_hex_and_shows_offsets() {
    let (a, b) = with_file("doc.txt", b"ABCDEFGH\n");
    let s = shot_keys("100x12", "Down,F3,h", &a, &b);
    assert!(
        s.contains("00000000"),
        "hex view needs an offset column\n{s}"
    );
    assert!(s.contains("41 42 43 44"), "ABCD in hex\n{s}");
    assert!(s.contains("ABCDEFGH"), "the ascii column\n{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_binary_file_opens_in_hex_without_being_asked() {
    let (a, b) = with_file("prog.bin", b"\x7fELF\x02\x01\x01\x00\x00\x00rest");
    let s = shot_keys("100x12", "Down,F3", &a, &b);
    assert!(s.contains("hex"), "{s}");
    assert!(s.contains("7f 45 4c 46"), "{s}");
    assert!(
        s.contains("binary file"),
        "it should say why it chose hex\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn search_jumps_to_the_matching_line() {
    let (a, b) = with_file("doc.txt", b"alpha\nbravo\ncharlie\ndelta\n");
    let s = shot_keys("100x12", "Down,F3,/,c,h,a,r,Enter", &a, &b);
    let body = s.lines().skip(1).collect::<Vec<_>>().join("\n");
    assert!(body.contains("charlie"), "{s}");
    assert!(
        !body.contains("alpha"),
        "the view should have scrolled past the earlier lines\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn f3_and_f4_refuse_a_directory_instead_of_doing_something_odd() {
    let (a, b) = fixture();
    fs::create_dir_all(a.join("sub")).unwrap();

    let view = shot_keys("100x12", "Down,F3", &a, &b);
    assert!(view.contains("F3 views a file"), "{view}");

    let edit = shot_keys("100x12", "Down,F4", &a, &b);
    assert!(edit.contains("F4 edits a file"), "{edit}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── config ─────────────────────────────────────────────────────────────────────────

fn rcmd(args: &[&std::ffi::OsStr]) -> std::process::Output {
    isolated().args(args).output().expect("rcmd should run")
}

fn write_config(dir: &std::path::Path, body: &str) -> PathBuf {
    let p = dir.join("config.toml");
    fs::write(&p, body).unwrap();
    p
}

fn shot_with(
    cfg: &std::path::Path,
    keys: &str,
    a: &std::path::Path,
    b: &std::path::Path,
) -> String {
    let out = rcmd(&[
        "--config".as_ref(),
        cfg.as_os_str(),
        "--screenshot=110x24".as_ref(),
        format!("--keys={keys}").as_ref(),
        a.as_os_str(),
        b.as_os_str(),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn a_rebound_key_works_and_the_old_one_stops() {
    let (a, b) = fixture();
    let cfg = write_config(a.parent().unwrap(), "[keys]\ncopy = [\"F2\"]\n");

    let new_key = shot_with(&cfg, "Down,Insert,F2", &a, &b);
    assert!(
        new_key.contains("[Enter] go"),
        "F2 should now copy\n{new_key}"
    );

    let old_key = shot_with(&cfg, "Down,Insert,F5", &a, &b);
    assert!(
        !old_key.contains("[Enter] go"),
        "F5 should no longer copy\n{old_key}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn the_key_bar_shows_the_rebound_key_not_the_default() {
    let (a, b) = fixture();
    let cfg = write_config(a.parent().unwrap(), "[keys]\ncopy = [\"F2\"]\n");
    let s = shot_with(&cfg, "", &a, &b);
    assert!(s.contains("2 Copy"), "{s}");
    assert!(
        !s.contains("5 Copy"),
        "the bar must not advertise a dead key\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn the_terminal_can_start_hidden() {
    let (a, b) = fixture();
    let cfg = write_config(a.parent().unwrap(), "[terminal]\nshow = false\n");
    let s = shot_with(&cfg, "", &a, &b);
    assert!(
        !s.contains('\u{2302}'),
        "no shell pane should be drawn\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_broken_config_still_starts_and_says_why() {
    let (a, b) = fixture();
    let cfg = write_config(a.parent().unwrap(), "[theme]\naccent = \"mauve\"\n");
    let s = shot_with(&cfg, "", &a, &b);
    assert!(
        s.contains("config ignored"),
        "the rejection must be visible\n{s}"
    );
    assert!(s.contains("Name"), "and the app must still be usable\n{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn print_config_output_passes_check_config() {
    let (a, _) = fixture();
    let printed = rcmd(&["--print-config".as_ref()]);
    assert!(printed.status.success());
    let cfg = write_config(
        a.parent().unwrap(),
        &String::from_utf8_lossy(&printed.stdout),
    );

    let checked = rcmd(&[
        "--check-config".as_ref(),
        "--config".as_ref(),
        cfg.as_os_str(),
    ]);
    assert!(
        checked.status.success(),
        "what rcmd prints must be something rcmd accepts: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert!(String::from_utf8_lossy(&checked.stdout).starts_with("ok:"));
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn check_config_fails_loudly_and_names_the_field() {
    let (a, _) = fixture();
    let cfg = write_config(a.parent().unwrap(), "[files]\nsort = \"colour\"\n");
    let out = rcmd(&[
        "--check-config".as_ref(),
        "--config".as_ref(),
        cfg.as_os_str(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("sort"));
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn an_unknown_option_is_refused() {
    let out = rcmd(&["--colour".as_ref()]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--colour"));
}

// ── help ───────────────────────────────────────────────────────────────────────────

#[test]
fn f1_lists_keys_and_tab_switches_to_settings() {
    let (a, b) = fixture();
    let cfg = write_config(
        a.parent().unwrap(),
        "[keys]\ncopy = [\"F2\"]\n[viewer]\ntab_width = 8\n",
    );

    let keys = shot_with(&cfg, "F1", &a, &b);
    assert!(keys.contains("Keys") && keys.contains("Settings"), "{keys}");
    assert!(keys.contains("Quit rcmd"), "{keys}");

    let rebound = shot_with(&cfg, "F1,PageDown", &a, &b);
    assert!(
        rebound.contains("\u{2022} F2"),
        "a rebound key should be listed, and marked as changed\n{rebound}"
    );

    let settings = shot_with(&cfg, "F1,Tab", &a, &b);
    assert!(
        settings.contains("loaded"),
        "the file's status should be shown\n{settings}"
    );

    // [viewer] sits below the fold at 24 rows; scroll to it the way a user would.
    let viewer = shot_with(&cfg, "F1,Tab,PageDown", &a, &b);
    assert!(
        viewer.contains("\u{2022} tab_width = 8"),
        "a changed value should be marked\n{viewer}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── mouse ──────────────────────────────────────────────────────────────────────────
//
// Coordinates are found by locating text in a first screenshot rather than hardcoded,
// so a layout change moves the clicks with it instead of breaking the tests.

fn locate(screen: &str, needle: &str) -> (usize, usize) {
    screen
        .lines()
        .enumerate()
        .find_map(|(y, l)| l.find(needle).map(|byte| (l[..byte].chars().count(), y)))
        .unwrap_or_else(|| panic!("{needle:?} is not on screen:\n{screen}"))
}

fn files(names: &[&str]) -> (PathBuf, PathBuf) {
    let (a, b) = fixture();
    fs::remove_file(a.join("victim.txt")).unwrap();
    for n in names {
        fs::write(a.join(n), n.as_bytes()).unwrap();
    }
    (a, b)
}

const SIZE: &str = "100x24";

#[test]
fn ctrl_click_marks_the_row_it_lands_on() {
    let (a, b) = files(&["alpha.txt", "bravo.txt"]);
    let (x, y) = locate(&shot_keys(SIZE, "", &a, &b), "bravo");
    let s = shot_keys(SIZE, &format!("ctrl-click:{x}:{y}"), &a, &b);
    assert!(s.contains("\u{2022}bravo"), "bravo should be marked\n{s}");
    assert!(!s.contains("\u{2022}alpha"), "and only bravo\n{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn dragging_down_a_pane_marks_the_range() {
    let (a, b) = files(&["alpha.txt", "bravo.txt", "charlie.txt", "delta.txt"]);
    let base = shot_keys(SIZE, "", &a, &b);
    let (x, y1) = locate(&base, "alpha");
    let (_, y2) = locate(&base, "charlie");
    let s = shot_keys(SIZE, &format!("drag:{x}:{y1}:{x}:{y2}"), &a, &b);
    for n in ["alpha", "bravo", "charlie"] {
        assert!(
            s.contains(&format!("\u{2022}{n}")),
            "{n} should be marked\n{s}"
        );
    }
    assert!(
        !s.contains("\u{2022}delta"),
        "the range stops where the drag did\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn dropping_on_the_other_pane_asks_then_copies() {
    let (a, b) = files(&["alpha.txt", "bravo.txt"]);
    let (x, y) = locate(&shot_keys(SIZE, "", &a, &b), "bravo");
    let drag = format!("drag:{x}:{y}:75:{y}");

    let asked = shot_keys(SIZE, &drag, &a, &b);
    assert!(asked.contains("Copy bravo.txt here"), "{asked}");
    assert!(asked.contains("Move bravo.txt here"), "{asked}");
    assert!(!b.join("bravo.txt").exists(), "nothing happens until asked");

    let (mx, my) = locate(&asked, "Copy bravo.txt here");
    shot_keys(SIZE, &format!("{drag},click:{}:{my}", mx + 1), &a, &b);
    assert_eq!(fs::read(b.join("bravo.txt")).unwrap(), b"bravo.txt");
    assert!(a.join("bravo.txt").exists(), "copy, not move");
    assert!(
        !b.join("alpha.txt").exists(),
        "only the grabbed file travels"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn right_click_opens_a_menu_whose_hints_follow_the_keymap() {
    let (a, b) = files(&["alpha.txt"]);
    let cfg = write_config(a.parent().unwrap(), "[keys]\ncopy = [\"F2\"]\n");
    let (x, y) = locate(&shot_with(&cfg, "", &a, &b), "alpha");
    let s = shot_with(&cfg, &format!("rclick:{x}:{y}"), &a, &b);
    assert!(s.contains("View"), "{s}");
    let line = s
        .lines()
        .find(|l| l.contains("Copy to other pane"))
        .unwrap_or_else(|| panic!("no copy item\n{s}"));
    assert!(
        line.contains("F2"),
        "the hint must be the rebound key\n{line}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn dragging_the_border_between_panes_resizes_them() {
    let (a, b) = files(&["alpha.txt"]);
    let base = shot_keys(SIZE, "", &a, &b);
    let (border, _) = locate(&base, "\u{2510}\u{250c}");
    let s = shot_keys(SIZE, &format!("drag:{border}:5:30:5"), &a, &b);
    let (moved, _) = locate(&s, "\u{2510}\u{250c}");
    assert!(
        moved < border,
        "the boundary should follow the pointer left\n{s}"
    );
    assert!(
        s.contains("left") && s.contains("right"),
        "the status should say the split\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn the_key_bar_and_dialog_buttons_are_clickable() {
    let (a, b) = files(&["alpha.txt"]);
    let base = shot_keys(SIZE, "", &a, &b);
    let (vx, vy) = locate(&base, "3 View");
    let viewed = shot_keys(SIZE, &format!("Down,click:{}:{vy}", vx + 1), &a, &b);
    assert!(
        viewed.contains("view "),
        "clicking 3 View should open the viewer\n{viewed}"
    );

    let asking = shot_keys(SIZE, "Down,F8", &a, &b);
    let (cx, cy) = locate(&asking, "[Esc] cancel");
    let closed = shot_keys(SIZE, &format!("Down,F8,click:{}:{cy}", cx + 1), &a, &b);
    assert!(
        !closed.contains("[Enter] go"),
        "clicking cancel should close it\n{closed}"
    );
    assert!(a.join("alpha.txt").exists());
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn help_tabs_are_clickable() {
    let (a, b) = files(&["alpha.txt"]);
    let open = shot_keys(SIZE, "F1", &a, &b);
    let (x, y) = locate(&open, "Settings");
    let s = shot_keys(SIZE, &format!("F1,click:{}:{y}", x + 1), &a, &b);
    assert!(s.contains("Config file"), "{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── terminals ──────────────────────────────────────────────────────────────────────

/// The left half of the status line: what the focused pane reports about itself.
fn status(s: &str) -> String {
    let lines: Vec<&str> = s.lines().collect();
    lines[lines.len() - 2].to_string()
}

/// The terminal panel's tab bar, and the row it is on.
fn tab_bar(s: &str) -> (String, usize) {
    s.lines()
        .enumerate()
        .find(|(_, l)| l.contains("\u{2302} 1 "))
        .map(|(y, l)| (l.to_string(), y))
        .unwrap_or_else(|| panic!("no tab bar:\n{s}"))
}

/// `name` after the platform's path separator: `/b` here, `\b` on Windows.
fn sep(name: &str) -> String {
    format!("{}{name}", std::path::MAIN_SEPARATOR)
}

fn col(line: &str, needle: &str) -> usize {
    let b = line
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {line:?}"));
    line[..b].chars().count()
}

#[test]
fn alt_n_opens_tabs_and_ctrl_page_keys_move_between_them() {
    let (a, b) = files(&["alpha.txt"]);
    let s = shot_keys(SIZE, "F12,A-n,A-n", &a, &b);
    assert!(status(&s).contains("terminal 3/3"), "{s}");
    let (bar, _) = tab_bar(&s);
    assert!(
        bar.contains("\u{2302} 2 ") && bar.contains("\u{2302} 3 "),
        "{bar}"
    );

    let back = shot_keys(SIZE, "F12,A-n,A-n,C-PageUp", &a, &b);
    assert!(status(&back).contains("terminal 2/3"), "{back}");
    let round = shot_keys(SIZE, "F12,A-n,A-n,C-PageDown", &a, &b);
    assert!(
        status(&round).contains("terminal 1/3"),
        "wraps around\n{round}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn split_keys_split_the_shell_when_it_has_focus() {
    // Tall enough for a top/bottom split: a shorter panel is refused, since one-row
    // shells were what crashed vt100.
    let (a, b) = files(&["alpha.txt"]);
    let s = shot_keys("100x50", "F12,A-s,A-d", &a, &b);
    assert!(status(&s).contains("split 3/3"), "{s}");
    let cycled = shot_keys("100x50", "F12,A-s,A-d,A-o", &a, &b);
    assert!(
        status(&cycled).contains("split 1/3"),
        "Alt+O wraps to the first\n{cycled}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn closing_shells_then_the_last_one_removes_the_panel() {
    let (a, b) = files(&["alpha.txt"]);
    let one_left = shot_keys(SIZE, "F12,A-s,A-w", &a, &b);
    assert!(status(&one_left).contains("terminal 1/1"), "{one_left}");
    assert!(!status(&one_left).contains("split"), "{one_left}");

    let none = shot_keys(SIZE, "F12,A-w", &a, &b);
    assert!(!none.contains('\u{2302}'), "no panel should remain\n{none}");
    assert!(
        status(&none).contains("items"),
        "focus goes back to the files\n{none}"
    );

    let fresh = shot_keys(SIZE, "F12,A-w,F12", &a, &b);
    assert!(
        status(&fresh).contains("terminal 1/1"),
        "F12 starts a new one\n{fresh}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn the_tab_bar_buttons_are_clickable() {
    let (a, b) = files(&["alpha.txt"]);

    let one = shot_keys(SIZE, "F12", &a, &b);
    let (bar, y) = tab_bar(&one);
    // --screenshot trims trailing spaces, so the button's padding is not there.
    let plus = col(&bar, "+");
    let added = shot_keys(SIZE, &format!("F12,click:{plus}:{y}"), &a, &b);
    assert!(
        status(&added).contains("terminal 2/2"),
        "+ should open a tab\n{added}"
    );

    let two = shot_keys(SIZE, "F12,A-n", &a, &b);
    let (bar, y) = tab_bar(&two);
    let first = col(&bar, "\u{2302} 1");
    let picked = shot_keys(SIZE, &format!("F12,A-n,click:{first}:{y}"), &a, &b);
    assert!(
        status(&picked).contains("terminal 1/2"),
        "a tab label selects it\n{picked}"
    );

    let close = col(&bar, "\u{00d7}");
    let closed = shot_keys(SIZE, &format!("F12,A-n,click:{close}:{y}"), &a, &b);
    assert!(
        status(&closed).contains("terminal 1/1"),
        "\u{00d7} closes that tab\n{closed}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn dragging_the_boundary_between_shells_resizes_them() {
    let (a, b) = files(&["alpha.txt"]);
    let s = shot_keys(SIZE, "F12,A-s", &a, &b);
    let (_, bar_y) = tab_bar(&s);
    let line = s.lines().nth(bar_y + 1).unwrap();
    let x = col(line, "\u{2510}\u{250c}");
    let y = bar_y + 3;
    let moved = shot_keys(SIZE, &format!("F12,A-s,drag:{x}:{y}:25:{y}"), &a, &b);
    let line = moved.lines().nth(bar_y + 1).unwrap();
    assert!(
        col(line, "\u{2510}\u{250c}") < x,
        "the shells' boundary should move\n{moved}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── rename ─────────────────────────────────────────────────────────────────────────

#[test]
fn shift_f6_opens_rename_with_only_the_name_selected() {
    let (a, b) = files(&["report.docx"]);
    let s = shot_keys(SIZE, "Down,S-F6", &a, &b);
    assert!(s.contains("Rename"), "{s}");
    assert!(
        s.contains("report.docx"),
        "the current name is filled in\n{s}"
    );
    assert!(s.contains("[Enter] rename"), "{s}");

    let f2 = shot_keys(SIZE, "Down,F2", &a, &b);
    assert!(f2.contains("[Enter] rename"), "F2 opens it too\n{f2}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn typing_replaces_the_name_keeps_the_extension_and_follows_the_file() {
    let (a, b) = files(&["report.docx", "zzz.txt"]);
    let s = shot_keys(SIZE, "Down,S-F6,a,b,c,Enter", &a, &b);
    assert!(a.join("abc.docx").exists(), "renamed on disk\n{s}");
    assert!(!a.join("report.docx").exists());
    assert!(s.contains("renamed report.docx \u{2192} abc.docx"), "{s}");
    // "abc" now sorts first; the cursor must have followed it rather than stayed on
    // whatever took its old row.
    assert!(status(&s).contains("abc.docx"), "{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn an_existing_name_is_refused_and_the_dialog_stays_open() {
    let (a, b) = files(&["a.txt", "b.txt"]);
    fs::write(a.join("a.txt"), b"keep me").unwrap();
    // Down twice to b.txt, select all, type "a.txt", Enter.
    let s = shot_keys(SIZE, "Down,Down,S-F6,C-a,a,.,t,x,t,Enter", &a, &b);
    assert!(s.contains("already exists"), "the reason is shown\n{s}");
    assert!(
        s.contains("[Enter] rename"),
        "and the dialog is still there\n{s}"
    );
    assert_eq!(fs::read(a.join("a.txt")).unwrap(), b"keep me");
    assert!(a.join("b.txt").exists());
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_name_with_a_slash_is_refused_not_turned_into_a_move() {
    let (a, b) = files(&["note.txt"]);
    fs::create_dir(a.join("sub")).unwrap();
    // "sub" sorts first as a directory; the file is the second row.
    let s = shot_keys(SIZE, "Down,Down,S-F6,C-a,s,u,b,/,x,Enter", &a, &b);
    assert!(s.contains("path separator"), "{s}");
    assert!(a.join("note.txt").exists() && !a.join("sub/x").exists());
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn renaming_a_vietnamese_name_edits_letters_not_bytes() {
    let (a, b) = files(&["báo cáo.docx"]);
    shot_keys(SIZE, "Down,S-F6,t,u,ầ,n,Enter", &a, &b);
    assert!(
        a.join("tuần.docx").exists(),
        "{:?}",
        fs::read_dir(&a).unwrap().collect::<Vec<_>>()
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn the_context_menu_offers_rename() {
    let (a, b) = files(&["alpha.txt"]);
    let (x, y) = locate(&shot_keys(SIZE, "", &a, &b), "alpha");
    let s = shot_keys(SIZE, &format!("rclick:{x}:{y}"), &a, &b);
    let line = s
        .lines()
        .find(|l| l.contains("Rename"))
        .unwrap_or_else(|| panic!("{s}"));
    assert!(line.contains("F2") || line.contains("Shift+F6"), "{line}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── Alt+arrows ─────────────────────────────────────────────────────────────────────

#[test]
fn alt_right_enters_a_directory_and_alt_left_comes_back_to_it() {
    let (a, b) = files(&["zfile.txt"]);
    fs::create_dir(a.join("sub")).unwrap();
    fs::write(a.join("sub/inner.txt"), b"x").unwrap();

    let inside = shot_keys(SIZE, "Down,A-Right", &a, &b);
    assert!(inside.contains("inner"), "should be inside sub\n{inside}");

    let back = shot_keys(SIZE, "Down,A-Right,A-Left", &a, &b);
    assert!(back.contains("zfile"), "back in the parent\n{back}");
    assert!(
        status(&back).contains("sub"),
        "the cursor lands on the directory just left\n{back}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn alt_right_on_a_file_does_nothing_but_say_so() {
    let (a, b) = files(&["plain.txt"]);
    let s = shot_keys(SIZE, "Down,A-Right", &a, &b);
    assert!(s.contains("not a directory"), "{s}");
    assert!(!s.contains("view "), "it must not open the viewer\n{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn alt_shift_arrows_resize_now() {
    let (a, b) = files(&["alpha.txt"]);
    let base = shot_keys(SIZE, "", &a, &b);
    let (x0, _) = locate(&base, "\u{2510}\u{250c}");
    let grown = shot_keys(SIZE, "Alt+Shift+Right", &a, &b);
    let (x1, _) = locate(&grown, "\u{2510}\u{250c}");
    assert!(x1 > x0, "the focused pane should grow\n{grown}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── many panes and terminals ───────────────────────────────────────────────────────

#[test]
fn split_stops_at_max_panes_and_terminals_stop_at_max_terminals() {
    let (a, b) = files(&["alpha.txt"]);
    let panes = shot_keys("140x30", "A-s,A-s", &a, &b);
    assert!(panes.contains("at most 3 file panes"), "{panes}");

    let shells = shot_keys("140x30", "F12,A-n,A-n,A-n", &a, &b);
    assert!(status(&shells).contains("terminal 3/3"), "{shells}");
    assert!(shells.contains("at most 3 terminals"), "{shells}");

    // Splits inside a tab count towards the same limit.
    let mixed = shot_keys("140x40", "F12,A-n,A-s,A-s", &a, &b);
    assert!(mixed.contains("at most 3 terminals"), "{mixed}");

    let cfg = write_config(a.parent().unwrap(), "[files]\nmax_panes = 4\n");
    let more = shot_with(&cfg, "A-s,A-s", &a, &b);
    assert!(
        !more.contains("at most"),
        "the limit comes from the config\n{more}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn with_three_panes_f5_goes_to_the_pane_used_last_not_a_clone() {
    let (a, b) = files(&["file.txt"]);
    // Alt+S clones the left pane's directory into a new middle pane. F5 from the left
    // pane used to target that clone — the source's own directory — and "copy" ended
    // as "0 copied, 1 skipped".
    let s = shot_keys("120x30", "A-s,Tab,Tab,Down,F5", &a, &b);
    let to = s
        .lines()
        .find(|l| l.contains("to    "))
        .unwrap_or_else(|| panic!("{s}"));
    assert!(
        to.contains(&sep("b")),
        "should target the other directory\n{s}"
    );
    assert!(
        s.contains("[Tab] other pane"),
        "with a choice, offer it\n{s}"
    );

    let switched = shot_keys("120x30", "A-s,Tab,Tab,Down,F5,Tab", &a, &b);
    assert!(
        switched.contains("Same directory as the source"),
        "a same-directory target must be called out before Enter\n{switched}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn dropping_onto_a_pane_showing_the_same_directory_says_so() {
    let (a, b) = files(&["file.txt"]);
    let s = shot_keys("120x30", "A-s", &a, &b);
    let (x, y) = locate(&s, "file");
    // The middle pane is the clone; drop onto it.
    let s = shot_keys("120x30", &format!("A-s,drag:{x}:{y}:50:{y}"), &a, &b);
    assert!(s.contains("same directory"), "{s}");
    assert!(
        !s.contains("Copy file.txt here"),
        "no menu for a no-op\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_terminal_split_too_small_to_use_is_refused() {
    let (a, b) = files(&["alpha.txt"]);
    let s = shot_keys("80x24", "F12,A-d", &a, &b);
    assert!(s.contains("too small to split"), "{s}");
    assert!(!status(&s).contains("split 2/2"), "{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

/// Every layout at every size, from 1x1 up: no panic, no line wider than the screen,
/// exactly as many lines as rows. Started as a one-off sweep that found a panic in
/// vt100 on one-row shells; kept so it cannot come back.
#[test]
fn no_layout_at_any_size_panics_or_overflows() {
    let (a, b) = files(&[
        "alpha.txt",
        "báo cáo tuần.docx",
        "x-very-long-name-that-fits-nowhere.tar.gz",
    ]);
    fs::create_dir(a.join("sub")).unwrap();
    let long_output = "s,e,q,Space,-,s,:,Space,1,Space,3,0,0,Enter";
    let shrink = ["Alt+Shift+Left"; 12].join(",");
    let grow = ["Alt+Shift+Right"; 12].join(",");
    let layouts = [
        String::new(),
        "A-s,A-s".into(),
        "A-s,A-d".into(),
        "F12,A-n,A-n".into(),
        "F12,A-s,A-d".into(),
        format!("A-s,A-s,F12,A-s,A-d,{grow}"),
        format!("A-s,A-s,F12,{shrink},{long_output}"),
        "A-s,A-s,Tab,A-w,A-w,F12,A-s,A-w,A-w".into(),
        "F1,Tab".into(),
        "Down,Down,F3,h".into(),
        "Down,Down,F8".into(),
        "Down,Down,S-F6".into(),
        "rclick:3:3".into(),
        "drag:3:3:50:3".into(),
    ];
    let sizes = [(1, 1), (8, 3), (20, 6), (40, 12), (80, 24), (200, 60)];

    let jobs: Vec<(usize, usize, String)> = sizes
        .iter()
        .flat_map(|(w, h)| layouts.iter().map(move |l| (*w, *h, l.clone())))
        .collect();
    let failures = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for chunk in jobs.chunks(jobs.len().div_ceil(8)) {
            let (a, b, failures) = (&a, &b, &failures);
            scope.spawn(move || {
                for (w, h, keys) in chunk {
                    let mut c = isolated();
                    c.env("RCMD_SCREENSHOT_WAIT_MS", "0")
                        .env("SHELL", "/bin/sh")
                        .arg(format!("--screenshot={w}x{h}"));
                    if !keys.is_empty() {
                        c.arg(format!("--keys={keys}"));
                    }
                    let out = c.arg(a).arg(b).output().expect("rcmd should run");
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let lines: Vec<&str> = stdout.lines().collect();
                    let mut why = Vec::new();
                    if !out.status.success() {
                        why.push(format!("exit {:?}", out.status.code()));
                    }
                    if stderr.contains("panicked") {
                        why.push(format!("panic: {}", stderr.lines().next().unwrap_or("")));
                    }
                    if lines.iter().any(|l| l.chars().count() > *w) {
                        why.push("a line is wider than the screen".into());
                    }
                    if out.status.success() && lines.len() != *h {
                        why.push(format!("{} lines for {h} rows", lines.len()));
                    }
                    if !why.is_empty() {
                        failures
                            .lock()
                            .unwrap()
                            .push(format!("{w}x{h} [{keys}]: {}", why.join("; ")));
                    }
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    assert!(
        failures.is_empty(),
        "{} failing:\n{}",
        failures.len(),
        failures.join("\n")
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── zoom, four-way resize ──────────────────────────────────────────────────────────

#[test]
fn alt_z_fills_the_screen_with_the_pane_and_puts_it_back() {
    let (a, b) = files(&["alpha.txt"]);
    let zoomed = shot_keys("110x30", "A-z", &a, &b);
    assert!(status(&zoomed).contains("zoomed"), "{zoomed}");
    assert!(
        !zoomed.contains(&b.display().to_string()) && !zoomed.contains('\u{2302}'),
        "the other pane and the terminal are hidden\n{zoomed}"
    );
    assert!(
        !zoomed.lines().nth(1).unwrap().contains("\u{2510}\u{250c}"),
        "one pane across the whole width\n{zoomed}"
    );

    let back = shot_keys("110x30", "A-z,A-z", &a, &b);
    assert!(
        back.lines().nth(1).unwrap().contains("\u{2510}\u{250c}"),
        "{back}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn moving_focus_ends_the_zoom_and_its_message() {
    let (a, b) = files(&["alpha.txt"]);
    let s = shot_keys("110x30", "A-z,Tab", &a, &b);
    assert!(!status(&s).contains("zoomed"), "{s}");
    assert!(!s.contains("to put it back"), "no stale hint\n{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn the_terminal_zooms_too() {
    let (a, b) = files(&["alpha.txt"]);
    let s = shot_keys("110x30", "F12,A-z", &a, &b);
    let (_, bar_y) = tab_bar(&s);
    assert_eq!(
        bar_y, 1,
        "the terminal panel starts right below the menu bar\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn alt_shift_arrows_move_the_border_in_the_arrow_direction() {
    let (a, b) = files(&["alpha.txt"]);
    let row = |keys: &str| tab_bar(&shot_keys("110x30", keys, &a, &b)).1;
    let start = row("F12");
    assert!(
        row("F12,Alt+Shift+Up") < start,
        "up raises the terminal's top edge"
    );
    assert!(row("F12,Alt+Shift+Down") > start, "down lowers it");
    // From a file pane on top, Down moves the same edge down: the terminal shrinks.
    assert!(row("Alt+Shift+Down") > start);

    let (x0, _) = locate(&shot_keys("110x30", "", &a, &b), "\u{2510}\u{250c}");
    let right = shot_keys("110x30", "Tab,Alt+Shift+Right", &a, &b);
    let (x1, _) = locate(&right, "\u{2510}\u{250c}");
    assert!(
        x1 > x0,
        "right moves the border right even from the right pane\n{right}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_config_written_by_an_older_rcmd_still_loads() {
    let (a, b) = files(&["alpha.txt"]);
    let cfg = write_config(
        a.parent().unwrap(),
        "[keys]\ngrow_pane = [\"Alt+Shift+Right\"]\nshrink_pane = [\"Alt+Shift+Left\"]\n",
    );
    let s = shot_with(&cfg, "", &a, &b);
    assert!(!s.contains("config ignored"), "{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── session ────────────────────────────────────────────────────────────────────────

/// rcmd with no directories, reading the session at `state`.
fn shot_session(state: &std::path::Path, extra: &[&str]) -> String {
    let out = isolated()
        .env("RCMD_STATE", state)
        .arg("--screenshot=110x30")
        .args(extra)
        .output()
        .expect("rcmd should run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn session_file(
    dir: &std::path::Path,
    a: &std::path::Path,
    b: &std::path::Path,
    panes: usize,
) -> PathBuf {
    let tab = |p: &std::path::Path| {
        format!(
            "[[{{}}.tabs]]\npath = {:?}\nsort = \"name\"\nreverse = false\nhidden = false\n",
            p.display().to_string()
        )
    };
    // Built by hand rather than by rcmd, so this tests the format as a user's file
    // would have it on disk.
    let mut text = String::from("version = 1\nfocus = 0\nterminal_focused = false\n\n");
    text += "[terminal]\nshown = false\nheight_pct = 30\ntabs = []\n\n";
    if panes == 2 {
        text += "[panes]\nkind = \"split\"\naxis = \"cols\"\npct = 30\n\n";
        text += "[panes.a]\nkind = \"pane\"\nactive = 1\n";
        text += &tab(a).replace("{}", "panes.a");
        text += &tab(&a.join("sub")).replace("{}", "panes.a");
        text += "\n[panes.b]\nkind = \"pane\"\nactive = 0\n";
        text += &tab(b).replace("{}", "panes.b");
    } else {
        // Nested splits, one pane per leaf, `panes` deep.
        let mut node = "panes".to_string();
        for i in 0..panes - 1 {
            text += &format!("[{node}]\nkind = \"split\"\naxis = \"cols\"\npct = 50\n");
            text += &format!("[{node}.a]\nkind = \"pane\"\nactive = 0\n");
            text += &tab(a).replace("{}", &format!("{node}.a"));
            node = format!("{node}.b");
            if i == panes - 2 {
                text += &format!("[{node}]\nkind = \"pane\"\nactive = 0\n");
                text += &tab(b).replace("{}", &node);
            }
        }
    }
    let p = dir.join("session.toml");
    fs::write(&p, text).unwrap();
    p
}

#[test]
fn starting_without_directories_reopens_the_last_session() {
    let (a, b) = files(&["alpha.txt"]);
    fs::create_dir(a.join("sub")).unwrap();
    let state = session_file(a.parent().unwrap(), &a, &b, 2);

    let s = shot_session(&state, &[]);
    let first = s.lines().nth(1).unwrap();
    assert!(
        first.contains(" 1 a ") && first.contains(" 2 sub "),
        "both tabs come back\n{s}"
    );
    assert!(
        status(&s).contains("tab 2/2"),
        "the second was the active one\n{s}"
    );
    // The left pane's tab bar puts its border a row lower, so measure the right pane
    // by its own top-left corner on the first row.
    let right_pane = first.find('\u{250c}').map(|b| first[..b].chars().count());
    assert!(
        right_pane.is_some_and(|x| x < 50),
        "the 30% split is kept, not reset to half\n{s}"
    );
    assert!(
        !s.contains('\u{2302}'),
        "the terminal was hidden and stays hidden\n{s}"
    );

    let fresh = shot_session(&state, &["--no-restore"]);
    assert!(
        !fresh.lines().nth(1).unwrap().contains(" 2 sub "),
        "--no-restore starts fresh\n{fresh}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_session_with_more_panes_than_allowed_starts_fresh() {
    let (a, b) = files(&["alpha.txt"]);
    let state = session_file(a.parent().unwrap(), &a, &b, 5);
    let s = shot_session(&state, &[]);
    let borders = s
        .lines()
        .nth(1)
        .unwrap()
        .matches("\u{2510}\u{250c}")
        .count();
    assert_eq!(borders, 1, "two panes, the default, not five\n{s}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── secure modes ───────────────────────────────────────────────────────────────────

fn shot_flags(flags: &[&str], keys: &str, a: &std::path::Path, b: &std::path::Path) -> String {
    let out = isolated()
        .args(flags)
        .arg("--screenshot=110x24")
        .arg(format!("--keys={keys}"))
        .arg(a)
        .arg(b)
        .output()
        .expect("rcmd should run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn listing(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn read_only_turns_every_write_off_and_says_so() {
    let (a, b) = files(&["keep.txt"]);
    let before = (listing(&a), listing(&b));
    for keys in [
        "Down,F5",
        "Down,F6",
        "Down,F8",
        "Down,S-F8",
        "Down,S-F6",
        "F7",
        "Down,F4",
        "Down,F2",
    ] {
        let s = shot_flags(&["--read-only"], keys, &a, &b);
        assert!(s.contains("READ-ONLY"), "the badge is always there\n{s}");
        assert!(s.contains("is turned off"), "[{keys}] must say why\n{s}");
        assert!(!s.contains("[Enter]"), "[{keys}] no dialog may open\n{s}");
    }
    assert_eq!(
        (listing(&a), listing(&b)),
        before,
        "nothing on disk changed"
    );

    let bar = shot_flags(&["--read-only"], "", &a, &b);
    let keys = bar.lines().last().unwrap();
    for gone in ["5 Copy", "6 Move", "8 Del", "4 Edit", "7 MkDir"] {
        assert!(
            !keys.contains(gone),
            "the key bar must not offer {gone}\n{keys}"
        );
    }
    assert!(keys.contains("3 View"), "viewing still works\n{keys}");
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn read_only_also_blocks_the_mouse_paths() {
    let (a, b) = files(&["keep.txt"]);
    let base = shot_flags(&["--read-only"], "", &a, &b);
    let (x, y) = locate(&base, "keep");
    // Drag and drop: no copy-or-move menu.
    let dropped = shot_flags(&["--read-only"], &format!("drag:{x}:{y}:75:{y}"), &a, &b);
    assert!(!dropped.contains("here"), "{dropped}");
    assert!(dropped.contains("is turned off"), "{dropped}");
    // The context menu's Copy item: clicking it must not start anything.
    let menu = shot_flags(&["--read-only"], &format!("rclick:{x}:{y}"), &a, &b);
    let (mx, my) = locate(&menu, "Copy to other pane");
    let clicked = shot_flags(
        &["--read-only"],
        &format!("rclick:{x}:{y},click:{}:{my}", mx + 1),
        &a,
        &b,
    );
    assert!(!clicked.contains("[Enter] go"), "{clicked}");
    assert!(!b.join("keep.txt").exists());
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn restricted_has_no_shell_and_no_way_to_start_one() {
    let (a, b) = files(&["keep.txt"]);
    for keys in ["", "F12", "A-n", "A-t", "A-c"] {
        let s = shot_flags(&["--restricted"], keys, &a, &b);
        assert!(s.contains("RESTRICTED"), "{s}");
        assert!(
            !s.contains('\u{2302}'),
            "[{keys}] no terminal may appear\n{s}"
        );
    }
    let s = shot_flags(&["--restricted"], "Down,F5", &a, &b);
    assert!(
        s.contains("is turned off"),
        "restricted includes read-only\n{s}"
    );
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_flag_can_tighten_the_config_but_never_loosen_it() {
    let (a, b) = files(&["keep.txt"]);
    let dir = a.parent().unwrap();
    let ro = write_config(dir, "[security]\nmode = \"read_only\"\n");
    let s = shot_with(&ro, "", &a, &b);
    assert!(s.contains("READ-ONLY"), "the config alone applies\n{s}");
    let out = isolated()
        .args(["--config".as_ref(), ro.as_os_str(), "--restricted".as_ref()])
        .arg("--screenshot=110x24")
        .arg(&a)
        .arg(&b)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("RESTRICTED"),
        "tighter wins"
    );

    let rs = write_config(dir, "[security]\nmode = \"restricted\"\n");
    let out = isolated()
        .args(["--config".as_ref(), rs.as_os_str(), "--read-only".as_ref()])
        .arg("--screenshot=110x24")
        .arg(&a)
        .arg(&b)
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("RESTRICTED"), "a looser flag must not win\n{s}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_root_holds_against_dotdot_backspace_and_symlinks() {
    let (a, b) = files(&["keep.txt"]);
    fs::create_dir(a.join("inner")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/", a.join("inner/escape")).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", a.join("inner/leak.txt")).unwrap();
    }
    let root = a.display().to_string();
    let flags = ["--root", root.as_str()];

    let top = shot_flags(&flags, "", &a, &b);
    let first_rows: String = top.lines().skip(2).take(1).collect();
    assert!(!first_rows.contains(".."), "no way up from the root\n{top}");
    assert!(
        !top.lines()
            .next()
            .unwrap()
            .contains(&b.display().to_string()),
        "a start directory outside the root opens at the root\n{top}"
    );
    let up = shot_flags(&flags, "Backspace,A-Left", &a, &b);
    assert!(up.contains("this is the top"), "{up}");

    #[cfg(unix)]
    {
        // inner/ lists: .., escape, leak.txt
        let through = shot_flags(&flags, "Enter,Down,Enter", &a, &b);
        assert!(
            through.contains("confined"),
            "a directory link out is refused\n{through}"
        );
        let read = shot_flags(&flags, "Enter,Down,Down,F3", &a, &b);
        assert!(
            read.contains("outside the root"),
            "a file link out is not read\n{read}"
        );
        assert!(
            !read.contains("root:x:0:0"),
            "and nothing of it is shown\n{read}"
        );
    }
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

#[test]
fn a_root_flag_outside_the_configured_root_is_refused() {
    let (a, b) = files(&["keep.txt"]);
    let cfg = write_config(
        a.parent().unwrap(),
        &format!("[security]\nroot = {:?}\n", a.display().to_string()),
    );
    let out = isolated()
        .args([
            "--config".as_ref(),
            cfg.as_os_str(),
            "--root".as_ref(),
            b.as_os_str(),
        ])
        .arg("--screenshot=80x20")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("must be inside"));

    let missing = isolated()
        .args(["--root", "/definitely/not/here", "--screenshot=80x20"])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(2));
    let _ = fs::remove_dir_all(a.parent().unwrap());
}

// ── group A: search, filter, patterns, go to, history, hotlist, sizes, open ─────────

use std::ffi::OsStr;

/// A screenshot with its own state directory, so bookmarks one test writes are never
/// seen by another.
fn shot_state(state: &std::path::Path, args: &[&std::ffi::OsStr], keys: &str) -> String {
    let out = isolated()
        .env("RCMD_STATE", state.join("session.toml"))
        .arg("--screenshot=110x24")
        .arg(format!("--keys={keys}"))
        .args(args)
        .output()
        .expect("rcmd should run");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn slash_jumps_to_a_name_and_down_steps_to_the_next_match() {
    let (a, b) = files(&["my-readme", "README.md", "zeta"]);
    let s = shot_keys(SIZE, "/,r,e,a", &a, &b);
    assert!(status(&s).contains("README.md"), "a prefix wins\n{s}");
    assert!(s.contains("/  rea"), "the line shows what was typed\n{s}");
    let s = shot_keys(SIZE, "/,r,e,a,Down", &a, &b);
    assert!(
        status(&s).contains("my-readme"),
        "Down: the next match\n{s}"
    );
    let s = shot_keys(SIZE, "/,z,z,z", &a, &b);
    assert!(s.contains("no match"), "{s}");
}

#[test]
fn a_key_the_search_line_does_not_use_ends_it_and_does_its_job() {
    let (a, b) = files(&["keep.txt", "victim.txt"]);
    let s = shot_keys(SIZE, "/,v,i,c,F8", &a, &b);
    assert!(s.contains("Delete"), "F8 still deletes\n{s}");
    assert!(s.contains("victim.txt"), "the file search landed on\n{s}");
    assert!(!s.contains("keep.txt\u{2502}"), "{s}");
}

#[test]
fn the_filter_hides_what_does_not_match_until_esc() {
    let (a, b) = files(&["one.rs", "two.rs", "three.md"]);
    let s = shot_keys(SIZE, "Ctrl+f,.,r,s,Enter", &a, &b);
    assert!(
        s.contains("filter: .rs"),
        "the title says a filter is on\n{s}"
    );
    assert!(s.contains("one") && s.contains("two"), "{s}");
    assert!(!s.contains("three"), "hidden\n{s}");
    let s = shot_keys(SIZE, "Ctrl+f,.,r,s,Enter,Esc", &a, &b);
    assert!(s.contains("three") && !s.contains("filter:"), "{s}");
    assert!(s.contains("filter off"), "{s}");
    // Esc while still typing puts back what was there before: nothing.
    let s = shot_keys(SIZE, "Ctrl+f,.,r,s,Esc", &a, &b);
    assert!(s.contains("three") && !s.contains("filter:"), "{s}");
    // A pattern, rather than text found anywhere.
    let s = shot_keys(SIZE, "Ctrl+f,t,*,Enter", &a, &b);
    assert!(
        s.contains("two") && s.contains("three") && !s.contains("one"),
        "{s}"
    );
}

#[test]
fn a_mark_the_filter_hides_is_dropped_so_f8_cannot_reach_it() {
    let (a, b) = files(&["one.rs", "three.md"]);
    let s = shot_keys(SIZE, "Ctrl+a,Ctrl+f,.,r,s,Enter,F8", &a, &b);
    assert!(
        s.contains("one.rs"),
        "the visible mark is what F8 deletes\n{s}"
    );
    assert!(s.contains("1 file \u{2022}") && !s.contains("three"), "{s}");
}

#[test]
fn plus_and_minus_mark_by_pattern() {
    let (a, b) = files(&["a.jpg", "b.JPG", "c.png", "d.txt"]);
    let s = shot_keys(SIZE, "+,*,.,j,p,g,Space,*,.,p,n,g,Enter", &a, &b);
    assert!(s.contains("marked 3 matching *.jpg *.png"), "{s}");
    assert!(status(&s).contains("3 selected"), "{s}");
    let s = shot_keys(SIZE, "+,Enter,-,*,.,p,n,g,Enter", &a, &b);
    assert!(s.contains("unmarked 1"), "{s}");
    assert!(
        status(&s).contains("3 selected"),
        "4 marked, 1 taken off\n{s}"
    );
    let s = shot_keys(SIZE, "+,*,.,z,i,p,Enter", &a, &b);
    assert!(s.contains("nothing matches *.zip"), "{s}");
}

#[test]
fn go_to_completes_with_tab_and_lands_on_a_file() {
    let (a, b) = fixture();
    fs::create_dir_all(a.join("nested")).unwrap();
    fs::write(a.join("nested/deep.txt"), b"x").unwrap();
    let s = shot_keys(SIZE, "Ctrl+l,n,e,Tab,d,e,e,p,.,t,x,t,Enter", &a, &b);
    assert!(
        status(&s).contains("deep.txt"),
        "the cursor is on the file\n{s}"
    );
    assert!(s.contains("nested"), "{s}");
    let s = shot_keys(SIZE, "Ctrl+l,n,o,p,e,Enter", &a, &b);
    assert!(s.contains("Go to"), "a wrong path keeps the box open\n{s}");
}

#[test]
fn back_and_forward_retrace_the_way_and_history_lists_it() {
    let (a, b) = fixture();
    fs::create_dir_all(a.join("inner")).unwrap();
    let s = shot_keys(SIZE, "Down,Enter,Alt+comma", &a, &b);
    assert!(
        status(&s).contains("inner"),
        "back lands on where it came from\n{s}"
    );
    let s = shot_keys(SIZE, "Down,Enter,Alt+comma,Alt+period", &a, &b);
    assert!(status(&s).contains(".."), "forward is inside again\n{s}");
    let s = shot_keys(SIZE, "Alt+comma", &a, &b);
    assert!(s.contains("nothing to go back to"), "{s}");
    let s = shot_keys(SIZE, "Down,Enter,Alt+Down", &a, &b);
    assert!(s.contains("History"), "{s}");
    let s = shot_keys(SIZE, "Down,Enter,Alt+Down,1", &a, &b);
    assert!(status(&s).contains("inner"), "1 picks the first entry\n{s}");
}

#[test]
fn the_hotlist_bookmarks_privately_and_goes_there() {
    let (a, b) = fixture();
    let state = a.parent().unwrap().join("state");
    let args = [a.as_os_str(), b.as_os_str()];
    let s = shot_state(&state, &args, "Ctrl+d,Enter");
    assert!(s.contains("bookmarked"), "{s}");
    let file = state.join("bookmarks.toml");
    let text = fs::read_to_string(&file).expect("saved beside the session");
    assert!(
        text.contains(&*a.file_name().unwrap().to_string_lossy()),
        "{text}"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "it lists where you work: owner only");
    }
    // From the other pane: Tab, then 1 jumps to the bookmark.
    let s = shot_state(&state, &args, "Tab,Ctrl+d,1");
    let left_title = s.lines().nth(1).unwrap().to_string();
    assert_eq!(
        left_title.matches("/a ").count() + left_title.matches("\\a ").count(),
        2,
        "both panes now show a\n{s}"
    );
    // Read-only: the hotlist still goes places, but adding is greyed out and refused.
    let ro = [OsStr::new("--read-only"), a.as_os_str(), b.as_os_str()];
    let s = shot_state(&state, &ro, "Tab,Ctrl+d,Enter");
    assert!(!s.contains("bookmarked"), "{s}");
}

#[test]
fn space_on_a_directory_works_out_its_size() {
    let (a, b) = fixture();
    fs::create_dir_all(a.join("box/inner")).unwrap();
    fs::write(a.join("box/inner/f"), vec![0u8; 3000]).unwrap();
    fs::write(a.join("box/g"), vec![0u8; 2000]).unwrap();
    let s = shot_keys(SIZE, "Down,Space", &a, &b);
    assert!(s.contains("4.9 K"), "5000 bytes below it\n{s}");
    assert!(status(&s).contains("1 selected  4.9 K"), "{s}");
    let s = shot_keys(SIZE, "Ctrl+Space", &a, &b);
    assert!(s.contains("4.9 K"), "Ctrl+Space: every directory\n{s}");
    assert!(!status(&s).contains("selected"), "and marks nothing\n{s}");
}

#[test]
fn swap_trades_the_panes_and_mirror_copies_the_directory_across() {
    let (a, b) = fixture();
    let top = |s: &str| s.lines().nth(1).unwrap().to_string();
    let s = shot_keys(SIZE, "Ctrl+u", &a, &b);
    let t = top(&s);
    assert!(col(&t, &sep("b")) < col(&t, &sep("a")), "{s}");
    let s = shot_keys(SIZE, "Alt+=", &a, &b);
    assert!(!top(&s).contains("/b ") && !top(&s).contains("\\b "), "{s}");
}

#[test]
fn o_opens_with_the_configured_program_and_asks_before_running_one() {
    let (a, b) = fixture();
    let cfg_dir = a.parent().unwrap().join("cfg");
    fs::create_dir_all(&cfg_dir).unwrap();
    let cfg = write_config(&cfg_dir, "[commands]\nopener = \"my-viewer\"\n");
    let s = shot_with(&cfg, "Down,o", &a, &b);
    assert!(s.contains("would open victim.txt with my-viewer"), "{s}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let prog = a.join("victim.txt");
        fs::set_permissions(&prog, fs::Permissions::from_mode(0o755)).unwrap();
        let s = shot_with(&cfg, "Down,o", &a, &b);
        assert!(s.contains("Run it?"), "a program is asked about\n{s}");
        let s = shot_with(&cfg, "Down,o,Enter", &a, &b);
        assert!(!s.contains("would open"), "Enter is Cancel\n{s}");
    }
    let s = shot_flags(&["--read-only"], "Down,o", &a, &b);
    assert!(
        s.contains("opening files in other programs is turned off"),
        "{s}"
    );
}

#[test]
fn auto_refresh_is_configurable_and_checked() {
    let dir = std::env::temp_dir().join(format!("rcmd-cfg-refresh-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    for (body, ok) in [
        ("[files]\nauto_refresh_ms = 0\n", true),
        ("[files]\nauto_refresh_ms = 250\n", true),
        ("[files]\nauto_refresh_ms = 5\n", false),
        ("[bookmarks]\nwork = \"~/projects\"\n", true),
    ] {
        let cfg = write_config(&dir, body);
        let out = rcmd(&[
            OsStr::new("--config"),
            cfg.as_os_str(),
            OsStr::new("--check-config"),
        ]);
        assert_eq!(out.status.success(), ok, "{body}");
    }
}

// ── group B: find, quick view ───────────────────────────────────────────────────────

#[test]
fn alt_f7_finds_by_name_and_content_into_a_new_tab() {
    let (a, b) = fixture();
    fs::create_dir_all(a.join("src/deep")).unwrap();
    fs::write(a.join("src/main.rs"), "fn main() { todo!() }").unwrap();
    fs::write(a.join("src/deep/lib.rs"), "// fine").unwrap();
    let s = shot_keys("110x24", "Alt+F7,*,.,r,s,Enter", &a, &b);
    assert!(s.contains("src/main") || s.contains("src\\main"), "{s}");
    assert!(s.contains("deep/lib") || s.contains("deep\\lib"), "{s}");
    assert!(
        s.contains(" 1 a ") && s.contains(" 2 a "),
        "a new tab, the old one kept\n{s}"
    );
    assert!(s.contains("2 found"), "{s}");
    let s = shot_keys("110x24", "Alt+F7,Tab,T,O,D,O,Enter", &a, &b);
    assert!(
        s.contains("main") && !s.contains("deep"),
        "content, any case\n{s}"
    );
    // Enter on a result goes to it.
    let s = shot_keys("110x24", "Alt+F7,*,.,r,s,Enter,End,Enter", &a, &b);
    assert!(status(&s).contains("main.rs"), "{s}");
    assert!(
        !s.lines().nth(2).unwrap().contains("find "),
        "back in a directory\n{s}"
    );
}

#[test]
fn alt_q_previews_the_file_under_the_cursor_in_the_other_pane() {
    let (a, b) = files(&["first.txt", "second.txt"]);
    let s = shot_keys(SIZE, "Down,Alt+q", &a, &b);
    assert!(s.contains("view") && s.contains("first.txt"), "{s}");
    let s = shot_keys(SIZE, "Down,Alt+q,Down", &a, &b);
    assert!(s.contains("second.txt"), "it follows the cursor\n{s}");
    let s = shot_keys(SIZE, "Down,Alt+q,Tab", &a, &b);
    let top = s.lines().nth(1).unwrap();
    assert!(
        top.contains("/b ") || top.contains("\\b "),
        "taking the focus there ends it\n{s}"
    );
}

#[test]
fn bulk_rename_is_off_in_read_only_mode() {
    let (a, b) = fixture();
    let s = shot_flags(&["--read-only"], "Down,Shift+F2", &a, &b);
    assert!(s.contains("renaming is turned off"), "{s}");
}

// ── group B: small file operations ──────────────────────────────────────────────────

#[test]
fn alt_h_shows_the_sha256_and_checks_it_against_a_sums_file() {
    let (a, b) = files(&[]);
    fs::write(a.join("abc.bin"), b"abc").unwrap();
    let good = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    fs::write(a.join("SHA256SUMS"), format!("{good}  abc.bin\n")).unwrap();
    let s = shot_keys("110x24", "/,a,b,c,Enter,Alt+h", &a, &b);
    assert!(s.contains(good), "{s}");
    assert!(s.contains("matches SHA256SUMS"), "{s}");
    fs::write(
        a.join("SHA256SUMS"),
        format!("{}  abc.bin\n", "0".repeat(64)),
    )
    .unwrap();
    let s = shot_keys("110x24", "/,a,b,c,Enter,Alt+h", &a, &b);
    assert!(s.contains("DOES NOT MATCH"), "a mismatch is loud\n{s}");
}

#[test]
fn shift_f5_copies_beside_itself_and_shift_f4_creates_a_file() {
    let (a, b) = fixture();
    shot_keys(SIZE, "Down,Shift+F5,c,o,p,y,Enter", &a, &b);
    assert_eq!(
        fs::read(a.join("copy.txt")).unwrap(),
        b"content",
        "name selected, extension kept"
    );
    assert_eq!(fs::read(a.join("victim.txt")).unwrap(), b"content");
    shot_keys(SIZE, "Shift+F4,n,e,w,.,m,d,Enter", &a, &b);
    assert!(a.join("new.md").is_file());
    // Shift+F4 on an existing name does not empty it.
    shot_keys(SIZE, "Shift+F4,v,i,c,t,i,m,.,t,x,t,Enter", &a, &b);
    assert_eq!(fs::read(a.join("victim.txt")).unwrap(), b"content");
}

#[cfg(unix)]
#[test]
fn alt_p_sets_a_mode_and_ctrl_shift_f5_links_into_the_other_pane() {
    use std::os::unix::fs::PermissionsExt;
    let (a, b) = fixture();
    let s = shot_keys(SIZE, "Down,Alt+p,6,0,0,Enter", &a, &b);
    assert!(s.contains("permissions set on 1"), "{s}");
    let mode = fs::metadata(a.join("victim.txt"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let s = shot_keys(SIZE, "Down,Alt+p,9,Enter", &a, &b);
    assert!(
        s.contains("not an octal mode"),
        "the box stays open with why\n{s}"
    );

    shot_keys(SIZE, "Down,Ctrl+Shift+F5", &a, &b);
    let link = b.join("victim.txt");
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read(&link).unwrap(), b"content");
    let s = shot_keys(SIZE, "Down,Ctrl+Shift+F5", &a, &b);
    assert!(s.contains("already exists"), "never replaces\n{s}");
}

#[test]
fn the_new_writes_are_off_in_read_only_mode() {
    let (a, b) = fixture();
    for (keys, what) in [
        ("Shift+F4", "creating files"),
        ("Down,Shift+F5", "creating files"),
        ("Down,Ctrl+Shift+F5", "creating files"),
        ("Down,Alt+p", "changing permissions"),
    ] {
        let s = shot_flags(&["--read-only"], keys, &a, &b);
        assert!(s.contains(&format!("{what} is turned off")), "{keys}\n{s}");
    }
    assert_eq!(listing(&a), vec!["victim.txt"]);
    assert!(listing(&b).is_empty());
}

#[test]
fn a_menu_entry_with_a_bad_key_rejects_the_config() {
    let dir = std::env::temp_dir().join(format!("rcmd-cfg-menu-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    for (body, ok) in [
        (
            "[[menu]]\nkey = \"g\"\nlabel = \"git\"\ncommand = \"git status\"\n",
            true,
        ),
        (
            "[[menu]]\nkey = \"gg\"\nlabel = \"git\"\ncommand = \"git status\"\n",
            false,
        ),
        (
            "[[menu]]\nkey = \"g\"\nlabel = \"git\"\ncommand = \"\"\n",
            false,
        ),
    ] {
        let cfg = write_config(&dir, body);
        let out = rcmd(&[
            OsStr::new("--config"),
            cfg.as_os_str(),
            OsStr::new("--check-config"),
        ]);
        assert_eq!(out.status.success(), ok, "{body}");
    }
}

// ── group C: archives ───────────────────────────────────────────────────────────────

/// Build a zip with rcmd itself (Alt+F5), so the tests need no zip tool and no crate.
fn packed(a: &std::path::Path, b: &std::path::Path) {
    fs::create_dir_all(a.join("proj/src")).unwrap();
    fs::write(
        a.join("proj/src/main.rs"),
        "fn main() { println!(\"hi\"); }",
    )
    .unwrap();
    fs::write(a.join("proj/README"), "read me").unwrap();
    shot_keys(SIZE, "/,p,r,o,j,Enter,Alt+F5,Enter", a, b);
    assert!(
        b.join("proj.zip").is_file(),
        "Alt+F5 packs into the other pane"
    );
}

#[test]
fn enter_browses_an_archive_and_f3_reads_inside_it() {
    let (a, b) = fixture();
    packed(&a, &b);
    let s = shot_keys(SIZE, "Tab,/,p,r,o,j,Enter,Enter", &a, &b);
    assert!(s.contains("proj.zip/"), "the title says where inside\n{s}");
    let s = shot_keys(
        SIZE,
        "Tab,/,p,r,o,j,Enter,Enter,Down,Enter,/,R,E,A,D,Enter,F3",
        &a,
        &b,
    );
    assert!(s.contains("read me"), "F3 shows the file from inside\n{s}");
    // .. at the top leaves, cursor on the archive.
    let s = shot_keys(SIZE, "Tab,/,p,r,o,j,Enter,Enter,Backspace", &a, &b);
    assert!(status(&s).contains("proj.zip"), "{s}");
}

#[test]
fn f5_from_inside_an_archive_extracts_and_writes_are_refused() {
    let (a, b) = fixture();
    packed(&a, &b);
    fs::remove_dir_all(a.join("proj")).unwrap();
    // Browse b's archive, go into proj/src, F5 extracts main.rs into a.
    let s = shot_keys(
        SIZE,
        "Tab,/,p,r,o,j,Enter,Enter,Down,Enter,/,s,r,c,Enter,Enter,Down,F5",
        &a,
        &b,
    );
    assert!(s.contains("Extract"), "{s}");
    shot_keys(
        SIZE,
        "Tab,/,p,r,o,j,Enter,Enter,Down,Enter,/,s,r,c,Enter,Enter,Down,F5,Enter",
        &a,
        &b,
    );
    assert_eq!(
        fs::read_to_string(a.join("main.rs")).unwrap(),
        "fn main() { println!(\"hi\"); }",
        "only the chosen file, without the directories above it"
    );
    for key in ["F8", "F6", "F7", "Shift+F6", "F4"] {
        let s = shot_keys(
            SIZE,
            &format!("Tab,/,p,r,o,j,Enter,Enter,Down,{key}"),
            &a,
            &b,
        );
        assert!(s.contains("inside an archive"), "{key}\n{s}");
    }
}

#[test]
fn alt_f9_unpacks_everything_into_the_other_pane() {
    let (a, b) = fixture();
    packed(&a, &b);
    fs::remove_dir_all(a.join("proj")).unwrap();
    shot_keys(SIZE, "Tab,/,p,r,o,j,Enter,Alt+F9,Enter", &a, &b);
    assert_eq!(
        fs::read_to_string(a.join("proj/README")).unwrap(),
        "read me"
    );
}

#[test]
fn an_archive_with_escaping_names_hides_them_and_writes_nothing_outside() {
    let (a, b) = fixture();
    // A zip built by hand: one honest entry, one that climbs out.
    let z = a.join("evil.zip");
    write_zip(&z, &[("ok.txt", b"fine"), ("../../escaped.txt", b"x")]);
    let s = shot_keys(SIZE, "/,e,v,i,l,Enter,Enter", &a, &b);
    assert!(s.contains("names outside the archive are hidden"), "{s}");
    assert!(!s.contains("escaped"), "{s}");
    shot_keys(SIZE, "/,e,v,i,l,Enter,Alt+F9,Enter", &a, &b);
    assert_eq!(fs::read_to_string(b.join("ok.txt")).unwrap(), "fine");
    let root = a.parent().unwrap();
    assert!(
        !root.join("escaped.txt").exists() && !root.parent().unwrap().join("escaped.txt").exists()
    );
}

/// A minimal zip writer for the hostile-archive test: stored entries, names as given.
/// Written out by hand so the test does not depend on the code it is testing.
fn write_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    (c >> 1) ^ 0xEDB8_8320
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, body) in entries {
        let off = out.len() as u32;
        let crc = crc32(body);
        let (n, len) = (name.len() as u16, body.len() as u32);
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&len.to_le_bytes());
        central.extend_from_slice(&n.to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&off.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_off = out.len() as u32;
    let cd_len = central.len() as u32;
    out.extend_from_slice(&central);
    let k = entries.len() as u16;
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&k.to_le_bytes());
    out.extend_from_slice(&k.to_le_bytes());
    out.extend_from_slice(&cd_len.to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    fs::write(path, out).unwrap();
}

// ── group C: compare and synchronise ────────────────────────────────────────────────

fn aged(p: &std::path::Path, body: &str, age_secs: u64) {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).unwrap();
    }
    fs::write(p, body).unwrap();
    let t = std::time::SystemTime::now() - std::time::Duration::from_secs(age_secs);
    let f = fs::File::options().write(true).open(p).unwrap();
    f.set_times(fs::FileTimes::new().set_modified(t)).unwrap();
}

fn two_trees() -> (PathBuf, PathBuf) {
    let (a, b) = files(&[]);
    aged(&a.join("only-a.txt"), "a", 100);
    aged(&b.join("only-b.txt"), "b", 100);
    aged(&a.join("newer.txt"), "new", 10);
    aged(&b.join("newer.txt"), "old", 1000);
    aged(&a.join("clash.txt"), "aaa", 500);
    aged(&b.join("clash.txt"), "b", 500);
    aged(&a.join("same.txt"), "s", 300);
    aged(&b.join("same.txt"), "s", 300);
    aged(&a.join("sub/deep.txt"), "deep", 100);
    fs::create_dir_all(b.join("sub")).unwrap();
    (a, b)
}

#[test]
fn alt_k_marks_what_the_other_side_lacks_and_f5_copies_it() {
    let (a, b) = two_trees();
    let s = shot_keys("110x24", "Alt+k", &a, &b);
    assert!(s.contains("marked: 2 here, 1 there"), "{s}");
    assert!(s.contains("1 differ at the same time"), "{s}");
    let s = shot_keys("110x24", "Alt+k,F5", &a, &b);
    assert!(
        s.contains("2 items"),
        "F5 takes exactly the marked two\n{s}"
    );
    // newer.txt sorts first and exists there: the job stops to ask, as F5 always does.
    let s = shot_keys("110x24", "Alt+k,F5,Enter", &a, &b);
    assert!(s.contains("overwrite"), "{s}");
    assert_eq!(
        fs::read_to_string(b.join("newer.txt")).unwrap(),
        "old",
        "not before the answer"
    );
}

#[test]
fn alt_y_shows_the_plan_then_makes_both_trees_agree() {
    let (a, b) = two_trees();
    let s = shot_keys("110x30", "Alt+y", &a, &b);
    assert!(s.contains("Synchronise"), "{s}");
    assert!(
        s.contains("\u{2192} only-a.txt") && s.contains("\u{2190} only-b.txt"),
        "{s}"
    );
    assert!(s.contains("newer.txt  replaces an older copy"), "{s}");
    assert!(s.contains("clash.txt  same time, different size"), "{s}");
    assert!(
        s.contains("sub/deep.txt") || s.contains("sub\\deep.txt"),
        "deep\n{s}"
    );
    assert!(
        b.join("only-b.txt").exists() && !a.join("only-b.txt").exists(),
        "nothing yet"
    );

    shot_keys("110x30", "Alt+y,Enter", &a, &b);
    let read = |p: PathBuf| fs::read_to_string(p).unwrap();
    assert_eq!(read(b.join("only-a.txt")), "a");
    assert_eq!(read(a.join("only-b.txt")), "b");
    assert_eq!(
        read(b.join("newer.txt")),
        "new",
        "the newer one wins, as the plan said"
    );
    assert_eq!(read(b.join("sub/deep.txt")), "deep");
    assert_eq!(read(a.join("clash.txt")), "aaa", "a conflict is left alone");
    assert_eq!(read(b.join("clash.txt")), "b");

    let s = shot_keys("110x30", "Alt+y", &a, &b);
    assert!(s.contains("only differences that need a person"), "{s}");
}

#[test]
fn synchronising_is_off_in_read_only_mode() {
    let (a, b) = two_trees();
    let s = shot_flags(&["--read-only"], "Alt+y", &a, &b);
    assert!(s.contains("synchronising is turned off"), "{s}");
}

// ── group C: background jobs, looks, pictures ───────────────────────────────────────

/// A PNG written byte by byte (zlib stored blocks), so the test needs no encoder.
fn write_png(path: &std::path::Path, w: u32, h: u32, rgb: [u8; 3]) {
    fn crc(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    (c >> 1) ^ 0xEDB8_8320
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut c = kind.to_vec();
        c.extend_from_slice(body);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc(&c).to_be_bytes());
    }
    // Each row: filter byte 0, then the pixels.
    let row: Vec<u8> = std::iter::once(0u8)
        .chain(std::iter::repeat_n(rgb, w as usize).flatten())
        .collect();
    let raw: Vec<u8> = std::iter::repeat_n(row, h as usize).flatten().collect();
    let mut z = vec![0x78, 0x01, 0x01];
    z.extend_from_slice(&(raw.len() as u16).to_le_bytes());
    z.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
    z.extend_from_slice(&raw);
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    fs::write(path, out).unwrap();
}

#[test]
fn quick_view_draws_a_picture_in_half_blocks() {
    let (a, b) = files(&[]);
    write_png(&a.join("red.png"), 8, 4, [255, 0, 0]);
    let s = shot_keys(SIZE, "Down,Alt+q", &a, &b);
    assert!(s.contains("image 8\u{d7}4"), "{s}");
    assert!(s.contains('\u{2580}'), "drawn as half blocks\n{s}");
    fs::write(a.join("broken.png"), b"not a png").unwrap();
    let s = shot_keys(SIZE, "/,b,r,o,Enter,Alt+q", &a, &b);
    assert!(
        s.contains("image"),
        "a broken image says so, it does not crash\n{s}"
    );
}

#[test]
fn the_looks_are_off_by_default_and_valid_in_the_config() {
    let dir = std::env::temp_dir().join(format!("rcmd-cfg-look-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let cfg = write_config(
        &dir,
        "[files]\nls_colors = true\nicons = true\ngit_status = true\n",
    );
    let out = rcmd(&[
        OsStr::new("--config"),
        cfg.as_os_str(),
        OsStr::new("--check-config"),
    ]);
    assert!(out.status.success());
    let (a, b) = fixture();
    let s = shot_keys(SIZE, "", &a, &b);
    assert!(
        s.contains("\u{2502} victim"),
        "no icon, no git letter by default\n{s}"
    );
}

// ── menu bar and usage ──────────────────────────────────────────────────────────────

#[test]
fn f9_opens_the_menu_bar_and_left_right_move_along_it() {
    let (a, b) = fixture();
    let s = shot_keys("100x34", "", &a, &b);
    assert!(
        s.lines().next().unwrap().contains(" File   View   Help "),
        "{s}"
    );
    let s = shot_keys("100x34", "F9", &a, &b);
    assert!(
        s.contains("Rename many") && s.contains("Shift+F2"),
        "File, hints from the keymap\n{s}"
    );
    let s = shot_keys("100x34", "F9,Right", &a, &b);
    assert!(
        s.contains("Quick search") && !s.contains("Rename many"),
        "{s}"
    );
    let s = shot_keys("100x34", "F9,Left", &a, &b);
    assert!(
        s.contains("About rcmd"),
        "Left from File wraps to Help\n{s}"
    );
    let s = shot_keys("100x34", "F9,Left,Up,Enter", &a, &b);
    assert!(s.contains("No network, no telemetry"), "About\n{s}");
}

#[test]
fn a_click_on_a_title_opens_it_and_an_item_does_its_action() {
    let (a, b) = fixture();
    let s = shot_keys("100x34", "", &a, &b);
    let (x, y) = locate(&s, "View");
    let s = shot_keys("100x34", &format!("click:{}:{y}", x + 1), &a, &b);
    assert!(s.contains("Hidden files"), "{s}");
    let (hx, hy) = locate(&s, "Hidden files");
    let s = shot_keys(
        "100x34",
        &format!("click:{}:{y},click:{}:{hy}", x + 1, hx + 1),
        &a,
        &b,
    );
    assert!(s.contains("hidden files shown"), "{s}");
}

#[test]
fn what_the_mode_turns_off_is_refused_from_the_menu_too() {
    let (a, b) = fixture();
    // File > Delete is the 19th row: walk to it and press Enter.
    let downs = vec!["Down"; 17].join(",");
    let s = shot_flags(&["--read-only"], &format!("Down,F9,{downs},Enter"), &a, &b);
    assert!(!s.contains("Moved to the trash"), "no delete dialog\n{s}");
    assert!(a.join("victim.txt").exists());
}

#[test]
fn the_menu_bar_can_be_turned_off_and_f9_still_works() {
    let (a, b) = fixture();
    let cfg_dir = a.parent().unwrap().join("cfg");
    fs::create_dir_all(&cfg_dir).unwrap();
    let cfg = write_config(&cfg_dir, "[general]\nmenu_bar = false\n");
    let s = shot_with(&cfg, "", &a, &b);
    assert!(!s.lines().next().unwrap().contains("File"), "{s}");
    let s = shot_with(&cfg, "F9", &a, &b);
    assert!(s.contains("Rename many"), "{s}");
}

#[test]
fn shift_f1_shows_what_rcmd_is_using() {
    let (a, b) = fixture();
    let s = shot_keys("100x34", "Shift+F1", &a, &b);
    assert!(s.contains("Usage") && s.contains("holding"), "{s}");
    assert!(s.contains("2 panes"), "{s}");
    if cfg!(target_os = "linux") {
        assert!(s.contains("memory") && s.contains("threads"), "{s}");
    }
    let s = shot_keys("100x34", "Shift+F1,Esc", &a, &b);
    assert!(!s.contains("holding"), "{s}");
}

#[test]
fn the_readme_tagline_is_the_cargo_description() {
    // Cargo.toml is the one source; the README is the one place that cannot read it.
    let readme = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md"));
    let tagline = readme.lines().nth(2).unwrap_or_default();
    assert_eq!(
        tagline,
        env!("CARGO_PKG_DESCRIPTION"),
        "edit one, edit the other"
    );
    let out = rcmd(&[OsStr::new("--version")]);
    let v = String::from_utf8_lossy(&out.stdout);
    assert!(v.contains("MIT") && v.contains("XDevTeam"), "{v}");
}

/// macOS found this: its temp directory is under /var, a link to /private/var. A pane
/// opened through a link lists the resolved path after moving, so Enter on a Find
/// result has to find the file by name, not by the path the search walked.
#[cfg(unix)]
#[test]
fn enter_on_a_result_lands_on_the_file_when_the_pane_was_opened_through_a_link() {
    let (a, b) = fixture();
    fs::create_dir_all(a.join("src")).unwrap();
    fs::write(a.join("src/main.rs"), "fn main() {}").unwrap();
    let link = a.parent().unwrap().join("via-link");
    std::os::unix::fs::symlink(&a, &link).unwrap();
    let s = shot_keys("110x24", "Alt+F7,*,.,r,s,Enter,End,Enter", &link, &b);
    assert!(status(&s).contains("main.rs"), "{s}");
}
