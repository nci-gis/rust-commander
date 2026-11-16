# Development

## Build and run

```bash
cargo build --release                 # target/release/xdevcmd
cargo run -p xdevcmd -- /etc /var     # start with two directories
cargo install --path crates/xdevcmd --locked   # install your build as `xdevcmd`
```

Needs Rust 1.90 or newer (`rustup`). Nothing needs root.

## Check before you push

```bash
cargo test                                         # about 400 tests, ~30 s
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --target x86_64-pc-windows-gnu -- -D warnings
cargo fmt --check
cargo deny check                                   # vulnerabilities and licences
```

`rustup target add x86_64-pc-windows-gnu x86_64-apple-darwin` once, to check the other
platforms from Linux. CI runs the full tests on Linux, Windows and macOS.

## See the screen without a terminal

```bash
./target/release/xdevcmd --screenshot=118x34 --keys=F9 /etc /usr
```

Draws one frame as text, after typing the keys. Most UI tests work this way.

## Memory over time

```bash
XDEVCMD_SOAK_SECS=600 cargo test --release --test pty -- --ignored soak --nocapture
```

Uses xdevcmd heavily for ten minutes and fails if memory keeps growing. Linux only.
Samples go to `target/tmp/soak-rss.csv`.

## Benchmarks

`cargo xtask bench` measures xdevcmd the same way every time and keeps the numbers, so a
change can be compared with what came before instead of guessed at:

```bash
cargo xtask bench --quick --host my-laptop        # a minute or two
cargo xtask bench --host my-laptop                # the full set, before a release
cargo xtask bench --host my-laptop --save-baseline
```

It builds the release binary and runs each case five times (`--runs N`) after a warm-up,
printing the median, p95, best and peak memory, and the change from this host's
baseline, marked past ±15 %:

- the package: the binary's size, the crates in `Cargo.lock`;
- listing and sorting 10,000 and 100,000 entries;
- the copy planner on 10,000, 100,000 and 200,000 files;
- copying many small files and one large file; replacing small files (each synced
  before its rename, the old one to a trash of the run's own); undoing a copy;
- start-up to the first frame and the terminal check, in a terminal that answers and one
  that does not (from `--timings`), and the CPU used while idle (Linux).

Each case runs in a child process on trees made from a fixed recipe in a temporary
directory, never your files. Results go to `bench/` (its README says which file is
which); `--host` names the machine in them, and nothing is read from the system to name
it. Two machines' numbers are not comparable, nor a busy machine's (the load is printed
first). CI does not compare times, which vary too much on shared runners: it keeps hard
budgets only (`tests/budgets.rs`), and a tripwire for start-up in `tests/pty.rs`
(everything but the terminal check under 1 s, a debug build: an order of magnitude over
the 150 ms budget, which is for `cargo xtask bench` on a release build).

**On another machine or drive.** `--only CASE` runs just the cases whose name holds
`CASE`, with no build and no start-up cases, so a copy of `xtask` runs on a machine
without Rust: a release's `xdevcmd-bench-<tag>-<target>`, or, for code not released
yet, Actions → bench → Run workflow (or a push to a `bench/<name>` branch) and the run's
artifacts `xtask-Linux`, `xtask-macOS`, `xtask-Windows` (bare binaries: `chmod +x`;
kept 14 days). `--dir PATH` makes the trees on the
drive being measured instead of the temporary directory. Away from the source, results
go to `bench/results/` where it was started; with `XDEVCMD_DEBUG`, each run's jobs are
logged in `bench/results/debug-<host>-<date>/<case>/`. `XDEVCMD_BENCH_FAST=0` turns the
platform's fast copy off, to measure xdevcmd's own loop where the fast copy is a clone.

**Two versions on one machine.** Each version's `xtask` in its own folder, the same
names for both. `--save-baseline` writes `bench/baseline-<host>.json` whole, so one
`--host` per case:

```bash
cd old        # the older version's xtask
./xtask bench --only plan  --host mine-plan  --save-baseline
./xtask bench --only list  --host mine-list  --save-baseline
./xtask bench --only mixed --host mine-mixed --save-baseline
mkdir -p ../new/bench && cp bench/baseline-*.json ../new/bench/
cd ../new     # each line now says how far it moved (◀ past 15 %)
./xtask bench --only plan  --host mine-plan
./xtask bench --only list  --host mine-list
./xtask bench --only mixed --host mine-mixed
```

Then the other way round (the newer as the baseline) before believing a small change:
a busy minute shows as one.

**The tester's sandbox.** `xtask try --make | --kill-when A|B|C | --check | --tamper |
--clean` (in the same archive) makes the folder that [How to test](how-to-test.md)
uses, with a SHA-256 manifest that counts its own lines (a cut one is an error, never a
PASS); kills xdevcmd by name when a check's copy reaches its point (a tenth replaced,
past the first checkpoint, a fifth of the tree); checks every copy; `--clean` removes
only what `--make` made, in a folder named `xdevcmd-try` holding its marker, the marker
last. It never runs or looks for xdevcmd otherwise.

How a tester measures on macOS, Windows and Linux: [Benchmarks](benchmark.md).

## Fuzzing

The parsers of input xdevcmd does not control are fuzzed by hand before a release
(the release bar): what a shell prints (OSC 7, 9;9, 133), paths in a line of output,
the config and theme files, names inside an archive, and archives themselves. The
targets are in `fuzz/` (a crate of its own, outside the workspace: it needs nightly);
the app's parsers are compiled there from their own source files.

```bash
rustup toolchain install nightly       # once
cargo install cargo-fuzz               # once
cargo xtask fuzz                       # every target, 60 s each
cargo xtask fuzz --secs 600 paths osc  # longer, chosen targets
```

A crash stops its target and is kept in `fuzz/artifacts/<target>/`; the summary names
it. Fix it with a regression test that fails without the fix (as `found.rs`'s
`a_quoted_name_with_a_tab_is_not_taken`), then `cargo +nightly fuzz run <target>
<file>` must pass. Each target also checks a property, not only "no panic": a path's
span lies in its line, an archive name never climbs out of its folder. The corpus and
artifacts stay local (`fuzz/.gitignore`). Not run in CI.

## Reading a debug log

`XDEVCMD_DEBUG` ([Troubleshooting](troubleshooting.md#sending-a-log-with-a-report);
`xdevcmd-core/src/debuglog.rs`) writes one JSON object a line, each with `t_ms` (since
the log started) and `ev`:

| `ev` | Topic | Fields |
|---|---|---|
| `start` | — | `version`, `os`, `arch`, `topics`, `unix_time`, `cpus` |
| `job_start` | jobs | `job`, `kind`, `sources`, `files`, `bytes`, `existing`, `planned`, `safe`, `fast`, `group_threads`, `src_fs`, `dst_fs`, `dst_network` |
| `resume` | jobs | `job`, `sources_left`, `decisions_kept` |
| `batch` | jobs | group commit: `files`, `bytes`, `threads`, `sync_ms`, `sync_failed`, `probing` |
| `probe` | jobs | `chose`, and `ms_per_file_<threads>` for each count tried |
| `job_end` | jobs | `ms`, `scan_ms`, `journal_ms`, the counts (`done`, `skipped`, `failed`, `cloned`, `by_kernel` …); the time per phase (`check_ms`, `open_ms`, `create_ms`, `write_ms`, `attrs_ms`, `sync_ms`, `finish_ms`); files up to 1 MiB and larger apart (`small_files`, `small_bytes`, `small_ms`, `large_…`); big files' checkpoints (`checkpoints`, `checkpoint_ms`) and bytes taken over from one on a resume (`resumed_bytes`); folders told to the journal as finished (`dirs_done`) and folders a resume skipped (`dirs_skipped`) |
| `retry` | errors | `file`, `phase`, `kind`, `os`, `attempt`, `of`, `wait_ms` |
| `failed` | errors | a file that failed for good: `file`, `phase`, `kind`, `os`, `tries`, `network` |
| `ask` | errors | a question: `q`, `phase`, `answer`, `waited_ms`, `file` |
| `error` | errors | an error counted in the summary: `file`, `phase`, `kind`, `os` |
| `journal_failed` | errors | the journal could not be written: `phase` |

`file` is a number for a path, the same within one run (a keyed hash; the key is never
written). `kind` is Rust's `io::ErrorKind`, `os` the system's code (`errno`, or Windows'
`GetLastError`). Nothing in a line is a name: words are kept to letters, digits and
`_ . -`.

Where a job's time went: the phases add up to the time spent on files; `ms` minus them
is the walk, directories, the journal and waiting for answers. `small_ms` against
`large_ms` is what item 43's lanes would overlap.

## Docs that are generated

- `docs/keys.md`: `xdevcmd --print-keys > docs/keys.md`. A test fails if it is out of date.
- The tagline in `README.md` is the `description` in `Cargo.toml`. A test checks it.
- The GitHub About box: `scripts/sync-about.sh --apply`.

## Icon and logo

The SVGs in `assets/` are the source: `icon.svg`, `icon-16.svg` (the same picture
redrawn on whole pixels, because a scaled-down one blurs at 16 px), and the README's
`logo-light.svg` / `logo-dark.svg`, whose wordmark is drawn as strokes so it looks the
same without any font. After changing an icon SVG, rebuild
`crates/xdevcmd/assets/xdevcmd.ico` and `crates/xdevcmd/assets/about-logo.png` (inside
the crate because the build reads them and `cargo package` ships only the crate; the
second is the logo About draws, built into the binary, drawn by hand
one colour per character cell by `scripts/logo_pixels.py`, because half blocks depend
on the terminal's font):

```bash
cargo install --locked resvg   # once
scripts/icons.sh
```

`scripts/icons.sh` also needs `python3`. `scripts/logo_pixels.py` redraws icon.svg's
shapes on whole pixels by hand, so change it too when the picture changes.

On Windows, `build.rs` puts the icon and the version details into `xdevcmd.exe`. It needs
the Windows SDK's `rc.exe` (or MinGW's `windres`); without one it warns and builds
without the icon, unless `XDEVCMD_REQUIRE_ICON=1`, which CI and releases set.

## Branches

- **`main`** is what people install. It changes only by a pull request from `dev`,
  merged with a merge commit once CI is green; a ruleset on GitHub enforces both.
- **`dev`** is where finished work collects. Every push to it runs CI, except one that
  touches only `.agents/` or `CHANGELOG.md`, which no test reads.
- **`feat/<topic>`** holds anything bigger than a commit or two, such as a round of the
  plan. Every push to it runs CI too (except `.agents/` or `CHANGELOG.md` only), so a
  broken build shows before the pull request. When it is ready, open a pull request
  into `dev`: CI runs on it, then merge.

A build to try on another system, from any branch: **Actions → ci → Run workflow**
and pick the branch, or

```bash
gh workflow run ci --ref feat/topic
```

The binaries are under that run's **Artifacts** (the macOS one runs on Intel and Apple
Silicon). A draft pull request into `dev` gets the same builds.

## Releasing

Write the notes as you go: every change that a user would notice gets a line under the
newest heading of `CHANGELOG.md`, in the same commit. A release then takes six steps.
`cargo release` needs installing once: `cargo install --locked cargo-release`.
Both crates go to crates.io (`xdevcmd-core` first, then `xdevcmd`, which pins it with
`=x.y.z`; cargo-release rewrites the pin), by hand in step 5 only. `release.toml` has
`publish = false`, so no `cargo release` publishes, even one that forgets
`--no-publish` (the commands below still say it).

1. **On `dev`**, date the notes and commit (add `minor`, `patch` or a version number to
   also raise the version; leave it out to release the version `Cargo.toml` has):

   ```bash
   cargo release minor --no-tag --no-push --no-publish          # look at what it would do
   cargo release minor --no-tag --no-push --no-publish --execute
   git push origin dev
   ```

2. **Pull request `dev` → `main`**, merged with a merge commit once CI passes.
3. **On `main`**, tag the release and push the tag:

   ```bash
   git checkout main && git pull --ff-only
   cargo release tag --execute
   git push origin v0.3.0            # the tag cargo release made: v<version>
   ```

4. **The `release` workflow** first checks that the tag, the version in `Cargo.toml` and a
   dated CHANGELOG section agree and that the tag is on `main`
   (`scripts/release-check.sh`), waits for that commit's `ci` run to pass
   (`scripts/ci-green.sh`), then builds Linux (x86_64 glibc and static musl, ARM64
   static musl), macOS (ARM and Intel) and Windows, smoke-tests each archive as a
   user gets it (`scripts/smoke.sh`: unpacked, its files, at most 7 MiB, run where the
   runner's CPU can, its version the tag's), attests their build provenance, and opens
   a **draft** release with
   the archives (each holding `LICENSE`, `README.md`, `THIRD-PARTY.md`, the example
   `themes/` and this version's `docs/`), `SHA256SUMS`, and that CHANGELOG section as its
   notes. Read it, then Publish.
5. **crates.io**, by the owner, from `main` at the tag (needs `cargo login` once, with a
   crates.io token kept on the owner's machine, never in a workflow). Check first that
   `cargo package --workspace --exclude xtask` passes (CI does it on Linux):

   ```bash
   cargo publish -p xdevcmd-core --locked --dry-run   # look
   cargo publish -p xdevcmd-core --locked
   cargo publish -p xdevcmd --locked --dry-run        # once the index has the core
   cargo publish -p xdevcmd --locked
   ```

   A version on crates.io cannot be replaced, only yanked; publish after the GitHub
   release is good, not before.
6. **Scoop**, from the published release's `SHA256SUMS`: see
   [packaging/README.md](../packaging/README.md).

## Pre-releases

A beta or release candidate (`0.4.0-beta.1`, `0.4.0-rc.1`) goes to **GitHub only**,
marked pre-release, never "Latest"; crates.io, Scoop and binstall wait for the
release itself (a version on crates.io can never be deleted, and package managers have
no beta channel worth keeping). It is cut from `dev`, without a pull request into
`main`. Semver orders them `0.4.0-beta.1 < 0.4.0-beta.2 < 0.4.0-rc.1 < 0.4.0`.

1. **Try the rules first** (nothing is built): Actions → release → Run workflow, with
   the tag to be (or `gh workflow run release --ref dev -f tag=v0.4.0-beta.1`). Before
   step 2 it fails on the version and the CHANGELOG, as it should; the branch rule is
   what this tries. Locally: `scripts/release-check.sh v0.4.0-beta.1 origin/dev`.
2. **On `dev`**, the version and the dated notes, never published:

   ```bash
   cargo release 0.4.0-beta.1 --no-tag --no-push --no-publish            # look
   cargo release 0.4.0-beta.1 --no-tag --no-push --no-publish --execute
   git push origin dev
   ```

   The CHANGELOG heading becomes `## [0.4.0-beta.1] - <date>`, holding everything
   since the last release; the next pre-release gets its own short section. At the
   release (0.4.0), its section says everything since the last release again, in full,
   so each release page reads on its own.
3. **Tag `dev`** and push the tag:

   ```bash
   cargo release tag --execute
   git push origin v0.4.0-beta.1
   ```

4. **The `release` workflow** checks the tag (`scripts/release-check.sh`: a pre-release
   on `dev` or `main`), waits for the `ci` run of the tag's commit (the one the push to
   `dev` started) and stops unless it passed (`scripts/ci-green.sh`), then builds the
   six archives and opens a **draft marked pre-release**. Read it, then Publish. Help →
   Documentation in that build links the tag's `docs/`.
5. **Not done for a pre-release**: crates.io (step 5 of "Releasing") and Scoop (step 6). `release.toml` (`publish = false`) keeps every `cargo release` off crates.io.

A bad pre-release tag is deleted like any other (`git push origin :v0.4.0-beta.1`,
`git tag -d v0.4.0-beta.1`, and the draft deleted on GitHub) before it is published.
A published pre-release is not deleted: the next one supersedes it.

CI runs on pushes to `dev`, `main` and `feat/**` and on pull requests into `dev` and
`main`, and builds once on the `rust-version` in Cargo.toml (the `msrv` job); the `release`
workflow runs only for a `v*` tag (a release on `main`, a pre-release on `dev` or
`main`), or by hand to check a tag's rules. A change to the packaging
(`release.yml`, `about.toml`, `about.hbs`) is therefore first exercised by the next
tag. If that run fails, nothing is released: fix it on `dev`, merge, delete the tag
(`git push origin :v0.2.0` and `git tag -d v0.2.0`) and tag `main` again.

## Testing a pre-release by hand

Before a pre-release is called good, on real hardware the CI cannot reach, besides
the tester's steps in [How to test](how-to-test.md). "✗" marks a
bug. Most of what is not here runs in CI on every system (render.rs, the engine's
tests) or, on Linux and macOS, in a real terminal (pty.rs).

### 1. Data survives what goes wrong

1. **Kill a copy that replaces files.** Two folders with the same file names, the
   destination's files older. F5, choose **replace** for the older ones, and while it
   runs, `kill -9` xdevcmd (Task Manager → End task on Windows). Start it again.
   - You see "The last run of xdevcmd ended in the middle of a job" and, if it was
     between the trash and the rename, "replacement finished … the old one is in the
     trash". Continue finishes the job without asking again what you already chose.
   - ✗ a file that is empty, short, or missing under its own name; an old file gone
     without being in the trash.
2. **Pull a USB stick mid-copy** (FAT32, then exFAT). Copy a folder of a few hundred
   MB to it, pull it out halfway, put it back, start xdevcmd.
   - The job reports an error, not success. Files that were complete are whole;
     the one being written is not there under its real name.
   - ✗ the job says done; a file under its real name with the wrong size.
3. **A big copy, killed past a checkpoint.** Copy a file of 1 GB or more, kill
   xdevcmd (`kill -9`, or close the terminal window) once the bar is past 30 %. Start
   again. Copy to another drive, or with `[general] fast_copy = false`: on the same
   APFS, Btrfs or XFS volume the copy is a clone, instant, with no checkpoint.
   - The dialog says "kept part way"; Continue's bar starts near where it stopped, and
     the copy is the same as the source (`cmp`; on Windows `fc.exe /b`).
   - ✗ the bar starts at 0 %, or the copy differs.
4. **A checked continue (C).** As in 3 but with a folder of small files: kill
   xdevcmd mid-way, then overwrite one finished copy with zeros of the same size and
   give it the source's date (`truncate -s0 f && truncate -s <size> f && touch -r src f`).
   Start again and press **C**.
   - The summary says "1 found different … copied again"; the file equals its source.
   - ✗ the zeroed file is kept, or Enter (not C) reads every file.
5. **Move across drives, killed.** Move a folder from your disk to a USB stick and kill
   xdevcmd mid-way. Start again.
   - Every file is on one side or the other, or on both ("copied completely … both are
     kept"). Never on neither.
6. **A network share pulled mid-copy** (SMB or NFS: unplug the cable or stop the
   server; sshfs: kill the ssh connection). Copy a large file onto it.
   - The job box shows "network error — trying again in 1 s (1 of 5)", counting down.
     Bring the share back within about 30 s: the copy finishes by itself. Leave it
     down: after the fifth try, the Failed dialog (R retry, S skip, Esc abort), saying it tried 6 times. Esc
     during a wait stops at once.
   - ✗ the job fails at once without trying again; it never asks; Esc waits for the
     countdown.
7. **A hard NFS mount whose server is gone**: the job may hang inside the system;
   Esc stops it once that call returns. Note how long it took.

### 2. Undo, and the trash

1. **Undo a copy.** F5 a folder of a few files, then Ctrl+Z, Enter.
   - It asks "Undo the copy of …?" and lists what it will remove. After it, the copies
     are gone and the originals untouched.
2. **Undo keeps what you changed.** F5 a folder, edit one of the copies, drop a new
   file into the copied folder, then Ctrl+Z.
   - The edited copy and the new file stay, and their folder too; the summary says why.
3. **A replaced file comes back.** Copy over an older file (choose replace), Ctrl+Z.
   - The new file is gone and the old one is back, from the trash. The table before the
     copy said "ℹ a file replaced goes to the trash: Ctrl+Z brings it back".
   - On Linux: any disk, a USB stick too. On macOS: a local disk and a USB stick. On
     Windows: the system drive (C:) only.
4. **Where there is no trash**: on macOS and Windows a network share, and on Windows
   any drive but the system drive (Linux makes a `.Trash-<uid>` on any writable mount). The table marks a replace ⚠ and its default is skip.
   - ✗ a replace there that claims to go to the trash.
5. **F8 on a Windows USB stick.** Delete a file with F8 (to the Recycle Bin).
   - Note what Windows does: asks to delete for good, or deletes. Either is to be
     written down; a silent delete for good would be a bug to report.

### 3. Before a copy: the plan

1. **The table.** Copy a folder onto one that has some of the same files, newer and
   older and of another size.
   - One line per group, worst first; ↑↓ moves between groups, Space changes skip /
     replace / ask each; the summary line counts what is created, replaced and skipped,
     and the space needed.
   - Nothing to decide (all new): the plain F5 dialog, as before.
2. **Enter before the check ends** (a big folder, a slow disk): the copy starts and asks
   about each existing file as it meets it. **Tab** checks again for the other pane.
3. **A USB stick in FAT32**: a file of 4 GiB or more is ⛔ "left out: 4 GiB or more,
   onto a FAT drive"; a name with `:` or `?` is ⛔ "a name the destination cannot
   hold"; on exFAT the 4 GiB file is fine.
4. **A case-insensitive drive** (macOS's default disk, Windows, a FAT stick):
   `Readme.txt` copied where `README.txt` is: listed as differing only in case.
5. **Free space**: a copy bigger than the free space is ⛔ "it needs …, and only … is
   free there". (Windows says nothing here yet: free space is unknown there.)
6. **A big tree** (100,000 files or more): the dialog opens at once, the count grows,
   and the memory in Help → Usage stays reasonable.

### 4. While it runs

1. **The time left**: a copy of a few GB to a USB stick. After a few seconds the job box
   says "about N min left"; it should be roughly right, not swing wildly.
   - ✗ a number from the first second; a number while nothing moves.
2. **Lists follow a move**: Alt+F7 a search, F6 one of the results elsewhere. The row
   stays in the list, under its new place.

### 5. Start-up and the screen

1. **Start-up time**: `xdevcmd --timings` (or Help → Usage). Note the total and the
   "terminal check" step on each terminal you use: on a local terminal the whole start
   should be well under 150 ms. Over SSH, or in a terminal that does not answer, the
   terminal check is the slow step.
2. **The look**: the table, the undo question, the job box with its countdown and time
   left, in Windows Terminal (cmd, PowerShell), Terminal.app, iTerm2, GNOME Terminal,
   kitty, tmux and over SSH. Vietnamese and CJK names in each.

### 6. Each system and terminal

What the automated tests run only on Linux, or only as a screenshot.

1. **xdevcmd inside its own panel** (Windows, cmd and PowerShell): `xdevcmd` typed in
   the panel asks first, Enter ends it; `start cmd` from the panel, then `xdevcmd`, asks
   and `y` starts it; a terminal opened from the Start menu does not ask. The menu bar
   shows the process number.
   - ✗ a second xdevcmd starts inside the first without asking.
2. **Paths in the output** ([Shell](shell.md#paths-in-the-output)): in bash with the
   prompt-mark snippet and without it, and in Windows cmd and PowerShell (a build's
   MSBuild errors, `C:\` paths). Shift+F12, Tab to a path, Enter: the pane goes there.
3. **A list from the output** ([Shell](shell.md#a-list-from-the-output)): `rg -l` and
   `git status --porcelain` in bash; `dir /b /s` in cmd; `Get-ChildItem -Name -Recurse`
   in PowerShell; then `p`. On macOS: Terminal.app with and without Option as Meta, and
   iTerm2: Alt+J, Alt+L, and F9 → View → Last list.
4. **A slow mount**: sshfs with its server stopped, or an SMB share with the network
   off, on Linux and Windows. Go into it, then Esc.
   - ✗ the screen stops answering, or Esc does nothing.
5. **Japanese names**: a folder of them on Windows, and the same folder copied from a
   Mac (macOS writes some names decomposed). Both sort the same and copy back intact.
6. **F1 Commands** in macOS Terminal.app, Windows Terminal and a Linux terminal: it
   opens, finds by typing, runs with Enter; Alt+1…9 run its rows only where Option is
   Meta (Terminal.app: off by default).
7. **The light theme** in Terminal.app (256 colours, a light profile) and a light Linux
   terminal: readable, the focused frame clear. With 16 colours, on a real light palette,
   the two cursor rows are the same grey and marked, archive and error files are all
   red (a known limit; the `ansi` theme suits 16 colours better).
8. **Syntax colours in the viewer** (F3 on source code) on a real terminal, with 256 and
   16 colours, on Windows and macOS. A jump (End, a search) draws plain first, then in
   colour.
9. **Logging a tab** ([Shell](shell.md#logging-a-tab-to-a-file)) on Windows and macOS:
   right-click a terminal tab → Log to file…, run a command, Stop logging (also from
   F1). The tab shows ● while logged. On a full USB stick the tab shows `!` and the
   shell goes on.
10. **Help → Documentation** in a build from a release archive: it names the `docs/`
    beside the program and the link at this version's tag. The link opens on GitHub;
    copying it works in a real terminal (it goes through the terminal, OSC 52), and
    through a symlinked program (as Homebrew installs it).

## How the code is organised

Start with [.agents/context/project.md](../.agents/context/project.md): the rules,
the layout and the conventions. Topic notes are in [.agents/reference/](../.agents/reference/).
