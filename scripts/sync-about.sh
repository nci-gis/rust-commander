#!/usr/bin/env bash
# sync-about.sh - copy xdevcmd's description and keywords from Cargo.toml to the GitHub
# repository's About box (description, topics). Cargo.toml is the one source; this only
# copies it. Needs `gh` logged in with admin rights on the repository.
#
#   scripts/sync-about.sh             show what would change, change nothing
#   scripts/sync-about.sh --apply     change it
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

meta=$(cargo metadata --no-deps --format-version 1)
field() { python3 -c 'import json,sys; p=[p for p in json.load(sys.stdin)["packages"] if p["name"]=="xdevcmd"][0]; v=p[sys.argv[1]]; print(" ".join(v) if isinstance(v,list) else v)' "$1" <<<"$meta"; }
desc=$(field description)
repo_url=$(field repository)
# crates.io keywords, plus `rust`: a GitHub topic of its own, pointless on crates.io.
keywords="$(field keywords) rust"
repo=${repo_url#https://github.com/}

have_desc=$(gh repo view "$repo" --json description --jq .description)
have_topics=$(gh repo view "$repo" --json repositoryTopics --jq '[.repositoryTopics[]?.name] | join(" ")')

echo "repository   $repo"
echo "description  now:  $have_desc"
echo "             want: $desc"
echo "topics       now:  ${have_topics:-<none>}"
echo "             want: $keywords"

if [[ "${1:-}" != "--apply" ]]; then
  echo "(dry run; --apply to change it)"
  exit 0
fi

args=(--description "$desc")
for t in $keywords; do args+=(--add-topic "$t"); done
for t in $have_topics; do
  [[ " $keywords " == *" $t "* ]] || args+=(--remove-topic "$t")
done
gh repo edit "$repo" "${args[@]}"
echo "done"
