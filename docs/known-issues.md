# Known issues and limits

What does not work yet, and what will not change, as of 0.4.0-beta.3. One line each;
the details are where you would meet the thing, behind the link. Not listed here?
[Report it](troubleshooting.md#reporting-a-problem).

## Not yet: will change

| Issue | System | What to do | Planned | Details |
|---|---|---|---|---|
| The terminal's close button is not caught: a job in progress stops without tidying up | Windows | Quit with F10; after a close, the next start tidies up and offers to continue | later | [Security](security.md#limits) |
| A prompt built into the shell, such as bash's `read -s`, looks like the shell's own prompt: Alt+C can be typed as its answer | Linux, macOS | Press Ctrl+C in the shell first, or turn on the prompt marks | later | [Shell](shell.md#before-anything-is-typed-into-the-shell) |
| An open single quote in fish or PowerShell is not seen: a name typed in lands inside it | all | Press Ctrl+C in the shell first | later | [Shell](shell.md#before-anything-is-typed-into-the-shell) |
| The fish and PowerShell versions of the shell set-up (staying in the directory you quit in, prompt marks) have not been run | all | Tell us if they work for you | later | [Shell](shell.md#stay-in-the-directory-you-were-in) |
| The usage view does not measure memory | Windows | — | later | — |
| The config file's permissions are not checked (elsewhere, a file anyone can write is refused) | Windows | Keep it in its default place, under `%APPDATA%`, which only you can write | later | [Security](security.md#the-config-file-is-trusted) |
| An older version started after a kill may not understand all of what a newer one was doing (a Copy As name, for one) | all | Continue with the same or a newer version | rc.1 | — |
| The Intel macOS build is checked before a release but not run: it is built on Apple Silicon | macOS Intel | Tell us if it does not start | later | [Security](security.md#checking-a-download) |
| Stopping a job with Esc ends it: it is not offered to continue at the next start, as a killed one is | all | When a share is down, stop with Esc; once it is back, copy again with F5 and answer for the files already there | 0.4.x | [Copy and move](guide.md#copy-and-move-f5-f6) |
| With a share down, Skip meets the same error on the next file, after its tries (about 30 s a file) | all | Stop with Esc; the question says so when the error is a network one | 0.4.x | [Remote files](guide.md#remote-files-through-a-mount) |
| A job sent to the background with B comes back only to ask a question or when it ends with errors | all | Its progress is on the status line; it comes forward by itself when it needs you | 0.4.x | [Copy and move](guide.md#copy-and-move-f5-f6) |

## By design: will not change

| Limit | System | Why, and what to do | Details |
|---|---|---|---|
| A cut-short file up to 256 MiB, a copy with safe operations off or by a clone, extracting and packing start again after a kill instead of going on | all | A checkpoint costs a sync every 256 MiB, so smaller files carry none and start again; a clone is whole or nothing | [Security](security.md#limits) |
| Folders are not synced: after a crash, the newest renames may be lost | Windows | Not possible without code xdevcmd does not use (no `unsafe`, no extra library) | [Security](security.md#limits) |
| The look before replacing compares size, modification and creation time only | Windows | The same reason; another file with all three the same is not told apart | [Security](security.md#limits) |
| A rename that must not replace anything is a check, then a rename, with a small window | Windows FAT drives, some shares | The file system offers nothing better | [Security](security.md#limits) |
| A rename into place may be refused or not atomic | some network and FUSE file systems | Copy to a local disk first when it matters | [Security](security.md#limits) |
| Temporary files of a killed run are tidied at the next start, on that machine and account only | all | Start xdevcmd again where it was killed | [Security](security.md#limits) |
| A hard NFS mount whose server is gone blocks inside the system call: no retry is shown, and Esc stops the job only once that call returns | Linux, macOS | The system waits for the server; mount with `soft` (or bring the server back) | [Remote files](guide.md#remote-files-through-a-mount) |
| Without prompt marks, xdevcmd cannot see what runs in the shell, so every send asks | Windows | Turn on the prompt marks | [Shell](shell.md#telling-xdevcmd-where-the-shell-is) |
| After xdevcmd is killed, the window prints odd characters when the mouse moves | Linux, macOS | A killed program cannot put the terminal back: type `reset`, Enter | [Troubleshooting](troubleshooting.md#every-system) |
