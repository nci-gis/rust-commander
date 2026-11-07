# Security

xdevcmd shows names and files that other people made, runs a shell, and changes files with
your permissions. It was tested against attacks on all three before release.

## What xdevcmd guards against

- **File names that try to run commands.** A name with control characters is never
  typed into a shell or put on the clipboard.
- **Files that hang a program.** FIFOs and devices are refused before they are opened.
- **Hostile archives.** Names that point outside the target folder are never written.
  Links inside archives are not created. Set-id bits are dropped. An entry bigger than
  it claims is stopped.
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
- **Typing into the wrong program.** Alt+C and the user menu type a line into the
  terminal only when a shell is at its prompt there, never into a program that owns
  it — an editor, a password prompt, a program run in a terminal tab.
- **A config file others can write.** xdevcmd refuses it on Unix, because it names the
  programs xdevcmd runs.

No network access, no telemetry. Dependencies are checked for known vulnerabilities and
licences (`cargo deny`) on every change and weekly.

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
