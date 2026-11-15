# Round 034: beta.2 review fixes (winget out, B1, B2, B4–B7)

**Status**: Review
**Part of**: the v0.4.0 release plan (owner's doc, 2026-11-15): backlog B1–B7, gate beta.2
**Date started**: 2026-11-15
**Date completed**:

## Goal

Land the fixes the 2026-11-15 review found for beta.2, and take winget out of the plan
(owner: an unsigned build meets SmartScreen and Defender warnings), so beta.2 can be cut
with no overclaim and no known accident key.

## Plan

Owner's decisions, 2026-11-15:
- winget: `packaging/winget/` and its test deleted; ROADMAP item 48 [draft].
- B2: ask at recovery. Enter trusts what was copied (fast, as before); **C** checks:
  reads every copied file with its source, content not size+mtime, copies again what
  differs. In beta.2.
- B3: batched sync of new files before the job ends: **rc.1**, with R4/E11. The docs
  say "a killed run"; a power cut is best effort (B7).
- B4: `Q` → `Alt+x q`.
- PRs into `feat/v0.4.0-beta.2`; one round for all of it (this file).

- [x] 1. Remove winget (README, ROADMAP, development.md, packaging README, CHANGELOG)
- [x] 2. B1 temporary name fits 255 bytes
- [x] 3. B4 Quit without cd off `Q`
- [x] 4. B5 rust-version 1.90, `msrv` CI job
- [x] 5. B6 CI on `feat/**` pushes
- [x] 6. B7 CHANGELOG and security.md narrowed
- [x] 7. B2 checked continue (C)
- [ ] 8. B3 → rc.1 (not this round)

## Do

- 1 (`d91259a`): manifests and test deleted; item 48 in Later, Open and a note; restore
  point `3a84988`. Earlier CHANGELOG sections, rounds and `.agents/memory/` untouched.
- 2 (`6dbbea3`): `part_name` cuts the original name at a char boundary so the whole name
  fits `NAME_MAX` (255 bytes; bytes ≥ UTF-16 units, so NTFS too). The run's mark that
  cleanup matches on is unchanged.
- 3 (`228b074`): every action must keep a default key (test), so not "no key" but a
  two-key chord. The round-trip test now handles two-key bindings. docs/keys.md
  regenerated.
- 4, 5 (`ade95f7`): the lockfile needs 1.90 (quantette 0.6.0, ordered-float 5.5.1),
  not only quantette as the review said. The `msrv` job reads the version from
  Cargo.toml. Deviation: B5 and B6 in one commit (overlapping hunks in development.md).
- 6 (`9208657`): "after a killed run"; security.md adds the skipped finished folder.
- 7 (`641b727`): `Request::verify`; on a resume, `done_dirs` is ignored and an
  identical-looking file is compared byte by byte, replaced without a question if it
  differs (old one to the trash where there is one). `Summary::repaired`. The dialog's
  `also` button: `C check (slow)`, copy and move only. Content, not size+mtime: XFS can
  leave a right-size file of zeros.
- CI fixes after the push: `688eb7b` (B1 tests' `mut` unused on Windows/macOS, clippy
  `-D warnings`); `49f0c52` (the dialog's C line said "copied", which an older pty test
  waits for as the job's end: on the slower ARM runner F10 reached the running job).

## Check

- [x] Each fix has a test that fails without it: B1 (both tests fail with
      `File name too long`), B2 (trusted run keeps the zeros, checked run repairs),
      B4 (pty: `Q` alone does not quit).
- [x] Locally (Linux, Rust 1.97): fmt, clippy `-D warnings`, full suite, cargo-deny.
- [x] CI green on `49f0c52`: ubuntu, ubuntu ARM, Windows, macOS, msrv (1.90),
      cross-check, cargo-deny (run 43691227720).
- [ ] Not verified: the checked continue on a real power cut (the tests fake one by
      writing zeros with the source's date).
- [ ] Owner: hand test 1.4 in testing-by-hand.md (the new checked-continue step).

## Act

**Learnings**:

- A review's "needs X" for an MSRV is a lower bound: check every locked dependency's
  `rust-version` (`cargo metadata`), not the one named.
- "Every action has a default key" makes removing an accident key a key change, not a
  deletion: a chord is the cheap safe default.

**Promotions**:

- [ ] → ROADMAP: B3 in rc.1 next to R4/E11 (the release plan doc already has it).
