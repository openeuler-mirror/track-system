# Track-System

自动化的上游代码追踪系统，用于追踪和分析上游社区（L0）、上游发行版（L1）和企业定制仓库（L2）之间的代码差异。

## 核心功能

- **元数据采集**：支持 GitHub、GitLab、Gitee、Gitea 和本地仓库
- **智能对比**：L1 vs L0 版本对比、L2 vs L1 内容对比
- **自动调度**：基于软件包等级的智能调度系统，支持 6 阶段流水线
- **RESTful API**：完整的 HTTP API 接口
- **认证授权**：可选的 JWT 认证和 CORS 支持
- **客户端工具**：命令行工具和独立采集器
- **组件管理**：支持组件（Component）管理，用于组织和分类软件包
- **回溯建议**：提供基于变更分析的 Backport 建议
- **数据导入导出**：支持 JSON/CSV 格式的元数据导入导出

## 系统架构

```
┌──────────────────┐         HTTP/API         ┌──────────────────┐
│                  │ ◄─────────────────────►  │                  │
│   track-cli      │                          │  track-server    │
│  (客户端CLI)      │   配置、控制、查询          │   (服务器)        │
│                  │                          │                  │
└──────────────────┘                          └────────┬─────────┘
                                                       │
                                                       │ 数据库
                                                       ▼
                                              ┌──────────────────┐
                                              │   PostgreSQL/    │
                                              │   SQLite         │
                                              └──────────────────┘
                                                       ▲
                                                       │ 采集数据
                                                       │
┌──────────────────┐                                  │
│                  │                                  │
│ track-collector  │ ─────────────────────────────────┘
│ (独立采集工具)     │   导出JSON或直接导入
│                  │
└──────────────────┘
```

## 快速开始

### 1. 编译工具

```bash
# 编译所有工具
cargo build --release

# 编译单个工具
cargo build --release --bin track-server
cargo build --release --bin track-cli
cargo build --release --bin track-collector
```

### 2. 数据库迁移

在使用系统之前，需要初始化数据库：

```bash
# 设置数据库连接 URL (示例使用 SQLite)
export DATABASE_URL=sqlite://data/track-system.db?mode=rwc

# 运行迁移
cargo run --bin track-server -- migration up
```

### 3. 启动 track-server

#### 方式：直接运行

```bash
# 服务器模式（Web API + 后台调度器）
./target/release/track-server server --addr 0.0.0.0:3000

# 仅调度器模式
./target/release/track-server scheduler-only --interval 3600

# 单次执行模式
./target/release/track-server run-once
```

#### 配置选项

可以通过命令行参数或环境变量配置：

- `--addr`: 服务器监听地址（默认：0.0.0.0:3000）
- `--database-url`: 数据库连接 URL（默认：sqlite://data/track-system.db?mode=rwc）
- `--log-level`: 日志级别（默认：info）
- `--interval`: 调度间隔（秒，默认：3600）
- `--max-concurrent`: 最大并发任务数（默认：10）

### 4. 使用客户端 (track-cli)

```bash
# 配置服务器连接
./target/release/track-cli server config --url http://localhost:3000

# 测试连接
./target/release/track-cli server ping

# 添加软件包
./target/release/track-cli package add \
  --name nginx \
  --description "High performance web server"

# 查看报告
./target/release/track-cli report list
```

### 5. 使用采集工具 (track-collector)

```bash
# 采集 L0 元数据（上游社区）
./target/release/track-collector collect l0 \
  --platform github \
  --owner nginx \
  --repo nginx \
  --output /tmp/nginx_l0.json

# 采集 L1 元数据（发行版）
./target/release/track-collector collect l1 \
  --platform gitee \
  --owner src-openeuler \
  --repo nginx \
  --output /tmp/nginx_l1.json

# 采集 L2 元数据（本地仓库）
./target/release/track-collector collect l2 \
  --local-path /path/to/nginx \
  --output /tmp/nginx_l2.json
```

## 核心概念

