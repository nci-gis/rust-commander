# Benchmarks

The speed of xdevcmd's copies on your own machine and drives, measured the same way
every time. The release's numbers come from the developers' machines: a reference, not
yours. About 25 minutes, unattended; no Rust needed.

## What it does, and what it never touches

`xtask bench --only mixed` copies one large file (1000 MiB) followed by 5,000 small
files (4 KiB each), the way xdevcmd copies by default, five times after one warm-up run,
and prints how long each copy took and how much memory it used.

- **Its own files only**: made from a fixed recipe in a folder of its own (in the
  system's temporary folder, or under `--dir`), copied within that folder, removed at
  the end. It never reads or writes your files, and uses no network.
- **Space**: about 3 GiB free on the drive measured.

## Before you start

1. **Name what you measure** with `--host`: machine, drive and file system, such as
   `mbp-m2-apfs` or `win11-usb-exfat` (letters, digits, `.`, `_`, `-`). It is the only
   record of what was measured.
2. **One name per run on the same day** (UTC): the results file is
   `<version>-<host>-<date>.json`, and a second run with the same name replaces it.
3. **A quiet machine**: plugged in, nothing syncing, building or updating, the CPU close
   to idle (Linux prints the load first).
4. **Leave antivirus on**, and say which one runs: people who use xdevcmd have it.
5. **One drive at a time**: the internal drive (no `--dir`), then `--dir` on each other
   drive. The same stick in exFAT and in NTFS is two drives.

## Get it

The same download as [How to test](how-to-test.md), step 1: `xdevcmd-testkit-<tag>-<target>`
from Releases (named `xdevcmd-bench-…` in 0.4.0-beta.2), built from that release's code, so it measures that
release. Every option: `xtask bench --help`. With Rust, any commit: `cargo xtask bench …`
in the repository ([Development](https://github.com/nci-gis/rust-commander/blob/main/docs/development.md#benchmarks)).

Check the archive first ([Checking a download](https://github.com/nci-gis/rust-commander/blob/main/docs/security.md#checking-a-download)).
The lines below that unblock it are only for an archive from this repository's Releases.

## macOS

```bash
cd ~/Downloads
tar xzf xdevcmd-testkit-v0.4.0-beta.3-aarch64-apple-darwin.tar.gz
cd xdevcmd-testkit-v0.4.0-beta.3-aarch64-apple-darwin
xattr -d com.apple.quarantine xtask 2>/dev/null   # this repository's download only
export XDEVCMD_DEBUG=jobs
./xtask bench --only mixed --host mbp-m2-apfs
XDEVCMD_BENCH_FAST=0 ./xtask bench --only mixed --host mbp-m2-apfs-own
./xtask bench --only mixed --dir /Volumes/STICK/xdevcmd-bench --host mbp-m2-usb-exfat
```

- **APFS twice**: there the default fast copy is a clone, no bytes written; the second
  run (`XDEVCMD_BENCH_FAST=0`) measures xdevcmd's own writing, as a copy to another
  drive does. A stick in APFS: twice too; in exFAT or FAT: once.
- Low Power Mode the same for every run; after plugging a drive in, wait until
  Spotlight (`mds`, `mdworker`) is quiet.

## Windows

```powershell
cd $HOME\Downloads
Expand-Archive xdevcmd-testkit-v0.4.0-beta.3-x86_64-pc-windows-msvc.zip -DestinationPath .
cd xdevcmd-testkit-v0.4.0-beta.3-x86_64-pc-windows-msvc
Unblock-File .\xtask.exe                           # this repository's download only
$env:XDEVCMD_DEBUG = "jobs"
.\xtask.exe bench --only mixed --host win11-ssd-ntfs
.\xtask.exe bench --only mixed --dir E:\xdevcmd-bench --host win11-usb-exfat
```

- Windows has no fast copy in xdevcmd yet: one run per drive.
- The same power mode (Settings → System → Power) for every run; Defender on.

## Linux

```bash
tar xzf xdevcmd-testkit-v0.4.0-beta.3-x86_64-unknown-linux-musl.tar.gz
cd xdevcmd-testkit-v0.4.0-beta.3-x86_64-unknown-linux-musl
export XDEVCMD_DEBUG=jobs
./xtask bench --only mixed --host thinkpad-ext4
./xtask bench --only mixed --dir /media/$USER/STICK/xdevcmd-bench --host thinkpad-usb-exfat
```

- Btrfs or XFS: the fast copy is a clone, as on APFS; run again with
  `XDEVCMD_BENCH_FAST=0` and a second name. ext4: once.
- A load above 1 per CPU core: wait and run again.

**A slow stick**: `--runs 3` (the default is 5), and say so. Fewer says nothing about
how steady the numbers are.

## Reading what it prints

```text
case                                                     median        p95       best  peak mem   vs base
mixed: one large file and small files                  84.33 ms   94.61 ms   78.69 ms   4.3 MiB         —
```

(That one is a clone, on APFS.) **median** is the number; **p95** far above it means
something else ran, so run again. **vs base** is the change from this host's baseline
(`—` without one; ◀ past ±15 %). With `XDEVCMD_DEBUG=jobs`, the debug logs in
`bench/results/debug-<host>-<date>/` say where the time went
([each field](https://github.com/nci-gis/rust-commander/blob/main/docs/development.md#reading-a-debug-log)).

## What to send

With your report ([How to test](how-to-test.md), step 11),
or alone as a comment on the release:

- `bench/results/*.json` and `bench/results/debug-*/`, from the folder you ran it in
  (numbers only: times, memory, counts, the OS name, the version, the date, your
  `--host` names; the logs hold no file, folder or user names);
- a note: the release you took it from, the machine and OS version, each drive and its
  file system, plugged in or not, the power mode, the antivirus, and any `--runs 3`.
