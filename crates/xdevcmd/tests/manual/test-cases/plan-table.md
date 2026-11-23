# The table before a copy

Promise: F5 and F6 check the destination before anything starts; what would lose data
is skipped unless you choose otherwise ([guide.md](../../../../../docs/guide.md#copy-and-move-f5-f6)).
The table itself is automated (render.rs); these cases need real drives.

### M-PT-01 The table on real folders
**Platform**: all · **Automated**: render.rs plan tests
**Steps**: copy a folder onto one with some of the same files: newer, older, another size.
**Expected**: one line per group, worst first; ↑↓ between groups; Space: skip / replace /
ask each; the first line sums up "create · replace · skip · space needed and free". All
new files: the plain F5 dialog.
**Fail if**: a newer file in the destination is replaced without choosing **replace**; a
group missing from the table; the counts do not match the folders.

### M-PT-02 Enter before the check ends
**Platform**: all, a big folder on a slow disk · **Automated**: none (timing)
**Steps**: F5, Enter at once.
**Expected**: the job starts "as before, asking as it goes": it asks about each existing
file as it meets it. Tab checks again for the other pane.
**Fail if**: an existing file replaced without a question.

### M-PT-03 A FAT32 stick
**Platform**: all · **Automated**: none
**Steps**: copy a file of 4 GiB or more, and a name with `:` or `?`, onto FAT32; then the
4 GiB file onto exFAT.
**Expected**: both are ⛔ "cannot be done (left out, with the reason)": the drive cannot
take that size, or that name; exFAT takes the 4 GiB file.
**Fail if**: the 4 GiB file starts copying onto FAT32 and fails part way; a file with
another name than its source appears on the stick.

### M-PT-04 A case-insensitive drive
**Platform**: macOS default disk, Windows, a FAT stick · **Automated**: none
**Steps**: copy `Readme.txt` where `README.txt` is.
**Expected**: listed as differing only in case.
**Fail if**: one silently replaces the other.

### M-PT-05 Not enough free space
**Platform**: Linux, macOS (Windows does not know free space yet) · **Automated**: none
**Steps**: copy more than the destination's free space.
**Expected**: the first line shows "space needed and free", and the copy is ⛔ "cannot
be done (left out, with the reason)", the reason naming the space.
**Fail if**: the copy starts and stops on a full disk.

### M-PT-06 A big tree
**Platform**: all · **Automated**: bench `plan` cases
**Steps**: F5 a tree of 100,000 files or more.
**Expected**: the dialog "opens at once and checks the destination while you read it":
the count grows; memory in Help → Usage stays reasonable.
**Fail if**: the dialog waits for the whole check before it shows; the screen stops
answering.
