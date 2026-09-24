#!/usr/bin/env bash
set -euo pipefail

# Regenerates the alt-auth patch series under patches/alt-auth/.
#
# Each patch captures the Elyrinth fork's divergence from <base> over one feature
# area, so the alt-auth feature set (offline accounts + ely.by) can be re-applied
# to a fresh upstream checkout or re-derived after every upstream rebase.
#
# Usage: scripts/generate-patches.sh [base]
#   default base=upstream/main (must be fetchable locally, e.g. git fetch upstream main)

BASE="${1:-upstream/main}"

if ! git rev-parse --verify --quiet "$BASE" >/dev/null; then
	echo "error: '$BASE' is not a local ref; run 'git fetch upstream main' first" >&2
	exit 1
fi

OUT_DIR="patches/alt-auth"
mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR"/*.patch "$OUT_DIR"/MANIFEST.txt

# feature areas — files outside these paths (docs/, scripts/, .github/, pnpm-lock.yaml)
# are intentionally not patched (they are fork-only, not part of the feature set).
git diff --binary "$BASE"...HEAD -- apps/app > "$OUT_DIR/01-app.patch"
git diff --binary "$BASE"...HEAD -- apps/app-frontend > "$OUT_DIR/02-app-frontend.patch"
git diff --binary "$BASE"...HEAD -- packages/app-lib > "$OUT_DIR/03-app-lib.patch"
git diff --binary "$BASE"...HEAD -- packages/api-client > "$OUT_DIR/04-api-client.patch"

# drop empty patches (e.g. base == HEAD for an untouched area)
for p in "$OUT_DIR"/*.patch; do
	[ -s "$p" ] || rm -f "$p"
done

{
	echo "base: $BASE ($(git rev-parse "$BASE"))"
	echo "generated: $(date -u +%FT%TZ)"
	echo "apply: git apply --3way patches/alt-auth/*.patch"
} > "$OUT_DIR/MANIFEST.txt"

echo "patches regenerated against $BASE in $OUT_DIR/"