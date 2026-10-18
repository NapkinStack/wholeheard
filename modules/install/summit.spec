# Summit's package for Fedora, made from the artefacts each module's release left in dist/.
# rpmbuild finds the libraries every program links and requires their packages itself.
Name:           summit
Version:        %{summit_version}
Release:        1%{?dist}
Summary:        Records a meeting, transcribes it and writes its minutes, on your machine
License:        LicenseRef-Proprietary
ExclusiveArch:  x86_64 aarch64
# Run, not linked: capture asks PipeWire which device the default stands for.
Requires:       pipewire-utils
# Opened at run time, never linked: the shell's tray icon (ayatana's first, the older one else).
Requires:       (libayatana-appindicator-gtk3 or libappindicator-gtk3)
# GNOME shows a tray icon only with this extension.
Recommends:     gnome-shell-extension-appindicator

%description
Summit records both sides of a call from the system tray, transcribes it on the machine with
Whisper, and writes its minutes with the model you choose. Your meetings stay in your folder.

%install
install -Dm755 %{artefacts}/shell/summit %{buildroot}%{_bindir}/summit
install -Dm755 %{artefacts}/capture/summit-capture %{buildroot}%{_bindir}/summit-capture
install -Dm755 %{artefacts}/transcription/summit-transcription %{buildroot}%{_bindir}/summit-transcription
install -Dm755 %{artefacts}/summary/summit-summary %{buildroot}%{_bindir}/summit-summary
# Lighter without their symbols; a program that is no binary is left as it is.
for program in %{buildroot}%{_bindir}/*; do strip "$program" 2>/dev/null || :; done
install -Dm644 %{artefacts}/shell/dev.summit.Summit.desktop %{buildroot}%{_datadir}/applications/dev.summit.Summit.desktop
install -Dm644 %{artefacts}/shell/dev.summit.Summit.png %{buildroot}%{_datadir}/icons/hicolor/512x512/apps/dev.summit.Summit.png

%files
%{_bindir}/summit
%{_bindir}/summit-capture
%{_bindir}/summit-transcription
%{_bindir}/summit-summary
%{_datadir}/applications/dev.summit.Summit.desktop
%{_datadir}/icons/hicolor/512x512/apps/dev.summit.Summit.png
