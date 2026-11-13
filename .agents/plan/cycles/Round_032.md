# Round 032: Pre-release plumbing and 0.4.0-beta.1 (v0.4.0-beta.1, part 4 of 4)

**Status**: Do (started 2026-11-13; the owner: "close r30 trước, xong go as
recommended")
**Part of**: v0.4.0, step 6 of `ROADMAP.md`; Decisions "Release path" and "Where
pre-releases go" (owner, 2026-11-13: GitHub only, marked pre-release, never "Latest";
crates.io, Scoop, winget and binstall start at 0.4.0); the benchmark harness (owner,
2026-11-13: "Thêm harness benchmark vào Round 032"), so that beta.1's numbers, which set
beta.2's scope (item 43), are measured the same way each time
**Date started**: 2026-11-13
**Date completed**: YYYY-MM-DD

## Goal

A `v0.4.0-beta.1` tag on `dev` builds the six archives and opens a draft GitHub release
marked pre-release; nothing goes to crates.io, Scoop or winget. Before the cut, one
command measures xdevcmd the same way every time and keeps the numbers, so each later
feature is compared with a baseline instead of a guess. Then beta.1 is cut, its baseline
recorded, and the owner uses it for about a week.

## Kickoff review (2026-11-13, after Round 031 closed)

### 1. What changed since the outline

- Round 031 is Complete; Round 030 is still at Review (its hand tests not run). Both
  rounds' "Not verified" lists are the core of the beta.1 checklist (step 3).
- Added to this round by the owner: **the benchmark harness** (step 4). Round 031 gave
  it its first input: `--timings`' JSON and the Usage view's start-up line.
- Measured in Round 031, for decision 4: every start-up step but one takes 0.2–1.6 ms
  (about 4 ms); the terminal check takes up to **1.7 s** when the terminal does not
  answer (its 1.5 s limit and 0.2 s of quiet).
- `[Unreleased]` in CHANGELOG.md holds everything since 0.3.0 (rounds 018–031), about
  210 lines.

### 2. The code this round touches

| Where | What is there today | What this round does |
|---|---|---|
| `.github/workflows/release.yml` `check` | the tag = Cargo.toml's version, a dated CHANGELOG section (grep with unescaped dots), the tag on `origin/main` | a pre-release tag (`-beta.N`, `-rc.N`) on `origin/dev` or main; a stable tag on main only; dots escaped; a `workflow_dispatch` that runs only `check` |
| `release.yml` `release` | `gh release create … --draft` | `--prerelease --latest=false` for a pre-release tag |
| `release.toml` | `allow-branch = ["dev", "main"]`, `publish = true`, tag `v{{version}}` | unchanged; every pre-release command says `--no-publish` |
| `crates/xdevcmd/Cargo.toml` | the dev-dependency pins `xdevcmd-core = "=0.3.0"` | rewritten by cargo-release to `=0.4.0-beta.1` (it did so for 0.3.0) |
| `crates/xdevcmd/src/docs.rs` | Help → Documentation links `tree/v{version}/docs`, tested for `0.4.0-rc.1` | nothing: the tag will exist |
| `docs/development.md` "Releasing" | six steps for a stable release | a "Pre-releases" section |
| `crates/xdevcmd/src/main.rs:422`, `graphics.rs` `probe` | asked before the first frame, up to 1.5 s + 0.2 s | decision 4 |
| (new) `xtask/` | — | the harness (step 4) |
| `core/ops.rs` `measure_a_move_across_devices…` | an ignored test, run in CI only to print | absorbed by the harness; CI keeps printing it |
| `xdevcmd/tests/budgets.rs` | binary ≤ 7 MB (CI), 100,000 entries < 1 s (ignored) | a start-up budget without the terminal check, from `--timings` |
| `xdevcmd/src/usage.rs` `read` | RSS from `/proc` (Linux), `ps` (macOS), none on Windows | the harness reads a child's peak RSS the same ways |

### 3. Outside check

- **Pre-releases on GitHub**: `gh release create --prerelease` marks one; `--latest=false`
  keeps the "Latest" badge on 0.3.0 (to be confirmed on the runner's gh in the dry run). cargo-release takes an explicit
  version (`cargo release 0.4.0-beta.1`) and semver orders `beta.1 < beta.2 < rc.1 <
  0.4.0`, as the ROADMAP says.
- **Benchmarks for a CLI/TUI**: hyperfine's practice (warm-up runs, several runs, median
  and spread, the machine's state printed) is the model for timing a whole program;
  ripgrep and fd keep their benchmark scripts in the repository and their numbers in
  the README per machine, not as CI gates. That matches the outline's "not a CI gate".
  No hyperfine dependency: it is not on every machine, and the cases that call
  `xdevcmd-core` directly cannot use it anyway.
- **Peak memory of a child**: Linux `/proc/<pid>/status` VmHWM (read before it exits)
  or the children's `ru_maxrss` (`getrusage`, if rustix offers it; else `/proc` and
  `ps` as `usage.rs` reads its own); macOS `ru_maxrss` is in bytes, Linux's in KiB.
  Windows: unknown, as in Usage.

### 4. Critique of the outline

1. **`xtask` must stay out of the release machinery.** A workspace member is packaged
   by CI's `cargo package --workspace` and versioned by cargo-release: the crate needs
   `publish = false` and `[package.metadata.release] release = false`, and `cargo deny`
   covers it. It should depend only on crates already in `Cargo.lock` (xdevcmd-core,
   rustix, portable-pty for the pty start), so the shipped dependency tree and
   `deny.toml` do not move.
2. **Start-up is measured twice, on purpose**: through a pty that answers the
   terminal's queries at once (what a local terminal costs: the budget), and through
   one that answers nothing (what a silent terminal or a slow SSH link costs: the 1.7 s).
   Both come from `--timings`' JSON; the harness never parses the screen.
3. **The 150 ms start-up budget can be a CI test now**: everything but the terminal
   check, from `--timings` in a pty, which the pty tests can already run. A generous
   bound (150 ms on a runner) catches a real regression without flaking.
4. **The terminal check can move after the first frame without the risk it was put
   first for.** It runs first so that no reply reaches crossterm's input reader as keys;
   but that reader is made lazily in the first turn of the loop, *after* the first
   draw (`run.rs`: `self.input.get_or_insert_with(KeyReader::new)`). Probing between
   the first draw and the reader's creation keeps the replies away from it, and the
   screen is up in ~4 ms. Keys typed during the probe still wait, as today. This is
   decision 4's recommendation; it needs a pty test with a silent terminal (the screen
   is drawn before 100 ms) and the existing picture tests.
5. **The harness's trees must be the same every run**: generated from a fixed recipe
   (counts, sizes, names, depth) into a scratch directory, not from the user's files.
   A `--quick` set (seconds) for each feature's before/after, the full set (minutes)
   before a release.
6. **What the owner's machine is called**: `--host <name>` given by the owner, never
   the hostname or user name read from the system (private data in a committed file).
7. **The beta.1 checklist is long**: 029's, 030's and 031's "by hand" lists together.
   Ordered by risk (data first: kill and power-pull, USB FAT/exFAT, SMB pulled mid-copy,
   trash and undo; then tables, ETA, timings), each with how to set it up and what to
   see, so a week's use covers the top first.

### 5. Steps (each a commit; behaviour first as a failing test where there is one)

1. **`release.yml`**: pre-release tags on `dev`, `--prerelease --latest=false`, escaped
   dots, a `workflow_dispatch` dry run of `check` (tried with `v0.4.0-beta.1` before any
   tag exists: it must fail on the version and pass on the branch rule). **S.**
2. **`docs/development.md`, "Pre-releases"** (outline). **S.**
3. **The beta.1 hand-test checklist**, `docs/` or the round's Check (decision 5),
   ordered by risk (critique 7). **S.**
4. **The benchmark harness** (outline step 4, with critiques 1, 2, 5 and 6): `xtask`
   with `bench [--quick] [--host NAME] [--save-baseline]`; the start-up budget test in
   `budgets.rs` (critique 3). **M.**
5. **The terminal check after the first frame** (decision 4), with its pty test. **S.**
6. **The cut**: `feat/v0.4.0` into `dev` by pull request with CI green; `cargo release
   0.4.0-beta.1 --no-publish --no-tag --no-push`; CHANGELOG dated; tag on `dev`; the
   draft read; the harness run on the owner's machine and saved as beta.1's baseline;
   Publish by the owner. **S.**

About a week, then the week of use.

### 6. Decisions for the owner

1. **CHANGELOG for pre-releases**: `## [0.4.0-beta.1] - date` holds everything since
   0.3.0 now; beta.2 and rc.1 get their own short sections; at 0.4.0 one section says
   everything since 0.3.0 again, in full (recommended: each release page reads on its
   own) — or one section that grows until 0.4.0.
2. **Where the plumbing lands**: in this round, as above (recommended; it is small and
   the cut is here).
3. **The harness**: `cargo xtask bench` (recommended, critique 1) or a shell script;
   results in `bench/` in the repository (recommended: reviewed with the change that
   moved them) or outside it.
4. **The terminal check**: after the first frame (recommended, critique 4), or a
   shorter limit before it (e.g. 300 ms), or as it is.
5. **The beta.1 checklist's home**: `docs/testing-by-hand.md` (recommended: kept for
   beta.2 and rc.1, and for anyone else testing) or only this round's Check.

### 7. Risks

- **A pre-release tag on `dev` is new**: the check job's rules are tried by the
  `workflow_dispatch` dry run first; a wrong tag can be deleted (development.md says how)
  before anything is published.
- **The harness measures the machine** (outline); per-host baselines and the machine's
  load printed with each run.
- **Moving the terminal check** changes the order of the first second for every user;
  the picture tests (kitty, iTerm2, sixel, the cell size) and a silent-terminal test
  guard it.
- ~~Round 030 still at Review~~: closed by the owner before this round started.

## Plan (outline)

### Steps (each a commit)

1. **`release.yml`** (`check` job): a tag matching `v<x.y.z>-beta.N` or `-rc.N` must be
   on `origin/dev` (or main); a stable tag `v<x.y.z>` must still be on main; anything
   else is refused. The CHANGELOG check escapes the dots of the version in its regular
   expression (today `0.4.0` matches `0x4y0`; harmless, but a pre-release has more
   dots). The `release` job passes `--prerelease --latest=false` for such a tag, and
   keeps `--draft`. A `workflow_dispatch` with a `tag` input that runs only `check`, so
   the rules are tried before a real tag. **S.**
2. **`docs/development.md`, "Pre-releases"**: `cargo release 0.4.0-beta.1 --no-publish
   --no-tag --no-push` on `dev`, the CHANGELOG heading `## [0.4.0-beta.1] - date`, the
   tag on `dev`, the draft, Publish; steps 5 and 6 (crates.io, Scoop, winget) skipped;
   how a later beta and the rc follow; deleting a bad pre-release tag. **S.**
3. **The beta.1 hand-test checklist** (the owner's, a week of use): Round 029's §7 "by
   hand" list (a USB stick in FAT32 and exFAT, an SMB share, a stick pulled mid-copy,
   a kill mid-move), the planner's tables, undo, the trash, pause and retry, on Linux,
   Windows and macOS. **S.**
4. **The benchmark harness** (owner, 2026-11-13). **M.**
   - **One command, not a framework** (pillar 2's rule): `cargo xtask bench` (an
     `xtask` crate in the workspace, not published) or `scripts/bench`; it builds the
     release binary, makes fixed trees in a scratch directory, runs each case N times
     (default 5, after one warm-up), and reports median, p95 and best. No criterion,
     no new dependency in the shipped crates.
   - **What it measures** (each case names its unit):
     | Group | Cases |
     |---|---|
     | Package | the binary's size per target built; dependencies counted from `Cargo.lock` |
     | Start-up | `--timings`' JSON (Round 031): total and each step; RSS once idle |
     | Listing | 10,000 and 100,000 entries: list and sort time, peak RSS |
     | Planner | scan of 10,000 / 100,000 / 200,000 files (the cap): time, peak RSS |
     | Copy, move | 10,000 × 1 KiB and 1 × 1 GiB (smaller with `--quick`): files/s and MB/s; replace into the trash vs for good; one device and two (where `XDEVCMD_TEST_OTHER_DEV` is set) |
     | Undo | undoing a copy of 10,000 files |
     | Steady | idle CPU over 30 s; RSS over a soak (the existing `soak` test's sampler, shortened) |
   - **Driven through the program, not around it**: the planner and copy cases call
     `xdevcmd-core` directly (as `measure_a_move` does today, which it absorbs); start-up
     runs the real binary in a pty (the `script`-style run that showed the terminal
     check's 1.7 s, plus one that answers the terminal's queries, so both are known).
   - **Results kept**: `bench/results/<version>-<host>-<date>.json` (host = a name the
     owner gives, never the machine's real name or user), and `bench/baseline.json` per
     host, updated only on `--save-baseline`. A run prints a table against the baseline:
     each case's change in %, marked when past ±15 % (tunable).
   - **Not a CI gate on time**: GitHub's runners vary by 20–50 %, so CI keeps only the
     hard budgets already there or cheap to add: binary ≤ 7 MB (exists); start-up's
     steps other than the terminal check ≤ 150 ms; 100,000 entries < 1 s (made to run
     in CI with a smaller tree, or kept by hand). The harness's comparison is run on
     the owner's machine.
   - **Measured before the cut and recorded** in this round's Check as beta.1's
     baseline, on Linux at least, macOS and Windows if the owner runs it there.
   - Windows: RSS not measured yet (Usage says so, `forbid(unsafe_code)`); the harness
     reports it as unknown there rather than guessing.
5. **The cut**: `feat/v0.4.0` into `dev` by pull request with CI green on all four
   runners; the version raised; CHANGELOG dated; tag; the draft read; Publish by the
   owner. **S.**

### Risks

- The workflow is exercised only by a tag: the `workflow_dispatch` dry run covers the
  check; the build part is unchanged from 0.3.0.
- Help → Documentation (`crates/xdevcmd/src/docs.rs`) links to the tag `v{version}`:
  for beta.1 that is `v0.4.0-beta.1`, which will exist. Checked by hand on the draft.
- `cargo release` with `publish = true` in `release.toml`: every pre-release command
  says `--no-publish`; a pre-release on crates.io could never be deleted.
- **A benchmark that measures the machine, not xdevcmd**: page cache, CPU frequency,
  other load. Each case drops what it can (fresh files per run, a warm-up run, the
  median of several), prints the machine's load before it starts, and says that two
  hosts' numbers are not comparable. The baseline is per host.
- **The start-up budget is the terminal's**: Round 031 measured every step at ~4 ms
  except the terminal check (up to 1.7 s when the terminal does not answer). Whether
  that check gets a shorter limit or moves after the first frame is decision 4.

### Open decisions (recommendations at kickoff)

1. **CHANGELOG for pre-releases**: a dated section per pre-release, and 0.4.0's section
   saying what changed since 0.3.0 in full (leaning), or one growing section.
2. **Where the plumbing lands**: in this round (leaning) or pulled forward as a small
   commit earlier, since it does not depend on 029–031.
3. **The harness's form**: `cargo xtask bench` (leaning: Rust, cross-platform, no shell
   to port to Windows) or a script; where results live (`bench/` in the repository,
   leaning, so they are reviewed with the code that changed them; or outside it).
4. **The terminal check at start-up**: keep 1.5 s, shorten it (e.g. 300 ms, then a
   second chance later), or ask after the first frame is drawn (leaning: the screen is
   there at once; pictures may take their protocol a moment later).

### Not doing

- crates.io, Scoop, winget, binstall for a pre-release (owner, 2026-11-13).
- A benchmark gate in CI on relative times (noise; see step 4); a dashboard or a
  benchmarking service (no network, and none is needed for one owner's machine).

## Do

### 2026-11-13, the owner's decisions

Round 030 closed first, then "go as recommended": all five as the kickoff recommends.

1. CHANGELOG: a dated section per pre-release; 0.4.0's section says everything since
   0.3.0 again, in full.
2. The plumbing lands in this round.
3. The harness is `cargo xtask bench`; its results in `bench/` in the repository.
4. The terminal check moves after the first frame.
5. The beta.1 checklist lives in `docs/testing-by-hand.md`.
- The cut itself (step 6: the pull request into `dev`, the version, the tag) waits for
  the owner's word once steps 1–5 are done and green: it publishes a draft.

### 2026-11-13, steps 1–3 (released rules, docs)

- Step 1 (`release.yml`, `scripts/release-check.sh`): pre-release tags on `dev` or
  `main`, `--prerelease --latest=false`, escaped dots, a `workflow_dispatch` dry run.
  Tried locally (v0.3.0 passes; `-alpha.1`, `v0.4` refused; a beta on this feature
  branch refused, on `origin/dev` accepted by the branch rule) and on GitHub: run
  43402853985 failed as it should, on the version, the CHANGELOG and the branch, and
  built nothing.
- Steps 2–3: `docs/development.md` "Pre-releases"; `docs/testing-by-hand.md` (029–031's
  hand tests, data first), linked from `docs/README.md`.

### 2026-11-13, step 5 stopped: the terminal check after the first frame loses keys

- Done and reverted the same day. The kickoff (critique 4) said keys typed during the
  check "still wait, as today". **Wrong**: the check reads stdin itself
  (`graphics::read_until`), so whatever arrives before the device-attributes reply is
  taken and dropped; crossterm cannot be handed those bytes back. Before the first
  frame nobody types; after it, on a terminal that does not answer, the first 1.7 s of
  keys would be lost. Found by the kitty picture tests, which type as soon as the panes
  show (three failed: "written 0 times, not 1").
- Back to the owner (decision 4 again): keep the check before the first frame (as it
  is), shorten its limit there, or move it after the first frame and parse the keys it
  swallowed ourselves (more code, a second key parser).

### 2026-11-13, step 4: the benchmark harness

- `xtask/` (`publish = false`, `release = false`, only crates already in the lock),
  `.cargo/config.toml` alias, `default-members` so `cargo build`/`test` at the root still
  mean xdevcmd. `deny.toml`: `allow-wildcard-paths = true`, which cargo-deny applies to
  unpublished crates only (checked: xdevcmd's pin taken away is still refused).
- Cases as the outline, each in a child process (peak RSS from VmHWM on Linux), start-up
  in portable-pty answering and silent, idle CPU from `/proc/<pid>/stat`; results and
  per-host baselines in `bench/`; `docs/development.md` "Benchmarks", `bench/README.md`.
- The budget: start-up without the terminal check < 150 ms, asserted in the pty test.
- CI run 43409507585 (aa410e7) failed that assertion on all three Unix runners: the
  first frame alone took 195–240 ms in a debug build sharing the runner with the other
  pty tests (locally 11 ms in all). Critique 3 of the kickoff was wrong about 150 ms
  holding on a runner. The pty test is now a tripwire at 1 s; the 150 ms budget stays
  with `cargo xtask bench` (release build). Windows failed `render.rs`
  `tilde_goes_home_and_ctrl_backslash_to_the_root` in the same run: no source changed
  in aa410e7, and it passed on 4d4892b; taken as a flake, watched on the next run.
- CI run 43410360888 (578ac72): Windows passed (no flake this time); the start-up
  tripwire passed. `cargo package --workspace` failed on xtask: `publish = false` does
  not keep a member out of `cargo package`, and its path dependency has no version.
  Critique 1 was wrong there. CI and development.md now pass `--exclude xtask`. The
  owner set the rule for CI on 2026-11-13: run it before a round closes or a PR opens,
  not per commit.
- First quick run (this sandbox, 3 runs; not saved as a baseline, not the owner's
  machine): binary 5.39 MiB, 277 crates in the lock; list 10,000 8.9 ms; plan 10,000
  8.3 ms; copy 2,000 × 1 KiB 47 ms; one 32 MiB file 9.9 ms; undo 2,000 8.3 ms; start-up
  11 ms (answering) / 1,714 ms (silent: the check 1,702 ms); idle CPU 0.33 %.
- **Found by it**: replacing 1,000 small files took 7.1 s, ~7 ms a file. `strace -w`:
  the per-file `fdatasync` before the rename (Round 029's rule), 7.4 ms each on this
  disk; the trash step itself is a rename. Input for beta.2's group commit (item 43),
  which today is planned for moves only.

## Check

## Act
