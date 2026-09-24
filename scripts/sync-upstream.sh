#!/usr/bin/env bash
set -euo pipefail

# Fetches the latest upstream main and reports (or performs) a sync of local main.
#
# Always:  fetches <remote>/<branch> and prints how far local HEAD is behind/ahead.
# --rebase: additionally rebases local HEAD onto <remote>/<branch> and regenerates
#           the alt-auth patch series (scripts/generate-patches.sh) against the new base.
#
# Usage:
#   scripts/sync-upstream.sh [--rebase] [remote] [branch]
#   default remote=upstream, branch=main

REBASE=0
for arg in "$@"; do
	case "$arg" in
		--rebase) REBASE=1 ;;
	esac
done

REMOTE="upstream"
BRANCH="main"
# positionals after flags
POS=()
for arg in "$@"; do
	case "$arg" in
		--rebase) ;;
		*) POS+=("$arg") ;;
	esac
done
if [ "${#POS[@]}" -ge 1 ]; then REMOTE="${POS[0]}"; fi
if [ "${#POS[@]}" -ge 2 ]; then BRANCH="${POS[1]}"; fi

git fetch "$REMOTE" "$BRANCH"

AHEAD="$(git rev-list --count "$REMOTE/$BRANCH..HEAD")"
BEHIND="$(git rev-list --count "HEAD..$REMOTE/$BRANCH")"

echo "HEAD is $AHEAD commit(s) ahead of and $BEHIND commit(s) behind $REMOTE/$BRANCH"

if [ "$AHEAD" -gt 0 ] && [ "$BEHIND" -gt 0 ]; then
	echo "warning: branches have diverged; rebasing will rewrite local history" >&2
fi

if [ "$REBASE" -eq 1 ]; then
	git rebase "$REMOTE/$BRANCH"
	scripts/generate-patches.sh "$REMOTE/$BRANCH"
	echo "rebase done. verify with cargo fmt/clippy and pnpm prepr:frontend:app, then commit."
fi