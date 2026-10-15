#!/usr/bin/env bash
#
# Print the changelog section for a release tag.
#
#   ./packaging/release-notes.sh v0.1.2
#
# Exits non-zero when the changelog has no section for that version, so a
# release cannot ship without notes.

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $(basename "$0") <tag>" >&2
  exit 2
fi

version="${1#v}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
changelog="$(dirname "$here")/CHANGELOG.md"

if [[ ! -f "$changelog" ]]; then
  echo "no CHANGELOG.md at $changelog" >&2
  exit 1
fi

notes="$(
  awk -v heading="## [$version]" '
    index($0, heading) == 1 {
      rest = substr($0, length(heading) + 1)
      if (rest == "" || rest ~ /^[ \t]/) { found = 1; next }
    }
    found && /^## / { exit }
    found { print }
  ' "$changelog"
)"

# Trim leading and trailing blank lines.
notes="$(printf '%s' "$notes" | sed -e '/./,$!d' -e ':a' -e '/^\n*$/{$d;N;ba' -e '}')"

if [[ -z "${notes//[[:space:]]/}" ]]; then
  echo "CHANGELOG.md has no section for $version." >&2
  echo "Add one under '## [$version]', then tag again." >&2
  exit 1
fi

printf '%s\n' "$notes"
