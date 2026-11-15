# Running the benchmarks

How to measure xdevcmd's copies on your own machine and send the numbers back, step
by step for macOS, Windows and Linux. For every case `cargo xtask bench` has, baselines
and what the numbers in a debug log mean, see [Development](development.md#benchmarks).

Some decisions wait for numbers from systems and drives the developers do not have. A
number from your machine, measured as below, is worth more than a guess.

## What it does, and what it never touches

`xtask bench --only mixed` copies one large file (1000 MiB) followed by 5,000 small
files (4 KiB each), the way xdevcmd copies by default, five times after one warm-up
run. It prints how long each copy took and how much memory it used.

- **Its own files only.** It makes them from a fixed recipe in a folder of its own
  (`xdevcmd-bench-<number>`, in the system's temporary folder or under `--dir`), and
  removes that folder at the end. It never reads or writes your files.
- **No network.** The results are files on your disk; you decide what to send.
- **Space and time.** About 3 GiB free on the drive measured. A few minutes on an
  internal SSD; on a USB stick it can take much longer (see `--runs` below).

## Before you start, on every system

1. **Name what you measure** with `--host`: the machine, the drive and its file
   system, such as `mbp-m2-apfs`, `win11-ssd-ntfs`, `win11-usb-exfat`. Letters, digits,
   `.`, `_` and `-` only. Nothing is read from the system, so the name is the only
   record of what was measured.
2. **One name per run on the same day.** The results file is named
   `<version>-<host>-<date>.json`: a second run with the same `--host` on the same day
   replaces the first. Two runs on one drive (macOS's fast copy on and off, below) need
   two names.
3. **A quiet machine.** Plugged in (a laptop on battery slows down), with no build,
   backup, cloud sync (iCloud, OneDrive, Dropbox), indexing or update running, and other
   programs closed. Linux prints the load first; on macOS and Windows, look at Activity
   Monitor or Task Manager: the CPU should be close to idle.
4. **Leave antivirus and security features on.** People who use xdevcmd have them, and
   numbers without them would not be theirs. Say which one runs in your note (a company
   laptop's endpoint protection changes small-file numbers a lot).
5. **One drive at a time.** Once on the internal drive (no `--dir`), then once for
   each other drive, with `--dir` on that drive. The same USB stick in exFAT and in NTFS
   is two drives.

## Get the program

**With Rust** (any system): clone the repository and run `cargo xtask bench …` from it,
in place of `./xtask bench …` below. It builds what it needs; nothing else to install.
On an Intel Mac or an ARM Linux machine this is the only way.

**Without Rust**: download `xtask` from the bench workflow on GitHub. Actions → bench →
the run → Artifacts: `xtask-macOS` (Apple silicon only), `xtask-Windows`, `xtask-Linux`
(x86-64).

- The program must be built from the code being measured, and a run's artifacts last
  14 days. Take the newest run: an older one may not have the `mixed` case.
- To make a new run (a maintainer, with push access): push a branch whose name starts
  with `bench/` and does **not** end in `-<digits>`, which the workflow reads as a
  number of files:

  ```bash
  git push origin HEAD:bench/beta2-mixed
  ```

  Once `bench.yml` is on `main`, Actions → bench → Run workflow does the same.

## macOS

In Terminal, in the folder where the download is (if Safari already unzipped it, use
the `xtask-macOS` folder it made):

```bash
cd ~/Downloads
unzip xtask-macOS.zip -d xdevcmd-bench
cd xdevcmd-bench
chmod +x xtask
xattr -d com.apple.quarantine xtask
```

`chmod` because the zip keeps no permissions; `xattr` because macOS blocks a downloaded
program that is not notarised. Do this only for a file you downloaded from this
repository's Actions.

**The internal drive (APFS), twice.** On APFS, xdevcmd's default fast copy is a clone:
the file is not written at all, which is what a copy within one APFS volume does. The
second run turns the fast copy off and measures xdevcmd's own reading and writing, which
is what a copy to another drive does:

```bash
export XDEVCMD_DEBUG=jobs
./xtask bench --only mixed --host mbp-m2-apfs
XDEVCMD_BENCH_FAST=0 ./xtask bench --only mixed --host mbp-m2-apfs-own
```

**A USB stick or another drive.** There is no clone across drives, so one run is enough:

```bash
./xtask bench --only mixed --dir /Volumes/STICK/xdevcmd-bench --host mbp-m2-usb-exfat
```

- Turn Low Power Mode off (System Settings → Battery) for every run, or on for every
  run: never mixed.
- A drive just plugged in is indexed by Spotlight for a while: wait until Activity
  Monitor shows `mds` and `mdworker` quiet.

## Windows

In PowerShell, in the folder where the download is:

```powershell
cd $HOME\Downloads
Expand-Archive xtask-Windows.zip -DestinationPath xdevcmd-bench
cd xdevcmd-bench
Unblock-File .\xtask.exe
```

`Unblock-File` removes the "downloaded from the internet" mark, so SmartScreen does not
stop the program. Do this only for a file you downloaded from this repository's
Actions.

**The internal drive, then each other drive.** Windows has no fast copy in xdevcmd yet,
so one run per drive:

```powershell
$env:XDEVCMD_DEBUG = "jobs"
.\xtask.exe bench --only mixed --host win11-ssd-ntfs
.\xtask.exe bench --only mixed --dir E:\xdevcmd-bench --host win11-usb-exfat
```

- `$env:XDEVCMD_DEBUG` lasts for this PowerShell window only.
- Microsoft Defender scans each new file, which shows in the small files' time. That
  is what people get: leave it on.
- Keep the same power mode (Settings → System → Power) for every run, and say which.

## Linux

```bash
unzip xtask-Linux.zip -d xdevcmd-bench
cd xdevcmd-bench
chmod +x xtask
export XDEVCMD_DEBUG=jobs
./xtask bench --only mixed --host thinkpad-ext4
./xtask bench --only mixed --dir /media/$USER/STICK/xdevcmd-bench --host thinkpad-usb-exfat
```

- The load is printed first: above 1 per CPU core, wait and run again.
- On Btrfs or XFS the fast copy is a clone, as on APFS: run a second time with
  `XDEVCMD_BENCH_FAST=0` and a second name (`thinkpad-btrfs-own`). On ext4 the fast
  copy is the kernel's own copy, and one run is enough.
- **The full set** (listing, the planner, every copy case, start-up) needs Rust and
  the repository: `cargo xtask bench --host thinkpad-ext4`, after `--quick` once to see
  that it works. See [Development](development.md#benchmarks).

## Slow drives: fewer runs

On a slow USB stick one run of `mixed` can take minutes. `--runs 3` is enough there
(the default is 5); say so in your note. Fewer than three says nothing about how
steady the numbers are.

## Reading what it prints

```text
case                                    median        p95       best  peak mem   vs base
mixed: one large file and small files  84.33 ms   94.61 ms   78.69 ms   4.3 MiB         —
```

- **median** is the number. **p95** says how steady it was: far above the median means
  something else was running, so run again.
- **vs base** is the change from this host's baseline, marked past ±15 %. To compare
  two versions on one machine: run the older one with `--save-baseline`, then the newer
  one with the same `--host`.
- With `XDEVCMD_DEBUG=jobs`, each run's debug log says where the time went: the small
  files against the large one, and what the large file's checkpoints cost
  ([what each field means](development.md#reading-a-debug-log)).

## What to send

From the folder you ran it in, after all your runs:

- `bench/results/*.json`: one file per run;
- `bench/results/debug-*/`: the debug logs, one folder per run;
- a short note: the link of the bench run you downloaded from (or the commit you built),
  the machine, the OS version, each drive and its file system, plugged in or not, the
  power mode, the antivirus, and anything you changed (`--runs 3`). Between releases
  the version in the results stays the last one released, so the link says which code
  was measured.

They are small text files holding numbers only: times, memory, counts, the OS name,
xdevcmd's version, the date and the `--host` name you gave. The debug logs hold no file,
folder or user names (a test checks this). Open them before you send them if you like.
