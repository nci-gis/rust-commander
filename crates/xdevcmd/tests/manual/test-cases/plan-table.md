# The table before a copy

Promise: F5 and F6 check the destination before anything starts; what would lose data
is skipped unless you choose otherwise ([guide.md](../../../../../docs/guide.md#copy-and-move-f5-f6)).
The table itself is automated (render.rs); these cases need real drives.

### M-PT-01 The table on real folders
**Platform**: all · **Automated**: render.rs plan tests
**Steps**: copy a folder onto one with some of the same files: newer, older, another size.
**Expected**: one line per group, worst first; ↑↓ between groups; Space: skip / replace /
ask each; the summary counts create, replace, skip and the space needed. All new files:
the plain F5 dialog.

### M-PT-02 Enter before the check ends
**Platform**: all, a big folder on a slow disk · **Automated**: none (timing)
**Steps**: F5, Enter at once.
**Expected**: the copy starts and asks about each existing file as it meets it. Tab
checks again for the other pane.

### M-PT-03 A FAT32 stick
**Platform**: all · **Automated**: none
**Steps**: copy a file of 4 GiB or more, and a name with `:` or `?`, onto FAT32; then the
4 GiB file onto exFAT.
**Expected**: ⛔ "left out: 4 GiB or more, onto a FAT drive"; ⛔ "a name the destination
cannot hold"; exFAT takes the 4 GiB file.

### M-PT-04 A case-insensitive drive
**Platform**: macOS default disk, Windows, a FAT stick · **Automated**: none
**Steps**: copy `Readme.txt` where `README.txt` is.
**Expected**: listed as differing only in case.
**Fail if**: one silently replaces the other.

### M-PT-05 Not enough free space
**Platform**: Linux, macOS (Windows does not know free space yet) · **Automated**: none
**Steps**: copy more than the destination's free space.
**Expected**: ⛔ "it needs …, and only … is free there".

### M-PT-06 A big tree
**Platform**: all · **Automated**: bench `plan` cases
**Steps**: F5 a tree of 100,000 files or more.
**Expected**: the dialog opens at once, the count grows; memory in Help → Usage stays
reasonable.
