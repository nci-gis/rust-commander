# While a job runs

### M-RJ-01 The time left
**Platform**: all · **Automated**: unit tests of the estimate
**Steps**: copy a few GB to a USB stick.
**Expected**: after a few seconds "about … min left", roughly right, not swinging wildly.
**Fail if**: a number from the first second; a number while nothing moves.

### M-RJ-02 Lists follow a move
**Platform**: all · **Automated**: render.rs (one move)
**Steps**: Alt+F7 a search; F6 one result elsewhere.
**Expected**: the row stays in the list under its new place.
**Fail if**: the row is gone, or still names the old place.
