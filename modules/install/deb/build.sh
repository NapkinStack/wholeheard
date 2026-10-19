# Run in an Ubuntu container by scripts/release.sh: the .deb of the four programs, its library
# dependencies read by dpkg-shlibdeps from what each program links — the libraries installed
# here for that, as a user's system has them —, and what is run or opened at run time, not
# linked, named by hand: pw-metadata for capture, the tray's library for the shell.
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq --no-install-recommends dpkg-dev binutils \
  libwebkit2gtk-4.1-0 libgtk-3-0t64 librsvg2-2 libpipewire-0.3-0t64 libasound2t64 libvulkan1 >/dev/null
root=/tmp/package
for program in shell/summit capture/summit-capture transcription/summit-transcription summary/summit-summary; do
  install -Dm755 "/artefacts/$program" "$root/usr/bin/${program#*/}"
done
# Lighter without their symbols; a program that is no binary is left as it is.
for program in "$root"/usr/bin/*; do strip --strip-unneeded "$program" 2>/dev/null || :; done
install -Dm644 /artefacts/shell/dev.summit.Summit.desktop "$root/usr/share/applications/dev.summit.Summit.desktop"
install -Dm644 /artefacts/shell/dev.summit.Summit.png "$root/usr/share/icons/hicolor/512x512/apps/dev.summit.Summit.png"
# dpkg-shlibdeps reads a source package's control file where it runs: one is made for it.
mkdir -p /tmp/shlibs/debian
printf 'Source: summit\n\nPackage: summit\nArchitecture: any\n' > /tmp/shlibs/debian/control
binaries=()
for program in "$root"/usr/bin/*; do
  [ "$(head -c 4 "$program")" = $'\x7fELF' ] && binaries+=("$program")
done
linked=""
if [ ${#binaries[@]} -gt 0 ]; then
  linked=$(cd /tmp/shlibs && dpkg-shlibdeps -O "${binaries[@]}" | sed -n 's/^shlibs:Depends=//p')
fi
depends="${linked:+$linked, }pipewire-bin, libayatana-appindicator3-1 | libappindicator3-1"
mkdir -p "$root/DEBIAN"
# Filled in by bash, never sed: no character of a dependency can break the line it goes in.
shopt -u patsub_replacement 2>/dev/null || :   # bash 5.2: an & in what replaces is that, not the match
control=$(< /deb/control.in)
control=${control//@VERSION@/$version}
control=${control//@ARCH@/$(dpkg --print-architecture)}
control=${control//@SIZE@/$(du -sk "$root/usr" | cut -f1)}
control=${control//@DEPENDS@/$depends}
printf '%s\n' "$control" > "$root/DEBIAN/control"
dpkg-deb --build --root-owner-group "$root" "/out/summit_${version}_$(dpkg --print-architecture).deb" >/dev/null
