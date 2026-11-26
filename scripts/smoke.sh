#!/usr/bin/env bash
# Smoke test of one release archive, as a user gets it: unpacked into an empty
# directory, its binary run from there with nothing else from the build (pillar 4,
# "a smoke test on the release archives themselves").
#
#   scripts/smoke.sh <archive> <version>      e.g. xdevcmd-v0.4.0-x86_64-pc-windows-msvc.zip 0.4.0
#
# Checks: the files every archive carries; the binary at most 7 MiB (the release bar);
# and, where this machine can run it, `--version` names this version and the
# `--print-*` commands work (no terminal needed). An archive for another CPU (the Intel
# macOS build on an ARM runner) is checked without being run, and says so.
set -euo pipefail

archive=$1
version=${2#v}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() {
  echo "smoke: $archive: $*" >&2
  exit 1
}

case "$archive" in
  # Git Bash on Windows runners has 7-Zip, not always unzip.
  *.zip) if command -v unzip > /dev/null; then unzip -q "$archive" -d "$work"; else 7z x -o"$work" "$archive" > /dev/null; fi ;;
  *.tar.gz) tar xzf "$archive" -C "$work" ;;
  *) fail "not a .zip or .tar.gz" ;;
esac
dir=$(find "$work" -mindepth 1 -maxdepth 1 -type d | head -n 1)
[ -n "$dir" ] || fail "no folder inside"

bin="$dir/xdevcmd"
[ -f "$bin.exe" ] && bin="$bin.exe"
[ -f "$bin" ] || fail "no xdevcmd binary"
for f in LICENSE README.md THIRD-PARTY.md docs/README.md docs/security.md themes lang/template.txt lang/vi.toml lang/ja.toml; do
  [ -e "$dir/$f" ] || fail "missing $f"
done

size=$(wc -c < "$bin" | tr -d ' ')
max=$((7 * 1024 * 1024))
[ "$size" -le "$max" ] || fail "binary is $size bytes, over 7 MiB"
echo "smoke: binary $size bytes (at most $max)"

# The archive's CPU from its name, against this machine's.
want=$(case "$archive" in *aarch64*) echo arm64 ;; *) echo x86_64 ;; esac)
have=$(uname -m)
[ "$have" = aarch64 ] && have=arm64
if [ "$want" != "$have" ]; then
  echo "smoke: built for $want, this machine is $have: not run"
  exit 0
fi

# Its own state and config, so nothing of the runner's is read or written. Git Bash
# hands a Windows program its /tmp paths as they are: cygpath makes them Windows ones.
native() { if command -v cygpath > /dev/null; then cygpath -w "$1"; else echo "$1"; fi; }
export XDEVCMD_STATE="$(native "$work/state/session.toml")"
export XDEVCMD_CONFIG="$(native "$work/none.toml")"
out=$("$bin" --version) || fail "--version failed"
case "$out" in
  "xdevcmd $version "*) ;;
  *) fail "--version says '$out', not xdevcmd $version" ;;
esac
"$bin" --help > /dev/null || fail "--help failed"
"$bin" --print-config > "$work/config.toml" || fail "--print-config failed"
grep -q '^\[general\]' "$work/config.toml" || fail "--print-config has no [general]"
"$bin" --print-keys | grep -q 'Quit xdevcmd' || fail "--print-keys has no Quit"
"$bin" --print-man | grep -q '^\.TH' || fail "--print-man is not a man page"
echo "smoke: $out"
