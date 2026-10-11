#!/usr/bin/env bash
# Installs the NapkinStack engine this project records, for CI (PDR-0005).
#
# A published version (_commit: vX.Y.Z) comes from PyPI, the only published channel
# (ADR-0002). Anything else comes from the framework's repository at that commit, and is
# announced: nstack then says, on every run that judges, that its rules are unpublished.
# A ref that cannot be fetched fails here; no published version is ever used in its place.
set -euo pipefail

VERSION=$(yq '._commit' .copier-answers.yml)
if [[ "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  exec uv tool install "napkinstack==${VERSION#v}" --with-executables-from pre-commit
fi

SOURCE=$(yq '._src_path' .copier-answers.yml)
case "$SOURCE" in  # Copier's remote forms, as nstack doctor L7 reads them
  https://*|ssh://*) ;;
  gh:*) SOURCE="https://github.com/${SOURCE#gh:}" ;;
  gl:*) SOURCE="https://gitlab.com/${SOURCE#gl:}" ;;
  git@*:*) HOST=${SOURCE#git@}; SOURCE="ssh://git@${HOST%%:*}/${HOST#*:}" ;;
  *) echo "FAIL [nstack] framework source '$SOURCE' is a path on one machine: CI cannot fetch it."
     echo "      Action: nstack update from the framework's repository (PDR-0005)."
     exit 1 ;;
esac
REF=${VERSION##*-g}  # Copier records an unpublished commit as git describe: vX.Y.Z-N-g<sha>
echo "::warning::UNPUBLISHED NapkinStack: ${SOURCE}@${REF} judges this run (PDR-0005)"
uv tool install "napkinstack @ git+${SOURCE}@${REF}" --with-executables-from pre-commit || {
  echo "FAIL [nstack] framework ref '$REF' not found at $SOURCE: no published version is used in its place."
  echo "      Action: pin an existing commit with nstack update --ref, or a published vX.Y.Z (PDR-0005)."
  exit 1
}
