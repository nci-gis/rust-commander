# QA hunt: COPY, MOVE, RESUME (xdevcmd 0.4.0-beta.1 prebuilt, /tmp/qa/bin)

Sandbox: /tmp/qa/hunt-copy (XDEVCMD_CONFIG/STATE, XDG_DATA_HOME, HOME all inside it).
Helpers: `bin/x KEYS LEFT RIGHT [WxH]` = screenshot mode with the sandbox env + `--no-restore`;
`bin/run SESSION CONFIG LEFT RIGHT` = same env in tmux; `slow.toml` = `[general] fast_copy = false`.
Running as root (so chmod-based read-only tests are void; `chattr +i` used instead).
Cross-drive tests used /dev/shm/qa-hunt-copy (tmpfs), as the task asked; xdevcmd also created
/dev/shm/.Trash-0 there. Both removed at the end.
xtask `--kill-when` not used: it kills "xdevcmd" by name and other agents run xdevcmd; I killed
only the PID I started (tmux pane child) with `kill -9`.

## Findings

### 1. F5 copies INTO a symlinked folder at the destination, after the plan said "skipped" (REPRODUCED, P0)

Contradicts guide.md:183-184 ("a file where a folder is (skipped: never a file over a folder,
or the reverse)") and the table's own line `⛔ 1 folder where a file is: skipped, never replaced`.
The destination has `d -> ../outside/od` (a symlink to a folder somewhere else); the source has
folder `d`. The plan classes `b/d` as a file and says ⛔ skipped (the journal also records
`skip .../b/d`), but the job then walks through the symlink: it creates new files in
`outside/od` with no question, and asks "Already exists" about `b/d/in.txt` (really
`outside/od/in.txt`). O, A or U there replaces the file outside the target tree. The old one
goes to the trash. Same with one item selected or several. F6 (same drive) and Alt+Y skip it
correctly.

```sh
cd /tmp/qa/hunt-copy; rm -rf t7; mkdir -p t7/a/d t7/b t7/outside/od; cd t7
echo NEWINNER > a/d/in.txt; echo brandnew > a/d/new.txt; echo x > a/z.txt
echo precious > outside/od/in.txt; touch -d 2020-01-01 outside/od/in.txt
ln -s ../outside/od b/d; cd ..
bin/x Ctrl+A,F5 t7/a t7/b 120x40 | grep -a "⛔"       # ⛔  1 folder where a file is: skipped, never replaced
bin/x Ctrl+A,F5,Enter,O,wait t7/a t7/b 120x40 | tail -2  # 4 copied (3 by the kernel), 1 replaced (the old one in the trash)
cat t7/outside/od/in.txt   # NEWINNER   (was "precious")
ls t7/outside/od           # in.txt new.txt   (new.txt created with no question)
```
Single selection, live (tmux): `bin/run t none.toml t12/a t12/b`, Down, F5 (same ⛔ line), Enter,
then "Already exists /tmp/qa/hunt-copy/t12/b/d/in.txt", U -> `outside/od/in.txt` = NEWINNER, plus new.txt.
The question shows only "on disk 9 / incoming 9": no dates, and nothing says the path goes through a symlink.

### 2. F6 of a folder onto a folder of the same name on the same drive cannot merge: "Directory not empty" (REPRODUCED, P1)

Contradicts the plan the dialog shows (guide.md:188-196). For example `create 1 · skip 1`, or the plain
"1 file • 2 bytes" dialog when nothing clashes. After Enter the job asks "Already exists:
/…/b/dir, on disk 4.0 K, incoming 4.0 K, [O] overwrite…" about the folder itself. O or U gives
"0 moved, 1 failed: …/b/dir: Directory not empty (os error 39)". No file moves, not even the
ones the plan said it would create. Nothing is lost. F5 merges fine, and so does a move across drives.
This is a dead end for the usual mc/TC "move folder into existing folder" task.

```sh
cd /tmp/qa/hunt-copy; mkdir -p mv2/a/dir mv2/b/dir; echo y > mv2/a/dir/y; echo z > mv2/b/dir/z
bin/run m none.toml mv2/a mv2/b; tmux send-keys -t m Down F6; sleep 2; tmux send-keys -t m Enter; sleep 2
tmux capture-pane -p -t m     # "Already exists /tmp/qa/hunt-copy/mv2/b/dir"
tmux send-keys -t m u; sleep 2; tmux capture-pane -p -t m   # "0 moved, 1 failed … Directory not empty (os error 39)"
find mv2 -type f              # mv2/b/dir/z  mv2/a/dir/y   (y never moved)
```
With a clash inside (mv1: `x` older at the destination): the plan says create 1 · skip 1, and the same failure follows.

### 3. Alt+Y sync killed mid-job is not offered to continue, and its big-file checkpoint is thrown away (REPRODUCED, P1)

Contradicts security.md:52 ("The next start tidies up and offers to continue"), security.md:55
("Covered: … Alt+Y sync") and security.md:86-90 (a big file goes on from its checkpoint, "kept part way").
After a kill during Alt+Y, the journal holds `kind sync` and a `ckpt` line. The next start shows only
"Interrupted: The last run of xdevcmd ended in the middle of a job. 1 unfinished temporary file
removed. [Enter] close". It does not offer Continue or C. The 300 MB part is deleted and nothing of the sync is done.
The disk is safe: no short file under its real name, and the old `old.txt` is intact.

```sh
cd /tmp/qa/hunt-copy; mkdir -p y1/l y1/r; head -c 1200M /dev/urandom > y1/l/big.bin
mkdir y1/l/many; echo a > y1/l/many/a; echo OLDR > y1/r/old.txt; touch -d 2020-01-01 y1/r/old.txt; echo NEWL > y1/l/old.txt
bin/run y slow.toml y1/l y1/r; sleep 2; tmux send-keys -t y M-y; sleep 4; tmux send-keys -t y Enter
bin/killwhen y y1/r 300000000     # kill -9 of my own PID once the part file is > 300 MB
head -c 300 state/journal/*        # kind sync … ckpt …
bin/run y slow.toml y1/l y1/r; sleep 4; tmux capture-pane -p -t y   # only "[Enter] close"
ls -a y1/r                         # old.txt only; part removed, big.bin/many never copied
```
(For comparison, the same kill during F5 and F6 offers Continue with "kept part way (256 M / 512 M)" and it works.)

### 4. Plan table: no ⚠ icon, and not "worst first" (REPRODUCED, P2)

Contradicts guide.md:192-194 ("worst first: ⛔ …, ⚠ would lose data (a newer file replaced, an
older one, another size, a case clash…), ℹ only said"). On ext4 with a trash, every data-loss
group is drawn with ℹ (U+2139, checked with od). They also come before the ⛔ lines:
```
ℹ 1 would replace a newer file  [skip]
ℹ 1 would replace an older file  [skip]
ℹ 1 would replace a file of another size  [skip]
⛔  1 file where a folder is: skipped, never replaced
⛔  1 folder where a file is: skipped, never replaced
ℹ 1 already there, identical: left as it is
```
Repro: the t2 setup in this sandbox (`t2.orig`), `bin/x Ctrl+A,F5 t2/a t2/b 120x40`. Perhaps ⚠ is
meant only where there is no trash (guide.md:218). If so, line 193 says otherwise, and the order is still wrong.
Related: the replace on /dev/shm (tmpfs) is ℹ "goes to the trash". xdevcmd made /dev/shm/.Trash-0, which fits "the drive's own".

### 5. Alt+Y labels a file-vs-folder clash as "same time, different size" (REPRODUCED, P2)

guide.md:313 defines ≠ as "Same time, different size; or names that differ only in case…". The sync
plan shows `≠ df  same time, different size — left alone` where the left side is a folder and the right side is a
file (and the reverse for `fd`). It does the same for a folder vs a symlink-to-folder. The action (left alone) is right,
but the reason it gives is wrong. Repro: t10 setup, `bin/run s none.toml t10/a t10/b`, Alt+Y.

### 6. A cross-drive move asks "Different filesystem … [Y]/[A]/[S]", again on Continue and again on Ctrl+Z (REPRODUCED, P2, undocumented)

The guide says nothing about this question (guide.md:259 says only that it "copies then removes"). After a kill, Continue
asks it again (security.md:81-84 says decisions carry out "without asking again"; this question is
not a table decision, so the doc is silent). Undo of the move asks it a third time. This is friction, not loss.
Repro: `bin/run k slow.toml m1/src /dev/shm/qa-hunt-copy/dst`, Ctrl+A, F6, Enter.

### 7. Smaller screen/doc issues (REPRODUCED, P2)
- The Interrupted dialog that has a kept part shows `[Enter] continue  [C] check (slow)  [D] discard them`
  with no `[Esc]`. Esc still leaves the job (verified), as security.md:74/90 say.
- An immutable target folder (`chattr +i`): the plan says `create 1 …` together with `⛔ you cannot write`, and
  `[Enter] go`. Enter then refuses on the status line. The counts are not consistent.
- A plan with every group on skip still says "needs 1.1 G" (r1: `create 0 · replace 0 · skip 151 · needs 1.1 G`).
- keys.md:73 says Ctrl+z undoes "rename, move, new directory or new file". guide.md:223 also lists copy,
  sync and delete to the trash.
- The question dialogs are cut at 130 columns: "[Esc] abor".
- SUSPECT: U = "only if newer, skip the same" leaves a file that has the same date but another size (f298 in the
  U test). Robocopy, which guide.md:190 cites, would copy it as "changed".

## Tried and held (one line each)
- F5 onto an older/newer/other-size file: default skip kept every destination byte (t2).
- Space on one group (older-file) replaced only that file; others untouched (t2).
- File over folder and folder over file: both skipped, folder contents intact (t2, n1 nested).
- Case-only names on ext4 (case-sensitive): both kept, as expected. A case-insensitive drive was not available.
- Names with spaces, quotes, a newline, bytes \xff\xfe, ESC sequences: copied byte-exact. Shown without control chars.
- Hard links become separate files. A sparse 100M file is written out in full (cmp equal). Mode bits and file mtime kept.
- Empty folder and a symlink loop (`loop/self -> ../loop`) copied as a link, no hang. A dangling link was copied as a link.
- A destination file that is a symlink to a file outside: replace replaced the link. The target file was untouched and the link went to the trash.
- Replacing a hard-linked destination: the other link kept the old content.
- An immutable destination file: "could not go to the trash", old kept. Continue after unlock replaced it with no question.
- Kill -9 mid big-file F5: only `.big.bin.xdevcmd-part-*`, no short real file. Esc on the dialog left the job, and a restart offered it again.
  Continue went on from the 256 M checkpoint. Result cmp-equal, mtime and mode kept, no part files left.
- Kill -9 mid replace of 151 files: each destination was either the old file or the complete new one (sha256). Continue did not
  ask about table choices, asked only about the file I changed meanwhile ("Already exists, and changed").
- Kill -9 mid F5, then one finished copy zeroed (same size and date), then C: "1 found different … copied again". The whole tree is cmp-equal.
- Kill -9 twice mid cross-drive F6 (/tmp to /dev/shm): no source removed before its copy was complete (sha256 of all 301 files).
  Continue left the 55 "both" sources, as security.md:76-78 says.
- Ctrl+Z after the cross-drive move: everything back, with mtimes. The replaced file stays in the trash (guide.md:216).
- Ctrl+Z after a copy with replace: the copies were removed, the file edited since was kept and listed, the replaced file came back from the trash.
- U "leave all" / "only if newer" with files that appear mid-job: kept and replaced as the rule says.
- Esc mid-replace of a 700 MB file: the old file is intact (sha256), no part file.
- Alt+Y did not write through a symlinked folder. Alt+K marked only the missing file.
