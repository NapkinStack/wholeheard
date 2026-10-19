#!/usr/bin/env bash
# Run in the e2e's container once the package is installed: what it installed, as the system lists
# it ($1, a command), is the spec's files ($expected, each a regular file) and the directories
# above them — nothing else, no link, no directory of its own —; and the package carries no script
# run as it is installed or removed ($2, a command that prints them).
set -uo pipefail
listed=$1 scripts=$2
declare -A allowed=()
for file in $expected; do
  allowed[$file]=file
  parent=$file
  while parent=${parent%/*}; [ -n "$parent" ]; do allowed[$parent]=directory; done
done
wrong=()
while IFS= read -r path; do
  case $path in "" | "/.") continue ;; esac
  case ${allowed[$path]:-} in
    file) [ -f "$path" ] && [ ! -L "$path" ] && continue ;;
    directory) [ -d "$path" ] && [ ! -L "$path" ] && continue ;;
  esac
  wrong+=("$path")
done < <(eval "$listed")
for file in $expected; do
  [ -f "$file" ] || wrong+=("missing: $file")
done
run=$(eval "$scripts" 2>/dev/null)
[ -z "$run" ] || wrong+=("scripts: $run")
[ ${#wrong[@]} -eq 0 ] && exit 0
printf '        got: %s\n' "${wrong[@]}"
exit 1
