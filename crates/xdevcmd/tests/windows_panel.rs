//! Windows: xdevcmd started from a terminal panel asks before starting, under a real
//! ConPTY. The pty suite (tests/pty.rs) is Unix only; this is the Windows side of its
//! `xdevcmd_will_not_start_inside_its_own_terminal_panel`.
//!
//! The test process plays the panel's xdevcmd: its pid is the `XDEVCMD_PANEL` a panel's
//! shell would hand down. Windows cannot tell that panel from a new window opened from
//! it, so xdevcmd asks; Enter is no.
#![cfg(windows)]

mod winpty;

use winpty::Run;

const QUESTION: &str = "Start another xdevcmd here?";

#[test]
fn started_from_a_panel_it_asks_and_enter_is_no() {
    let mut run = Run::start("no", "", Some(std::process::id()));
    run.wait_for_text(QUESTION);
    assert!(
        run.text()
            .contains(&format!("xdevcmd {}", std::process::id())),
        "the question names the panel's xdevcmd:\n{:?}",
        run.text()
    );
    run.send(b"\r");
    assert_eq!(run.exit_code(), 1, "{:?}", run.text());
}

#[test]
fn started_from_a_panel_yes_starts_it() {
    let mut run = Run::start("yes", "", Some(std::process::id()));
    run.wait_for_text(QUESTION);
    run.send(b"y\r");
    // The menu bar, drawn: it started. q quits, as in tests/pty.rs.
    run.wait_for("Help");
    run.send(b"q");
    assert_eq!(run.exit_code(), 0, "{:?}", run.text());
}
