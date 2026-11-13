#!/usr/bin/env bash
# Waits for the `ci` workflow on one commit and succeeds only if a run of it passed.
# release.yml calls it before anything is built for a tag: a tag push starts the release
# at once, in parallel with the CI of the commit it names (Round 032), so without this a
# draft could be built from a commit whose tests have not finished, or failed.
#
#   scripts/ci-green.sh <repo> <sha> [max-minutes]
#
# Needs `gh` with a token that can read Actions. A run in progress is waited for; no run
# at all after 3 minutes, or only failed ones, fails with what to do. Exit 0 when one
# run of ci on <sha> concluded "success".
set -euo pipefail

repo=${1:?repo, owner/name}
sha=${2:?commit sha}
max=${3:-60}

deadline=$((SECONDS + max * 60))
appear=$((SECONDS + 180))
while :; do
  # Every ci run on this commit, newest first: "status conclusion".
  runs=$(gh api "repos/$repo/actions/workflows/ci.yml/runs?head_sha=$sha&per_page=20" \
    --jq '.workflow_runs[] | "\(.status) \(.conclusion // "none") \(.html_url)"')
  if grep -q '^completed success ' <<<"$runs"; then
    echo "ci passed on $sha: $(grep -m1 '^completed success ' <<<"$runs" | cut -d' ' -f3)"
    exit 0
  fi
  if grep -qv '^completed ' <<<"$runs" && [[ -n $runs ]]; then
    : # one is queued or running: wait for it
  elif [[ -n $runs ]]; then
    echo "::error::ci did not pass on $sha:"
    echo "$runs"
    exit 1
  elif ((SECONDS > appear)); then
    echo "::error::no ci run on $sha. Start one (gh workflow run ci --ref <tag>), then"
    echo "run this release again (Actions -> release -> Re-run all jobs)."
    exit 1
  fi
  if ((SECONDS > deadline)); then
    echo "::error::ci on $sha still not finished after $max minutes"
    exit 1
  fi
  sleep 30
done
