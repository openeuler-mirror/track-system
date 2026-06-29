%global debug_package %{nil}
%define pkg_name track-system
%define pkg_version 1.2.0
%define pkg_release 2
%define pkg_user track
%define pkg_group track
%define pkg_home /opt/track-system
%define pkg_data_dir /var/lib/track-system
%define pkg_log_dir /var/log/track-system
%define pkg_config_dir /etc/track-system

Name:           %{pkg_name}
Version:        1.2.0
Release:        2
Summary:        Automated Source Code Repository Tracking and Analysis Tool

License:        MIT
Source0:        %{pkg_name}-%{pkg_version}.tar.gz

# Build Dependencies for Rust project
BuildRequires:  cargo
BuildRequires:  gcc
BuildRequires:  pkg-config
BuildRequires:  openssl-devel
BuildRequires:  libgit2-devel
BuildRequires:  git

# Runtime Dependencies
Requires:       openssl-libs
Requires:       libgit2
Requires:       postgresql-libs
Requires:       glibc >= 2.28
Requires:       git

# Create user and group during install
Requires(pre):  /usr/sbin/useradd, /usr/sbin/groupadd

%description
Track System is an automated source code repository tracking and analysis tool
written in Rust. It focuses on tracking and comparing L0 (upstream), L1 
