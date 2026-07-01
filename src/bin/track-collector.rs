/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. ctscat is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

//! Track Collector - 轻量化元数据采集工具
//!
//! 无需数据库依赖，直接从 Gitee/GitHub/GitLab/Gitea/Local 采集仓库元数据并导出为 JSON
//! 适用于隔离环境（如内网）的数据采集场景

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};
use track_system::collectors::traits::{CollectConfig, Collector, Platform};
use track_system::collectors::{
    adapters::{GitHubAdapter, GitLabAdapter, GiteaAdapter, GiteeAdapter},
    gitea::GiteaClient,
    gitee::GiteeClient,
    github::GitHubClient,
    gitlab::GitLabClient,
    local::LocalClient,
};
use track_system::i18n::{apply_clap_i18n, apply_help_i18n, detect_lang_from_args, init_i18n};

/// 采集层级
#[derive(Debug, Clone, Copy, ValueEnum)]
enum Level {
    /// L0 - 上游社区仓库
    L0,
    /// L1 - 发行版仓库
    L1,
    /// L2 - 企业发行版仓库
    L2,
}

impl Level {
    fn as_str(&self) -> &'static str {
        match self {
            Level::L0 => "l0",
            Level::L1 => "l1",
            Level::L2 => "l2",
        }
    }
}

/// 平台类型（用于 CLI）
#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "lowercase")]
enum PlatformArg {
    GitHub,
    GitLab,
    Gitee,
    Gitea,
    Local,
}

impl From<PlatformArg> for Platform {
    fn from(arg: PlatformArg) -> Self {
        match arg {
            PlatformArg::GitHub => Platform::GitHub,
            PlatformArg::GitLab => Platform::GitLab,
            PlatformArg::Gitee => Platform::Gitee,
            PlatformArg::Gitea => Platform::Gitea,
            PlatformArg::Local => Platform::Local,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "track-collector",
    about = "轻量化仓库元数据采集工具",
    long_about = None
)]
#[command(version)]
struct Cli {
    /// 语言（zh-CN / en-US）
    #[arg(long, global = true)]
    lang: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 采集单个仓库元数据
    Collect {
        /// 采集层级（l0/l1/l2）
        level: Level,

        /// 平台类型
        #[arg(long, value_enum)]
        platform: PlatformArg,

        /// 仓库所有者（远端平台需要）
        #[arg(long)]
        owner: Option<String>,

        /// 仓库名称（远端平台需要）
        #[arg(long)]
        repo: Option<String>,

        /// 本地仓库路径（local 平台需要）
        #[arg(long)]
        repo_path: Option<PathBuf>,

        /// 分支名称
        #[arg(long, default_value = "master")]
        branch: String,

        /// API 地址（自建平台需要，如 Gitea）
        #[arg(long)]
        api_url: Option<String>,

        /// 认证 token
        #[arg(long, env = "COLLECTOR_TOKEN")]
        token: Option<String>,

        /// 采集数量限制
        #[arg(long)]
        limit: Option<u32>,

        /// 输出文件路径
        #[arg(short, long, default_value = "metadata.json")]
        output: PathBuf,
    },

    /// 批量采集多个仓库
    Batch {
        /// 配置文件路径（YAML 格式）
        #[arg(short, long)]
        config: PathBuf,

        /// 输出目录
        #[arg(short, long, default_value = "output")]
        output_dir: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().collect();
    let arg_lang = detect_lang_from_args(&raw_args);
    let locale = init_i18n(arg_lang.as_deref());

    let mut cmd = Cli::command();
    apply_clap_i18n(&mut cmd, "track_collector");
    apply_help_i18n(&mut cmd, "track_collector", &locale);
    let matches = cmd.get_matches_from(raw_args);
    let cli = <Cli as clap::FromArgMatches>::from_arg_matches(&matches).unwrap();
    init_i18n(cli.lang.as_deref());

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .without_time()
        .init();

    run(cli).await
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Collect {
            level,
            platform,
            owner,
            repo,
            repo_path,
            branch,
            api_url,
            token,
            limit,
            output,
        } => {
            collect_single(
                level, platform, owner, repo, repo_path, branch, api_url, token, limit, output,
            )
            .await?;
        }
        Commands::Batch { config, output_dir } => {
            collect_batch(&config, &output_dir).await?;
        }
    }

    Ok(())
}

/// 采集单个仓库
#[allow(clippy::too_many_arguments)]
async fn collect_single(
    level: Level,
    platform: PlatformArg,
    owner: Option<String>,
    repo: Option<String>,
    repo_path: Option<PathBuf>,
    branch: String,
    api_url: Option<String>,
    token: Option<String>,
    limit: Option<u32>,
    output: PathBuf,
) -> Result<()> {
    info!(" 开始采集元数据");
    info!("   层级: {}", level.as_str());
    info!("   平台: {:?}", platform);

    // 构建配置
    let platform: Platform = platform.into();
    let mut config = CollectConfig::new(platform, branch.clone());

    // 设置远端或本地仓库
    match platform {
        Platform::Local => {
            let path = repo_path.context("Local platform requires --repo-path")?;
            config = config.with_local_path(path);
            info!("   仓库路径: {:?}", config.repo_path);
        }
        _ => {
            let owner = owner.context("Remote platform requires --owner")?;
            let repo = repo.context("Remote platform requires --repo")?;
            config = config.with_remote(owner.clone(), repo.clone());
            info!("   仓库: {}/{}", owner, repo);
        }
    }

    // 设置可选参数
    if let Some(url) = api_url {
        config = config.with_api_url(url);
    }
    if let Some(t) = token {
        config = config.with_token(t);
    }
    if let Some(l) = limit {
        config = config.with_limit(l);
    }
    config = config.with_level(level.as_str());

    info!("   分支: {}", branch);

    // 创建采集器
    let collector: Box<dyn Collector> = match platform {
        Platform::GitHub => {
            let token = config
                .token
                .as_ref()
                .context("GitHub requires token")?
                .clone();
            let client = GitHubClient::new(token).context("Failed to create GitHub client")?;
            Box::new(GitHubAdapter::new(client, Platform::GitHub))
        }
        Platform::Gitee => {
            let token = config
                .token
                .as_ref()
                .context("Gitee requires token")?
                .clone();
            let client = GiteeClient::new(token).context("Failed to create Gitee client")?;
            Box::new(GiteeAdapter::new(client, Platform::Gitee))
        }
        Platform::Gitea => {
            let token = config
                .token
                .as_ref()
                .context("Gitea requires token")?
                .clone();
            let api_url = config
                .api_url
                .as_ref()
                .context("Gitea requires --api-url")?
                .clone();
            let client =
                GiteaClient::new(token, api_url).context("Failed to create Gitea client")?;
            Box::new(GiteaAdapter::new(client, Platform::Gitea))
        }
        Platform::GitLab => {
            let token = config
                .token
                .as_ref()
                .context("GitLab requires token")?
                .clone();

            // 如果提供了 api_url，使用自定义 GitLab 实例
            let client = if let Some(api_url) = &config.api_url {
                GitLabClient::with_base_url(api_url, token)
            } else {
                GitLabClient::new(token)
            }
            .context("Failed to create GitLab client")?;

            Box::new(GitLabAdapter::new(client, Platform::GitLab))
        }
        Platform::Local => {
            let path = config
                .repo_path
                .as_ref()
                .context("Local requires --repo-path")?
                .clone();
            let client = LocalClient::new(path).context("Failed to create Local client")?;
            Box::new(client)
        }
    };

    // 验证配置
    collector
        .validate_config(&config)
        .context("Invalid configuration")?;

    // 执行采集
    info!(" 正在采集...");
    let mut result = collector
        .collect(&config)
        .await
        .context("Failed to collect metadata")?;

    // 设置正确的层级
    result.level = level.as_str().to_string();

    info!("采集完成");
    info!("   Commits: {}", result.commits.len());
    if let Some(snapshot) = &result.snapshot {
        info!("   Spec 版本: {:?}", snapshot.spec_version);
        info!("   Patches: {}", snapshot.patches.len());
        info!("   文件数: {}", snapshot.file_count);
    }

    // 保存到文件
    let json = serde_json::to_string_pretty(&result).context("Failed to serialize result")?;
    fs::write(&output, json).context("Failed to write output file")?;

    info!(" 已保存到: {}", output.display());

    Ok(())
}

/// 批量采集配置
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct BatchConfig {
    tasks: Vec<BatchTask>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BatchTask {
    name: String,
    level: String,
    platform: String,
    owner: Option<String>,
    repo: Option<String>,
    repo_path: Option<String>,
    branch: String,
    api_url: Option<String>,
    token: Option<String>,
    limit: Option<u32>,
}

/// 批量采集
async fn collect_batch(config_path: &PathBuf, output_dir: &PathBuf) -> Result<()> {
    info!(" 开始批量采集");
    info!("   配置文件: {}", config_path.display());
    info!("   输出目录: {}", output_dir.display());

    // 读取配置文件
    let config_content = fs::read_to_string(config_path).context("Failed to read config file")?;
    let batch_config: BatchConfig =
        serde_yaml::from_str(&config_content).context("Failed to parse config file")?;

    info!("   任务数: {}", batch_config.tasks.len());

    // 创建输出目录
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    // 执行每个任务
    let mut success_count = 0;
    let mut failed_count = 0;

    for (idx, task) in batch_config.tasks.iter().enumerate() {
        info!(
            "\n[{}/{}] 处理任务: {}",
            idx + 1,
            batch_config.tasks.len(),
            task.name
        );

        // 解析层级
        let level = match task.level.to_lowercase().as_str() {
            "l0" => Level::L0,
            "l1" => Level::L1,
            "l2" => Level::L2,
            _ => {
                error!("    无效的层级: {}", task.level);
                failed_count += 1;
                continue;
            }
        };

        // 解析平台
        let platform = match Platform::from_str(&task.platform) {
            Some(p) => p,
            None => {
                error!("    无效的平台: {}", task.platform);
                failed_count += 1;
                continue;
            }
        };

        // 构建输出文件名
        let output_filename = format!("{}.json", task.name);
        let output = output_dir.join(output_filename);

        // 构建配置
        let mut config = CollectConfig::new(platform, task.branch.clone());

        match platform {
            Platform::Local => {
                if let Some(path) = &task.repo_path {
                    config = config.with_local_path(PathBuf::from(path));
                } else {
                    error!("    Local platform requires repo_path");
                    failed_count += 1;
                    continue;
                }
            }
            _ => {
                if let (Some(owner), Some(repo)) = (&task.owner, &task.repo) {
                    config = config.with_remote(owner.clone(), repo.clone());
                } else {
                    error!("    Remote platform requires owner and repo");
                    failed_count += 1;
                    continue;
                }
            }
        }

        if let Some(url) = &task.api_url {
            config = config.with_api_url(url.clone());
        }
        if let Some(token) = &task.token {
            config = config.with_token(token.clone());
        }
        if let Some(limit) = task.limit {
            config = config.with_limit(limit);
            config = config.with_level(level.as_str());
        }

        // 创建采集器
        let collector_result: Result<Box<dyn Collector>> = match platform {
            Platform::GitHub => {
                let token = config
                    .token
                    .as_ref()
                    .context("GitHub requires token")?
                    .clone();
                let client = GitHubClient::new(token).context("Failed to create GitHub client")?;
                Ok(Box::new(GitHubAdapter::new(client, Platform::GitHub)))
            }
            Platform::Gitee => {
                let token = config
                    .token
                    .as_ref()
                    .context("Gitee requires token")?
                    .clone();
                let client = GiteeClient::new(token).context("Failed to create Gitee client")?;
                Ok(Box::new(GiteeAdapter::new(client, Platform::Gitee)))
            }
            Platform::Gitea => {
                let token = config
                    .token
                    .as_ref()
                    .context("Gitea requires token")?
                    .clone();
                let api_url = config
                    .api_url
                    .as_ref()
                    .context("Gitea requires api_url")?
                    .clone();
                let client =
                    GiteaClient::new(api_url, token).context("Failed to create Gitea client")?;
                Ok(Box::new(GiteaAdapter::new(client, Platform::Gitea)))
            }
            Platform::GitLab => {
                let token = config
                    .token
                    .as_ref()
                    .context("GitLab requires token")?
                    .clone();

                // 如果提供了 api_url，使用自定义 GitLab 实例
                let client = if let Some(api_url) = &config.api_url {
                    GitLabClient::with_base_url(api_url, token)
                } else {
                    GitLabClient::new(token)
                }
                .context("Failed to create GitLab client")?;

                Ok(Box::new(GitLabAdapter::new(client, Platform::GitLab)))
            }
            Platform::Local => {
                let path = config
                    .repo_path
                    .as_ref()
                    .context("Local requires repo_path")?
                    .clone();
                let client = LocalClient::new(path).context("Failed to create Local client")?;
                Ok(Box::new(client))
            }
        };

        let collector = match collector_result {
            Ok(c) => c,
            Err(e) => {
                error!("    创建采集器失败: {}", e);
                failed_count += 1;
                continue;
            }
        };

        // 执行采集
        match collector.collect(&config).await {
            Ok(mut result) => {
                // 设置正确的层级
                result.level = level.as_str().to_string();

                // 保存结果
                match serde_json::to_string_pretty(&result) {
                    Ok(json) => match fs::write(&output, json) {
                        Ok(_) => {
                            info!(
                                "   成功: {} commits, 保存到 {}",
                                result.commits.len(),
                                output.display()
                            );
                            success_count += 1;
                        }
                        Err(e) => {
                            error!("    写入文件失败: {}", e);
                            failed_count += 1;
                        }
                    },
                    Err(e) => {
                        error!("    序列化失败: {}", e);
                        failed_count += 1;
                    }
                }
            }
            Err(e) => {
                error!("    采集失败: {}", e);
                failed_count += 1;
            }
        }
    }

    info!("\n批量采集完成");
    info!("   成功: {}", success_count);
    info!("   失败: {}", failed_count);
    info!("   总计: {}", batch_config.tasks.len());

    if failed_count > 0 {
        anyhow::bail!("{} tasks failed", failed_count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::process::Command;

    fn init_git_repo(repo: &std::path::Path) {
        assert!(Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "init"])
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args([
                "-C",
                repo.to_str().unwrap(),
                "config",
                "user.email",
                "a@b.c",
            ])
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args([
                "-C",
                repo.to_str().unwrap(),
                "config",
                "user.name",
                "tester",
            ])
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn level_as_str_returns_expected_values() {
        assert_eq!(Level::L0.as_str(), "l0");
        assert_eq!(Level::L1.as_str(), "l1");
        assert_eq!(Level::L2.as_str(), "l2");
    }

    #[test]
    fn platform_arg_into_platform_maps_all_variants() {
        assert!(matches!(
            Platform::from(PlatformArg::GitHub),
            Platform::GitHub
        ));
        assert!(matches!(
            Platform::from(PlatformArg::GitLab),
            Platform::GitLab
        ));
        assert!(matches!(
            Platform::from(PlatformArg::Gitee),
            Platform::Gitee
        ));
        assert!(matches!(
            Platform::from(PlatformArg::Gitea),
            Platform::Gitea
        ));
        assert!(matches!(
            Platform::from(PlatformArg::Local),
            Platform::Local
        ));
    }

    #[test]
    fn cli_parse_collect_command_basic() {
        let args = vec![
            "track-collector",
            "collect",
            "l0",
            "--platform",
            "github",
            "--owner",
            "owner",
            "--repo",
            "repo",
        ];
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::Collect {
                level, platform, ..
            } => {
                assert!(matches!(level, Level::L0));
                assert!(matches!(platform, PlatformArg::GitHub));
            }
            _ => panic!("expected Collect command"),
        }
    }

    #[test]
    fn cli_parse_batch_command_basic() {
        let args = vec![
            "track-collector",
            "batch",
            "--config",
            "config.yaml",
            "--output-dir",
            "out",
        ];
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::Batch { config, output_dir } => {
                assert_eq!(config, PathBuf::from("config.yaml"));
                assert_eq!(output_dir, PathBuf::from("out"));
            }
            _ => panic!("expected Batch command"),
        }
    }

    #[tokio::test]
    async fn collect_single_returns_error_when_required_owner_missing_for_remote() {
        let result = collect_single(
            Level::L0,
            PlatformArg::GitHub,
            None,
            Some("repo".to_string()),
            None,
            "master".to_string(),
            None,
            Some("token".to_string()),
            None,
            PathBuf::from("out.json"),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn collect_single_returns_error_when_token_missing_for_github() {
        let result = collect_single(
            Level::L0,
            PlatformArg::GitHub,
            Some("owner".to_string()),
            Some("repo".to_string()),
            None,
            "master".to_string(),
            None,
            None,
            None,
            PathBuf::from("out.json"),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn run_dispatches_collect_and_batch() {
        let collect_result = run(Cli {
            lang: None,
            command: Commands::Collect {
                level: Level::L0,
                platform: PlatformArg::GitHub,
                owner: None,
                repo: Some("repo".to_string()),
                repo_path: None,
                branch: "master".to_string(),
                api_url: None,
                token: Some("token".to_string()),
                limit: None,
                output: PathBuf::from("out.json"),
            },
        })
        .await;
        assert!(collect_result.is_err());

        let tmp_dir = tempfile::tempdir().unwrap();
        let missing = tmp_dir.path().join("missing.yaml");
        let batch_result = run(Cli {
            lang: None,
            command: Commands::Batch {
                config: missing,
                output_dir: tmp_dir.path().join("out"),
            },
        })
        .await;
        assert!(batch_result.is_err());
    }

    #[tokio::test]
    async fn collect_single_local_success_writes_output() {
        let repo_dir = tempfile::tempdir().unwrap();
        let repo = repo_dir.path();
        init_git_repo(repo);

        std::fs::write(repo.join("a.txt"), "hello\n").unwrap();
        assert!(Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "add", "."])
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "commit", "-m", "init"])
            .status()
            .unwrap()
            .success());

        let out_dir = tempfile::tempdir().unwrap();
        let out = out_dir.path().join("out.json");
        collect_single(
            Level::L0,
            PlatformArg::Local,
            None,
            None,
            Some(repo.to_path_buf()),
            "master".to_string(),
            None,
            None,
            Some(10),
            out.clone(),
        )
        .await
        .unwrap();

        let json = std::fs::read_to_string(&out).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["platform"], "local");
        assert_eq!(v["level"], "l0");
    }

    #[tokio::test]
    async fn collect_single_returns_error_when_repo_path_missing_for_local() {
        let result = collect_single(
            Level::L0,
            PlatformArg::Local,
            None,
            None,
            None,
            "master".to_string(),
            None,
            None,
            None,
            PathBuf::from("out.json"),
        )
        .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Local platform requires --repo-path"));
    }

    #[tokio::test]
    async fn collect_single_returns_error_when_token_missing_for_gitee() {
        let result = collect_single(
            Level::L1,
            PlatformArg::Gitee,
            Some("owner".to_string()),
            Some("repo".to_string()),
            None,
            "master".to_string(),
            None,
            None,
            None,
            PathBuf::from("out.json"),
        )
        .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Gitee requires token"));
    }

    #[tokio::test]
    async fn collect_single_returns_error_when_api_url_missing_for_gitea() {
        let result = collect_single(
            Level::L1,
            PlatformArg::Gitea,
            Some("owner".to_string()),
            Some("repo".to_string()),
            None,
            "master".to_string(),
            None,
            Some("token".to_string()),
            None,
            PathBuf::from("out.json"),
        )
        .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Gitea requires --api-url"));
    }

    #[tokio::test]
    async fn collect_single_returns_error_when_token_missing_for_gitlab() {
        let result = collect_single(
            Level::L0,
            PlatformArg::GitLab,
            Some("owner".to_string()),
            Some("repo".to_string()),
            None,
            "master".to_string(),
            None,
            None,
            None,
            PathBuf::from("out.json"),
        )
        .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("GitLab requires token"));
    }

    #[tokio::test]
    async fn collect_batch_invalid_level_increments_failed_count() {
        let tmp_dir = tempfile::tempdir().expect("create temp dir");
        let config_path = tmp_dir.path().join("batch.yaml");
        let output_dir = tmp_dir.path().join("out");

        let yaml = r#"
tasks:
  - name: invalid-level
    level: l3
    platform: github
    owner: owner
    repo: repo
    branch: master
"#;
        fs::write(&config_path, yaml).expect("write yaml");

        let result = collect_batch(&config_path, &output_dir).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn collect_batch_missing_owner_repo_for_remote_platform_returns_error() {
        let tmp_dir = tempfile::tempdir().expect("create temp dir");
        let config_path = tmp_dir.path().join("batch2.yaml");
        let output_dir = tmp_dir.path().join("out2");

        let yaml = r#"
tasks:
  - name: missing-owner-repo
    level: l0
    platform: github
    branch: master
"#;
        fs::write(&config_path, yaml).expect("write yaml");

        let result = collect_batch(&config_path, &output_dir).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn collect_batch_returns_error_when_config_file_missing() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("missing.yaml");
        let output_dir = tmp_dir.path().join("out");
        let result = collect_batch(&config_path, &output_dir).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read config file"));
    }

    #[tokio::test]
    async fn collect_batch_returns_error_when_yaml_invalid() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("bad.yaml");
        let output_dir = tmp_dir.path().join("out");
        fs::write(&config_path, "not: [valid").unwrap();
        let result = collect_batch(&config_path, &output_dir).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to parse config file"));
    }

    #[tokio::test]
    async fn collect_batch_invalid_platform_increments_failed_count() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("bad-platform.yaml");
        let output_dir = tmp_dir.path().join("out");
        let yaml = r#"
tasks:
  - name: bad-platform
    level: l0
    platform: not-a-platform
    branch: master
"#;
        fs::write(&config_path, yaml).unwrap();

        let result = collect_batch(&config_path, &output_dir).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn collect_batch_local_missing_repo_path_increments_failed_count() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("local-missing.yaml");
        let output_dir = tmp_dir.path().join("out");
        let yaml = r#"
tasks:
  - name: local-missing
    level: l0
    platform: local
    branch: master
"#;
        fs::write(&config_path, yaml).unwrap();

        let result = collect_batch(&config_path, &output_dir).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn collect_batch_missing_token_causes_collector_creation_failure() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("missing-token.yaml");
        let output_dir = tmp_dir.path().join("out");
        let yaml = r#"
tasks:
  - name: missing-token
    level: l0
    platform: github
    owner: owner
    repo: repo
    branch: master
"#;
        fs::write(&config_path, yaml).unwrap();

        let result = collect_batch(&config_path, &output_dir).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn collect_batch_success_local_writes_output_with_limit() {
        let repo_dir = tempfile::tempdir().unwrap();
        let repo = repo_dir.path();
        init_git_repo(repo);
        std::fs::write(repo.join("a.txt"), "hello\n").unwrap();
        assert!(Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "add", "."])
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "commit", "-m", "init"])
            .status()
            .unwrap()
            .success());

        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("ok.yaml");
        let output_dir = tmp_dir.path().join("out");
        let yaml = format!(
            r#"
tasks:
  - name: local-ok
    level: l0
    platform: local
    repo_path: "{}"
    branch: master
    limit: 1
"#,
            repo.to_string_lossy()
        );
        fs::write(&config_path, yaml).unwrap();

        collect_batch(&config_path, &output_dir).await.unwrap();
        let out = output_dir.join("local-ok.json");
        let json = std::fs::read_to_string(&out).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["platform"], "local");
        assert_eq!(v["level"], "l0");
    }

    #[tokio::test]
    async fn collect_batch_write_failure_increments_failed_count() {
        let repo_dir = tempfile::tempdir().unwrap();
        let repo = repo_dir.path();
        init_git_repo(repo);
        std::fs::write(repo.join("a.txt"), "hello\n").unwrap();
        assert!(Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "add", "."])
            .status()
            .unwrap()
            .success());
        assert!(Command::new("git")
            .args(["-C", repo.to_str().unwrap(), "commit", "-m", "init"])
            .status()
            .unwrap()
            .success());

        let tmp_dir = tempfile::tempdir().unwrap();
        let config_path = tmp_dir.path().join("write-fail.yaml");
        let output_dir = tmp_dir.path().join("out");
        std::fs::create_dir_all(&output_dir).unwrap();
        std::fs::create_dir_all(output_dir.join("local-write-fail.json")).unwrap();

        let yaml = format!(
            r#"
tasks:
  - name: local-write-fail
    level: l0
    platform: local
    repo_path: "{}"
    branch: master
    limit: 1
"#,
            repo.to_string_lossy()
        );
        fs::write(&config_path, yaml).unwrap();

        let result = collect_batch(&config_path, &output_dir).await;
        assert!(result.is_err());
    }
}
