#!/usr/bin/env bash
# Enforces that the exported symbols of the built bubbles-ffi cdylib are exactly the ones
# declared in include/bubbles_ffi.h.
#
# The header is the contract native hosts compile against. An exported symbol that is not
# in the header is undiscoverable; a declared symbol that is not exported is a link error
# in someone else's build. Both are caught here.
#
# The complementary direction - that the header's macros agree with the Rust constants -
# is covered by crates/bubbles-ffi/tests/header.rs.
#
# Linux only (uses `nm -D`); run as part of the Linux CI job.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

HEADER="crates/bubbles-ffi/include/bubbles_ffi.h"
LIB="target/release/libbubbles_ffi.so"

if [[ ! -f "$LIB" ]]; then
  echo "building $LIB ..."
  cargo build -p bubbles-ffi --release
fi

# Exported, defined, dynamic symbols with a bubbles_ prefix.
exported=$(nm -D --defined-only "$LIB" | awk '{print $3}' | grep '^bubbles_' | sort -u)

# Declared function names in the header. Block comments are stripped first so that prose
# like "see bubbles_last_error()" is not mistaken for a declaration.
declared=$(
  awk '
    { line = $0 }
    # Drop single-line /* ... */ comments outright.
    { gsub(/\/\*([^*]|\*[^\/])*\*\//, "", line) }
    /\/\*/ && !/\*\// { in_comment = 1; sub(/\/\*.*/, "", line) }
    in_comment && /\*\// { in_comment = 0; sub(/.*\*\//, "", line); print line; next }
    in_comment { next }
    { print line }
  ' "$HEADER" \
    | grep -oE '\bbubbles_[a-z0-9_]+[[:space:]]*\(' \
    | sed -E 's/[[:space:]]*\($//' \
    | sort -u
)

if [[ -z "$declared" ]]; then
  echo "error: parsed zero declarations out of $HEADER - the parser is broken." >&2
  exit 1
fi

if ! diff_out=$(diff <(echo "$declared") <(echo "$exported")); then
  echo "error: $HEADER and $LIB disagree on the exported ABI." >&2
  echo "  '<' declared in the header but not exported by the library" >&2
  echo "  '>' exported by the library but not declared in the header" >&2
  echo "$diff_out" >&2
  echo >&2
  echo "Removing or renaming a symbol is a breaking change: bump BUBBLES_ABI_VERSION in" >&2
  echo "the header and in src/lib.rs, and record it in crates/bubbles-ffi/ABI.md." >&2
  exit 1
fi

count=$(echo "$exported" | wc -l | tr -d ' ')
echo "FFI ABI check passed ($count symbols match between header and cdylib)."
