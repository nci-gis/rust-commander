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

## Branches

Work on `dev`. `main` changes only through a pull request from `dev`, with CI green.

## Releasing

1. Move the `[Unreleased]` notes in `CHANGELOG.md` under the new version, and set
   `version` in `Cargo.toml` to match.
2. Merge `dev` into `main` by pull request.
3. Tag `main`: `git tag v0.1.0 && git push origin v0.1.0`.
4. The `release` workflow builds Linux (glibc and static musl), macOS (ARM and Intel)
   and Windows, and opens a **draft** release with the archives, `SHA256SUMS`, and
   `THIRD-PARTY.md` (the licences of everything inside). Read it, then Publish.

The packaging runs, without releasing, on any push to `dev` that changes
`.github/workflows/release.yml`, `about.toml` or `about.hbs`.

## How the code is organised

Start with [.agents/context/project.md](../.agents/context/project.md): the rules,
the layout and the conventions. Topic notes are in [.agents/reference/](../.agents/reference/).
