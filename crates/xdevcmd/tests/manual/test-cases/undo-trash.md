# Undo and the trash

Promise: Ctrl+Z puts things back and never removes what you changed since; a replaced
file goes to the trash where the trash is on the same drive
([guide.md](../../../../../docs/guide.md#undo-ctrlz)).

### M-UT-01 Undo a copy
**Platform**: all · **Automated**: render.rs undo tests (screenshot)
**Steps**: 1. F5 a folder of a few files. 2. Ctrl+Z, Enter.
**Expected**: "Undo the copy of …?" lists what it will remove; then the copies are gone,
the originals untouched.

### M-UT-02 Undo keeps what you changed
**Platform**: all · **Automated**: engine undo tests
**Steps**: 1. F5 a folder. 2. Edit one copy; drop a new file into the copied folder.
3. Ctrl+Z.
**Expected**: the edited copy, the new file and their folder stay; the summary says why.
**Fail if**: any of them removed.

### M-UT-03 A replaced file comes back
**Platform**: Linux any disk and a USB stick; macOS a local disk and a USB stick;
Windows the system drive (C:) · **Automated**: Linux, in a sandboxed trash
**Steps**: 1. Copy over an older file, choose replace. 2. Ctrl+Z.
**Expected**: the table said "ℹ a file replaced goes to the trash"; after Ctrl+Z the new
file is gone and the old one back, from the trash.
**Fail if**: the old file is not in the trash, or Ctrl+Z leaves the new one.

### M-UT-04 Where there is no trash
**Platform**: macOS and Windows on a network share; Windows on any drive but C: ·
**Automated**: none
**Steps**: copy over an older file there.
**Expected**: the table marks the replace ⚠, default skip.
**Fail if**: a replace there that claims to go to the trash.

### M-UT-05 F8 on a Windows USB stick
**Platform**: Windows · **Automated**: none
**Steps**: F8 a file on the stick.
**Expected**: write down what Windows does: asks to delete for good, or deletes.
**Fail if**: it is deleted for good without a word.
