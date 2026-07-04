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

