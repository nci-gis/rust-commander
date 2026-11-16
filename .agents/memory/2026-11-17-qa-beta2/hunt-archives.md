# Hunt: ARCHIVES and FILE NAMES (xdevcmd 0.4.0-beta.1 prebuilt, /tmp/qa/bin/xdevcmd)

Sandbox: /tmp/qa/hunt-archives. Harness: `scripts/run.sh LEFT RIGHT KEYS` = screenshot mode
(110x24) with XDEVCMD_CONFIG/STATE, XDG_DATA_HOME, HOME, TMPDIR all inside the sandbox;
`scripts/tm.sh NAME LEFT RIGHT` = same env in tmux for live runs. Generators:
`scripts/gen.py`, `scripts/gen2.py` (+ inline python shown below). No repo changes.

## Findings

### F1 Alt+F5 pack changes file names: non-UTF-8 bytes become U+FFFD, `\` becomes a folder separator — REPRODUCED, P1
Contradicts guide.md:289-290 ("Kept exactly ... any bytes on Linux, even not UTF-8"); round-trip
of pack→extract is not identical (undocumented for archives; tar can store raw bytes).
```
# t14/src holds raw names b'raw\xff\xfe', b'dir\xffsub/inner/f', b'back\\slash', ... (python os.open)
scripts/run.sh t14/src t14/dst "Ctrl+a,Alt+F5,Enter,wait"                                   # zip
scripts/run.sh t14/src t14/dst "Ctrl+a,Alt+F5,End,Backspace,Backspace,Backspace,t,a,r,Enter,wait"  # tar
tar -tvf t14/dst/src.tar  ->  back/slash ; dir\357\277\275sub ; raw\357\277\275\357\277\275
scripts/run.sh t14/dst t14/rt-zip "Down,Alt+F9,Enter,wait"; diff of trees:
only in src: [b'back\\slash', b'dir\xffsub', ..., b'raw\xff\xfe']
only in rt:  [b'back', b'back/slash', b'dir\xef\xbf\xbdsub', ..., b'raw\xef\xbf\xbd\xef\xbf\xbd']
```
Contents were identical; only names (and the folder shape for `back\slash`) changed.

### F2 Tar entries with distinct non-UTF-8 names collapse into one; one is hidden and can be overwritten — REPRODUCED, P1
Contradicts guide.md:289 ("Kept exactly"). Two entries `a\xff.txt` (FIRST-ff) and `a\xfe.txt`
(SECOND-fe):
```
python: tarfile GNU, names 'a\udcff.txt','a\udcfe.txt' (surrogateescape) -> t13/src/collide.tar
tar -tf collide.tar | od -c   ->  a \377 . t x t \n a \376 . t x t
scripts/run.sh t13/src t13/dst "Down,Enter"            -> listing shows ONE row "a�  txt 8"
scripts/run.sh t13/src t13/dst "Down,Alt+F9"           -> "2 files • 17 bytes"
scripts/run.sh t13/src t13/dst "Down,Alt+F9,Enter,wait" -> "Already exists  t13/dst/a�.txt  on disk 8 incoming 9"
disk: b'a\xef\xbf\xbd.txt' = b'FIRST-ff'   (overwrite / overwrite all would lose FIRST-ff)
```
Same lossy mapping for single names: t12 `bad\xff\xfe.bin` extracted as `bad\xef\xbf\xbd\xef\xbf\xbd.bin`.

### F3 F2 rename of a non-UTF-8 name silently replaces the bytes the user did not touch — REPRODUCED, P2
guide.md:289 ("Kept exactly"); mc keeps the raw bytes when editing.
```
t17/src holds b'a\xff', b'a\xfe'
scripts/run.sh t17/src t17/dst "Down,F2,End,X,Enter"
os.listdir -> b'a\xef\xbf\xbdX'   (was b'a\xff'; expected b'a\xffX')
```
(Enter with the field unchanged did nothing — fine.)

### F4 Extracting drops modification dates — REPRODUCED, P2 (undocumented)
guide.md:265 says a copy keeps file dates; TC/mc/tar keep them on extract. The pane shows the
archive's date inside the archive, and a different one after extracting.
```
python: tar entry old.txt mtime=1000000000 -> t20/src/m.tar
scripts/run.sh t20/src t20/dst "Down,Alt+F9,Enter,wait"
ls -l --time-style=full-iso t20/dst -> old.txt 2026-11-17 04:06:33 (expected 2001-09-09)
Round trip: t14/src/-dashed 04:00:06 -> packed (tar -tv shows 04:00) -> t14/rt-tar/-dashed 04:00:42
```

### F5 Packing only a FIFO/device says "1 packed, 1 skipped" and leaves an empty archive — REPRODUCED, P2
```
mkfifo t16/src/fifo; mknod t16/src/chrdev c 1 3
scripts/run.sh t16/src t16/dst "Down,Down,Space,Alt+F5,Enter,wait" -> "1 packed, 1 skipped ... fifo: not a regular file"
ls -l t16/dst -> fifo.zip 22 bytes (empty zip); same for chrdev.zip
```
Also t14 (12 items incl. 1 symlink): "10 packed, 1 skipped" — counts do not add up to 12.

### F6 Refusal messages are cut in the middle, hiding the reason — REPRODUCED, P2 (misleading)
At 110 columns the line elides the middle of the sentence:
```
t11: "/tmp/qa/hunt-archives/t11/d2: it wou…used as a likely decompression bomb"
t21: "/tmp/qa/hunt-archives/t21/dst: it ne…s 28 G, and only 18 G is free there"
t18: "/tmp/qa/hunt-archives/t18/dst: 10000… files — more than 1000000; refused"
t9:  "/tmp/qa/hunt-archives/t9/d1/big.bin:…than its declared uncompressed size"
```
The path takes the space; the reason ("would unpack to …", "needs …") is lost.

### F7 tar.gz bomb is decompressed fully, twice, before it is refused — REPRODUCED, P2
security.md:28 promise holds (nothing written), but a 3 MB → 3 GB tar.gz:
```
scripts/tm.sh qa t11/src t11/d1 ; Down, Alt+F9 -> dialog shows "counting…" > 20 s (Enter allowed)
Enter -> "scanning… 0 files · 0 of 0" for ~50-60 s, then refused as a bomb (d1 stays 0 bytes)
Enter on bomb.tar.gz itself: ~10-20 s before the listing appears, no progress shown
```
A bigger stream costs CPU time in proportion; it could stop at the 1 GB / 1000x point.
Note for test authors: `--screenshot ... wait` returned while the job was still "scanning…" (~40 s cap).

### F8 Duplicate entry names: tar asks against its own first entry, zip silently keeps the last — REPRODUCED, P2 (undocumented)
```
t7/src/a.tar: dup.txt "first", dup.txt "second-longer"
scripts/run.sh t7/src t7/d2 "Down,Alt+F9,Enter,wait" -> "Already exists t7/d2/dup.txt on disk 5 incoming 13"
t7/src/b.zip: same two entries
scripts/run.sh t7/src t7/d3 "Down,Down,Alt+F9,Enter,wait" -> no question; dup.txt = "second-longer"; listing shows one dup.txt
```

### F9 "Already exists" for a symlink shows the link's own size, the pane shows the target's — REPRODUCED, P2
```
t5/dst/file.txt -> ../outside/target.txt (5 bytes)
scripts/run.sh t5/src t5/dst "Down,Alt+F9,Enter,wait" -> "on disk 43 / incoming 3"; pane row "file txt 5"
```

### F10 F3 on a large entry inside an archive: no feedback, keys held until extraction ends — REPRODUCED, P2
```
t15/src/two.tar.gz (m250.bin 250 MB, m500.bin 500 MB, zeros)
tmux: Enter on archive, Down, F3 on m250.bin, Esc after 2.4 s -> 230 MB already in $TMPDIR/xdevcmd-run-*/view-0,
  status line unchanged (no progress), the file is then removed (held).
F3 then F10 at once -> xdevcmd exits ~8 s later (after extraction), temp dir removed (held).
m500.bin / 900 MB / 3 GB: "too big to view from inside — F5 extracts it" — the limit is not in the docs
  (config.md:356); for the 3 GB bomb entry F5 is then refused, so the hint leads to a refusal.
```

## What held (all REPRODUCED, disk checked)
- Traversal entries never written (zip and tar): `../evil1`, absolute `/tmp/qa/hunt-archives/outside_abs*`,
  `a/../../evil2`, `..\evil3` — hidden from the listing with a status note ("4 entries with names
  outside the archive are hidden and will not be extracted"). `find` for evil*/outside_abs* on / : none.
- Symlink entries not created ("links in archives are not extracted"); `link/x` after `link -> ../outside`
  lands in `<dst>/link/x` as a real dir; t3/outside stays empty (F5 inside archive and Alt+F9).
- Hard-link entries (`hl -> ../victim`, absolute) not created; t4/victim unchanged.
- Pre-existing symlink in target at a file's name: replaced by a regular file, target.txt still "orig";
  at a directory's name: "…link in the destination — not followed", nothing written through it.
- Set-id/sticky bits dropped (tar 06777 -> 0777, 01777 -> 0777, dir 02777 -> 0755; zip 04755 -> 0755).
- Entry bigger than it claims (deflated and stored zip): stopped, no partial file left. Truncated tar: error, nothing left.
- Declared-size bomb (3 × 2 GB in 319 B), honest zip bomb (2 GB in 2 MB), tar.gz bomb (3 GB in 3 MB),
  "would not fit" (28 G declared, 18 G free), 1,000,001 entries: all refused, nothing written.
- FIFO / char / block entries in a tar: "not a regular file", skipped.
- FIFO and /dev/null-like device on disk: F3, Enter, F5, Alt+F5 refuse at once, no hang.
- Archives never modified: sha256sum of every source archive identical before/after.
- No write outside targets: `find -newer marker` shows only the chosen destination folders.
- Names with ESC/OSC/CSI/CR/TAB/DEL/newline/U+202E shown without control chars in screenshot and in a
  real tmux pane; pane title unchanged ("vm"), no clear-screen sequence reached the terminal; extracted
  names keep their bytes (b'esc\x1b]0;PWN\x07title.txt', b'nl\nname.txt', ...).
- Copy and delete act on the exact file among lookalikes (`a\xff`/`a\xfe`, `nl\nfile`/`nlfile`,
  `esc\x1b[2Jx`/`esc[2Jx`): bytes and trash entry checked.
- F3 temp copy removed on Esc and on quit; `xdevcmd-run-<pid>` removed at F10 (TMPDIR honoured).
- Nested archive: Enter on a zip inside a zip views it as bytes; Alt+F9 there says "not that".
