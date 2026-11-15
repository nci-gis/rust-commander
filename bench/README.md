# Benchmarks

`cargo xtask bench` writes here (see `docs/development.md`, "Benchmarks"; to measure on
your own machine and send the numbers, `docs/benchmark.md`):

- `results/<version>-<host>-<date>[-quick].json`: every run, and with `XDEVCMD_DEBUG`
  its debug logs. Local only (ignored by git): numbers worth keeping go into the round
  or the pull request they measured.
- `baseline-<host>[-quick].json`: the run each later one is compared with, per machine,
  replaced only by `--save-baseline`.

`<host>` is the name given with `--host`, never read from the machine. Numbers from two
hosts, or from a quick and a full run, are not comparable.
