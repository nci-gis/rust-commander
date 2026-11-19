# How to test

For anyone who wants to see for themselves that a version of xdevcmd does what it says,
on their own machine, before trusting it with real files. About 45 minutes, then a week
of normal use. The test files are in a sandbox folder made for it: never use your only
copy of anything.

You need two terminal windows, about 4 GiB free disk, and no Rust (unless the test kit
for your system is missing from the release: step 1).

## The steps

**1. Download** from [Releases](https://github.com/nci-gis/rust-commander/releases), for
your system: this version's `xdevcmd-…` archive; the previous version's (the release
just below it on that page: for 0.4.0-beta.2, 0.4.0-beta.1), to compare with; and this
version's `xdevcmd-bench-…` (it holds `xtask`, the test tool; `xtask --help`). Unpack
each into a folder of its own. No `xdevcmd-bench-…` for your system? Its build failed
for that release, and the release notes say so. With Rust, build it from the release's
tag: `git clone --branch <tag> https://github.com/nci-gis/rust-commander` then, in it,
`cargo build --release --locked -p xtask`; it is `target/release/xtask`. The builds are not signed: the first start may need
[a click or two](https://github.com/nci-gis/rust-commander/blob/main/docs/troubleshooting.md#macos) ([Windows](https://github.com/nci-gis/rust-commander/blob/main/docs/troubleshooting.md#windows)).

**2. Turn the debug log on** for the week. It holds no file, folder, user or host name.

```bash
echo 'export XDEVCMD_DEBUG=all' >> ~/.zshrc     # macOS (Linux: ~/.bashrc); new windows
```
```powershell
setx XDEVCMD_DEBUG all                         # Windows; new windows
```

**3. Make the sandbox.** In a new window, in a folder of your choice, give the full path
of the `xtask` you unpacked:

```bash
/path/to/xtask try --make
```
```powershell
& 'C:\path\to\xtask.exe' try --make
```

Every later `xtask` line means that same full path, run from this same folder. It makes `xdevcmd-try/` (2.5 GiB): `big.log`, `many/`, `tree/`, `replace-src/`,
`replace-dst/`, `big/` and an empty `copy-to/`. Every command and option: `xtask try --help`.

**4. Five tasks with the previous version.** Start it on the sandbox:
`/path/to/old/xdevcmd xdevcmd-try xdevcmd-try/replace-dst`. Score each task from 1 (no
thought) to 5 (gave up), then quit (F10).

1. F3 on `big.log`; End; `/`, type `INFO`, Enter; Esc.
2. Enter on `many`, then Esc while it is still opening.
3. Enter on `replace-src`, Ctrl+A, F5: read what it would do; Esc.
4. Tab to `replace-dst`; F8 on a file, Enter; Ctrl+Z, Enter: it is back.
5. F1, type `sync`, Enter; Esc.

**5. The same five tasks with this version**, scored the same way:
`/path/to/xdevcmd xdevcmd-try xdevcmd-try/replace-dst`. Then `xtask try --check`.

**In checks A to C, start xdevcmd through `xtask try --launch`.** It keeps that
xdevcmd's process number, and `--kill-when` kills that one and nothing else (`kill -9`,
End task): an xdevcmd you have open elsewhere is safe. `--bin` names the xdevcmd to
try (the one from this release, as in step 5); its arguments go after `--`. It prints
which xdevcmd it started, and its version: check it is this release. When the killed
xdevcmd ends, the window is put back; start it again with the same `--launch` line.

**6. Check A, a replace killed mid-way.** First window:
`xtask try --launch --bin /path/to/xdevcmd -- xdevcmd-try xdevcmd-try/replace-dst`. Second window:
`xtask try --kill-when A`. In xdevcmd: in `replace-src`, Ctrl+A, F5; Space until the
line says `[replace]`; Enter. The second window kills xdevcmd mid-job. Start it again
with the same `--launch` line: it says "ended in the middle of a job"; Enter. It
finishes without asking again.

**7. Check B, a big file killed mid-way.** On macOS and Linux, start xdevcmd with the
sandbox's config, which turns the fast copy off: on APFS, Btrfs or XFS a copy is a
clone, done at once, with no checkpoint to stop at.
`XDEVCMD_CONFIG=xdevcmd-try/no-fast-copy.toml xtask try --launch --bin /path/to/xdevcmd -- xdevcmd-try xdevcmd-try/copy-to`
(on Windows: `xtask try --launch --bin /path/to/xdevcmd -- xdevcmd-try xdevcmd-try/copy-to`). In the second
window: `xtask try --kill-when B`. In xdevcmd: F5 on `big`, Enter. Start again with the
same line: "kept part way"; Enter. The bar starts near where it stopped.

**8. Check C, after a power cut.** First window: the `--launch` line of check B (or
step 7's on Windows). Second window: `xtask try --kill-when C`; in xdevcmd: F5 on
`tree`, Enter. Then `xtask try --tamper` (it spoils one finished copy, as a power cut can). Start it
again with the same `--launch` line and press **C**: "1 found different … copied again".

**9. Check every file**: `xtask try --check`. The last line must be
**PASS: every copy whole, no file lost or wrong**. Then `xtask try --clean`.

**10. Use it for a week** for your normal file work, where you would use Explorer,
Finder, mc or Total Commander: copies and moves onto folders that share some names, F8
and Ctrl+Z, big logs, the shell (F12, Alt+C), compare and sync, find (Alt+F7), a USB
stick or a share if you have one. F1 finds every command by typing.

**11. Send what you found**, as a comment on the release page, even if nothing went
wrong:

```text
version:      xdevcmd --version
system/shell: e.g. Windows 11, pwsh 7.4, Windows Terminal
lost data:    no (if yes: what you did, and is it in the trash?)
friction:     the five scores, previous version / this one
check:        the output of the last `xtask try --check`
refusals:     any time xdevcmd refused something it should have done
attached:     the debug logs (and bench/results/*.json if you ran the benchmark)
```

## Good to know

- **A lost or wrong file** (gone, short, empty, or not the one you chose), at any step
  or in the week: stop, and report it the same day as **P0**. Before starting xdevcmd
  again, copy the debug log and the state folder
  ([how](https://github.com/nci-gis/rust-commander/blob/main/docs/troubleshooting.md#sending-a-log-with-a-report)); keep the trash.
- **Each problem as its own [issue](https://github.com/nci-gis/rust-commander/issues)**,
  with a level in the title: **P0** data lost or the wrong file touched; **P1** a crash,
  a hang, the terminal left broken; **P2** a wrong screen or doc; **P3** cosmetic. Say
  the version, system, terminal and shell, what you did and what you saw. Describe file
  names you would not share in public ("a folder of 3,000 small files on a share").
- **On a Mac**: F-keys need Fn (or "Use F1, F2, etc. keys as standard function keys" in
  System Settings → Keyboard); End is Fn+→; for Alt+ keys, Terminal → Settings →
  Profiles → Keyboard → "Use Option as Meta key".
- **After a kill**, `--launch` puts the window back. If xdevcmd was started another way,
  on macOS and Linux the window may print odd characters: type `reset`, Enter.
- **To do a check again**: `xtask try --reset A` (or `B`, `C`) puts back the files that
  check's copy changed, in seconds, with no new `--make`; then do its steps again. If
  xdevcmd still offers the job at its start, press **F** (forget it) first.
- **If `--kill-when` says the copy finished first**, the disk is fast and the sandbox
  small: `--reset` that check and try again; if it keeps finishing first, `--clean`,
  then `--make` without `--small`.
- **`--small`** (0.6 GiB, about 1.1 GiB with the copies; 2.5 and 3.8 GiB without it) is
  for a slow disk or little space.
- **`--clean`** removes the whole sandbox, the copies in it included, and refuses only
  when its top holds something `--make` did not make. Keep nothing of your own inside
  it. What the checks sent to the trash stays there.
- **The speed** on your own drives, unattended, any day: [Benchmarks](benchmark.md).
- **With Rust 1.90+**: the automated tests at this version's tag,
  `cargo test --workspace --locked --no-fail-fast` ([Development](https://github.com/nci-gis/rust-commander/blob/main/docs/development.md)).
