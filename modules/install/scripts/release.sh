#!/usr/bin/env bash
# What a user installs, for the system named: `rpm` for Fedora — one package of the four
# programs, made in a Fedora of this system's release so that rpmbuild reads their libraries.
# Reads what the assembled modules' releases left beside this one's folder ($NSTACK_DIST/..).
set -euo pipefail
format=${1:-rpm}
[ "$format" = rpm ] || { echo "release: '$format' is not made here yet (rpm)" >&2; exit 2; }
here=$(cd "$(dirname "$0")/.." && pwd)
dist=${NSTACK_DIST:?run through nstack release, which names this module\'s artefacts folder}
artefacts=$(cd "$dist/.." && pwd)
version=$(cat "$artefacts/shell/VERSION")
release=$(rpm -E %fedora)
# No relabelling of what is mounted (`:Z` would relabel the files in place): labels left alone.
podman run --rm --security-opt label=disable \
  -v "$artefacts:/artefacts:ro" -v "$here/summit.spec:/summit.spec:ro" -v "$dist:/out" \
  "registry.fedoraproject.org/fedora:$release" bash -euo pipefail -c "
dnf install -y -q rpm-build >/dev/null
rpmbuild -bb /summit.spec --define 'artefacts /artefacts' --define 'summit_version $version' \
  --define '_rpmdir /out' --define '_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm' --quiet
"
ls "$dist"/*.rpm
