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
- **Typing into the wrong program.** Alt+C and the user menu type a line into the
  terminal only when a shell is at its prompt there, never into a program that owns
  it — an editor, a password prompt, a program run in a terminal tab.
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

**What it guarantees.** However xdevcmd ends — a crash, `kill -9`, a power cut — no file
is ever left cut short under its real name: the real name holds either the old file
or the complete new one. The next start finds the journal of the run that ended, removes
its temporary files, puts rename many's files back under their names, and offers to
continue the job. Continuing skips what is already there and identical (same size and
date: what the first run finished), starts again the file that was cut short, and asks
about any other file already there, never deciding for you: a file you chose to keep
the first time is asked about again, not replaced because its source is newer. A move
continues with the sources still there; a source whose identical copy is already at the
destination (a move across drives stopped between the copy and the removal) is left
where it is, and the summary says so. A delete continues with what is left.

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

## Safe modes

| | normal | read-only | restricted |
|---|---|---|---|
| copy, move, delete, rename, edit | yes | no | no |
| saves anything (session, bookmarks, File → Settings…) | yes | no | no |
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
