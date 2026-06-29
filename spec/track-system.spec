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

# 创建符号链接到 /usr/local/bin 以便全局访问
mkdir -p %{buildroot}%{_bindir}
ln -s %{pkg_home}/bin/track-cli %{buildroot}%{_bindir}/track-cli
ln -s %{pkg_home}/bin/track-collector %{buildroot}%{_bindir}/track-collector
ln -s %{pkg_home}/bin/track-server %{buildroot}%{_bindir}/track-server

# 安装配置文件
install -m 640 .env.example %{buildroot}%{pkg_config_dir}/track-system.env.example
install -m 640 .env.example %{buildroot}%{pkg_config_dir}/track-system.env
install -m 644 config/track-cli.toml %{buildroot}%{pkg_config_dir}/track-cli.toml

# 安装 systemd 服务文件（仅 track-server 需要）
install -m 644 packaging/systemd/track-system.service %{buildroot}%{_sysconfdir}/systemd/system/track-server.service

# 安装日志轮转配置（仅 track-server 需要）
mkdir -p %{buildroot}%{_sysconfdir}/logrotate.d
install -m 644 packaging/logrotate/track-system %{buildroot}%{_sysconfdir}/logrotate.d/track-server

# 安装预置数据库文件
install -m 640 database/track-system.db %{buildroot}%{pkg_data_dir}/track-system.db

# 安装文档
mkdir -p %{buildroot}%{_docdir}/%{pkg_name}
install -m 644 README.md %{buildroot}%{_docdir}/%{pkg_name}/

# 创建空日志文件
touch %{buildroot}%{pkg_log_dir}/track-server.log

# 创建 track-cli 配置目录
mkdir -p %{buildroot}%{_sysconfdir}/track-cli

%pre
# 创建 track 用户和组（如果不存在）
getent group %{pkg_group} >/dev/null || groupadd -r %{pkg_group}
getent passwd %{pkg_user} >/dev/null || \
  useradd -r -g %{pkg_group} -d %{pkg_home} -s /sbin/nologin \
    -c "Track System service user" %{pkg_user}

%post
# 设置权限
chown -R %{pkg_user}:%{pkg_group} %{pkg_home}
chown -R %{pkg_user}:%{pkg_group} %{pkg_data_dir}
chown -R %{pkg_user}:%{pkg_group} %{pkg_log_dir}
chown %{pkg_user}:%{pkg_group} %{pkg_config_dir}/track-system.env
chmod 640 %{pkg_config_dir}/track-system.env

# 重新加载 systemd 配置
systemctl daemon-reload

# 输出安装完成提示
echo "========================================"
echo "Track System RPM 安装完成"
echo "========================================"

%preun
# 在卸载前停止服务
if [ $1 -eq 0 ]; then
    systemctl stop track-server >/dev/null 2>&1 || true
    systemctl disable track-server >/dev/null 2>&1 || true
fi

%postun
# 删除用户和组（可选，取决于策略）
# getent passwd %{pkg_user} >/dev/null && userdel -r %{pkg_user}

# 重新加载 systemd 配置
systemctl daemon-reload >/dev/null 2>&1 || true

%files
# 指定要打包的文件和目录

# 三个可执行文件
%{pkg_home}/bin/track-server
%{pkg_home}/bin/track-cli
%{pkg_home}/bin/track-collector

# 全局命令符号链接
%{_bindir}/track-cli
%{_bindir}/track-collector
%{_bindir}/track-server

# 配置文件（%config 表示配置文件，升级时不会覆盖用户修改）
%config(noreplace) %{pkg_config_dir}/track-system.env
%config(noreplace) %{_sysconfdir}/logrotate.d/track-server
%config(noreplace) %{pkg_config_dir}/track-system.env.example
%config(noreplace) %{pkg_config_dir}/track-cli.toml

# systemd 服务文件（仅 track-server）
%{_sysconfdir}/systemd/system/track-server.service

# 数据和日志目录
%dir %{pkg_home}
%dir %{pkg_home}/bin
%dir %{pkg_data_dir}
%dir %{pkg_log_dir}
%dir %{pkg_config_dir}
%dir %{_sysconfdir}/track-cli

# 预置数据库文件
%config(noreplace) %{pkg_data_dir}/track-system.db

# 文档
%doc %{_docdir}/%{pkg_name}/

# 日志文件
%{pkg_log_dir}/track-server.log

%changelog
* Wed Jan 21 2026 Si Wang <wangs88@chinatelecom.cn> - 1.2.0-2
-Fix bug #111746 #112451

* Mon Jan 05 2026 Si Wang <wangs88@chinatelecom.cn> - 1.2.0-1
- Fix bug #111656/#111794/#111788/#111785/#111779/#111683 \
- #111791/#111680/#111707/#111659

* Wed Dec 10 2025 Si Wang <wangs88@chinatelecom.cn> - 1.1.0-1
- Update version to 1.1.0

* Tue Nov 11 2025 Yong Qin <qiny15@chinatelecom.cn> - 1.0.0-1
- Initial commit
