#!/usr/bin/env bash
# release-check.sh - may this tag be released? The release workflow's first job, as a
# script so the rules can be tried before a tag exists (Actions → release → Run
# workflow, or here).
#
#   scripts/release-check.sh v0.4.0-beta.1 [commit]
#
# The commit is the tag's (by default the tag's own, else HEAD: a dry run before the
# tag). It checks that:
# - the tag is a stable version (v1.2.3) or a pre-release (v1.2.3-beta.N, -rc.N),
#   nothing else;
# - Cargo.toml's version is the tag's;
# - CHANGELOG.md has a dated section for it, `## [1.2.3-beta.1] - YYYY-MM-DD`;
# - a stable release is on origin/main; a pre-release on origin/dev or origin/main
#   (the owner, 2026-11-13: pre-releases are cut from dev, GitHub only).
# Prints `prerelease=true|false` on the last line (for $GITHUB_OUTPUT) and exits 1 on
# any failure, after saying every one.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

tag=${1:?usage: release-check.sh TAG [COMMIT]}
commit=${2:-$(git rev-parse -q --verify "refs/tags/$tag^{commit}" || git rev-parse HEAD)}
v=${tag#v}
bad=0
err() { echo "::error::$*"; bad=1; }

if [[ $v =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  pre=false
elif [[ $v =~ ^[0-9]+\.[0-9]+\.[0-9]+-(beta|rc)\.[0-9]+$ ]]; then
  pre=true
else
  err "$tag is neither a release (v1.2.3) nor a pre-release (v1.2.3-beta.N, v1.2.3-rc.N)"
  pre=false
fi

cargo_v=$(cargo metadata --no-deps --format-version 1 |
  python3 -c 'import json,sys; print([p for p in json.load(sys.stdin)["packages"] if p["name"]=="xdevcmd"][0]["version"])')
[ "$cargo_v" = "$v" ] || err "tag $tag, but Cargo.toml says $cargo_v"

# The version's dots taken as dots: 0.4.0 must not match a heading "0x4y0".
ve=$(printf '%s' "$v" | sed 's/[.]/\\./g')
grep -Eq "^## \[$ve\] - [0-9]{4}-[0-9]{2}-[0-9]{2}\$" CHANGELOG.md ||
  err "CHANGELOG.md has no dated section '## [$v] - YYYY-MM-DD' (run cargo release)"

on() { git merge-base --is-ancestor "$commit" "origin/$1" 2>/dev/null; }
if [ "$pre" = true ]; then
  on dev || on main || err "$tag is a pre-release, and is on neither dev nor main"
else
  on main || err "$tag is not on main; releases are tagged on main"
fi

echo "prerelease=$pre"
exit $bad
