# Docs

> **Read this when**: writing or editing `docs/`, the README or the CHANGELOG.

---

The owner's values behind every page: safety and security, no over-promising, tidy
docs, convenience, fair to other tools.

## Writing

- **Plain English, short sentences.** Say what happens, not how it is built. Bold the
  thing, then the effect: "**`q` no longer quits**: F10 and Ctrl+q do."
- **One place per fact.** Steps live in one page; others link to it
  (`[Benchmarks](benchmark.md)`). Moving text means deleting the old copy.
- **No over-promising.** Say what is tested and where (the README's Platforms table);
  say what is not done and what to use instead (rsync, robocopy, rclone). Numbers come
  with their conditions (Linux, warm cache, 5,000 files).
- **Safety first in steps.** Never tell a reader to turn off antivirus or a security
  feature; a command that unblocks a download says "only for a file from this
  repository". Say what a tool never touches, and that nothing is sent.
- **Commands** in fenced blocks, one shell per block (`bash`, `powershell`), runnable as
  they are; placeholders that look like placeholders (`STICK`, `mbp-m2-apfs`).

## The CHANGELOG

- One or two lines a bullet: what a user notices; detail is left to the docs.
- Most important first in each group (Added, Changed, Fixed).
- Released sections are never rewritten.

## Pages that ship

- `docs/*.md` go into the release archives: no links to `.agents/` or other private
  files, and no dates that go stale in a user's page.
- A new page goes into `docs/README.md`'s list; check every `#anchor` it links to.
- `docs/keys.md` is generated (`xdevcmd --print-keys`); see development.md, "Docs that
  are generated".
