# Security

rcmd shows names and files that other people made, runs a shell, and changes files with
your permissions. It was tested against attacks on all three before release.

## What rcmd guards against

- **File names that try to run commands.** A name with control characters is never
  typed into a shell or put on the clipboard.
- **Files that hang a program.** FIFOs and devices are refused before they are opened.
- **Hostile archives.** Names that point outside the target folder are never written.
  Links inside archives are not created. Set-id bits are dropped. An entry bigger than
  it claims is stopped.
- **Huge images.** A picture claiming billions of pixels is refused before it is
  decoded.
- **Running programs by accident.** Opening a program or script with **o** asks first,
  and Cancel is the default.
- **A config file others can write.** rcmd refuses it on Unix, because it names the
  programs rcmd runs.

No network access, no telemetry. Dependencies are checked for known vulnerabilities and
licences (`cargo deny`) on every change and weekly.

## Safe modes

| | normal | read-only | restricted |
|---|---|---|---|
| copy, move, delete, rename, edit | yes | no | no |
| saves anything (session, bookmarks) | yes | no | no |
| view and browse | yes | yes | yes |
| terminal, clipboard | yes | yes | no |

```bash
rcmd --read-only        # rcmd changes nothing on disk
rcmd --restricted       # read-only, and no shell, no editor, no clipboard
rcmd --root /srv/data   # never go above /srv/data, links included
```

The same settings exist in the config (`[security] mode` and `root`). A command-line
flag can make the config stricter, never looser.

These modes protect against mistakes and hostile files. They do not stop someone who
can start rcmd without the flag. For a real boundary, use the operating system's own
sandboxing.

## Opt-in features that run programs

`[files] git_status = true` runs `git` in each directory you open. A repository's own
git config can make git run other programs. Turn it on only for repositories you trust.

## Reporting a problem

Open an issue at https://github.com/nci-gis/rust-commander/issues. For something that
should not be public yet, say so in the issue title and leave the details out; we will
reply with a private way to share them.
