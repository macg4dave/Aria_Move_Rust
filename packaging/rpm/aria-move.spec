# Built by scripts/docker-rpm-check.sh on AlmaLinux 9 (glibc 2.34 baseline) with
# rustup Rust, so BuildRequires lists only what rustup cannot provide.
%global debug_package %{nil}
%global _build_id_links none

Name:           aria-move
Version:        %{pkg_version}
Release:        %{?pkg_release}%{!?pkg_release:1}%{?dist}
Summary:        Safely move completed aria2 downloads
License:        MIT
URL:            https://github.com/macg4dave/Aria_Move_Rust
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  gcc
BuildRequires:  systemd-rpm-macros
Requires:       aria2
Requires(pre):  shadow-utils
%{?systemd_requires}

%description
Verified cross-filesystem moves of completed aria2 downloads, with a managed,
restricted aria2 service (aria-move.service) and authenticated localhost RPC.

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --locked --all-features

%check
cargo test --locked --all-features

%install
install -D -m755 target/release/aria_move %{buildroot}%{_bindir}/aria_move
install -D -m755 packaging/start-service %{buildroot}%{_libexecdir}/aria-move/start-service
install -D -m755 packaging/prepare-state %{buildroot}%{_libexecdir}/aria-move/prepare-state
# The shared unit names the Debian launcher path; RPM systems use libexec.
install -d %{buildroot}%{_unitdir}
sed 's|/usr/lib/aria-move/|%{_libexecdir}/aria-move/|g' debian/aria-move.service \
    > %{buildroot}%{_unitdir}/aria-move.service
install -D -m644 packaging/config.xml %{buildroot}%{_sysconfdir}/aria_move/config.xml
install -D -m644 packaging/aria2.conf %{buildroot}%{_sysconfdir}/aria_move/aria2.conf
install -D -m644 packaging/aria_move.1 %{buildroot}%{_mandir}/man1/aria_move.1
install -d %{buildroot}%{_sharedstatedir}/aria-move %{buildroot}%{_localstatedir}/log/aria-move

%pre
getent group aria-move >/dev/null || groupadd -r aria-move
getent passwd aria-move >/dev/null || \
    useradd -r -g aria-move -d %{_sharedstatedir}/aria-move -M -s /sbin/nologin \
        -c "Aria Move service" aria-move
exit 0

%post
%{_libexecdir}/aria-move/prepare-state
%systemd_post aria-move.service

%preun
%systemd_preun aria-move.service

%postun
%systemd_postun_with_restart aria-move.service

%files
%license LICENSE
%doc README.md docs/INSTALLATION.md
%{_bindir}/aria_move
%dir %{_libexecdir}/aria-move
%{_libexecdir}/aria-move/start-service
%{_libexecdir}/aria-move/prepare-state
%{_unitdir}/aria-move.service
%dir %{_sysconfdir}/aria_move
%config(noreplace) %{_sysconfdir}/aria_move/config.xml
%config(noreplace) %{_sysconfdir}/aria_move/aria2.conf
%{_mandir}/man1/aria_move.1*
# Downloads, token, session and logs are kept on removal, like the Debian package.
%dir %attr(0750,aria-move,aria-move) %{_sharedstatedir}/aria-move
%dir %attr(0750,aria-move,aria-move) %{_localstatedir}/log/aria-move

%changelog
* Sat Oct 31 2026 Dave <macg4dave@users.noreply.github.com> - 1.5.1-1
- Add native RPM packaging with the managed aria2 service.
