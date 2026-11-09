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
newest heading of `CHANGELOG.md`, in the same commit. A release then takes four steps.
`cargo release` needs installing once: `cargo install --locked cargo-release`.

1. **On `dev`**, date the notes and commit (add `minor`, `patch` or a version number to
   also raise the version; leave it out to release the version `Cargo.toml` has):

   ```bash
   cargo release minor --no-tag --no-push          # look at what it would do
   cargo release minor --no-tag --no-push --execute
   git push origin dev
   ```

2. **Pull request `dev` → `main`**, merged with a merge commit once CI passes.
3. **On `main`**, tag the release and push the tag:

   ```bash
   git checkout main && git pull --ff-only
   cargo release tag --execute
   git push origin v0.2.0
   ```

4. **The `release` workflow** first checks that the tag, the version in `Cargo.toml` and a
   dated CHANGELOG section agree and that the tag is on `main`, then builds Linux (x86_64
   glibc and static musl, ARM64 static musl), macOS (ARM and Intel) and Windows, and opens
   a **draft** release with the archives (each holding `LICENSE`, `README.md`,
   `THIRD-PARTY.md` and the example `themes/`),
   `SHA256SUMS`, and that CHANGELOG section as its notes.
   Read it, then Publish.

CI runs on pushes to `dev` and `main` and on pull requests into them; the `release`
workflow runs only for a `v*` tag on `main`. A change to the packaging
(`release.yml`, `about.toml`, `about.hbs`) is therefore first exercised by the next
tag. If that run fails, nothing is released: fix it on `dev`, merge, delete the tag
(`git push origin :v0.2.0` and `git tag -d v0.2.0`) and tag `main` again.

## How the code is organised

Start with [.agents/context/project.md](../.agents/context/project.md): the rules,
the layout and the conventions. Topic notes are in [.agents/reference/](../.agents/reference/).
