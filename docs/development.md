# Development

## Build and run

```bash
cargo build --release                 # target/release/xdevcmd
cargo run -p xdevcmd -- /etc /var     # start with two directories
cargo install --path crates/xdevcmd --locked   # install your build as `xdevcmd`
```

Needs Rust 1.88 or newer (`rustup`). Nothing needs root.

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
  plan. Pushing it runs nothing, so commit as often as you like. When it is ready, open
  a pull request into `dev`: CI runs on it, then merge.

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
`=x.y.z`; cargo-release rewrites the pin); `release.toml` says so with `publish = true`,
so **every `cargo release` before step 5 says `--no-publish`**.

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
   (`scripts/release-check.sh`), then builds Linux (x86_64
   glibc and static musl, ARM64 static musl), macOS (ARM and Intel) and Windows, and opens
   a **draft** release with the archives (each holding `LICENSE`, `README.md`,
   `THIRD-PARTY.md`, the example `themes/` and this version's `docs/`),
   `SHA256SUMS`, and that CHANGELOG section as its notes.
   Read it, then Publish.
5. **crates.io**, by the owner, from `main` at the tag (needs `cargo login` once, with a
   crates.io token kept on the owner's machine, never in a workflow). Check first that
   `cargo package --workspace` passes (CI does it on Linux):

   ```bash
   cargo release publish                 # look: xdevcmd-core, then xdevcmd
   cargo release publish --execute
   ```

   A version on crates.io cannot be replaced, only yanked; publish after the GitHub
   release is good, not before.
6. **Scoop and winget**, from the published release's `SHA256SUMS`: see
   [packaging/README.md](../packaging/README.md).

## Pre-releases

A beta or release candidate (`0.4.0-beta.1`, `0.4.0-rc.1`) goes to **GitHub only**,
marked pre-release, never "Latest"; crates.io, Scoop, winget and binstall wait for the
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
   on `dev` or `main`), builds the six archives and opens a **draft marked
   pre-release**. Read it, then Publish. Help → Documentation in that build links the
   tag's `docs/`.
5. **Not done for a pre-release**: crates.io (step 5 of "Releasing") and Scoop and winget
   (step 6). Every `cargo release` here says `--no-publish`.

A bad pre-release tag is deleted like any other (`git push origin :v0.4.0-beta.1`,
`git tag -d v0.4.0-beta.1`, and the draft deleted on GitHub) before it is published.
A published pre-release is not deleted: the next one supersedes it.

CI runs on pushes to `dev` and `main` and on pull requests into them; the `release`
workflow runs only for a `v*` tag (a release on `main`, a pre-release on `dev` or
`main`), or by hand to check a tag's rules. A change to the packaging
(`release.yml`, `about.toml`, `about.hbs`) is therefore first exercised by the next
tag. If that run fails, nothing is released: fix it on `dev`, merge, delete the tag
(`git push origin :v0.2.0` and `git tag -d v0.2.0`) and tag `main` again.

## How the code is organised

Start with [.agents/context/project.md](../.agents/context/project.md): the rules,
the layout and the conventions. Topic notes are in [.agents/reference/](../.agents/reference/).
