/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

//! 配置管理模块

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// 主配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub api: ApiConfig,
    pub scheduler: SchedulerConfig,
    pub rate_limit: RateLimitConfig,
    pub packages: Vec<PackageConfig>,
    pub distros: Vec<DistroConfig>,
    pub trackings: Vec<TrackingConfig>,
    pub server: ServerConfig,
    pub logging: LoggingConfig,
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(rename = "type")]
    pub db_type: String,
    pub sqlite: Option<SqliteConfig>,
    pub postgresql: Option<PostgresqlConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresqlConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

/// API 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub gitee: GiteeConfig,
    pub github: GithubConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiteeConfig {
    pub token: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubConfig {
    pub token: String,
    pub base_url: String,
}

/// 调度器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub max_concurrent_jobs: usize,
    pub job_timeout_secs: u64,
    pub cleanup_interval_secs: u64,
    pub health_check_interval_secs: u64,
}

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub gitee_per_minute: u32,
    pub github_per_minute: u32,
    pub burst_size: u32,
}

/// 软件包配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageConfig {
    pub name: String,
    pub level: i32,
    pub sync_interval_hours: i32,
    pub l0_repo_url: String,
    pub description: Option<String>,
}

/// 发行版配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistroConfig {
    pub name: String,
    pub version: String,
}

/// 跟踪配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingConfig {
    pub package: String,
    pub distro: String,
    pub l1: L1Config,
    pub l2: L2Config,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Config {
    pub branch: String,
    pub repo_owner: String,
    pub repo_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Config {
    pub branch: String,
    pub repo_path: String,
}

/// Web 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub output: String,
    pub file_path: Option<String>,
}

impl Config {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("无法读取配置文件: {:?}", path.as_ref()))?;

        let config: Config = serde_yaml::from_str(&content).context("无法解析配置文件")?;

        config.validate()?;

        Ok(config)
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 验证数据库配置
        match self.database.db_type.as_str() {
            "sqlite" => {
                if self.database.sqlite.is_none() {
                    anyhow::bail!("SQLite 配置缺失");
                }
            }
            "postgresql" => {
                if self.database.postgresql.is_none() {
                    anyhow::bail!("PostgreSQL 配置缺失");
                }
            }
            _ => anyhow::bail!("不支持的数据库类型: {}", self.database.db_type),
        }

        // 验证软件包配置
        if self.packages.is_empty() {
            anyhow::bail!("至少需要配置一个软件包");
        }

        // 验证发行版配置
        if self.distros.is_empty() {
            anyhow::bail!("至少需要配置一个发行版");
        }

        // 验证跟踪配置
        for tracking in &self.trackings {
            // 检查软件包是否存在
            if !self.packages.iter().any(|p| p.name == tracking.package) {
                anyhow::bail!("跟踪配置引用了不存在的软件包: {}", tracking.package);
            }

            // 检查发行版是否存在
            if !self.distros.iter().any(|d| d.name == tracking.distro) {
                anyhow::bail!("跟踪配置引用了不存在的发行版: {}", tracking.distro);
            }

            // 检查 L2 仓库路径
            let l2_path = Path::new(&tracking.l2.repo_path);
            if !l2_path.exists() {
                eprintln!("警告: L2 仓库路径不存在: {}", tracking.l2.repo_path);
            }
        }

        Ok(())
    }

    /// 获取数据库连接字符串
    pub fn database_url(&self) -> String {
        match self.database.db_type.as_str() {
            "sqlite" => {
                let sqlite = self.database.sqlite.as_ref().unwrap();
                format!("sqlite://{}?mode=rwc", sqlite.path)
            }
            "postgresql" => {
                let pg = self.database.postgresql.as_ref().unwrap();
                format!(
                    "postgresql://{}:{}@{}:{}/{}",
                    pg.username, pg.password, pg.host, pg.port, pg.database
                )
            }
            _ => panic!("不支持的数据库类型"),
        }
    }

    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self).context("无法序列化配置")?;

        fs::write(path.as_ref(), content)
            .with_context(|| format!("无法写入配置文件: {:?}", path.as_ref()))?;

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                db_type: "sqlite".to_string(),
                sqlite: Some(SqliteConfig {
                    path: "track_system.db".to_string(),
                }),
                postgresql: None,
            },
            api: ApiConfig {
                gitee: GiteeConfig {
                    token: String::new(),
                    base_url: "https://gitee.com/api/v5".to_string(),
                },
                github: GithubConfig {
                    token: String::new(),
                    base_url: "https://api.github.com".to_string(),
                },
            },
            scheduler: SchedulerConfig {
                max_concurrent_jobs: 10,
                job_timeout_secs: 1800,
                cleanup_interval_secs: 3600,
                health_check_interval_secs: 30,
            },
            rate_limit: RateLimitConfig {
                gitee_per_minute: 60,
                github_per_minute: 5000,
                burst_size: 10,
            },
            packages: vec![],
            distros: vec![],
            trackings: vec![],
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
                log_level: "info".to_string(),
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
                output: "stdout".to_string(),
                file_path: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> Config {
        let mut config = Config::default();
        config.packages.push(PackageConfig {
            name: "openssl".to_string(),
            level: 1,
            sync_interval_hours: 24,
            l0_repo_url: "https://example.test/openssl.git".to_string(),
            description: Some("crypto library".to_string()),
        });
        config.distros.push(DistroConfig {
            name: "CTyunOS".to_string(),
            version: "25.07".to_string(),
        });
        config.trackings.push(TrackingConfig {
            package: "openssl".to_string(),
            distro: "CTyunOS".to_string(),
            l1: L1Config {
                branch: "openEuler-24.03-LTS-SP3".to_string(),
                repo_owner: "src-openeuler".to_string(),
                repo_name: "openssl".to_string(),
            },
            l2: L2Config {
                branch: "CTyunOS25.07".to_string(),
                repo_path: "/path/that/does/not/need/to/exist".to_string(),
            },
            status: "active".to_string(),
        });
        config
    }

    #[test]
    fn validate_accepts_minimal_sqlite_config() {
        let config = valid_config();

        assert!(config.validate().is_ok());
        assert_eq!(config.database_url(), "sqlite://track_system.db?mode=rwc");
    }

    #[test]
    fn database_url_formats_postgresql_connection() {
        let mut config = valid_config();
        config.database = DatabaseConfig {
            db_type: "postgresql".to_string(),
            sqlite: None,
            postgresql: Some(PostgresqlConfig {
                host: "db.example.test".to_string(),
                port: 5432,
                database: "track".to_string(),
                username: "track_user".to_string(),
                password: "secret".to_string(),
            }),
        };

        assert!(config.validate().is_ok());
        assert_eq!(
            config.database_url(),
            "postgresql://track_user:secret@db.example.test:5432/track"
        );
    }

    #[test]
    fn validate_rejects_missing_database_details() {
        let mut config = valid_config();
        config.database.sqlite = None;

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("SQLite 配置缺失"));
    }

    #[test]
    fn validate_rejects_unknown_database_type() {
        let mut config = valid_config();
        config.database.db_type = "mysql".to_string();

        let err = config.validate().unwrap_err().to_string();

        assert!(err.contains("不支持的数据库类型"));
    }

    #[test]
    fn validate_rejects_empty_packages_and_distros() {
        let mut config = valid_config();
        config.packages.clear();
        assert!(config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("至少需要配置一个软件包"));

        let mut config = valid_config();
        config.distros.clear();
        assert!(config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("至少需要配置一个发行版"));
    }

    #[test]
    fn validate_rejects_tracking_references_to_missing_entities() {
        let mut config = valid_config();
        config.trackings[0].package = "missing".to_string();
        assert!(config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("跟踪配置引用了不存在的软件包"));

        let mut config = valid_config();
        config.trackings[0].distro = "missing".to_string();
        assert!(config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("跟踪配置引用了不存在的发行版"));
    }

    #[test]
    fn save_to_file_and_from_file_round_trip_yaml_config() {
        let config = valid_config();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");

        config.save_to_file(&path).unwrap();
        let loaded = Config::from_file(&path).unwrap();

        assert_eq!(loaded.packages[0].name, "openssl");
        assert_eq!(loaded.distros[0].version, "25.07");
        assert_eq!(loaded.trackings[0].l1.repo_name, "openssl");
    }

    #[test]
    fn from_file_reports_yaml_parse_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.yaml");
        std::fs::write(&path, "database: [").unwrap();

        let err = Config::from_file(&path).unwrap_err().to_string();

        assert!(err.contains("无法解析配置文件"));
    }
}
