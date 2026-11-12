# Round 027: Log a terminal tab to a file (v0.4.0, step 4, item 35)

**Status**: Review
**Part of**: v0.4.0, step 4 of `ROADMAP.md` ("Next"); item 35 (the owner's go,
2026-11-12: "go ahead")
**Date started**: 2026-11-12
**Date completed**: YYYY-MM-DD

## Goal

A terminal tab's output can be written to a file while it runs, as PuTTY's session
logging does: turned on per tab from the tab's menu ("Log to file…"), off by default,
with a ● on the tab while it runs. Shift+F12 stays a view of what the shell keeps; the
log keeps everything, including what scrolled past the scrollback. Nothing is lost
without the user being told.

## Plan (kickoff review)

### 1. Where it starts

- ROADMAP, note on 35 (and the scope proposal's trims for **L**): per tab, off by
  default, the tab menu's "Log to file…", a ● on the tab while it runs. `text` (escapes
  stripped, lines a CR rewrote kept once) or `raw`. File `0600`, appended. Refused in
  read-only and restricted modes. The docs' "never written to disk" becomes "unless you
  turn on a log for that tab". `[terminal] log = "always"` waits for 0.5.
- Pillar 1, B04 (`memory/2026-11-12-four-pillars-for-rc1.md`): the journal ignores its
  write errors (`let _ =`). The same failure is not repeated here: a log that cannot be
  written stops and says so.
- **The bytes today.** `TermPane::start` (pane/term.rs) runs three threads per shell;
  the reader reads up to 8 KiB from the pty, counts cursor-position queries, and feeds
  the chunk to the vt100 parser under its write lock. vt100 keeps `[terminal]
  scrollback` lines (default 5000); what falls off the top is gone.
- **The tab menu** (`App::open_tab_menu`, Round 015): View history · Save history as…
  (greyed where `App::refused(Action::CopyAs)` refuses creating files) · List from the
  last command… · ─ · Close tab. Items carry the tab's `TabId`, resolved when run
  (`tab_still_there`), never its position.
- **Save as…** (`begin_save_as`/`apply_save_as`): an `Input` prefilled with
  `xdevcmd-history-<date>-<time>.txt` in the shell's directory when the system gave it
  (`History::cwd`, trusted only), else the pane's; refuses `..` and a parent outside
  `--root`, control characters, a directory, a missing folder. Core
  `document::save_bytes`: `policy::guard_write`, `create_new`, `0600` on Unix.
- A tab holds a split tree of shells; the menu's history items act on the tab's
  **focused** shell.

### 2. Critique: what must be got right

- **Where the bytes are tapped.** In the reader thread, on the chunk as read, **before**
  vt100: nothing the parser drops (scrolled-off lines, a full-screen program's
  redraws) is lost to the log, and the log does not depend on the scrollback setting.
- **The reader never waits on the disk.** It hands a copy of the chunk to a bounded
  channel (`sync_channel`, 256 chunks of ≤ 8 KiB: at most 2 MiB queued) and a writer
  thread per log does the filtering and the writing, through a 64 KiB `BufWriter`,
  flushed whenever the queue runs dry. The reader takes the log's handle out of its
  slot before sending, so the UI thread never waits behind it.
- **A slow or full disk.** Full queue: the reader retries for at most 500 ms (a real
  terminal that logs slows its program to the disk's speed; a bounded pause is the same
  thing, briefly), then **gives the log up**: no more output is queued, the writer
  writes what it has and, in text mode, a last line saying output after this point is
  not in the file. A write or flush that fails (disk full, a removed mount): the writer
  stops at once with the system's error. Either way the log's state becomes
  **Failed**, the status line says so once ("log stopped: … — <path>"), and the tab's
  ● becomes a **!** in the error colour, which stays until Stop logging or the tab
  closes. Never silently dropped: a chunk is either queued or the log is marked failed
  before the reader moves on.
- **text mode** (the default), a byte-wise state machine that keeps its state across
  chunks, so a sequence or a character split by a read is handled as a whole:
  - CSI, OSC (BEL or ST), DCS/SOS/PM/APC (to ST) and two-byte ESC sequences are
    removed; other C0 controls (BEL) are dropped; C1 controls are dropped.
  - A line is kept as a terminal would leave it: **CR returns to its start and what
    follows overwrites**, `CSI K` erases, `CSI G/C/D` move along it, BS steps back; the
    line is written once, at LF. A progress bar `10%\r50%\r100%\n` becomes `100%`.
  - UTF-8 is decoded across chunk boundaries (a lead byte at the end of one read, its
    continuation in the next); invalid bytes become U+FFFD, never a broken file.
  - **Bounded lines**: past 16 384 characters without a LF the line is written and a
    new one started (`cat` of a binary, `yes | tr -d '\n'`).
  - **Full-screen programs** (alternate screen, `?1049h`/`?1047h`/`?47h`): their
    redraws are not text; the log writes one line saying a full-screen program ran
    there, and resumes at `?1049l`. `raw` keeps it all.
  - Trailing spaces are trimmed; a partial last line is written when the log ends.
  - A header line when the log starts and a footer when it ends (time, shell, tab), so
    an appended file says where each run begins.
- **raw mode**: the bytes as received, nothing added (no header, no notes: the file
  replays with `cat`). The status line is the only report of a failure there.
- **The file.** Core `termlog::open`: `policy::guard_write` (read-only and restricted
  refuse), the parent inside `--root`, an existing name that is not a regular file
  (a link, a FIFO, a device) refused, then `append` + `create` with mode `0600` on Unix
  (a new file; an existing one keeps its mode, and the status line says "appending to"
  and warns when others can read it). Windows: the directory's permissions, as Save
  as… (said in the docs).
- **The prompt**: Save as… style, title "Log to file", prefilled
  `xdevcmd-<shell>-<date>-<time>.log` in the shell's directory when the system gave it,
  else the pane's (the history's rule: a directory the shell's output claims is never
  written to). The same refusals as Save as… (root, `..`, control characters, a
  directory, a missing folder). **Tab** in the prompt switches text ↔ raw; the label
  says which.
- **Both walls**: the menu item greyed and the command refused by
  `App::refused(Action::CopyAs)` ("creating files is turned off"); core refuses again.
- **One log per tab**: the menu shows "Log to file…" or "Stop logging", never both; a
  second start on a tab already logging is refused. The log follows the shell that had
  focus in the tab when it started (a split started later is not in it; said in the
  docs).
- **Endings**: Stop logging, the shell exiting (the reader's EOF ends the log), the tab
  closing, and quitting all end it the same way: the sender is dropped, the writer
  drains the queue, writes the footer, flushes, and the UI waits for it at most 2 s
  (a hung disk must not hang quit; past that it says the log may be incomplete). The
  status line names the file and its size.
- **F1**: two rows in Terminal, "Log this tab to a file…" and "Stop logging this tab",
  acting on the active terminal tab; the first refused as the menu item is.
- **Secrets**: a log keeps everything the shell prints. Typed passwords are not echoed
  so not logged, but a token a command prints is. Said in the guide and security.md;
  `0600` is why.

### 3. Steps

1. This kickoff; ROADMAP: step 4 and item 35 → Round 027. **[skip ci]**
2. Core `termlog.rs`: `TextFilter`, `open`, `Log` (writer thread, bounded queue,
   states). Unit tests: escapes stripped, CR overwrite, `CSI K`, split UTF-8 and split
   escapes across chunks, very long lines bounded, alternate screen; `open` (0600,
   append, refused in read-only via `tests/read_only.rs`, a link refused); a sink that
   fails → Failed with the error; a sink that blocks → the queue fills → Failed, the
   reader not blocked past the bound.
3. The app: `TermPane` tap and EOF; `TermPanel` per-tab log, ● / ! on the tab, notes
   polled each turn, endings (stop, exit, close, quit); the tab menu items; the prompt
   with Tab for raw; F1 rows. Pty tests: from the tab menu, `printf` with colours and a
   `\r` progress line, stop → text file as expected; the same in raw; output past the
   scrollback still in the log; closing the tab ends the log; read-only refuses with the
   usual message and writes nothing.
4. Docs: guide (terminal), shell.md, security.md ("never written to disk" → "unless you
   turn on a log for that tab"; the secrets warning), keys.md regenerated if it
   changes, CHANGELOG Unreleased. No new setting.
5. Do/Check: fmt, clippy, tests twice, the pty suite three times; the reader's
   throughput with logging off and on (`yes | head -c 50M`); CI.

### 4. Decisions (recommendations; the coder follows them unless the owner objects)

1. **Tap before vt100, in the reader; filter and write on a writer thread** behind a
   bounded queue (2 MiB). Not a `BufWriter` in the reader: a full disk would stall the
   reader, and with it the screen and ConPTY's cursor answers.
2. **On a full queue, wait at most 500 ms, then stop the log and say so** (Failed, the
   !, the status line, a last line in a text log). Not "drop and carry on": a log with
   a silent hole is worse than one that ends where it says it ends.
3. **text strips; CR, `CSI K`/`G`/`C`/`D` and BS edit the line; a full-screen program is
   one line**; raw is bytes. Default text.
4. **Append, `0600` when created, a non-regular file refused**; an existing file's
   mode is not changed, and a warning is given when it is readable by others.
5. **Mode chosen in the prompt with Tab**, not a setting (no `[terminal]` key in
   0.4.0).
6. **The log belongs to the shell focused when it started**; the tab shows its mark.
7. **Header and footer lines in text mode only.**

### Not doing (this round)

- `[terminal] log = "always"`, a log directory setting, timestamps per line, rotation
  (0.5 at the earliest).
- Logging what is typed (the input side); a password typed at a prompt that echoes it
  is in the output anyway.

## Do

- **2026-11-12, kickoff** (`7fe6470`): this file; ROADMAP step 4 and item 35 → Round 027.
- **Core `termlog.rs`** (`0ed3170`): `TextFilter` (byte-wise state machine: CSI, OSC,
  DCS/SOS/PM/APC, two-byte ESC removed; CR, BS, `CSI K/G/C/D` edit the line, written at
  LF; UTF-8 decoded across reads, invalid → U+FFFD; C1 dropped; lines bounded at
  16 384 chars; the alternate screen is one line; trailing blanks trimmed). `open`
  (`guard_write`, the root, a non-regular file refused before and after opening,
  `append`+`create`, `0600`). `Log`: `sync_channel(256)`, a writer thread with a
  64 KiB `BufWriter` flushed when the queue runs dry; `tap` never holds the lock while
  waiting, waits at most 500 ms for room, then marks Failed and drops the sender;
  a write or flush error marks Failed with the system's words; `close` waits at most a
  given time and says which of closed / stopped / may be incomplete. Text logs get a
  header, a footer and, after an overflow, a last line saying output after it is not
  there; raw gets nothing added. `tests/read_only.rs` covers new and existing files.
- **The app** (`021367b`): `TermPane` holds a log slot shared with the reader, which
  taps each chunk before `parser.process` and calls `finish` at EOF. `TermPanel`:
  `start_log` (refused when the tab already logs or its shell ended), `stop_log`,
  `take_notes` (ended logs closed and said; a failure said once, kept with its mark),
  `close_logs`; `remove`/`close_tab` end a log before the shell goes. The tab bar draws
  `●`, or a bold `!` in `th.err`. Tab menu: Log to file… (greyed by
  `refused(CopyAs)`) or Stop logging. `InputKind::LogTo { tab, raw }`, Tab switches;
  the prompt's checks are Save as…'s, now one function (`typed_destination`) used by
  both. F1: "Log tab to file…" and "Stop logging tab" in Terminal (the first titles
  tried, "Log this tab to a file…", were cut off in the small window). The loop polls
  notes each turn; `reap_terminals` takes them before the panel can go with its last
  shell (found by the pty test: the "log closed" was lost when `exit` closed the last
  shell); `close_logs` after the loop, before saving the session.
  - Tests: render (the menu item, the prompt's title, name and label, Tab → raw); pty
    (text: colours, a `\r` progress line kept once, header/footer, `0600`, nothing after
    Stop; raw: bytes with CR LF; `seq 1 3000` with `scrollback = 100`, every line in
    order; closing a tab ends its log, `exit` ends the other's; quitting flushes;
    read-only: the menu item does nothing, F1's row says "creating files is turned
    off", nothing written).
- **Docs** (`62cfb19`): guide (terminal: the tab menu, Log to file…), shell.md (a new
  section, "Logging a tab to a file"; the history's "never written to disk, unless you
  turn on a log for that tab"), security.md (the safe-modes table; the shell panel: what
  a log keeps, secrets, `0600`), CHANGELOG. `docs/keys.md` regenerated: unchanged (F1's
  menu-only rows are not in it). No setting added; `docs/config.md` untouched.
- **Measurements** (`f1badf6`, two `#[ignore]` tests): `log_throughput` (release, a real
  pty, `yes | head -c 50000000`, 120×40, scrollback 5000), three runs each, seconds
  until the program ended: off 16.5 / 17.6 / 18.8; text 15.4 / 15.9 / 17.5; raw 16.1 /
  16.9 / 16.7 (about 3 MB/s in every case). The reader is bound by vt100, not the log:
  no difference beyond the noise, and the log was closed within 0.1 ms of the program
  ending (nothing queued). `filter_throughput`: the text filter alone, 50 MB of
  coloured build output in 0.34 s (148 MB/s), about 50 times the reader's rate, so the
  queue does not fill on a working disk.

- **Independent review, 2026-11-12, fixes** (one commit each, each test shown to fail
  with its fix reverted):
  - `e5fbf7c` A cursor count of `usize::MAX` (`ESC[…C`) overflowed the column (panic in
    debug, wrap in release): saturating arithmetic for the column and counts. A writer
    thread that panicked left the log Running with `done` unset, so Stop waited 2 s and
    said "may be incomplete": a drop guard now marks it Failed ("the log's writer
    stopped") and sets `done`; the reader's Disconnected branch says the same if nothing
    else did. Tests: the huge counts; a panicking sink in core; in `TermPanel` (a real
    `sh`), said once, the mark Running → Failed, Stop clears it.
  - `7c4bcd2` `open` looked, then opened: a link or a FIFO swapped in between was
    followed, or blocked the UI thread. Now a new file is `create_new` (`O_EXCL`), an
    existing one `append` with `O_NOFOLLOW | O_NONBLOCK | O_NOCTTY` (libc constants, no
    unsafe), then `fstat` must say regular file. `O_NONBLOCK` is left set: it changes
    nothing for a regular file. Windows: `FILE_FLAG_OPEN_REPARSE_POINT`, so a link is
    opened as itself and refused by the check (not tested on Windows). Test: a FIFO
    (nobody reading) refused within 2 s, a link refused, its target untouched.
  - `d4c964a` Appending to a file another user owns is refused (Unix, `uid` ≠ `geteuid`).
    The test needs root to make such a file: it runs fully here (root), and on GitHub's
    runners (not root) checks only that one's own file is accepted.
  - `bdcd54e` Text filter: U+009B is CSI; U+0090/0098/009D/009E/009F start a string,
    ended by U+009C (0xC2 0x9C), ESC \ or BEL; ESC N / ESC O skip the next byte; a tab
    moves the cursor to the next multiple of 8 without writing. Tests split anywhere.
  - Docs (this commit): Stop, closing a tab and quitting can each wait up to 2 s per log
    on a disk that has stopped answering; a crash of xdevcmd itself loses the footer and
    what was still queued (up to about 2 MB); another user's file refused.

## Check

- [x] Text filter: escapes, CR overwrite (`CSI K` too), split UTF-8 and escapes at every
  cut and byte by byte, invalid bytes, long lines bounded, the alternate screen
- [x] Core: 0600, append, a link/directory refused, others-can-read warned; read-only
  refusal (new and existing file); a failing sink → Failed "No space left on device",
  the reader told; a stuck sink → the queue fills, Failed "did not keep up" in well
  under the bound, `close` returns after its wait, the writer finishes once the disk
  answers
- [x] Pty: start from the tab menu, colours and a `\r` line, stop, text and raw; past
  the scrollback; tab close and `exit` end the log; quit flushes; read-only refuses
- [x] fmt --check, clippy `-D warnings` (Linux), `cargo test --workspace` twice green,
  the pty suite three times more (105 passed each)
- [x] Throughput with logging off vs on (above)
- [x] CI on three systems: run 43234324692 green first time (Linux x86_64 and ARM,
  Windows, macOS, cargo-deny, the windows-gnu/darwin cross-check); no flake seen
- [x] Review fixes: huge counts, a dying writer (core and panel), the open race (FIFO,
  link), another user's file, C1/SS2/SS3/tab; fmt, clippy, workspace tests, pty 2×;
  CI: see below
- [ ] **Not verified**: Windows and macOS logging by hand (CI compiles and runs the
  render test there; the pty tests are Unix-only and ran on Linux here, macOS in CI);
  a real full disk (simulated by a failing writer) and a real slow disk (simulated by
  a blocked one); the `!` mark drawn (no test forces a failure in the running app);
  F1's Stop logging row run end to end; clippy for windows-gnu and darwin (targets not
  installed here; CI's cross-check is `cargo check`); a ConPTY-fed raw log replayed.
  The 500 ms wait means a disk stalled that long briefly holds the shell's output
  (and, on Windows, ConPTY's cursor answers) before the log gives up: by design,
  bounded, not measured on Windows.

## Act

**Learnings**:

- ...

**Promotions**:

- [ ] → reference/ui.md : the tab menu's Log to file… / Stop logging, the ● and !
