# Packaging for Windows: Scoop and winget

Templates, filled by hand for each release from the files the release workflow makes
(`.github/workflows/release.yml`). Both point at the same archive,
`xdevcmd-v<version>-x86_64-pc-windows-msvc.zip`, which holds one folder of the same name
with `xdevcmd.exe`, `themes/`, `LICENSE`, `README.md` and `THIRD-PARTY.md`.
`crates/xdevcmd/tests/packaging.rs` checks that the URLs and folder names here still match
what release.yml makes.

Both need the release **published** (not a draft), so the archive can be downloaded.
The hash of the archive is its line in the release's `SHA256SUMS`:

```bash
v=0.3.0
gh release download "v$v" -R nci-gis/rust-commander -p SHA256SUMS -O - \
  | grep x86_64-pc-windows-msvc.zip
```

## Scoop

`scoop/xdevcmd.json` goes into the bucket repository `nci-gis/scoop-xdevcmd` (created by
the owner, once: an empty public repository with a `bucket/` folder). For each release:

1. Set `version`, the version in `url` and `extract_dir`, and `hash` from `SHA256SUMS`.
   Or let Scoop do it, once the file is in the bucket: Scoop's
   `bin\checkver.ps1 -App xdevcmd -Dir <bucket>\bucket -Update` reads `checkver` and
   `autoupdate` here, finds the newest release and fills all four.
2. Copy it to `bucket/xdevcmd.json` in the bucket repository and push.
3. Try it on Windows:

   ```powershell
   scoop bucket add xdevcmd https://github.com/nci-gis/scoop-xdevcmd
   scoop install xdevcmd/xdevcmd
   xdevcmd --version
   ```

## winget

`winget/` holds the three manifests for `XDevTeam.xdevcmd` (version, default locale,
installer): a portable program inside a zip, called as `xdevcmd`. Replace `<VERSION>`
(without the `v`) and `<SHA256>` (upper or lower case both work).

The first release goes in with `wingetcreate new`, which asks the same questions these
files answer, then validates and opens the pull request to `microsoft/winget-pkgs`:

```powershell
winget install wingetcreate
wingetcreate new https://github.com/nci-gis/rust-commander/releases/download/v0.3.0/xdevcmd-v0.3.0-x86_64-pc-windows-msvc.zip
```

Or, with the files here filled in: `winget validate --manifest packaging\winget`, try
`winget install --manifest packaging\winget`, then submit the folder by pull request.

Later releases, once the package exists in winget:

```powershell
$v = "0.4.0"
wingetcreate update XDevTeam.xdevcmd --version $v `
  --urls "https://github.com/nci-gis/rust-commander/releases/download/v$v/xdevcmd-v$v-x86_64-pc-windows-msvc.zip" `
  --submit
```

`--submit` needs a GitHub token (`--token`, or `wingetcreate token --store`); it lives on
the owner's machine, never in this repository's workflows.
