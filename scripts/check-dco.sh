#!/usr/bin/env bash
# Verifies that every non-merge commit in a range carries a Developer
# Certificate of Origin sign-off whose address matches the commit author.
#
# Usage: scripts/check-dco.sh [<base>] [<head>]
#   Defaults to origin/main..HEAD, so contributors can run it locally
#   before pushing.
set -euo pipefail

BASE="${1:-origin/main}"
HEAD_REF="${2:-HEAD}"

if ! git rev-parse --verify --quiet "$BASE" >/dev/null; then
  echo "check-dco: cannot resolve base ref '$BASE'." >&2
  echo "Fetch it first (e.g. 'git fetch origin main') or pass an explicit base." >&2
  exit 2
fi

mapfile -t commits < <(git rev-list --no-merges "${BASE}..${HEAD_REF}")

if [[ ${#commits[@]} -eq 0 ]]; then
  echo "check-dco: no non-merge commits in ${BASE}..${HEAD_REF}; nothing to check."
  exit 0
fi

lower() { tr '[:upper:]' '[:lower:]'; }

failed=0
for sha in "${commits[@]}"; do
  author_email="$(git show -s --format='%ae' "$sha" | lower)"
  subject="$(git show -s --format='%s' "$sha")"

  mapfile -t signoffs < <(
    git show -s --format='%B' "$sha" \
      | grep -iE '^[[:space:]]*Signed-off-by:[[:space:]]*.+<[^>]+>' \
      | sed -E 's/.*<([^>]*)>.*/\1/' \
      | lower
  )

  if [[ ${#signoffs[@]} -eq 0 ]]; then
    echo "MISSING  ${sha:0:8}  ${subject}" >&2
    failed=1
    continue
  fi

  matched=0
  for addr in "${signoffs[@]}"; do
    if [[ "$addr" == "$author_email" ]]; then
      matched=1
      break
    fi
  done

  if [[ "$matched" -eq 0 ]]; then
    echo "MISMATCH ${sha:0:8}  ${subject}" >&2
    echo "         authored by <${author_email}> but signed off by: ${signoffs[*]}" >&2
    failed=1
  else
    echo "ok       ${sha:0:8}  ${subject}"
  fi
done

if [[ "$failed" -ne 0 ]]; then
  cat >&2 <<'MSG'

------------------------------------------------------------------------
One or more commits are missing a Developer Certificate of Origin
sign-off. The sign-off is a single trailer line per commit:

    Signed-off-by: Your Name <your.email@example.com>

The address must match the commit author's. Git writes the line for you
if you commit with -s:

    git commit -s -m "your message"

To fix the most recent commit:

    git commit --amend -s --no-edit
    git push --force-with-lease

To sign off every commit on your branch at once:

    git rebase --signoff origin/main
    git push --force-with-lease

What you are certifying is the text in the DCO file at the repository
root: that you wrote the patch, or otherwise have the right to submit it
under this project's MIT OR Apache-2.0 licence. It is not a copyright
assignment, and there is nothing to sign separately.
------------------------------------------------------------------------
MSG
  exit 1
fi

echo "DCO check passed (${#commits[@]} commit(s))."
