#!/usr/bin/env bash
# Live view of the GitHub Actions runs a push started (gh run watch, run after run).
#
#   tools/ci-watch.sh                the runs of HEAD (CI)
#   tools/ci-watch.sh <commit>       the runs of that commit
#   tools/ci-watch.sh --tag v2.8.0   a release: CI of the tagged commit and the tag's
#                                    Release builds, then the draft release's files
#
# Waits up to 2 minutes for the runs to show up after a push. Exit status 0 when every
# run passed. Needs gh, logged in to the repository.
set -euo pipefail
cd "$(dirname "$0")/.."

tag=""
case "${1:-}" in
    -h | --help)
        sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    --tag)
        tag=${2:?usage: tools/ci-watch.sh --tag vX.Y.Z}
        sha=$(git rev-list -n 1 "$tag")
        ;;
    "") sha=$(git rev-parse HEAD) ;;
    *) sha=$(git rev-parse "$1") ;;
esac

# run IDs of the commit, oldest first; with a tag, also wait for the tag's own run
runs() {
    gh run list --commit "$sha" --json databaseId,headBranch \
        --jq "sort_by(.databaseId) | .[] | \"\(.databaseId) \(.headBranch)\""
}
ready() {
    local r
    r=$(runs)
    [ -n "$r" ] && { [ -z "$tag" ] || grep -q " $tag\$" <<<"$r"; }
}

printf 'Waiting for the runs of %s%s' "${sha:0:7}" "${tag:+ ($tag)}"
for _ in $(seq 60); do
    ready && break
    printf '.'
    sleep 2
done
echo
ready || { echo "No run for ${sha:0:7} after 2 minutes: was it pushed?" >&2; exit 1; }

failed=0
while read -r id branch; do
    echo
    echo "── run $id ($branch) ──────────────────────────────"
    gh run watch "$id" --exit-status --interval 3 || failed=1
done < <(runs)

echo
gh run list --commit "$sha"
if [ -n "$tag" ]; then
    echo
    gh release view "$tag" --json isDraft,assets \
        --jq '"Release \(if .isDraft then "draft" else "published" end): \(.assets | length) files", (.assets[] | "  \(.name)  \(.size / 1048576 * 10 | floor / 10) MB")' \
        || echo "No release for $tag yet."
fi
[ "$failed" = 0 ] && echo "All runs passed." || echo "A run failed." >&2
exit "$failed"
