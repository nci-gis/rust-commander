# Security

xdevcmd shows names and files that other people made, runs a shell, and changes files with
your permissions. It was tested against attacks on all three before release.

## What xdevcmd guards against

- **File names that try to run commands.** A name with control characters is never
  typed into a shell or put on the clipboard.
- **Files that hang a program.** FIFOs and devices are refused before they are opened.
- **Hostile archives.** Names that point outside the target folder are never written,
  and neither is a link already at a file's name followed (it is replaced). Links
  inside archives are not created. Set-id bits are dropped. An entry bigger than it
  claims is stopped. An archive that would not fit, or would unpack to over 1 GB at
  more than 1000 times its own size, or to more than a million files, is refused.
- **Losing a file half-way.** A file being replaced is written under a temporary name
  and swapped in once complete. A move across drives removes each source only after
  its own copy is done, so what you skipped stays. Shift+F8 needs **Y**, not Enter.
- **Huge images.** A picture claiming billions of pixels is refused before it is
  decoded.
- **Running programs by accident.** Opening a program, a script, a launcher, an
  installer or a disk image with **Enter** or **o** asks "Run it?" first, and Cancel
  is the default. Enter opens without asking only known document types (pictures,
  sound, video, PDF, text, and office files that cannot hold macros) or what your
  `[[open]]` rules name; for any other type it asks "Open it?", since what the system
  does with an unknown type is its choice, and some types run code. **o** is the
  explicit "open with the system": it asks only for what runs.
- **File names that a command line would misread.** Programs are started with their
  arguments as a list, never through a shell. On Windows, a batch file (`.bat`, `.cmd`)
  started in a terminal tab is refused when a file name holds a character `cmd.exe`
  acts on (`&`, `%`, `^`, `,`, `;`, `=`…), or when there is a space anywhere in the
  batch file's path (as in `C:\Program Files\nodejs\npm.cmd`), which `cmd.exe` can
  misread.
- **A program of the same name in the directory you browse.** On Linux and macOS, a
  program named without a path (in an `[[open]]` rule, or run in a terminal tab) is
  looked up only in the absolute directories of `PATH`: a `.` or an empty entry, which
  would mean the directory being browsed, is skipped.
- **A file that appears while xdevcmd writes.** Renaming (F2, renaming many, moving
  into the Linux trash) is refused by the system itself if the new name is taken, even
  by a file created a moment earlier. Copying and moving write to a name found free
  only if it is still free; a file that appeared in between gets the usual overwrite
  question. Nothing is replaced without asking.
- **Typing into the wrong program.** Alt+C, typing names (Alt+Enter) and the user menu
  type into the terminal only when a shell is at its prompt there, never into a program
  that owns it — an editor, a password prompt, `ssh`, a program run in a terminal tab.
  When xdevcmd cannot tell, it asks first (see [The shell panel](#the-shell-panel)).
- **A config file others can write.** xdevcmd refuses it on Unix, because it names the
  programs xdevcmd runs.

No network access, no telemetry. Dependencies are checked for known vulnerabilities and
licences (`cargo deny`) on every change and weekly.

## Safe operations

**What it is.** A setting, `[general] safe_operations` (Settings → General), on by
default. With it, every copy writes the new file under a hidden temporary name beside
its destination, `.name.xdevcmd-part-<run>-<n>`, and renames it into place only when it
is complete; `<run>` is the process id and its start time, so a later process given
the same id never takes another run's files for its own. Each job also keeps a small
journal in xdevcmd's state directory (beside `session.toml`, in `journal/`): the
machine, what the job is, and which directories it makes temporary files in. A job
that ends while xdevcmd runs, finished, cancelled or failed, removes its journal.

**How a dead run is told from a live one.** A job holds a lock on its journal for as
long as it runs (on Windows, the journal is open with nobody else allowed to write to
it), and the system lets go of it when the process ends, however it ends. At start-up
xdevcmd tries each journal's lock: one it can take belongs to a run that is gone, and is
tidied up; one it cannot is a job still running (another xdevcmd window) and is left
alone. Where the file system has no locks (some network ones), the process id is
asked instead. A journal written on another machine is always left alone: with a home
directory shared over NFS, its job may be running there, and only that machine can
tidy up after it.

**What it covers.** F5 copy, F6 move (within a drive a move is a rename; across drives
it is a copy, then the source removed once its copy is complete), Shift+F5 copy as,
Alt+Y sync, extracting (F5 inside an archive, Alt+F9), Alt+F5 pack, and Shift+F2 rename
many, whose files spend a moment under temporary names. Not covered, because nothing
can be: Shift+F8 (there is nothing to keep), and F4 (your editor writes the file).

**What it guarantees.** However xdevcmd itself ends — a crash, `kill -9` — no file is
ever left cut short under its real name: the real name holds either the old file or
the complete new one. A power cut is different, and is below ("What survives a power
cut"). The next start finds the journal of the run that ended, removes
its temporary files, puts rename many's files back under their names, and offers to
continue the job. Continuing skips what is already there and identical (same size and
date: what the first run finished), starts again the file that was cut short, and asks
about any other file already there, never deciding for you: a file you chose to keep
the first time is asked about again, not replaced because its source is newer. A move
continues with the sources still there; a source whose identical copy is already at the
destination (a move across drives stopped between the copy and the removal) is left
where it is, and the summary says so. A delete continues with what is left.

**When the journal cannot be written.** A journal that cannot be made (its directory
read-only, the disk full) is asked about before the job does anything: **Stop** (the
default, and Esc) leaves everything as it is; **Continue without the journal** runs the
job as with safe operations off, and its summary says so. A journal that fails part-way
is asked about the same way before the next step that cannot be undone (a file
replaced, a source removed), once per job; a journal that failed once is never trusted
again, even if a later line would go through. Rename many without its journal renames
nothing and says why, with **rename anyway** to go on without it.

**What is synced, and when** (Round 029). Writing a file and renaming it is not enough
for a power cut: the system may put the rename on disk before the data. So, with safe
operations on or off:
- **A file replaced** (a copy over an older one, Alt+Y, extract over a file, pack over
  an archive, Save as… over a file) has its new data synced to the disk, the file still
  open, before it is renamed over the old one; with safe operations on, the journal's
  line for it is synced in between. A sync that fails removes the new file and keeps
  the old one; Retry copies it again from its source rather than trusting a second sync
  of the same file (on Linux a failed sync can drop the data and clear the error).
  Small files (up to 1 MiB) replaced on a local drive are synced **together**, up to
  64 files (or 16 MiB, or a second's worth) at a time, on several threads at once, and
  their journal lines in one sync. Each file is still renamed only after its own data
  and its journal line are on disk; only the waiting is shared (Round 033).
  - The threads: 16 to start, 8 on macOS. A job past 1,000 such files tries 2, 8, 16
    and 32 on its next batches and keeps the fastest for its drive.
  - Measured: 5,000 files replaced in 2.9 s instead of 59.7 s on Linux's ext4. On a
    Windows 11 laptop the syncs alone were 4.6× faster on 16 threads, up to 8.5× on 32.
  - A file whose sync fails in a batch is asked about on its own, as above; the rest of
    its batch goes on.
- **A move across drives** removes a source only once its copy is on disk: the copy's
  data synced, then its folder (so its name survives too), then the journal's line
  saying both exist, and only then the source. This is done 64 files (or 64 MiB) at a
  time: one sync per file cost a move of 10,000 small files eight times its time onto a
  disk, batched it is about two and a half (measured on Linux; about three on GitHub's
  macOS machine). A copy that cannot be synced is removed and its source kept.
- **The journal** is synced, with its folder, when a job starts, and before each step
  that cannot be undone.
- **Not synced, on purpose:** a new file copied onto the same drive, and the end of a
  job. Syncing every new file would make a copy of many small files several times
  slower, for a protection only a power cut needs.

**Who moves the bytes** (item 22, Round 033). The plan, the temporary name, the syncs
and the rename above are xdevcmd's, whatever copies the data in between. With
`[general] fast_copy` on (the default), the system's own fast copy does that part where
the drive allows it:
- **Linux**: a **clone** first (Btrfs, XFS, bcachefs: the copy shares the source's
  blocks until either changes, so no data is written and no space used), else the
  kernel's `copy_file_range` (the data does not pass through xdevcmd; NFS 4.2 and SMB
  servers copy on their side), else xdevcmd reads and writes as before.
- **macOS**: a clone (`fclonefileat`) on APFS, owned by you, else read and write.
- **Windows**: read and write, as before.

A step the drive refuses (another file system, an old kernel, a special file) falls back
to read and write where it stopped, without a question; a real error (disk full, I/O) is
the copy's error as before. The status line at the end says which ran: "120 copied
(cloned)", "5 copied (by the kernel)", or counts when mixed. Turn it off if a file
system's cloning is in doubt (OpenZFS 2.2.0 and 2.2.1 had a block-cloning bug, fixed in
2.2.2): xdevcmd then always reads and writes.

**What survives a kill, and what survives a power cut.**
- A **kill** (`kill -9`, a crash, the terminal closed): with safe operations on, never a
  short file under a real name; the next start tidies up and offers to continue. A move
  across drives killed after a copy was synced and before its source was removed says
  so at the next start ("copied completely … both are kept"), and removes neither.
- A **power cut**: a file replaced is the old one or the complete new one; a source moved
  across drives is never gone before its copy is on disk. But a **new** file copied
  just before the cut (not synced, by the rule above) can be short or empty under its
  real name, and continuing the job takes a file of the right size and date for done
  without reading it. If the power went during a big copy of new files, check them (or
  copy again with "replace all"). A later version will copy again what a run that died
  in a power cut had made itself.

**Replaced only if it is still the file you agreed to.** The answer to "Already exists"
is about the file it showed: right before replacing it, xdevcmd looks again (size,
date, and on Unix which file it is), and if another program changed or replaced it
meanwhile, asks again ("It changed since you answered"). Alt+Y's plan, and F5/F6's
(the table before a copy or move starts), do the same for each file they said they
would replace; a file the plan said to leave is left, whatever is there by then.

**A file replaced goes to the trash first** (Round 031), where the trash is on the
same drive (see the guide for which drives): the new file is complete and synced under
a temporary name, the journal says which temporary file replaces which, the old file
goes to the trash, and the new one is renamed in. A rename that fails then puts the old
file back. A kill between the trash and the rename leaves nothing under the name for a
moment; the next start renames the complete new file in and says so ("1 replacement
finished … the old one is in the trash"). Undoing a copy removes only what is still the
file it made (size, date and, on Unix, which file it is) and brings the replaced file
back from the trash; a folder the copy made goes only once empty.

**The plan's decisions survive a kill.** With safe operations on, a copy or move from
the table writes its decisions to the journal before its first step (which files to
leave, which to replace and what each one was), and marks each source as it finishes
it. Continuing it after a kill leaves the finished sources and carries out the same
decisions without asking again; a file to be replaced that has changed since is asked
about, as above. Files are "identical" when size and date agree (within a FAT drive's
two seconds), as rsync's and robocopy's quick check: the content is not read. "Overwrite all" and "Update" are rules, applied to whatever is there. What no
program can close without a rename that compares first: another program replacing the
file in the microseconds between that last look and the rename.

**Windows.** A folder cannot be synced on Windows without code xdevcmd does not use
(no `unsafe`, no extra library), so the folder steps above are skipped there. NTFS
journals its names, so a rename is either there or not after a crash, but the newest
renames may be lost: a move across drives on Windows relies on that. On FAT drives and
some network shares a rename that must not replace anything is a check followed by a
rename, with the same kind of small window.
Also on Windows, the look at a file before replacing it compares size, modification
time and creation time only (there is no file number to compare without that same
code): another file put there with all three the same is not told apart.

**Network drives.** Some network file systems cannot sync a folder at all; xdevcmd then
takes the folder as having nothing to sync, so on such a drive a copy's name rests on
the server, as a moved file's name did before.

**What happens whatever the setting.** A file being *replaced* is always written under
a temporary name first, so cancelling never loses the old one. Closing the terminal or
losing SSH (SIGHUP), or SIGTERM, makes xdevcmd cancel its jobs, let them remove their
temporary files (up to two seconds), save the session and end; a second signal ends it
at once. F10 with jobs running asks first, cancels them and waits until they have
cleaned up; if one is stuck after five seconds (a hung network drive) it asks again.

**Limits.**
- Nothing is resumed from the middle of a file: a file cut short starts again.
  Extracting and packing are not resumed; they are started again.
- A replacement needs room for the new file until it is renamed over the old one.
- Some network and FUSE file systems do not allow a rename into place, or do not make it
  atomic.
- On Windows a leading dot hides nothing: temporary files show until cleaned up, and the
  terminal's close button is not caught yet (the next start cleans up instead).
- Temporary files are cleaned up by the next start of xdevcmd on that machine and
  account; a run that never starts again leaves them. With a home directory shared
  between machines, each machine tidies up only after its own runs.
- Off, a new file is written under its own name, as before: fastest, but a killed run
  can leave it short.
- Measured on Linux (2026-11-08): copying 10,000 files of 1 KiB took about 9 µs more
  per file (the rename); one large file, no difference.
- macOS: a sync there is a full flush of the drive's own cache (`F_FULLFSYNC`, which is
  what Rust's file sync does on Apple systems); about 0.7 ms per small file on GitHub's
  macOS machines, more on a slow USB stick.
- Compare (Alt+K, Alt+Y) never takes a folder it could not read for an empty one: such
  a folder is a row with ⚠, and nothing under it is copied either way.

## Safe modes

| | normal | read-only | restricted |
|---|---|---|---|
| copy, move, delete, rename, edit | yes | no | no |
| saves anything (session, bookmarks, File → Settings…, a terminal tab's log) | yes | no | no |
| open files in other programs, run a file as a program | yes | no | no |
| view and browse | yes | yes | yes |
| terminal, clipboard | yes | yes | no |

```bash
xdevcmd --read-only        # xdevcmd changes nothing on disk
xdevcmd --restricted       # read-only, and no shell, no editor, no clipboard
xdevcmd --root /srv/data   # never go above /srv/data, links included
```

The same settings exist in the config (`[security] mode` and `root`). A command-line
flag can make the config stricter, never looser.

These modes protect against mistakes and hostile files. They do not stop someone who
can start xdevcmd without the flag. For a real boundary, use the operating system's own
sandboxing.

## The shell panel

What xdevcmd learns about the shell, and how far it trusts each source
([shell.md](shell.md#telling-xdevcmd-where-the-shell-is) has the details):

- **From the system** (Linux `/proc`, macOS `/bin/ps`, and `/usr/sbin/lsof`, both by
  their full paths, never through `PATH`; lsof only when a key asks, given two seconds):
  trusted. When the system's answer cannot be read, the shell's state is unknown, never
  idle: every send asks. On these
  systems a directory reported in the shell's output is never used where the system can
  say (Linux always; macOS when lsof answers).
- **From the shell's output** (OSC 7, OSC 9;9, OSC 133 marks): any program printing
  into the terminal — `cat` on a hostile file, a remote host over `ssh` — can print the
  same sequences. So:
  - A reported directory only ever **moves a file pane** (Alt+G, `follow`), and only to
    a directory that exists on this machine and lies inside the root xdevcmd is confined
    to. It is never typed, run or written to: typed names are bare only when the
    system gave the shell's directory (full paths otherwise), and the history's Save
    as… does not offer it. A host other than this machine's is ignored.
  - A mark that says "running" (`C`) is believed everywhere, once the shell has sent a
    prompt mark: it can only stop xdevcmd from typing. A mark that says "at the prompt" makes a shell idle **only on Windows**,
    where the system does not say; on Linux and macOS a program the system sees always
    wins. On Windows, then, a program that prints a prompt mark can make itself look
    idle. If that matters to you there, set `[terminal] send_when_idle = false`, and
    every send asks.
- **Paths printed in the shell's output** (the history's Tab, Enter, F3, F4) are found
  in text any program can print. They are only offered: nothing opens until a key is
  pressed on one, its full path is shown first, it must exist on this machine (paths
  on another, `\\server\…` or `//host/…`, are never looked up, so a lookup cannot
  hang), and it is checked as from a file pane: inside the root, and F4 off in the
  read-only modes. A directory taken from the prompt marks only ever resolves a
  relative path; it is read from the system where it can be (Linux), at the prompt.
- **A list from the output** (`p` in the history) holds only names found in that
  text, which any program can print. Each row must exist on this machine and lie inside
  the root; the list stays in memory; and deleting from a list says, in the
  confirmation, which list the names came from. No list is made while ssh runs in the
  shell, whose output names another machine's files.
- **The shell's output stays in memory** (Shift+F12, lists), never written to disk,
  unless you turn on a log for that tab (its menu's Log to file…). A log keeps
  **everything the shell prints**: a password typed at a prompt that does not echo it
  is not in it, but a token, a key or customer data that a command prints is. New log
  files are created readable by you only (`0600`; Windows: the folder's permissions);
  appending to an existing file keeps its permissions, and xdevcmd says so when others
  can read it. A link, FIFO or device at the chosen name is refused, and so is a file
  another user owns (Unix). Off in the read-only modes. Nothing is logged unless you ask, per tab.
- **What is typed** is always quoted for the shell by `safety.rs`; a name with a control
  character, or one that shell cannot be given safely, is refused whole. Names are
  wrapped in bracketed paste when the shell asked for it, so its line editor takes them
  as text, not keys. Nothing xdevcmd types ends with Enter except `cd` (Alt+C) and the
  user menu's commands.

## The config file is trusted

xdevcmd trusts its config file the way your shell trusts `.bashrc`: it names programs
xdevcmd runs (the shell, the editor, the opener, your `[[open]]` and `[[menu]]`
commands). Keep it yours. xdevcmd refuses one that every user can write (on Unix), and
never reads a config from the directory you are browsing — but do not point `--config`
or `XDEVCMD_CONFIG` at a file someone else can change.

## Opt-in features that run programs

`[[open]]` rules run the program they name when you press Enter on a matching file.

`[files] git_status = true` runs `git` in each directory you open. A repository's own
git config can make git run other programs. Turn it on only for repositories you trust.

## Reporting a problem

A security problem: report it privately, as [SECURITY.md](../SECURITY.md) explains —
not in a public issue. Anything else: open an issue at
https://github.com/nci-gis/rust-commander/issues.
