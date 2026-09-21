# Track-System

An automated upstream code tracking system for tracking and analyzing code differences between upstream communities (L0), upstream distributions (L1), and enterprise-customized repositories (L2).

## Core Features

- **Metadata collection**: Supports GitHub, GitLab, Gitee, Gitea, and local repositories
- **Smart comparison**: L1 vs L0 version comparison, L2 vs L1 content comparison
- **Automatic scheduling**: An intelligent scheduling system based on package level, supporting a 6-stage pipeline
- **RESTful API**: Complete HTTP API interfaces
- **Authentication and authorization**: Optional JWT authentication and CORS support
- **Client tools**: Command-line tool and standalone collector
- **Component management**: Supports component (Component) management for organizing and categorizing packages
- **Backport suggestions**: Provides Backport suggestions based on change analysis
- **Data import and export**: Supports metadata import and export in JSON/CSV format

## System Architecture

```
┌──────────────────┐         HTTP/API         ┌──────────────────┐
│                  │ ◄─────────────────────►  │                  │
│   track-cli      │                          │  track-server    │
│   (Client CLI)   │  Config, control, query  │    (Server)      │
│                  │                          │                  │
└──────────────────┘                          └────────┬─────────┘
                                                       │
                                                       │ Database
                                                       ▼
                                              ┌──────────────────┐
                                              │   PostgreSQL/    │
                                              │   SQLite         │
                                              └──────────────────┘
                                                       ▲
                                                       │ Collected data
                                                       │
┌──────────────────┐                                   │
│                  │                                   │
│ track-collector  │ ──────────────────────────────────┘
│ (Standalone tool)│   Export JSON or import directly
│                  │
└──────────────────┘
```

## Quick Start

### 1. Build Tools

```bash
# Build all tools
cargo build --release

# Build a single tool
cargo build --release --bin track-server
cargo build --release --bin track-cli
cargo build --release --bin track-collector
```

### 2. Database Migration

Before using the system, the database needs to be initialized:

```bash
# Set the database connection URL (the example uses SQLite)
export DATABASE_URL=sqlite://data/track-system.db?mode=rwc

# Run the migration
cargo run --bin track-server -- migration up
```

### 3. Start track-server

#### Method: Run Directly

```bash
# Server mode (Web API + background scheduler)
./target/release/track-server server --addr 0.0.0.0:3000

# Scheduler-only mode
./target/release/track-server scheduler-only --interval 3600

# Run-once mode
./target/release/track-server run-once
```

#### Configuration Options

It can be configured through command-line parameters or environment variables:

- `--addr`: Server listening address (default: 0.0.0.0:3000)
- `--database-url`: Database connection URL (default: sqlite://data/track-system.db?mode=rwc)
- `--log-level`: Log level (default: info)
- `--interval`: Scheduling interval (seconds, default: 3600)
- `--max-concurrent`: Maximum number of concurrent tasks (default: 10)

### 4. Use the Client (track-cli)

```bash
# Configure the server connection
./target/release/track-cli server config --url http://localhost:3000

# Test the connection
./target/release/track-cli server ping

# Add a package
./target/release/track-cli package add \
  --name nginx \
  --description "High performance web server"

# View reports
./target/release/track-cli report list
```

### 5. Use the Collection Tool (track-collector)

```bash
# Collect L0 metadata (upstream community)
./target/release/track-collector collect l0 \
  --platform github \
  --owner nginx \
  --repo nginx \
  --output /tmp/nginx_l0.json

# Collect L1 metadata (distribution)
./target/release/track-collector collect l1 \
  --platform gitee \
  --owner src-openeuler \
  --repo nginx \
  --output /tmp/nginx_l1.json

# Collect L2 metadata (local repository)
./target/release/track-collector collect l2 \
  --local-path /path/to/nginx \
  --output /tmp/nginx_l2.json
```

## Core Concepts

### Three Levels

- **L0 (Upstream community)**: The official repository of an open source project (e.g., github.com/nginx/nginx)
- **L1 (Upstream distribution)**: The source code repository of a Linux distribution (e.g., src-openeuler/nginx)
- **L2 (Enterprise customization)**: Repositories customized by enterprises based on a distribution

### Two Types of Comparison

#### L1 vs L0 Comparison (Version Comparison)
- **Purpose**: Discover the version differences of the distribution relative to the upstream community
- **Comparison method**: Based on version information (not commit SHA)
- **Output**: Version differences, upgradable versions, patch status, CVE analysis, upgrade suggestions

#### L2 vs L1 Comparison (Content Comparison)
- **Purpose**: Discover the differences of enterprise customization relative to the distribution
- **Comparison method**: Based on file content (spec, patches, source code)
- **Output**: Content differences, customization analysis, sync suggestions, conflict detection

### 6-Stage Pipeline

The scheduler automatically executes the complete synchronization pipeline:

1. **L0 metadata acquisition**: Synchronize data from the upstream community
2. **L1 metadata acquisition**: Synchronize data from the distribution
3. **L1 vs L0 comparison**: Generate a version difference report
4. **L2 snapshot generation**: Generate a local repository snapshot
5. **L2 vs L1 comparison**: Generate a content difference report
6. **Final report generation**: Aggregate all results

## Tech Stack

- **Language**: Rust 2021 Edition
- **Web framework**: Axum
- **Database ORM**: SeaORM (supports SQLite, PostgreSQL)
- **Async runtime**: Tokio
- **Command-line parsing**: Clap
- **HTTP client**: Reqwest
- **Serialization**: Serde
- **Authentication**: JWT (jsonwebtoken)
- **Task scheduling**: tokio-cron-scheduler
- **Logging**: Tracing

## Project Structure

```
track-sys/
├── src/
│   ├── bin/              # Binary entry points
│   │   ├── track_server.rs    # Server
│   │   ├── track_cli.rs       # Client CLI
│   │   └── track-collector.rs # Standalone collector
│   ├── server/           # Server module (API, Middleware, Routes)
│   ├── cli/              # CLI module (Commands, Parser)
│   ├── collectors/       # Collectors (GitHub, GitLab, Gitee, Gitea, Local)
│   ├── scheduler/        # Scheduler (Pipeline, Executors)
│   ├── diff/             # Diff engine (L1 vs L0, L2 vs L1)
│   ├── entities/         # Data entities (SeaORM Model)
│   ├── utils/            # Utility functions
│   ├── analyzer/         # Change analyzer
│   ├── backport_advisor/ # Backport advisor
│   ├── snapshot/         # Snapshot management
│   ├── component/        # Component management
│   ├── importer/         # Data import
│   ├── exporter/         # Data export
│   ├── workflow/         # Workflow engine
│   └── lib.rs            # Library entry
├── docs/                 # Documentation
├── tests/                # Tests
├── migration/            # Database migration scripts
└── config/               # Configuration files
```

## Configuration

### Environment Variables (Recommended)

```bash
# Server configuration
export HOST="0.0.0.0"
export PORT=8080

# Database configuration
export DATABASE_URL="postgresql://user:password@localhost/track_system"

# Authentication configuration
export AUTH_ENABLED=false              # Open mode (default)
export JWT_SECRET="your-secret-key"    # Required in secure mode
export JWT_EXPIRY_HOURS=24

# CORS configuration
export CORS_ALLOWED_ORIGINS="*"        # Development environment
export CORS_ALLOW_CREDENTIALS=true

# API token (used by the collector)
export GITHUB_TOKEN="your_github_token"
export GITLAB_TOKEN="your_gitlab_token"
export GITEE_TOKEN="your_gitee_token"
export GITEA_TOKEN="your_gitea_token"
```

### Configuration File

You can use `config.toml` or `test_config.yaml` for configuration.

## Testing

```bash
# Run all tests
cargo test --workspace

# Run a specific test
cargo test --test unit collectors_test

# Generate test coverage
./run_coverage.sh
```

## Contributing

Contributions are welcome! Please follow these steps:

1. Fork the project
2. Create a feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## License

This project is licensed under the Mulan PSL v2 license. See the LICENSE file for details.

## Authors

- Yong Qin <qiny15@chinatelecom.cn>
- Si Wang <wangs88@chinatelecom.cn>
