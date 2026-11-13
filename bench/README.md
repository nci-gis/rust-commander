# Benchmarks

`cargo xtask bench` writes here (see `docs/development.md`, "Benchmarks"):

- `results/<version>-<host>-<date>[-quick].json`: every run, kept with the change it
  measured.
- `baseline-<host>[-quick].json`: the run each later one is compared with, per machine,
  replaced only by `--save-baseline`.

`<host>` is the name given with `--host`, never read from the machine. Numbers from two
hosts, or from a quick and a full run, are not comparable.
