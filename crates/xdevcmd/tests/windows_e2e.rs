//! Windows end to end, under a real ConPTY: xdevcmd draws its panes, F12 opens a shell
//! (cmd.exe) that runs a command, F12 comes back and F10 ends it. tests/pty.rs does this
//! and much more on Unix; this is the start of the same on Windows.
#![cfg(windows)]

mod winpty;

use winpty::{Run, F10, F12};

const CMD: &str = "[terminal]\nshell = \"cmd.exe\"\n";

#[test]
fn it_draws_runs_a_command_in_its_shell_and_quits() {
    let mut run = Run::start("e2e", CMD, None);
    std::fs::write(run.dir.join("a").join("hello.txt"), "hi").unwrap();
    run.wait_for("Help");
    run.wait_for("Name");
    // The pane's listing (it was read before the file was made, so ask again).
    run.send(b"\x12");
    run.wait_for("hello");

    run.send(F12);
    // cmd's prompt, in the pane's directory.
    run.wait_for("\\a>");
    // A mark only the output can have: %OS:~0,0% is empty, the typed line keeps it.
    run.send(b"echo e2e-%OS:~0,0%done\r");
    run.wait_for("e2e-done");

    run.send(F12);
    std::thread::sleep(std::time::Duration::from_millis(300));
    run.send(F10);
    assert_eq!(run.exit_code(), 0, "{}", run.screen());
}
