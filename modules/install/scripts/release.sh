#!/usr/bin/env bash
# What a user installs, for the system WHOLEHEARD_TARGET names — as the programs were built for it
# (D27): `fedora-*`, an rpm made with rpmbuild; `ubuntu-*`, a .deb made with dpkg; unset, the
# package of the system it runs on. One package of the four programs, made in a container of that system so that
# the libraries each program links are read there and required by their packages.
# Reads what the assembled modules' releases left beside this one's folder ($NSTACK_DIST/..).
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
dist=${NSTACK_DIST:?run through nstack release, which names this module\'s artefacts folder}
artefacts=$(cd "$dist/.." && pwd)
version=$(cat "$artefacts/shell/VERSION")
# Unset, the system this runs on, as it names itself (fedora-44, ubuntu-24.04).
target=${WHOLEHEARD_TARGET:-$(. /etc/os-release && echo "$ID-$VERSION_ID")}
# No relabelling of what is mounted (`:Z` would relabel the files in place): labels left alone.
case $target in
  fedora-[0-9]*)
    podman run --rm --security-opt label=disable \
      -v "$artefacts:/artefacts:ro" -v "$here/wholeheard.spec:/wholeheard.spec:ro" -v "$dist:/out" \
      "registry.fedoraproject.org/fedora:${target#fedora-}" bash -euo pipefail -c "
dnf install -y -q rpm-build >/dev/null
rpmbuild -bb /wholeheard.spec --define 'artefacts /artefacts' --define 'wholeheard_version $version' \
  --define '_rpmdir /out' --define '_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm' --quiet
"
    ls "$dist"/*.rpm
    ;;
  ubuntu-[0-9]*)
    podman run --rm --security-opt label=disable -e version="$version" \
      -v "$artefacts:/artefacts:ro" -v "$here/deb:/deb:ro" -v "$dist:/out" \
      "docker.io/library/ubuntu:${target#ubuntu-}" bash -euo pipefail /deb/build.sh
    ls "$dist"/*.deb
    ;;
  *)
    echo "release: no package is made for '$target' here (fedora-<n>, ubuntu-<n>.<n>)" >&2
    exit 2
    ;;
esac
