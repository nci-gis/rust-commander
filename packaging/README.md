# Packaging for Windows: Scoop

A template, filled for each release from the files the release workflow makes
(`.github/workflows/release.yml`). It points at the archive
`xdevcmd-v<version>-x86_64-pc-windows-msvc.zip`, which holds one folder of the same name
with `xdevcmd.exe`, `themes/`, `LICENSE`, `README.md` and `THIRD-PARTY.md`.
`crates/xdevcmd/tests/packaging.rs` checks that the URL and folder name here still match
what release.yml makes.

It needs the release **published** (not a draft), so the archive can be downloaded.
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

winget is not planned (ROADMAP item 48 [draft]): it needs a signed Windows build. Its
manifests were removed after commit `3a84988`; restore them from there if it returns.
