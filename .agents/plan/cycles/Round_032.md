# Round 032: Pre-release plumbing and 0.4.0-beta.1 (v0.4.0-beta.1, part 4 of 4)

**Status**: Planning — outline; the kickoff is written when Round 031 is Complete
**Part of**: v0.4.0, step 6 of `ROADMAP.md`; Decisions "Release path" and "Where
pre-releases go" (owner, 2026-11-13: GitHub only, marked pre-release, never "Latest";
crates.io, Scoop, winget and binstall start at 0.4.0); the benchmark harness (owner,
2026-11-13: "Thêm harness benchmark vào Round 032"), so that beta.1's numbers, which set
beta.2's scope (item 43), are measured the same way each time
**Date started**: YYYY-MM-DD
**Date completed**: YYYY-MM-DD

## Goal

A `v0.4.0-beta.1` tag on `dev` builds the six archives and opens a draft GitHub release
marked pre-release; nothing goes to crates.io, Scoop or winget. Before the cut, one
command measures xdevcmd the same way every time and keeps the numbers, so each later
feature is compared with a baseline instead of a guess. Then beta.1 is cut, its baseline
recorded, and the owner uses it for about a week.

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

## Check

## Act
