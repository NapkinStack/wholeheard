# Wholeheard's package for Fedora, made from the artefacts each module's release left in dist/.
# rpmbuild finds the libraries every program links and requires their packages itself.
Name:           wholeheard
Version:        %{wholeheard_version}
Release:        1%{?dist}
# Summit became Wholeheard (D28): installing Wholeheard takes Summit's place; the user's settings,
# meetings and keys are carried over by Wholeheard itself at its first start.
Obsoletes:      summit < 0.2.0
Provides:       summit = %{version}-%{release}
Summary:        Records a meeting, transcribes it and writes its minutes, on your machine
License:        LicenseRef-Proprietary
ExclusiveArch:  x86_64 aarch64
# Run, not linked: capture asks PipeWire which device the default stands for.
Requires:       pipewire-utils
# Opened at run time, never linked: the shell's tray icon (ayatana's first, the older one else).
Requires:       (libayatana-appindicator-gtk3 or libappindicator-gtk3)
# GNOME shows a tray icon only with this extension.
Recommends:     gnome-shell-extension-appindicator
# The graphics card's Vulkan driver: the transcript made there, a tenth of the meeting's length;
# without it, on the processor. Installed on a desktop already.
Recommends:     mesa-vulkan-drivers

%description
Wholeheard records both sides of a call from the system tray, transcribes it on the machine with
Whisper, and writes its minutes with the model you choose. Your meetings stay in your folder.

%install
install -Dm755 %{artefacts}/shell/wholeheard %{buildroot}%{_bindir}/wholeheard
install -Dm755 %{artefacts}/capture/wholeheard-capture %{buildroot}%{_bindir}/wholeheard-capture
install -Dm755 %{artefacts}/transcription/wholeheard-transcription %{buildroot}%{_bindir}/wholeheard-transcription
install -Dm755 %{artefacts}/summary/wholeheard-summary %{buildroot}%{_bindir}/wholeheard-summary
# Lighter without their symbols; a program that is no binary is left as it is.
for program in %{buildroot}%{_bindir}/*; do strip "$program" 2>/dev/null || :; done
install -Dm644 %{artefacts}/shell/dev.wholeheard.Wholeheard.desktop %{buildroot}%{_datadir}/applications/dev.wholeheard.Wholeheard.desktop
install -Dm644 %{artefacts}/shell/dev.wholeheard.Wholeheard.png %{buildroot}%{_datadir}/icons/hicolor/512x512/apps/dev.wholeheard.Wholeheard.png

%files
%{_bindir}/wholeheard
%{_bindir}/wholeheard-capture
%{_bindir}/wholeheard-transcription
%{_bindir}/wholeheard-summary
%{_datadir}/applications/dev.wholeheard.Wholeheard.desktop
%{_datadir}/icons/hicolor/512x512/apps/dev.wholeheard.Wholeheard.png
