# How to test

For anyone who wants to see for themselves that a version of xdevcmd does what it says,
on their own machine, before trusting it with real files. About 45 minutes, then a week
of normal use. Everything happens in a sandbox folder made for it: never use your only
copy of anything.

You need two terminal windows, about 4 GiB free disk, and no Rust.

## The steps

**1. Download** from [Releases](https://github.com/nci-gis/rust-commander/releases), for
your system: this version's `xdevcmd-…` archive; the previous version's (the release
just below it on that page: for 0.4.0-beta.2, 0.4.0-beta.1), to compare with; and this
version's `xdevcmd-bench-…` (it holds `xtask`, the test tool; `xtask --help`). Unpack
each into a folder of its own. The builds are not signed: the first start may need
[a click or two](troubleshooting.md#macos) ([Windows](troubleshooting.md#windows)).

**2. Turn the debug log on** for the week. It holds no file, folder, user or host name.

```bash
echo 'export XDEVCMD_DEBUG=all' >> ~/.zshrc     # macOS (Linux: ~/.bashrc); new windows
```
```powershell
setx XDEVCMD_DEBUG all                         # Windows; new windows
```

**3. Make the sandbox.** In a new window, in a folder of your choice; run every later
`xtask` command from this same folder:

```bash
/path/to/xtask try --make            # Windows: C:\path\to\xtask.exe try --make
```

It makes `xdevcmd-try/` (2.5 GiB): `big.log`, `many/`, `tree/`, `replace-src/`,
`replace-dst/`, `big/` and an empty `copy-to/`.

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

**6. Check A, a replace killed mid-way.** In the second window:
`xtask try --kill-when A`. In xdevcmd: in `replace-src`, Ctrl+A, F5; Space until the
line says `[replace]`; Enter. The second window kills xdevcmd mid-job. Start xdevcmd
again (same line as step 5): it says "ended in the middle of a job"; Enter. It finishes
without asking again.

**7. Check B, a big file killed mid-way.** Start xdevcmd on
`xdevcmd-try xdevcmd-try/copy-to`. In the second window: `xtask try --kill-when B`. In
xdevcmd: F5 on `big`, Enter. Start again: "kept part way"; Enter. The bar starts near
where it stopped.

**8. Check C, after a power cut.** `xtask try --kill-when C`; in xdevcmd: F5 on `tree`,
Enter. Then `xtask try --tamper` (it spoils one finished copy, as a power cut can). Start
again and press **C**: "1 found different … copied again".

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
  ([how](troubleshooting.md#sending-a-log-with-a-report)); keep the trash.
- **Each problem as its own [issue](https://github.com/nci-gis/rust-commander/issues)**,
  with a level in the title: **P0** data lost or the wrong file touched; **P1** a crash,
  a hang, the terminal left broken; **P2** a wrong screen or doc; **P3** cosmetic. Say
  the version, system, terminal and shell, what you did and what you saw. Describe file
  names you would not share in public ("a folder of 3,000 small files on a share").
- **On a Mac**: F-keys need Fn (or "Use F1, F2, etc. keys as standard function keys" in
  System Settings → Keyboard); End is Fn+→; for Alt+ keys, Terminal → Settings →
  Profiles → Keyboard → "Use Option as Meta key". For check B start xdevcmd with
  `XDEVCMD_CONFIG=xdevcmd-try/no-fast-copy.toml` in front: on APFS a copy is a clone,
  done at once (the same on Linux with Btrfs or XFS).
- **After a kill**, on macOS and Linux the terminal may look wrong: type `reset`.
- **If `--kill-when` says the copy finished first**, the disk is fast and the sandbox
  small: `--clean`, `--make` (without `--small`), and do that check again.
- **`--small`** (0.8 GiB) is for a slow disk or little space.
- **`--clean`** removes only what `--make` made, and refuses while anything else is in
  the sandbox. What the checks sent to the trash stays there.
- **The speed** on your own drives, unattended, any day: [Benchmarks](benchmark.md).
- **With Rust 1.90+**: the automated tests at this version's tag,
  `cargo test --workspace --locked --no-fail-fast` ([Development](development.md)).
