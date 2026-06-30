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
(distribution) and L2 (local) repositories, supporting monitoring of openEuler, 
Anolis, and OpenCloud source code repository changes.

The system consists of three independent tools:
- track-server: RESTful API server with database and scheduler
- track-cli: Pure client tool for user interaction
- track-collector: Standalone metadata collection tool (no database required)

Key Features:
- Three-layer architecture (L0 → L1 → L2) tracking
- Multi-platform support (GitHub, GitLab, Gitee, Gitea, Local)
- Automated change classification (CVE, version upgrade, features, etc.)
- Git repository comparison and analysis
- Priority-based sync scheduling
- Workflow engine for custom processing
- Isolated environment deployment support

%prep
# 解包源码
%setup -q -n %{pkg_name}-%{pkg_version}

%build
# 使用 Release 模式编译以获得最佳性能
# 编译三个独立的二进制文件
sh build.sh

%install
# 创建安装目录结构
mkdir -p %{buildroot}%{pkg_home}/bin
mkdir -p %{buildroot}%{pkg_home}/lib
mkdir -p %{buildroot}%{pkg_data_dir}
mkdir -p %{buildroot}%{pkg_log_dir}
mkdir -p %{buildroot}%{pkg_config_dir}
mkdir -p %{buildroot}%{_sysconfdir}/systemd/system
mkdir -p %{buildroot}%{_localstatedir}/lib/track-system/migrations

# 安装三个二进制文件
install -m 755 target/release/track-server %{buildroot}%{pkg_home}/bin/
install -m 755 target/release/track-cli %{buildroot}%{pkg_home}/bin/
install -m 755 target/release/track-collector %{buildroot}%{pkg_home}/bin/
