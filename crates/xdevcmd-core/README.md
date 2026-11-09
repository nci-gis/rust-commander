# xdevcmd-core

The engine of [xdevcmd](https://crates.io/crates/xdevcmd), a two-pane file commander for
the terminal: directory listing and sorting, copy / move / delete on a worker thread,
archives (zip, tar, tar.gz), find, compare, bulk rename, undo. It has no terminal code.

**It has no stable API.** It is published only because `xdevcmd` depends on it, is
released with it under the same version, and may change in any release. To use the file
commander, install `xdevcmd`:

```bash
cargo install xdevcmd --locked
```

Source, issues and documentation: <https://github.com/nci-gis/rust-commander>.
MIT licence, XDevTeam.
