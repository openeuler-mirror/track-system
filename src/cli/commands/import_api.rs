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

//! 元数据导入命令实现（基于 API）
//!
//! 通过 HTTP API 导入元数据

use anyhow::{anyhow, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::cli::client::ApiClient;
use crate::cli::dto::{PackageDto, TrackingDto};
use crate::cli::formatter::format_datetime_local;
use crate::cli::parser::ImportAction;
use crate::snapshot::types::RepositorySnapshot;
use crate::snapshot::types::{
    ChangeStats, CommitEntry, FileEntry, IssueEntry, SnapshotOrigin, SpecEntry,
};
use chrono::{DateTime, Utc};
use serde_json::Value;

/// 导入响应
#[derive(Debug, Serialize, Deserialize)]
struct ImportResponse {
    snapshot_id: String,
    tracking_id: i32,
    file_count: usize,
    imported_at: chrono::DateTime<chrono::Utc>,
}

/// API 响应包装
#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    code: u16,
    message: String,
    data: Option<T>,
}

/// 导入请求
#[derive(Debug, Serialize, Deserialize)]
struct ImportRequest {
    tracking_id: i32,
    snapshot: RepositorySnapshot,
}

/// 列表响应
#[derive(Debug, Serialize, Deserialize)]
struct ListResponse<T> {
    items: Vec<T>,
    total: usize,
}

/// 执行导入命令
pub async fn execute(api_client: &ApiClient, action: ImportAction) -> Result<()> {
    match action {
        ImportAction::Metadata { file, tracking_id } => {
            let path = PathBuf::from(file);
            import_single_file(api_client, &path, tracking_id).await
        }
        ImportAction::Batch {
            files,
            tracking_id: _tracking_id,
        } => {
            let paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
            import_batch_files(api_client, paths).await
        }
    }
}

/// 从 JSON 内容中提取 repo 字段（package name）
fn extract_repo_from_json(content: &str) -> Result<String> {
    let root: Value =
        serde_json::from_str(content).map_err(|e| anyhow!("解析 JSON 失败: {}", e))?;

    // 尝试从通用采集格式提取 repo 字段
    if let Some(repo) = root.get("repo").and_then(|v| v.as_str()) {
        return Ok(repo.to_string());
    }

    // 如果是标准 RepositorySnapshot 格式，没有 repo 字段，需要提供 tracking_id
    Err(anyhow!(
        "JSON 文件中未找到 'repo' 字段，请使用 --tracking-id 参数明确指定"
    ))
}

/// 根据 package name 查询对应的 tracking_id
async fn resolve_tracking_id_from_package(
    api_client: &ApiClient,
    package_name: &str,
) -> Result<i32> {
    // 1. 查询 package 列表，找到匹配的 package_id
    let packages: Vec<PackageDto> = api_client
        .get("/packages")
        .await
        .map_err(|e| anyhow!("查询 package 列表失败: {}", e))?;

    let package = packages
        .iter()
        .find(|p| p.name == package_name)
        .ok_or_else(|| anyhow!("未找到名称为 '{}' 的 package", package_name))?;

    let package_id = package.id;

    // 2. 查询该 package 的 tracking 配置
    let query = format!("?page=1&page_size=100&package_id={}", package_id);
    let response: ApiResponse<ListResponse<TrackingDto>> = api_client
        .get(&format!("/tracking{}", query))
        .await
        .map_err(|e| anyhow!("查询 tracking 配置失败: {}", e))?;

    let trackings = response.data.ok_or_else(|| anyhow!("空响应"))?.items;

    if trackings.is_empty() {
        return Err(anyhow!(
            "package '{}' (ID: {}) 没有关联的 tracking 配置，请先创建",
            package_name,
            package_id
        ));
    }

    // 3. 优先选择状态为 active 的 tracking
    let active_tracking = trackings
        .iter()
        .find(|t| t.tracking_status == "active")
        .or_else(|| trackings.first());

    if let Some(tracking) = active_tracking {
        if trackings.len() > 1 {
            println!(
                "  {} package '{}' 有 {} 个 tracking 配置，使用 tracking_id: {} (状态: {})",
                "ℹ".cyan(),
                package_name,
                trackings.len(),
                tracking.id,
                tracking.tracking_status
            );
        }
        Ok(tracking.id)
    } else {
        Err(anyhow!("未找到可用的 tracking 配置"))
    }
}

/// 导入单个文件
async fn import_single_file(
    api_client: &ApiClient,
    file: &PathBuf,
    tracking_id: i32,
) -> Result<()> {
    println!("正在导入文件: {}", file.display().to_string().cyan());

    // 检查文件是否存在
    if !file.exists() {
        println!("{} 文件不存在: {}", "✗".red().bold(), file.display());
        anyhow::bail!("文件不存在");
    }

    // 读取并解析/转换快照文件
    let content = fs::read_to_string(file)?;
    let snapshot = parse_snapshot_or_convert(&content, tracking_id)?;

    // 确定导入端点（根据快照来源）
    let endpoint = match snapshot.origin {
        SnapshotOrigin::L1 => "/metadata/l1",
        SnapshotOrigin::L2 => "/metadata/l2",
        SnapshotOrigin::Unknown => {
            // 默认使用 L1
            println!("{}", "警告: 快照来源未知，默认使用 L1 端点".yellow());
            "/metadata/l1"
        }
    };

    println!("  快照来源: {:?}", snapshot.origin);
    println!("  文件数量: {}", snapshot.files.len());
    println!("  提交数量: {}", snapshot.commits.len());
    println!("  问题数量: {}", snapshot.issues.len());
    println!();

    // 构建请求
    let request = ImportRequest {
        tracking_id,
        snapshot,
    };

    // 发送导入请求
    match api_client
        .post::<_, ApiResponse<ImportResponse>>(endpoint, &request)
        .await
    {
        Ok(response) => {
            let data = response
                .data
                .ok_or_else(|| anyhow::anyhow!("API 响应缺少 data 字段"))?;
            println!("{} 导入成功", "✓".green().bold());
            println!("  快照 ID: {}", data.snapshot_id.cyan());
            println!("  跟踪配置 ID: {}", data.tracking_id);
            println!("  文件数量: {}", data.file_count);
            println!("  导入时间: {}", format_datetime_local(&data.imported_at));
            Ok(())
        }
        Err(e) => {
            println!("{} 导入失败: {}", "✗".red().bold(), e);
            Err(e.into())
        }
    }
}

/// 批量导入文件
/// tracking_id 将从每个 JSON 文件的 repo 字段自动解析
async fn import_batch_files(api_client: &ApiClient, files: Vec<PathBuf>) -> Result<()> {
    println!("开始批量导入 {} 个文件", files.len());
    println!();

    let mut success_count = 0;
    let mut failed_count = 0;
    let mut total_files = 0;

    for (i, file) in files.iter().enumerate() {
        println!("[{}/{}] 导入文件: {}", i + 1, files.len(), file.display());

        let content = fs::read_to_string(file)?;

        let package_name = extract_repo_from_json(&content)?;
        // 从 JSON 文件解析 tracking_id
        match resolve_tracking_id_from_package(api_client, &package_name).await {
            Ok(tracking_id) => {
                println!("  解析到 tracking_id: {}", tracking_id);
                match import_single_file(api_client, file, tracking_id).await {
                    Ok(_) => {
                        success_count += 1;
                        // 读取文件获取文件数量
                        if let Ok(content) = fs::read_to_string(file) {
                            if let Ok(snapshot) =
                                serde_json::from_str::<RepositorySnapshot>(&content)
                            {
                                total_files += snapshot.files.len();
                            }
                        }
                    }
                    Err(e) => {
                        println!("  导入失败: {}", e);
                        failed_count += 1;
                    }
                }
            }
            Err(e) => {
                println!("  无法解析 tracking_id: {}", e);
                failed_count += 1;
            }
        }
        println!();
    }

    println!("{}", "批量导入完成:".bold());
    println!("  成功: {}", success_count.to_string().green());
    println!("  失败: {}", failed_count.to_string().red());
    println!("  总文件数: {}", total_files);

    if failed_count > 0 {
        anyhow::bail!("{} 个文件导入失败", failed_count);
    }

    Ok(())
}

/// 尝试解析 RepositorySnapshot；失败则按通用采集 JSON 转换为 RepositorySnapshot
fn parse_snapshot_or_convert(
    content: &str,
    tracking_id: i32,
) -> anyhow::Result<RepositorySnapshot> {
    // 1) 优先当作 RepositorySnapshot 解析
    if let Ok(snapshot) = serde_json::from_str::<RepositorySnapshot>(content) {
        return Ok(snapshot);
    }

    // 2) 解析为通用 JSON，转换为 RepositorySnapshot
    let root: Value = serde_json::from_str(content)?;

