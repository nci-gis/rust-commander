# Security

xdevcmd shows names and files other people made, runs a shell, and changes files with
your permissions. This page says what it guards against, and where the limits are.

**In short**

- **Hostile names, files and archives** cannot run commands, hang xdevcmd or write
  outside the folder you chose. [What xdevcmd guards against](#what-xdevcmd-guards-against)
- **A killed copy never leaves a short file under its real name**, and the next start
  offers to continue it. After a power cut, continue with **C**.
  [Safe operations](#safe-operations)
- **`--read-only`, `--restricted` and `--root`** limit what xdevcmd can do.
  [Safe modes](#safe-modes)
- **Nothing is typed into a running program**, with two known gaps (a shell's own
  `read`; on Windows, a program that prints a prompt mark). What the shell prints is
  never trusted to run anything. [The shell panel](#the-shell-panel)
- **Your config is trusted like `.bashrc`**: keep it yours.
  [The config file is trusted](#the-config-file-is-trusted)
- **No network access, no telemetry.** Downloads can be checked:
  [Checking a download](#checking-a-download).

## What xdevcmd guards against

| Risk | What xdevcmd does |
|---|---|
| **File names that try to run commands** | A name with control characters is never typed into a shell or put on the clipboard, and is refused where you type a name for something new: a file, folder or archive, Save as, Log to file, Copy As and every rename (a copy or an extract keeps the names it is given). |
| **Files that hang a program** | FIFOs and devices are refused before they are opened. |
| **Hostile archives** | Names that point outside the target folder are never written, and a link already at a file's name is replaced, not followed. Links inside archives are not created. Set-id bits are dropped. An entry bigger than it claims is stopped. An archive is refused if it would not fit, would unpack to over 1 GB at more than 1000 times its own size, or to more than a million files. |
| **Losing a file half-way** | A file being replaced is written under a temporary name and swapped in once complete. A move across drives removes each source only after its own copy is done, so what you skipped stays. Shift+F8 needs **Y**, not Enter. |
| **Huge images** | A picture claiming billions of pixels is refused before it is decoded. |
| **Running programs by accident** | Enter or **o** on a program, script, launcher, installer or disk image asks "Run it?", with Cancel the default. Enter opens without asking only known document types (pictures, sound, video, PDF, text, office files that cannot hold macros) or what your `[[open]]` rules name; any other type asks "Open it?", since the system decides what an unknown type does and some types run code. **o** is the explicit "open with the system": it asks only for what runs. |
| **Names a command line would misread** | Programs get their arguments as a list, never through a shell. On Windows, a batch file (`.bat`, `.cmd`) started in a terminal tab is refused when a file name holds a character `cmd.exe` acts on (`&`, `%`, `^`, `,`, `;`, `=`…), or when its own path has a space (as in `C:\Program Files\nodejs\npm.cmd`). |
| **A same-named program in the folder you browse** | On Linux and macOS a program named without a path (in `[[open]]`, or run in a terminal tab) is looked up only in the absolute directories of `PATH`; `.` and empty entries are skipped. |
| **A folder copied or moved into itself** | Refused before anything is written, however the destination is reached: through a link into the folder, `..`, or a folder there not made yet. A copy onto itself is skipped. Pack leaves out the archive it replaces. |
| **A file that appears while xdevcmd writes** | Renaming (F2, rename many, moving into the Linux trash) is refused by the system if the new name is taken, even a moment earlier. Copy and move write to a free name only if it is still free; otherwise you get the usual overwrite question. Nothing is replaced without asking. |
| **Typing into the wrong program** | Alt+C, Alt+Enter and your own commands type only into a shell at its prompt, never into an editor, a password prompt, `ssh` or another program; when unsure, xdevcmd asks. It never adds to a line you typed and did not run. Known gaps: a prompt built into the shell, such as bash's `read -s`, looks like the shell's own prompt; on Windows, a program that prints a prompt mark can look idle ([shell.md](shell.md#before-anything-is-typed-into-the-shell)). |
| **The wrong folder** | The drive's root, your home, every folder a pane shows or a shell is in, and the folders above them, are never deleted, moved or renamed, however the path is written (`src/..`, `link/`); the engine checks again. Writes wait while a pane is still opening a folder. |
| **A file over a folder, or the reverse** | Never, even with "replace all": skipped, and the job says so. |
| **A config file others can write** | Refused on Unix, because it names the programs xdevcmd runs. |

Dependencies are checked for known vulnerabilities and licences (`cargo deny`) on every
change and weekly. xdevcmd was tested against attacks on names, files and the shell
before release.

## Safe operations

`[general] safe_operations` (Settings → General), **on by default**.

### What you get

| What ends the run | With safe operations on |
|---|---|
| **A kill** (`kill -9`, a crash; on Windows, the terminal's close button too) | No file is ever cut short under its real name: it holds the old file or the complete new one. The next start tidies up and offers to continue. |
| **A power cut** | A file replaced is the old one or the complete new one; a source moved across drives is never gone before its copy is on disk. But a **new** file copied just before the cut can be short or empty under its real name: continue with **C** (below). |

**Covered**: F5 copy, F6 move, Shift+F5 copy as, Alt+Y sync, extracting (F5 inside an
archive, Alt+F9), Alt+F5 pack, and Shift+F2 rename many. **Not covered**, because
nothing can be: Shift+F8 (nothing to keep) and F4 (your editor writes the file).

### Continuing after a kill or a power cut

At start, xdevcmd finds the run that ended, removes its temporary files, puts rename
many's files back under their names, and offers:

- **Continue** (Enter): fast; right after a kill. It skips what is already there and
  identical (same size and date), skips a folder the run had finished without looking
  inside (a file added there since is not copied: the job is the one you started), goes
  on with a big file from its last checkpoint, starts a smaller cut-short file again,
  and asks about any other file already there. A file you chose to keep the first time
  is asked about again, not replaced because its source is newer.
- **C** (check, slow): when a copy may be damaged, as after a power cut or a drive pulled
  out. Every file already copied is read again with its source, finished folders
  included, and one that differs is copied again (the old one to the trash). A Skip
  chosen in the table before the job is kept. Known gap: a "Skip" answered at an
  overwrite question in the run that died is not saved, so C copies that file again if
  it has its source's size and date but other bytes.
- **F** (forget it): nothing more is done. What the run did stays where it is (copied
  files at the destination, a replaced file in the trash), and Ctrl+Z cannot undo it:
  undo does not outlive a run. Any part of a big file kept is removed.
- **Esc**: leave the job for the next start.

With several jobs interrupted, the newest is asked first; the dialog says how many
older ones wait, and each is asked at a later start.

A move continues with the sources still there. A source whose identical copy is already
at the destination (a move across drives stopped between the copy and the removal) is
left where it is, and the summary says so ("copied completely … both are kept"). A
delete continues with what is left.

**The table's decisions survive a kill.** A copy or move from the table (shown before
it starts) writes its decisions to the journal before its first step: which files to
leave, which to replace, and what each one was. Continuing carries them out without
asking again; a file to be replaced that has changed since is asked about.

**A big file goes on from its last checkpoint.** A file over 256 MiB is synced every
256 MiB while it is copied, and the journal says how much is there. The next start keeps
that part (the dialog says "kept part way") and Continue goes on from it, at most
256 MiB back, as long as the source is the same file (size, date and identity);
otherwise it starts again. F (forget it) removes the kept parts with the job; Esc leaves
them for the next start. The cost is one sync per 256 MiB of a big file, none for smaller files.

### How it works

- **A temporary name.** Every copy writes under a hidden name beside its destination,
  `.name.xdevcmd-part-<run>-<n>`, and renames it into place when complete. `<run>` is
  the process id and its start time, so a later process with the same id never takes
  another run's files for its own.
- **A journal per job**, in `journal/` beside `session.toml`: the machine, the job, and
  the directories it makes temporary files in. A job that ends while xdevcmd runs
  (finished, cancelled or failed) removes it.
- **Dead or alive.** A job holds a lock on its journal while it runs (on Windows, the
  file is open with nobody else allowed to write), and the system lets go when the
  process ends, however it ends. At start a lock xdevcmd can take means the run is gone
  and is tidied up; one it cannot take is another window's live job, left alone. Where
  a file system has no locks (some network ones), the process id is asked instead. A
  journal from another machine is always left alone: with a home directory shared over
  NFS, only that machine can tidy up after its run.
- **If the journal cannot be written** (read-only directory, full disk), you are asked
  before the job does anything: **Stop** (default, and Esc) changes nothing; **Continue
  without the journal** runs as with safe operations off, and the summary says so. A
  journal that fails part-way is asked about the same way before the next step that
  cannot be undone, once per job, and is never trusted again. Rename many without its
  journal renames nothing, with **rename anyway** to go on.

### What is synced, and when

Writing a file and renaming it is not enough for a power cut: the system may put the
rename on disk before the data. So, with safe operations on or off:

- **A file replaced** (a copy over an older one, Alt+Y, extract over a file, pack over
  an archive, Save as… over a file): its new data is synced, the file still open,
  before it is renamed over the old one; with safe operations on, its journal line is
  synced in between. A failed sync removes the new file and keeps the old one; Retry
  copies again from the source (on Linux a failed sync can drop the data and clear the
  error).
- **Small replaced files are synced together**: up to 1 MiB each, on a local drive, 64
  files (or 16 MiB, or a second's worth) at a time on several threads, with their
  journal lines in one sync. Each is still renamed only after its own data and journal
  line are on disk; only the waiting is shared. Threads: 16 to start, 8 on macOS; a job
  past 1,000 such files tries 2, 8, 16 and 32 and keeps the fastest for its drive.
  Measured: 5,000 files replaced in 2.9 s instead of 59.7 s on Linux ext4; on a
  Windows 11 laptop the syncs alone were 4.6× faster on 16 threads, up to 8.5× on 32. A
  file whose sync fails is asked about on its own; the rest of its batch goes on.
- **A move across drives** removes a source only once its copy is on disk: data synced,
  then its folder (so the name survives), then the journal line, then the source is
  removed. 64 files (or 64 MiB) at a time: one sync per file made a move of 10,000 small
  files 8 times slower; batched it is about 2.5 times (Linux; about 3 on GitHub's macOS
  machine). A copy that cannot be synced is removed and its source kept.
- **The journal** is synced, with its folder, when a job starts and before each step
  that cannot be undone.
- **Not synced, on purpose**: a new file copied onto the same drive, and the end of a
  job. Syncing every new file would make copying many small files several times slower,
  for a protection only a power cut needs. A later version will sync new files before a
  job ends.

### Replacing a file

- **Only if it is still the file you agreed to.** Right before replacing, xdevcmd looks
  again (size, date, and on Unix which file it is). If another program changed it, it
  asks again ("It changed since you answered"). Alt+Y's plan and the F5/F6 table do the
  same for each file they said they would replace; a file the plan said to leave is
  left, whatever is there by then.
- **The old file goes to the trash first**, where the trash is on the same drive (the
  guide says which drives): the new file is complete and synced under a temporary name,
  the journal records the swap, the old file goes to the trash, the new one is renamed
  in. A failed rename puts the old file back. A kill between the two leaves the name
  empty for a moment; the next start renames the new file in and says so ("1 replacement
  finished … the old one is in the trash").
- **Undo** removes only what is still the file the copy made (size, date and, on Unix,
  which file) and brings the replaced file back from the trash; a folder the copy made
  goes only once empty.
- **"Identical" means size and date agree**, as rsync's and robocopy's quick check; the
  content is not read. Dates must agree to the precision of the less precise drive:
  exactly on ext4, APFS, NTFS and btrfs; to the second on HFS+ and ext3; 10 ms on exFAT;
  within 2 seconds on FAT. The precision is read from the dates themselves. An edit that
  keeps a file's size on a precise drive is never taken for the same file. To compare
  contents, use `rclone check` or `rsync -c`.
- **"Overwrite all" and "Update" are rules**, applied to whatever is there (never a file
  over a folder, or the reverse).
- **What no program can close** without a rename that compares first: another program
  replacing the file in the microseconds between the last look and the rename.

### Who copies the bytes

The plan, the temporary name, the syncs and the rename are always xdevcmd's. With
`[general] fast_copy` on (the default), the system's fast copy moves the data where the
drive allows:

| System | Fast copy |
|---|---|
| Linux | A **clone** first (Btrfs, XFS, bcachefs: shares the source's blocks until either changes, so no data written), else the kernel's `copy_file_range` (NFS 4.2 and SMB servers copy on their side), else read and write |
| macOS | A clone (`fclonefileat`) on APFS, owned by you, else read and write |
| Windows | Read and write |

A step the drive refuses (another file system, an old kernel, a special file) falls
back to read and write where it stopped, without a question; a real error (disk full,
I/O) is the copy's error. The status line says which ran: "120 copied (cloned)",
"5 copied (by the kernel)", or counts when mixed. Turn it off if a file system's cloning
is in doubt (OpenZFS 2.2.0 and 2.2.1 had a block-cloning bug, fixed in 2.2.2).

### Whatever the setting

- A file being **replaced** is always written under a temporary name first, so
  cancelling never loses the old one.
- **Closing the terminal, losing SSH (SIGHUP) or SIGTERM**: jobs are cancelled and
  remove their temporary files (up to two seconds), the session is saved, xdevcmd ends.
  A second signal ends it at once.
- **F10 with jobs running** asks first, cancels them and waits for the clean-up; if one
  is stuck after five seconds (a hung network drive), it asks again.
- **Compare (Alt+K, Alt+Y)** never takes an unreadable folder for an empty one: it is a
  row with ⚠, and nothing under it is copied either way.

### Limits

- **Restarted, not resumed**: a cut-short file up to 256 MiB; a big file copied with
  safe operations off or by a clone (a clone is whole or nothing); extracting and
  packing.
- **Room**: a replacement needs space for the new file until it is renamed over the old.
- **Some network and FUSE file systems** do not allow a rename into place, or do not make
  it atomic. Some cannot sync a folder at all; xdevcmd then takes it as having nothing
  to sync, so a copy's name rests on the server.
- **Clean-up** happens at the next start of xdevcmd on that machine and account; a run
  that never starts again leaves its temporary files. With a shared home directory,
  each machine tidies up only its own runs.
- **Off**: a new file is written under its own name: fastest, but a killed run can leave
  it short.
- **Windows**:
  - A folder cannot be synced without code xdevcmd does not use (no `unsafe`, no extra
    library), so the folder steps are skipped. NTFS journals its names, so a rename is
    there or not after a crash, but the newest renames may be lost; a move across drives
    relies on that.
  - On FAT drives and some network shares, a rename that must not replace anything is a
    check followed by a rename, with a small window.
  - The look before replacing compares size, modification and creation time only (no
    file number without that same code): another file with all three the same is not
    told apart.
  - A leading dot hides nothing: temporary files show until cleaned up. The terminal's
    close button is not caught yet; the next start cleans up instead.
- **Cost**, measured on Linux (2026-11-08): about 9 µs more per file for 10,000 files of
  1 KiB (the rename); none for one large file. On macOS a sync is a full flush of the
  drive's cache (`F_FULLFSYNC`, what Rust's file sync does there): about 0.7 ms per
  small file on GitHub's macOS machines, more on a slow USB stick.

## Safe modes

```bash
xdevcmd --read-only        # no copy, move, delete, rename, edit or save
xdevcmd --restricted       # read-only, and no shell, no editor, no clipboard
xdevcmd --root /srv/data   # never go above /srv/data, links included
```

| | normal | read-only | restricted |
|---|---|---|---|
| copy, move, delete, rename, edit | yes | no | no |
| saves anything (session, bookmarks, File → Settings…, a terminal tab's log) | yes | no | no |
| open files in other programs, run a file as a program | yes | no | no |
| view and browse | yes | yes | yes |
| terminal, clipboard | yes | yes | no |

The read-only modes still write the file named with `--cwd-file`, and a temporary copy
of a file viewed inside an archive ([Files xdevcmd keeps](config.md#files-xdevcmd-keeps)).
In read-only mode, a command you run in the shell panel can change files as anywhere.

The config has the same settings (`[security] mode` and `root`). A command-line flag can
make the config stricter, never looser.

These modes protect against mistakes and hostile files, not against someone who can
start xdevcmd without the flag. For a real boundary, use the operating system's own
sandboxing.

## The shell panel

What xdevcmd learns about the shell, and how far it trusts each source
([shell.md](shell.md#telling-xdevcmd-where-the-shell-is) has the details).

**From the system: trusted.** Linux `/proc`; macOS `/bin/ps` and `/usr/sbin/lsof`, by
full path, never through `PATH` (lsof only when a key asks, given two seconds). If the
answer cannot be read, the shell's state is unknown, never idle: every send asks. Where
the system can say (Linux always; macOS when lsof answers), a directory reported in the
shell's output is never used.

**From the shell's output** (OSC 7, OSC 9;9, OSC 133): any program printing into the
terminal — `cat` on a hostile file, a remote host over `ssh` — can print the same. So:

- **A reported directory only moves a file pane** (Alt+G, `follow`), and only to a
  directory that exists on this machine, inside xdevcmd's root. It is never typed, run
  or written to: typed names are bare only when the system gave the shell's directory,
  and the history's Save as… does not offer it. Another host's is ignored.
- **A "running" mark (`C`)** is believed everywhere, once the shell has sent a prompt
  mark: it can only stop xdevcmd from typing.
- **An "at the prompt" mark** makes a shell idle **only on Windows**, where the system
  does not say; on Linux and macOS the system wins. So on Windows a program that prints
  a prompt mark can make itself look idle. If that matters, set
  `[terminal] send_when_idle = false`: every send asks.

**Paths in the output** (the history's Tab, Enter, F3, F4) are only offered: nothing
opens until you press a key on one, its full path shows first, it must exist on this
machine (`\\server\…` and `//host/…` are never looked up, so a lookup cannot hang), and
it is checked as from a file pane (inside the root; F4 off in the read-only modes). A
directory from the prompt marks only ever resolves a relative path, and is read from the
system where it can be (Linux).

**A list from the output** (`p`) holds only names found in that text. Each row must
exist on this machine and lie inside the root; the list stays in memory; deleting from
it names the list in the confirmation. No list is made while ssh runs.

**The output stays in memory** (Shift+F12, lists), unless you log that tab. **A log
keeps everything the shell prints**: a password typed without echo is not in it, but a
token, a key or customer data a command prints is. New logs are readable by you only
(`0600`; Windows: the folder's permissions); appending keeps an existing file's
permissions, and xdevcmd says so when others can read it. A link, FIFO or device at the
name is refused, and so is another user's file (Unix). Off in the read-only modes;
nothing is logged unless you ask, per tab.

**What is typed** is always quoted for the shell (`safety.rs`): for sh, bash and zsh each
character the shell acts on gets its own backslash, so a name stays text even inside a
quote or here-doc left open on your line. A name with a control character, or one that
shell cannot be given safely ([shell.md](shell.md#before-anything-is-typed-into-the-shell)),
is refused whole. Names are wrapped
in bracketed paste when the shell asks for it, so its line editor takes them as text.
Only `cd` (Alt+C) and your own commands end with Enter.

## The config file is trusted

xdevcmd trusts its config the way your shell trusts `.bashrc`: it names the programs
xdevcmd runs (the shell, the editor, the opener, your `[[open]]` and `[[menu]]`
commands). Keep it yours.

- xdevcmd refuses a config every user can write (Unix), and never reads one from the
  directory you browse.
- Do not point `--config` or `XDEVCMD_CONFIG` at a file someone else can change.

**Settings that run programs**, off until you set them:

- `[[open]]` rules run the program they name when you press Enter on a matching file.
- `[files] git_status = true` runs `git` in each directory you open. A repository's own
  git config can make git run other programs: turn it on only for repositories you
  trust.

## Checking a download

Each release on GitHub has a `SHA256SUMS` file and, from 0.4.0-beta.2 on, build
provenance: a statement signed through GitHub that the archive was built by this
repository's release workflow, from the tag's commit. With the
[GitHub CLI](https://cli.github.com):

```bash
gh attestation verify xdevcmd-v0.4.0-x86_64-unknown-linux-gnu.tar.gz -R nci-gis/rust-commander
```

Without it, compare the archive's SHA-256 with its line in `SHA256SUMS` (`sha256sum`,
`shasum -a 256` on macOS, `Get-FileHash` in PowerShell).

The binaries are **not code-signed** yet: Windows SmartScreen and macOS Gatekeeper warn
the first time ([troubleshooting](troubleshooting.md#macos) says what to do). Before each
release, the release workflow unpacks every archive and checks its files and its size
(at most 7 MiB), then runs its version and its `--print` commands
(`scripts/smoke.sh`). The Intel macOS archive is built and checked on an Apple Silicon
machine, so it is not run there.

## Reporting a problem

A security problem: report it privately, as [SECURITY.md](https://github.com/nci-gis/rust-commander/blob/main/SECURITY.md) explains, not
in a public issue. Anything else:
[open an issue](https://github.com/nci-gis/rust-commander/issues).
