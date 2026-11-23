# hunt-delete: delete, trash, undo, protected folders, rename

Build: /tmp/qa/bin/xdevcmd 0.4.0-beta.1. Sandbox: /tmp/qa/hunt-delete (SB). Every run used
XDEVCMD_CONFIG=$SB/none.toml, XDEVCMD_STATE=$SB/st-*/session.toml, XDG_DATA_HOME=$SB/data, HOME=$SB/home.
Helpers: `$SB/bin/x KEYS LEFT RIGHT` (screenshot mode, 110x24, --no-restore), `$SB/bin/start NAME L R` +
`$SB/bin/k NAME keys...` (tmux, EDITOR=$SB/bin/ed.sh, which runs $SB/edscript on the file).
The test machine runs as root, so permission-based failures were not testable.

## Findings

### F1. F6 (same-drive move) that replaces a file deletes the old file for good; it never goes to the trash — REPRODUCED, P0
Contradicts guide.md:215-216 ("A file replaced goes to the trash ... (or restore it from the trash by hand
after a move)"), security.md:154 ("The old file goes to the trash first"), and the F6 dialog itself, which
says "ℹ a file replaced goes to the trash". F5 does trash it (same setup, same trash): only F6 loses it.
Both table "replace" and the during-job "[O] overwrite" lose it. Ctrl+Z moves the new file back and leaves
the destination name empty, so the old file is gone for good.
```
SB=/tmp/qa/hunt-delete; cd $SB; mkdir -p r4/a r4/b; echo "source new" > r4/a/x; echo old > r4/b/x; touch -d 2020-01-01 r4/b/x
bin/x "Down,F6,Space,Enter,wait" r4/a r4/b      # table shows "ℹ 1 would replace an older file [replace]"; status "1 moved"
cat r4/b/x            # source new
find data -type f     # nothing: "old" is not in the trash (with F5 instead: data/Trash/files/x + x.trashinfo)
grep -rl '^old$' $SB  # no match
# with Ctrl+Z:  bin/x "Down,F6,Space,Enter,wait,Ctrl+Z,Enter,wait" r3/a r3/b  -> r3/a/x back, r3/b/x missing, trash empty
# ask-each path: bin/x "Down,F6,Space,Space,Enter,wait,o,wait" r6/a r6/b -> same loss
```

### F2. Ctrl+Z refuses to undo a rename-many that swapped or chained names ("a1.txt exists again") — REPRODUCED, P1
Contradicts guide.md:223 ("Undoes the last rename (one or many)"). Nothing changed on disk after the
rename; the "name taken" is a name from the same batch. Rename-many itself handles swaps and chains correctly.
```
mkdir -p rm3/a rm3/b; for i in 1 2 3; do echo c$i > rm3/a/$i.txt; done
bin/start s6 $SB/rm3/a $SB/rm3/b; bin/k s6 C-a S-F2 BSpace BSpace BSpace; tmux send-keys -t s6 -l '[C2+1:1]'
bin/k s6 Enter      # renamed 3: 1->2, 2->3, 3->4 (correct)
bin/k s6 C-z        # status "2.txt exists again"; no Undo dialog; files stay 2,3,4
# swap: F4 with an editor that swaps lines 1 and 2 (a1.txt<->a2.txt): rename OK, Ctrl+Z -> "a1.txt exists again"
```

### F3. F6 of a folder onto a same-named folder on the same drive always fails ("Directory not empty") — REPRODUCED, P1
The F6 table offers "replace" for the file inside; the job then asks "Already exists" about the folder itself
([O] overwrite …), and O ends with "0 moved, 1 failed  …/b/d: Directory not empty (os error 39)". It fails the
same way with no conflict at all (only a new file inside). F5 merges the same folders fine. Dead end for a
TC/MC user (undocumented; both merge). Nothing is lost.
```
mkdir -p r7/a/d r7/b/d; echo n > r7/a/d/new; echo k > r7/b/d/k
bin/x "Down,F6,Enter,wait,o,wait" r7/a r7/b   # "0 moved, 1 failed ... Directory not empty (os error 39)"
```

### F4. F2 / Shift+F2 on a non-UTF-8 name replaces the invalid byte with U+FFFD on disk — REPRODUCED, P1
Contradicts guide.md:289-290 ("Kept exactly ... any bytes on Linux, even not UTF-8"). The user only adds "_";
the 0xFF byte they never touched becomes EF BF BD. Ctrl+Z restores the original bytes. (The F4 list editor
leaves an unedited invalid name alone; correct.)
```
mkdir -p rm7/a rm7/b; printf X > "rm7/a/$(printf 'bad\xff.dat')"
bin/x "Down,F2,End,Left,Left,Left,Left,_,Enter,wait" rm7/a rm7/b; ls rm7/a | od -c   # b a d 357 277 275 _ . d a t
mkdir -p rm6/a rm6/b; printf X > "rm6/a/$(printf 'bad\xff.dat')"
bin/x "Down,Shift+F2,End,_,x,Enter,wait" rm6/a rm6/b; ls rm6/a | od -c            # b a d 357 277 275 _ x . d a t
```

### F5. Undo of a move when the source name is taken again gives a false reason — REPRODUCED, P1 (misleading message)
The status says "nothing of that move is where it went", but the moved file is still exactly where it
went; the real reason is that the old name is taken (guide.md:226). Undo of a delete in the same situation
says the right thing ("f1 exists again where it was"). Nothing is lost.
```
mkdir -p um/a um/b; echo moved > um/a/m; bin/start s8 $SB/um/a $SB/um/b; bin/k s8 Down F6 Enter
echo NEWHERE > um/a/m; bin/k s8 C-z     # "nothing of that move is where it went"; um/b/m is untouched
```

### F6. Undo of a delete puts back whatever file is now in the trash under that trash name — REPRODUCED, P2
If the trash entry was emptied and another program then trashed a different file with the same name,
Ctrl+Z moves that other file (its .trashinfo says Path=$SB/other/zz8) to the old location and drops its
.trashinfo. Contrived, but the undo did not check that the trash entry is the one it made (guide.md:226
"It checks the disk first").
```
mkdir -p u2/a u2/b; echo orig2 > u2/a/zz8; bin/start s7 $SB/u2/a $SB/u2/b; bin/k s7 Down F8 Enter
rm data/Trash/files/zz8 data/Trash/info/zz8.trashinfo; mkdir -p other; echo OTHERPROG > other/zz8; mv other/zz8 data/Trash/files/zz8
printf '[Trash Info]\nPath=%s/other/zz8\nDeletionDate=2026-11-17T04:10:00\n' $SB > data/Trash/info/zz8.trashinfo
bin/k s7 C-z Enter; cat u2/a/zz8        # OTHERPROG; its trashinfo is gone
```

### F7. A folder that a program inside the shell is in (a nested shell, an editor) is not protected — REPRODUCED, P2
security.md:36: "every folder ... a shell is in ... never deleted, moved". Only the top shell's folder is
guarded: after `bash --norc; cd $SB/pr/w/v` in the shell panel, F8 on pr/w moves it to the trash ("1 deleted")
with the nested shell inside it. It goes to the trash (recoverable).
```
mkdir -p pr/w/v pr/z; bin/start s3 $SB/pr $SB/home; bin/k s3 F12; tmux send-keys -t s3 -l "bash --norc"; tmux send-keys -t s3 Enter
tmux send-keys -t s3 -l "cd $SB/pr/w/v"; tmux send-keys -t s3 Enter; bin/k s3 F12 Home Down F8 Enter   # pr/w -> trash
```
(With the top shell doing `cd pr/z`, F8/F6 on z are refused: correct.)

### F8. F8 on an item inside the trash moves it to the trash again and loses where it came from — REPRODUCED, P2 (undocumented)
F8 in $XDG_DATA_HOME/Trash/files on `hl` makes `hl.2` with Path=…/data/Trash/files/hl; the old hl.trashinfo
(Path=$SB/pr/hl) is left orphaned. TC/MC users would expect a permanent delete, or a warning.
```
bin/x "Down,F8,Enter,wait" data/Trash/files pr; cat data/Trash/info/hl.2.trashinfo data/Trash/info/hl.trashinfo
```

### F9. Closing the terminal (SIGHUP) writes a crash log every time — REPRODUCED, P2
Every `tmux kill-session` of a running xdevcmd leaves `crash-*.log`: "panicked at …/std/src/io/stdio.rs:1166:9:
failed printing to stderr: Input/output error (os error 5)" (from xdevcmd::crash::write, main.rs:456). The
session file is saved. guide.md:212 / security.md:196 say closing the terminal stops tidily.
```
bin/start s9 $SB $SB/pr; tmux kill-session -t s9; ls $SB/st-s9/   # crash-*.log next to session.toml
```

### Minor (P2), REPRODUCED
- F2 with the name " " (one space) says "a name cannot be empty"; " f1" and "f1 " are accepted (Linux allows " ").
- Rename-many preview says "Enter renames 2 of 2" for a selection that holds a protected folder; Enter then
  refuses all of it ("…holds a folder shown or in a shell — not removed"). Same wording "not removed" / "never
  deleted or moved" for a rename.
- F4 list with two equal lines says "sub already exists — nothing renamed" rather than "two files would be named sub".
- Undo of a copy that replaced a file reports "Undo the copy: 1 removed" without saying it put the old file back
  (it did). F5 folder merge with a replace reports "1 copied, 1 skipped" without "1 replaced (the old one in the trash)".
- A screenshot run that ends at a dialog leaves a journal that is offered as "The last run … ended in the middle of a
  job" at every start; Esc keeps it, so it is offered again next start.

## What held (all REPRODUCED, checked on disk)
- F8 + Ctrl+Z of 8 items (500-file folder with a subfolder, names with space, newline, 0xFF/0xFE bytes,
  `f1.trashinfo`, a file symlink, a dir symlink): every path, size, mode, mtime, md5 and link target back; target untouched.
- Same with names already in the trash (f1, many, an orphan `with space` file with no .trashinfo): new trash names
  (f1.1 …), the other trash entries untouched.
- Undo of a delete when the name was re-created (also re-created while the Undo dialog was open): new file kept,
  "f1 exists again where it was"; Ctrl+Z works again once the name is free.
- Undo of a copy: a copied file changed while the Undo dialog was open is kept; a folder you put a file in is kept
  and listed; F5-replaced file comes back from the trash with its bytes, mode and 2020 mtime.
- Undo is per session and single-level ("nothing to undo"); Ctrl+Z after Shift+F8 says it cannot be undone.
- Shift+F8 deletes only on Y (Enter, Enter×2, n, Space, Tab/arrows+Enter, clicks on the dialog text did nothing).
- "Cannot move to trash" fallback (trash dir broken): Enter, y, Space, Tab/Right+Enter do not delete.
- Shift+F8 on a symlink to a folder, and on a folder holding such links (with and without trailing slash): target untouched.
- Protected, refused: the other pane's folder, its parent, the home (F8, Shift+F8, F6, F2), home's parent when HOME has
  a trailing slash or is a symlink, `/`, the shell's folder (F8, F6), from Alt+F7 results (F8, F6, F2, marked), from a
  list from shell output with `pr/x/..`, `home/`, `./pr/`, `home/../home`, `/`; drag and drop Move of home and of the
  shown folder. A list row `pr/hl/` (link to home) trashes the link only; home intact.
- F8 on the folder holding the trash: "Cannot move to trash … Invalid argument", offers D/A/S, nothing lost.
- F2 onto an existing file or folder, `.`, `..`, `d/x`, empty: refused with a clear line, nothing changed; a name that
  appears while the F2 dialog is open is not replaced.
- Shift+F2: a pattern giving equal names is refused ("two files would be named x.txt"); swaps and chains rename
  correctly; F4 list with a `/`, an empty line, fewer or more lines, `..`, CRLF: refused or ignored, nothing renamed.
