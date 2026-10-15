#!/usr/bin/env bash
# The standard verbs of every changed module, one module after another, in the one CI job the
# project's ruleset requires: `Module checks` (.nstack/handbook/09-platform.md §2).
#
# nstack lists the modules with a file changed since BASE, and both sides of a contract version
# the change touches (.nstack/handbook/03-contracts.md §5). For each one: bootstrap, check, test;
# then compat, even after a failure, so that a broken frozen version is named in the same round
# (.nstack/handbook/03-contracts.md §4); then e2e, on a module still green; then clean, on a
# self-hosted runner only, even after a failure — a hosted machine is thrown away after the job.
# A module that fails does not stop the next one; the job fails at the end, naming them.
#
# The module names come from the paths the change touched: they reach the commands as variables,
# never as script text. The criticality is printed, never branched on: what a value requires is
# judged by the rules that read the matrix of .nstack/handbook/05-workflow.md §7.
set -uo pipefail

BASE=${BASE:?the commit the change is compared with}
if ! git rev-parse --verify --quiet "$BASE^{commit}" >/dev/null; then
  if [ "${GITHUB_EVENT_NAME:-}" = pull_request ]; then
    echo "::error::Base $BASE of the pull request not found: its modules cannot be listed." \
      "Action: run the jobs again; the checkout fetches the whole history."
    exit 1
  fi
  BASE=$(git rev-list --max-parents=0 HEAD | tail -1)  # a branch's first push: every file is new
fi

LIST=$(nstack modules --root . --changed-since "$BASE" --with-contract-sides) || exit 1
if [ -z "$LIST" ]; then
  echo "No module with a file changed: nothing to check."
  exit 0
fi

FAILED=()
while IFS=$'\t' read -r MODULE FOLDER <&3; do
  echo "::group::$MODULE"
  VALUE=$(sed -n "s/^  criticality: *['\"]\{0,1\}\([a-z]*\).*/\1/p" "$FOLDER/MANIFEST.yaml" | head -1)
  echo "Criticality: ${VALUE:-undeclared}"
  case "$VALUE" in
    prototype|standard|high|critical) ;;
    *) echo "Not a value the engine knows: judged as critical until M3 is satisfied." ;;
  esac
  GREEN=1
  { nstack bootstrap "$MODULE" && nstack check "$MODULE" && nstack test "$MODULE"; } || GREEN=0
  nstack compat "$MODULE" --base "$BASE" || GREEN=0
  if [ "$GREEN" = 1 ]; then nstack e2e "$MODULE" || GREEN=0; fi
  if [ "${RUNNER_ENVIRONMENT:-}" = self-hosted ]; then nstack clean "$MODULE" || GREEN=0; fi
  echo "::endgroup::"
  [ "$GREEN" = 1 ] || FAILED+=("$MODULE")
done 3<<<"$LIST"

if [ "${#FAILED[@]}" -gt 0 ]; then
  echo "Module checks failed: ${FAILED[*]}. Each one's log group above names the verb and its output."
  exit 1
fi
echo "Every changed module passed."
