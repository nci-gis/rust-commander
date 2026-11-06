# Development

## Build and run

```bash
cargo build --release                 # target/release/rcmd
cargo run -p rcmd-tui -- /etc /var    # start with two directories
cargo install --path crates/rcmd-tui --locked   # install your build as `rcmd`
```

Needs Rust 1.88 or newer (`rustup`). Nothing needs root.

## Check before you push

```bash
cargo test                                         # about 300 tests, ~15 s
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --target x86_64-pc-windows-gnu -- -D warnings
cargo fmt --check
cargo deny check                                   # vulnerabilities and licences
```

`rustup target add x86_64-pc-windows-gnu x86_64-apple-darwin` once, to check the other
platforms from Linux. CI runs the full tests on Linux, Windows and macOS.

## See the screen without a terminal

```bash
./target/release/rcmd --screenshot=118x34 --keys=F9 /etc /usr
```

Draws one frame as text, after typing the keys. Most UI tests work this way.

## Memory over time

```bash
RCMD_SOAK_SECS=600 cargo test --release --test pty -- --ignored soak --nocapture
```

Uses rcmd heavily for ten minutes and fails if memory keeps growing. Linux only.
Samples go to `target/tmp/soak-rss.csv`.

## Docs that are generated

- `docs/keys.md`: `rcmd --print-keys > docs/keys.md`. A test fails if it is out of date.
- The tagline in `README.md` is the `description` in `Cargo.toml`. A test checks it.
- The GitHub About box: `scripts/sync-about.sh --apply`.

## Icon and logo

The SVGs in `assets/` are the source: `icon.svg`, `icon-16.svg` (the same picture
redrawn on whole pixels, because a scaled-down one blurs at 16 px), and the README's
`logo-light.svg` / `logo-dark.svg`, whose wordmark is drawn as strokes so it looks the
same without any font. After changing an icon SVG, rebuild `assets/rcmd.ico`:

```bash
cargo install --locked resvg   # once
scripts/icons.sh
```

On Windows, `build.rs` puts the icon and the version details into `rcmd.exe`. It needs
the Windows SDK's `rc.exe` (or MinGW's `windres`); without one it warns and builds
without the icon, unless `RCMD_REQUIRE_ICON=1`, which CI and releases set.

## Branches

Work on `dev`. `main` changes only through a pull request from `dev`, with CI green.

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
   dated CHANGELOG section agree and that the tag is on `main`, then builds Linux (glibc
   and static musl), macOS (ARM and Intel) and Windows, and opens a **draft** release with
   the archives, `SHA256SUMS`, `THIRD-PARTY.md` and that CHANGELOG section as its notes.
   Read it, then Publish.

The packaging runs, without releasing, on any push to `dev` that changes
`.github/workflows/release.yml`, `about.toml` or `about.hbs`.

## How the code is organised

Start with [.agents/context/project.md](../.agents/context/project.md): the rules,
the layout and the conventions. Topic notes are in [.agents/reference/](../.agents/reference/).
