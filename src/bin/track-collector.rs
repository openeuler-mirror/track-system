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

//! Track Collector - 轻量化元数据采集工具
//!
//! 无需数据库依赖，直接从 Gitee/GitHub/GitLab/Gitea/Local 采集仓库元数据并导出为 JSON
//! 适用于隔离环境（如内网）的数据采集场景

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap::{Parser, Subcommand, ValueEnum};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info, warn};
use track_system::collectors::traits::{CollectConfig, Collector, GitClient, Platform};
use track_system::collectors::{
    adapters::{AtomGitAdapter, GitHubAdapter, GitLabAdapter, GiteaAdapter, GiteeAdapter},
    atomgit::AtomGitClient,
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

const DEFAULT_BRANCH_FILTERS: [&str; 5] = ["2.0.1", "22.06", "23.01", "25.05", "25.07"];

/// 平台类型（用于 CLI）
#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "lowercase")]
enum PlatformArg {
    GitHub,
    GitLab,
    AtomGit,
    Gitee,
    Gitea,
    Local,
}

impl From<PlatformArg> for Platform {
    fn from(arg: PlatformArg) -> Self {
        match arg {
            PlatformArg::GitHub => Platform::GitHub,
            PlatformArg::GitLab => Platform::GitLab,
            PlatformArg::AtomGit => Platform::AtomGit,
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
        #[arg(long)]
        branch: Option<String>,

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

fn sanitize_branch_name(branch: &str) -> String {
    branch.replace('/', "_")
}

fn sanitize_repo_name(repo: &str) -> String {
    repo.replace('/', "_")
}

fn build_output_path(
    base: &PathBuf,
    repo: &str,
    branch: &str,
    multi_branch: bool,
) -> Result<PathBuf> {
    let base_str = base.to_string_lossy();
    let is_dir = base_str.ends_with(std::path::MAIN_SEPARATOR)
        || base_str.ends_with('/')
        || base.file_name().is_none()
        || fs::metadata(base).map(|m| m.is_dir()).unwrap_or(false);
    let sanitized = sanitize_branch_name(branch);
    let sanitized_repo = if repo.is_empty() {
        "unknown".to_string()
    } else {
        sanitize_repo_name(repo)
    };

    if is_dir {
        fs::create_dir_all(base).context("Failed to create output directory")?;
        let file_name = if multi_branch {
            format!("{}-metadata-{}.json", sanitized_repo, sanitized)
        } else {
            format!("{}-metadata.json", sanitized_repo)
        };
        return Ok(base.join(file_name));
    }

    let file_name = base
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("metadata.json");
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);
    let ext = base
        .extension()
        .and_then(|s| s.to_str())
        .filter(|e| !e.is_empty())
        .unwrap_or("json");
    let name = if multi_branch {
        format!("{}-{}-{}", sanitized_repo, stem, sanitized)
    } else {
        format!("{}-{}", sanitized_repo, stem)
    };
    let final_name = if ext.eq_ignore_ascii_case("json") {
        format!("{}.json", name)
    } else {
        format!("{}.json", name)
    };
    Ok(base.with_file_name(final_name))
}

async fn resolve_branches_from_client(
    client: &dyn GitClient,
    owner: Option<&str>,
    repo: Option<&str>,
    explicit_branch: Option<String>,
    custom_branches: Option<&[String]>,
    branch_filters: Option<&[String]>,
) -> Result<Vec<String>> {
    if let Some(branch) = explicit_branch {
        return Ok(vec![branch]);
    }

    let branches = client
        .get_branches(owner.unwrap_or_default(), repo.unwrap_or_default())
        .await?;
    let branch_names: Vec<String> = branches.into_iter().map(|b| b.name).collect();

    if let Some(custom) = custom_branches {
        let available: HashSet<String> = branch_names.iter().cloned().collect();
        let mut matched = Vec::new();
        for branch in custom {
            if available.contains(branch) {
                matched.push(branch.clone());
            } else {
                warn!("   自定义分支未找到: {}", branch);
            }
        }
        if matched.is_empty() {
            anyhow::bail!("未找到配置中指定的分支");
        }
        return Ok(matched);
    }

    let mut filters: Vec<String> = DEFAULT_BRANCH_FILTERS
        .iter()
        .map(|s| s.to_string())
        .collect();
    if let Some(extra) = branch_filters {
        filters.extend(extra.iter().cloned());
    }
    let filters: HashSet<String> = filters.into_iter().collect();

    let mut filtered: Vec<String> = branch_names
        .into_iter()
        .filter(|name| filters.iter().any(|token| name.contains(token)))
        .collect();

    if filtered.is_empty() {
        anyhow::bail!("未找到匹配的分支");
    }

    filtered.sort();
    Ok(filtered)
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
    branch: Option<String>,
    api_url: Option<String>,
    token: Option<String>,
    limit: Option<u32>,
    output: PathBuf,
) -> Result<()> {
    info!(" 开始采集元数据");
    info!("   层级: {}", level.as_str());
    info!("   平台: {:?}", platform);

    let platform: Platform = platform.into();
    let owner = owner.as_ref().map(|s| s.as_str());
    let repo = repo.as_ref().map(|s| s.as_str());
    if !matches!(platform, Platform::Local) {
        owner.context("Remote platform requires --owner")?;
        repo.context("Remote platform requires --repo")?;
    }

    let (collector, branches) = match platform {
        Platform::GitHub => {
            let token = token.clone().context("GitHub requires token")?;
            let client = GitHubClient::new(token).context("Failed to create GitHub client")?;
            let branches =
                resolve_branches_from_client(&client, owner, repo, branch, None, None).await?;
            (
                Box::new(GitHubAdapter::new(client, Platform::GitHub)) as Box<dyn Collector>,
                branches,
            )
        }
        Platform::AtomGit => {
            let token = token.clone().context("AtomGit requires token")?;
            let default_branch = branch.clone().unwrap_or_else(|| "master".to_string());
            let client = AtomGitClient::new(token, default_branch)
                .context("Failed to create AtomGit client")?;
            let branches =
                resolve_branches_from_client(&client, owner, repo, branch, None, None).await?;
            (
                Box::new(AtomGitAdapter::new(client, Platform::AtomGit)) as Box<dyn Collector>,
                branches,
            )
        }
        Platform::Gitee => {
            let token = token.clone().context("Gitee requires token")?;
            let client = GiteeClient::new(token).context("Failed to create Gitee client")?;
            let branches =
                resolve_branches_from_client(&client, owner, repo, branch, None, None).await?;
            (
                Box::new(GiteeAdapter::new(client, Platform::Gitee)) as Box<dyn Collector>,
                branches,
            )
        }
        Platform::Gitea => {
            let token = token.clone().context("Gitea requires token")?;
            let api_url = api_url.clone().context("Gitea requires --api-url")?;
            let client =
                GiteaClient::new(token, api_url).context("Failed to create Gitea client")?;
            let branches =
                resolve_branches_from_client(&client, owner, repo, branch, None, None).await?;
            (
                Box::new(GiteaAdapter::new(client, Platform::Gitea)) as Box<dyn Collector>,
                branches,
            )
        }
        Platform::GitLab => {
            let token = token.clone().context("GitLab requires token")?;
            let client = if let Some(api_url) = &api_url {
                GitLabClient::with_base_url(api_url, token)
            } else {
                GitLabClient::new(token)
            }
            .context("Failed to create GitLab client")?;
            let branches =
                resolve_branches_from_client(&client, owner, repo, branch, None, None).await?;
            (
                Box::new(GitLabAdapter::new(client, Platform::GitLab)) as Box<dyn Collector>,
                branches,
            )
        }
        Platform::Local => {
            let path = repo_path
                .as_ref()
                .context("Local platform requires --repo-path")?
                .clone();
            let client = LocalClient::new(path).context("Failed to create Local client")?;
            let branches =
                resolve_branches_from_client(&client, None, None, branch, None, None).await?;
            (Box::new(client) as Box<dyn Collector>, branches)
        }
    };

    let multi_branch = branches.len() > 1;
    info!("   分支数: {}", branches.len());

    for branch_name in branches {
        let mut config = CollectConfig::new(platform, branch_name.clone());
        match platform {
            Platform::Local => {
                let path = repo_path
                    .as_ref()
                    .context("Local platform requires --repo-path")?
                    .clone();
                config = config.with_local_path(path);
                info!("   仓库路径: {:?}", config.repo_path);
            }
            _ => {
                let owner = owner.context("Remote platform requires --owner")?;
                let repo = repo.context("Remote platform requires --repo")?;
                config = config.with_remote(owner, repo);
                info!("   仓库: {}/{}", owner, repo);
            }
        }

        if let Some(url) = api_url.clone() {
            config = config.with_api_url(url);
        }
        if let Some(t) = token.clone() {
            config = config.with_token(t);
        }
        if let Some(l) = limit {
            config = config.with_limit(l);
        }
        config = config.with_level(level.as_str());

        info!("   分支: {}", branch_name);

        collector
            .validate_config(&config)
            .context("Invalid configuration")?;

        info!(" 正在采集...");
        let mut result = collector
            .collect(&config)
            .await
            .context("Failed to collect metadata")?;

        result.level = level.as_str().to_string();

        info!("采集完成");
        info!("   Commits: {}", result.commits.len());
        if let Some(snapshot) = &result.snapshot {
            info!("   Spec 版本: {:?}", snapshot.spec_version);
            info!("   Patches: {}", snapshot.patches.len());
            info!("   文件数: {}", snapshot.file_count);
        }

        let output_path = build_output_path(&output, &result.repo, &branch_name, multi_branch)?;
        let json = serde_json::to_string_pretty(&result).context("Failed to serialize result")?;
        fs::write(&output_path, json).context("Failed to write output file")?;

        info!(" 已保存到: {}", output_path.display());
    }

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
    branch: Option<String>,
    branches: Option<Vec<String>>,
    branch_filters: Option<Vec<String>>,
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
        if !matches!(platform, Platform::Local) && (task.owner.is_none() || task.repo.is_none()) {
            error!("    Remote platform requires owner and repo");
            failed_count += 1;
            continue;
        }
        if matches!(platform, Platform::Local) && task.repo_path.is_none() {
            error!("    Local platform requires repo_path");
            failed_count += 1;
            continue;
        }

        let owner = task.owner.as_ref().map(|s| s.as_str());
        let repo = task.repo.as_ref().map(|s| s.as_str());

        let collector_result: Result<(Box<dyn Collector>, Vec<String>)> = match platform {
            Platform::GitHub => {
                let token = task.token.clone().context("GitHub requires token")?;
                let client = GitHubClient::new(token).context("Failed to create GitHub client")?;
                let branches = resolve_branches_from_client(
                    &client,
                    owner,
                    repo,
                    task.branch.clone(),
                    task.branches.as_deref(),
                    task.branch_filters.as_deref(),
                )
                .await?;
                Ok((
                    Box::new(GitHubAdapter::new(client, Platform::GitHub)) as Box<dyn Collector>,
                    branches,
                ))
            }
            Platform::AtomGit => {
                let token = task.token.clone().context("AtomGit requires token")?;
                let default_branch = task.branch.clone().unwrap_or_else(|| "master".to_string());
                let client = AtomGitClient::new(token, default_branch)
                    .context("Failed to create AtomGit client")?;
                let branches = resolve_branches_from_client(
                    &client,
                    owner,
                    repo,
                    task.branch.clone(),
                    task.branches.as_deref(),
                    task.branch_filters.as_deref(),
                )
                .await?;
                Ok((
                    Box::new(AtomGitAdapter::new(client, Platform::AtomGit)) as Box<dyn Collector>,
                    branches,
                ))
            }
            Platform::Gitee => {
                let token = task.token.clone().context("Gitee requires token")?;
                let client = GiteeClient::new(token).context("Failed to create Gitee client")?;
                let branches = resolve_branches_from_client(
                    &client,
                    owner,
                    repo,
                    task.branch.clone(),
                    task.branches.as_deref(),
                    task.branch_filters.as_deref(),
                )
                .await?;
                Ok((
                    Box::new(GiteeAdapter::new(client, Platform::Gitee)) as Box<dyn Collector>,
                    branches,
                ))
            }
            Platform::Gitea => {
                let token = task.token.clone().context("Gitea requires token")?;
                let api_url = task.api_url.clone().context("Gitea requires api_url")?;
                let client =
                    GiteaClient::new(api_url, token).context("Failed to create Gitea client")?;
                let branches = resolve_branches_from_client(
                    &client,
                    owner,
                    repo,
                    task.branch.clone(),
                    task.branches.as_deref(),
                    task.branch_filters.as_deref(),
                )
                .await?;
                Ok((
                    Box::new(GiteaAdapter::new(client, Platform::Gitea)) as Box<dyn Collector>,
                    branches,
                ))
            }
            Platform::GitLab => {
                let token = task.token.clone().context("GitLab requires token")?;
                let client = if let Some(api_url) = &task.api_url {
                    GitLabClient::with_base_url(api_url, token)
                } else {
                    GitLabClient::new(token)
                }
                .context("Failed to create GitLab client")?;
                let branches = resolve_branches_from_client(
                    &client,
                    owner,
                    repo,
                    task.branch.clone(),
                    task.branches.as_deref(),
                    task.branch_filters.as_deref(),
                )
                .await?;
                Ok((
                    Box::new(GitLabAdapter::new(client, Platform::GitLab)) as Box<dyn Collector>,
                    branches,
                ))
            }
            Platform::Local => {
                let path = task.repo_path.clone().context("Local requires repo_path")?;
                let client = LocalClient::new(path).context("Failed to create Local client")?;
                let branches = resolve_branches_from_client(
                    &client,
                    None,
                    None,
                    task.branch.clone(),
                    task.branches.as_deref(),
                    task.branch_filters.as_deref(),
                )
                .await?;
                Ok((Box::new(client) as Box<dyn Collector>, branches))
            }
        };
        let (collector, branches) = match collector_result {
            Ok(value) => value,
            Err(e) => {
                error!("    创建采集器失败: {}", e);
                failed_count += 1;
                continue;
            }
        };
        let multi_branch = branches.len() > 1;
        info!("    分支数: {}", branches.len());

        for branch_name in branches {
            let output_filename = if multi_branch {
                format!("{}-{}.json", task.name, sanitize_branch_name(&branch_name))
            } else {
                format!("{}.json", task.name)
            };
            let output = output_dir.join(output_filename);

            let mut config = CollectConfig::new(platform, branch_name.clone());
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
            }
            config = config.with_level(level.as_str());

            match collector.collect(&config).await {
                Ok(mut result) => {
                    result.level = level.as_str().to_string();

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
            Platform::from(PlatformArg::AtomGit),
            Platform::AtomGit
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
            Some("master".to_string()),
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
            Some("master".to_string()),
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
                branch: Some("master".to_string()),
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
            Some("master".to_string()),
            None,
            None,
            Some(10),
            out.clone(),
        )
        .await
        .unwrap();

        let repo_name = repo
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let expected_out = build_output_path(&out, repo_name, "master", false).unwrap();
        let json = std::fs::read_to_string(&expected_out).unwrap();
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
            Some("master".to_string()),
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
            Some("master".to_string()),
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
            Some("master".to_string()),
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
            Some("master".to_string()),
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
