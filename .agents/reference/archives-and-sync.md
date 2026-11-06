# Archives, compare and synchronise

> **Read this when**: touching archive browsing, extraction or packing, `archive.rs`, `compare.rs`, or the sync plan.

---

## Archives

**Browsed as directories.** Enter on a `.zip` (and `.jar`, `.docx`… — they are zips),
`.tar`, `.tar.gz` or `.tgz` lists it; `cwd` stays the directory holding it, and each row's
path is `archive/inside/name`, so marks and the selection need nothing special.
`ArchiveView::inner` turns those back into names inside. Only reading and extracting
happen there: F8, F6, F2, F7, F4 and the other writes say so, an archive pane is never a
copy destination, and drags into or out of one are refused — rewriting an archive to add
one file is not something rcmd does.

**F5 inside extracts** through the copy engine (`Kind::Extract`), so it has the same
progress, overwrite question and cancel. The directory being browsed is taken off the
front: F5 on `proj/src/main.rs` while inside `proj/src` writes `main.rs`, not
`proj/src/main.rs`. **F3 inside** extracts that one entry (up to 256 MiB) into a fresh
owner-only temporary directory, removed when the viewer closes. **Alt+F5** packs
(`Kind::Pack`) into a name whose extension picks the format, written as `.part` and
renamed at the end, so a cancelled pack leaves nothing; **Alt+F9** unpacks everything.

**Hostile archives** — every rule here is a test in `archive.rs`, and the zip-slip one
also end to end in `render.rs` with a zip written byte by byte, independently of the
code under test:

| | |
|---|---|
| `../x`, `/etc/x`, `C:x`, `a\..\..\x` | hidden from the listing, counted in a notice, never written |
| a symlink already in the destination | not followed: the entry fails, nothing lands outside |
| symlinks and hard links in the archive | listed, skipped on extraction, reported |
| SUID, SGID, sticky | dropped; `rwx` kept |
| an entry larger than its header says | stopped one byte past, the partial file removed |

**`zip` 9 is the floor**, and why the workspace's `rust-version` is 1.88: `cargo deny`
rejected zip 7 for RUSTSEC-2026-0367, -0368, -0369 (set-id bits and symlinks escaping on
extraction) and -0429 (abort on a crafted symlink). rcmd does not call the crate's
`extract()` that those are about, but a known-vulnerable version is not shipped either way.

## Compare and synchronise

**Alt+K** compares this pane with the other one level deep and marks, on each side, what
is missing on the other or newer than the other's copy — so F5 from either pane copies
exactly that, with the usual questions. **Alt+Y** compares the whole trees and shows a
plan, `→` this pane to the other, `←` back, each overwrite named; Enter runs it as one
copy job (`Request::sync`, pairs of source and destination directory). Because every
overwrite is in the plan that was confirmed, the job does not ask again — the one place
`overwrite_confirmed` is set.

Size and modification time, with two seconds of slack (FAT and many shares round to
two); content is not read. **Same time, different size is a conflict** and is never
copied either way: there is no telling which is right. A directory on one side only is
one entry, copied whole.
