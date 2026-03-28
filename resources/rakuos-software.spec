%global app_id org.rakuos.Software

Name:           rakuos-software
Version:        1.0.0
Release:        1%{?dist}
Summary:        RakuOS Software Center
License:        GPL-3.0-or-later
URL:            https://rakuos.org
Source0:        https://github.com/RakuOS/rakuos-software/archive/refs/heads/main/%{name}-main.tar.gz

BuildRequires:  rust-packaging >= 21
BuildRequires:  cargo-rpm-macros
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(libadwaita-1)
BuildRequires:  pkgconfig(openssl)
BuildRequires:  pkgconfig(gdk-pixbuf-2.0)
BuildRequires:  pkgconfig(glib-2.0)
BuildRequires:  pkgconfig(Qt6Core)
BuildRequires:  pkgconfig(Qt6Quick)
BuildRequires:  pkgconfig(Qt6Widgets)
BuildRequires:  pkgconfig(dbus-1)

%description
RakuOS Software Center — a modern app store for the RakuOS operating
system. Supports RPM overlay packages via DNF, Flatpak from Flathub,
AppImages, and Web Apps. Provides system update management for
image-based OS updates.

# ─── Common subpackage ───────────────────────────────────────────────────────
%package common
Summary:        RakuOS Software Center — shared files and tray daemon
Requires:       curl
Requires:       flatpak
Requires:       rpm

%description common
Shared wrapper script, data files, desktop integration, and the
background tray daemon for the RakuOS Software Center.
The wrapper auto-detects which UI frontend (GTK or Qt) is installed.

# ─── GTK subpackage ──────────────────────────────────────────────────────────
%package gtk
Summary:        RakuOS Software Center — GTK4/libadwaita frontend
Requires:       %{name}-common = %{version}-%{release}
Requires:       gtk4
Requires:       libadwaita

%description gtk
GTK4/libadwaita frontend for the RakuOS Software Center.
Designed for GNOME and other GTK-based desktop environments.

# ─── Qt subpackage ───────────────────────────────────────────────────────────
%package qt
Summary:        RakuOS Software Center — Qt6/QML frontend
Requires:       %{name}-common = %{version}-%{release}
Requires:       qt6-qtbase
Requires:       qt6-qtdeclarative

%description qt
Qt6/QML frontend for the RakuOS Software Center.
Designed for KDE Plasma and other Qt-based desktop environments.

# ─── Prep ────────────────────────────────────────────────────────────────────
%prep
%autosetup -n %{name}-main

# ─── Build ───────────────────────────────────────────────────────────────────
%build
cargo build --locked --release --bin rakuos-software-gtk
cargo build --locked --release --bin rakuos-software-qt
cargo build --locked --release --bin rakuos-software-tray

# ─── Install ─────────────────────────────────────────────────────────────────
%install
# Frontend binaries
install -Dm755 target/release/rakuos-software-gtk \
    %{buildroot}%{_libexecdir}/rakuos/software/rakuos-software-gtk
install -Dm755 target/release/rakuos-software-qt \
    %{buildroot}%{_libexecdir}/rakuos/software/rakuos-software-qt

# Tray daemon binary
install -Dm755 target/release/rakuos-software-tray \
    %{buildroot}%{_libexecdir}/rakuos/software/rakuos-software-tray

# Wrapper scripts (from resources/ — used by both desktop files)
install -Dm755 resources/rakuos-software \
    %{buildroot}%{_bindir}/rakuos-software
install -Dm755 resources/rakuos-webapp-launcher \
    %{buildroot}%{_bindir}/rakuos-webapp-launcher
install -Dm755 resources/rakuos-webapp-launcher-libexec \
    %{buildroot}%{_libexecdir}/rakuos/software/rakuos-webapp-launcher

# Main app desktop file (/usr/share/applications/)
install -Dm644 resources/rakuos-software.desktop \
    %{buildroot}%{_datadir}/applications/%{app_id}.desktop

# XDG autostart — tray daemon via wrapper's --tray flag
install -Dm644 resources/rakuos-software-tray.desktop \
    %{buildroot}%{_sysconfdir}/xdg/autostart/rakuos-software-tray.desktop

# Web app catalog
install -dm755 %{buildroot}%{_datadir}/rakuos/webapps
install -m644 resources/webapps/*.json \
    %{buildroot}%{_datadir}/rakuos/webapps/

# AppStream overrides and data
install -dm755 %{buildroot}%{_datadir}/rakuos/appstream
install -m644 resources/appstream/appstream-overrides.json \
    %{buildroot}%{_datadir}/rakuos/appstream/
install -m644 resources/appstream/flatpak-to-rpm.json \
    %{buildroot}%{_datadir}/rakuos/appstream/
install -dm755 %{buildroot}%{_datadir}/rakuos/appstream/data
install -m644 resources/appstream/data/*.xml \
    %{buildroot}%{_datadir}/rakuos/appstream/data/ 2>/dev/null || true
install -dm755 %{buildroot}%{_datadir}/rakuos/appstream/icons
install -m644 resources/appstream/icons/*.png \
    %{buildroot}%{_datadir}/rakuos/appstream/icons/ 2>/dev/null || true

# QML files
install -dm755 %{buildroot}%{_datadir}/rakuos-software-qt
install -dm644 crates/ui-qt/qml/* \
    %{buildroot}%{_datadir}/rakuos-software-qt/
# ─── Files ───────────────────────────────────────────────────────────────────
%files common
%license LICENSE
%doc README.md
%{_bindir}/rakuos-software
%{_bindir}/rakuos-webapp-launcher
%{_libexecdir}/rakuos/software/rakuos-software-tray
%{_libexecdir}/rakuos/software/rakuos-webapp-launcher
%{_datadir}/applications/%{app_id}.desktop
%{_sysconfdir}/xdg/autostart/rakuos-software-tray.desktop
%{_datadir}/rakuos/

%files gtk
%{_libexecdir}/rakuos/software/rakuos-software-gtk

%files qt
%{_libexecdir}/rakuos/software/rakuos-software-qt
%{_datadir}/rakuos-software-qt/
# ─── Changelog ───────────────────────────────────────────────────────────────
%changelog
* Fri Mar 27 2026 RakuOS Project <dev@rakuos.org> - 1.0.0-1
- Initial package release with GTK4 and Qt6 frontends
